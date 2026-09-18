//! Windows.Graphics.Capture 按窗口截图（跟随窗口模式）。取景框的屏幕区域截图见 `screen_capture`。
//!
//! 这里每次调用都新建截图会话、取一帧后结束（M0-01 实测首帧 594 ms）。跟随窗口模式目前没有界面入口，
//! 所以暂不改成常驻会话（已列入 architecture.md「已知问题」）。

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use trans_application::capture::{CaptureError, Frame, WindowCapture, WindowId, WindowInfo};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as CaptureFrame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, GraphicsCaptureItemType,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

/// 最小化的窗口不产出帧（M0-01：Chrome 最小化时 3 s 内无帧），超时后明确报错而不是一直等。
const FRAME_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Default, Clone, Copy)]
pub struct GraphicsCapture;

impl WindowCapture for GraphicsCapture {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError> {
        let windows = Window::enumerate().map_err(|e| CaptureError::Backend(e.to_string()))?;
        Ok(windows
            .into_iter()
            .filter_map(|window| {
                let title = window.title().ok()?;
                if title.trim().is_empty() {
                    return None;
                }
                Some(WindowInfo {
                    id: WindowId(window.as_raw_hwnd() as i64),
                    title,
                    process_name: window.process_name().unwrap_or_default(),
                })
            })
            .collect())
    }

    fn grab_frame(&self, id: WindowId) -> Result<Frame, CaptureError> {
        let window = Window::from_raw_hwnd(id.0 as *mut std::ffi::c_void);
        if !window.is_valid() {
            return Err(CaptureError::WindowNotFound);
        }
        grab_once(window)
    }
}

/// 把窗口排除在所有截图之外（Windows 10 2004+）。取景框浮在内容上面，不排除就只会拍到自己。
pub fn exclude_from_capture(hwnd: *mut std::ffi::c_void) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

    // SAFETY: hwnd 由调用方从 Tauri 窗口取得，调用期间窗口存活。
    unsafe { SetWindowDisplayAffinity(HWND(hwnd), WDA_EXCLUDEFROMCAPTURE) }.map_err(|error| error.to_string())
}

/// 把窗口裁成一个圆：圆外的部分不显示，也不接收鼠标事件（点击穿到下面的窗口）。
///
/// 为什么需要：Windows 对窗口有最小宽度限制（实测无边框窗口最窄约 135 px），
/// 想要一个更小的圆形悬浮图标只能先做大窗口再裁掉多余部分。`diameter` 是物理像素。
pub fn clip_to_circle(hwnd: *mut std::ffi::c_void, diameter: i32) -> Result<(), String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{CreateEllipticRgn, SetWindowRgn};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    if diameter <= 0 {
        return Err("圆形直径必须大于 0".into());
    }
    let hwnd = HWND(hwnd);
    let mut rect = RECT::default();
    // SAFETY: hwnd 由调用方从 Tauri 窗口取得，调用期间窗口存活。
    unsafe { GetWindowRect(hwnd, &raw mut rect) }.map_err(|error| error.to_string())?;
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let diameter = diameter.min(width).min(height);
    let (left, top) = ((width - diameter) / 2, (height - diameter) / 2);

    // SAFETY: 区域句柄交给 SetWindowRgn 后由系统拥有，不需要我们释放（bRedraw = TRUE）。
    unsafe {
        let region = CreateEllipticRgn(left, top, left + diameter, top + diameter);
        if region.is_invalid() {
            return Err("创建圆形区域失败".into());
        }
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            return Err("设置窗口区域失败".into());
        }
    }
    Ok(())
}

type Slot = Arc<Mutex<Option<Frame>>>;

fn grab_once<T>(item: T) -> Result<Frame, CaptureError>
where
    T: TryInto<GraphicsCaptureItemType> + Send + 'static,
{
    let slot: Slot = Arc::new(Mutex::new(None));
    let settings = Settings::new(
        item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        slot.clone(),
    );
    let control = OneShot::start_free_threaded(settings).map_err(|e| CaptureError::Backend(e.to_string()))?;

    let deadline = Instant::now() + FRAME_TIMEOUT;
    while Instant::now() < deadline && !control.is_finished() {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !control.is_finished() {
        let _ = control.stop();
    }
    let frame = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
    frame.ok_or(CaptureError::NoFrame { timeout_ms: FRAME_TIMEOUT.as_millis() as u64 })
}

struct OneShot {
    slot: Slot,
}

impl GraphicsCaptureApiHandler for OneShot {
    type Flags = Slot;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { slot: ctx.flags })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut CaptureFrame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let buffer = frame.buffer()?;
        let (width, height) = (buffer.width(), buffer.height());
        let mut scratch = Vec::new();
        let bgra = buffer.as_nopadding_buffer(&mut scratch).to_vec();
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(Frame::new(width, height, bgra)?);
        control.stop();
        Ok(())
    }
}
