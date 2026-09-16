//! Windows.Graphics.Capture 截图：按窗口，或按屏幕区域（取景框用）。
//!
//! 当前实现每次调用都新建截图会话、取一帧后结束。M0-01 实测建立会话到首帧要 594 ms，
//! 持续识别时应改成常驻会话（已列入 architecture.md「已知问题」）。

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use trans_application::capture::{CaptureError, Frame, ScreenCapture, WindowCapture, WindowId, WindowInfo};
use trans_domain::geometry::Rect;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as CaptureFrame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
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

/// 取景框用：截取屏幕上的一块绝对区域。
///
/// 做法是找出区域所在的显示器、整屏截一帧、再按显示器原点裁出这块区域。
/// 本程序自己的窗口要调用 [`exclude_from_capture`] 排除，否则截到的就是取景框自己。
#[derive(Debug, Default, Clone, Copy)]
pub struct MonitorCapture;

impl ScreenCapture for MonitorCapture {
    fn grab_region(&self, rect: Rect) -> Result<Frame, CaptureError> {
        if rect.width == 0 || rect.height == 0 {
            return Err(CaptureError::Backend("取景框大小为 0".into()));
        }
        let center = POINT { x: rect.x + rect.width as i32 / 2, y: rect.y + rect.height as i32 / 2 };
        // SAFETY: MonitorFromPoint 只读系统状态；MONITOR_DEFAULTTONEAREST 保证返回有效句柄。
        let handle = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: handle 有效，info 已按要求填好 cbSize。
        let ok = unsafe { GetMonitorInfoW(handle, &raw mut info) }.as_bool();
        if !ok {
            return Err(CaptureError::Backend("读取显示器信息失败".into()));
        }

        let frame = grab_once(Monitor::from_raw_hmonitor(handle.0))?;
        let local = Rect {
            x: rect.x - info.rcMonitor.left,
            y: rect.y - info.rcMonitor.top,
            width: rect.width,
            height: rect.height,
        };
        frame.crop(local).map_err(|error| CaptureError::Backend(format!("取景框超出显示器范围：{error}")))
    }
}

/// 把窗口排除在所有截图之外（Windows 10 2004+）。取景框浮在内容上面，不排除就只会拍到自己。
pub fn exclude_from_capture(hwnd: *mut std::ffi::c_void) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};

    // SAFETY: hwnd 由调用方从 Tauri 窗口取得，调用期间窗口存活。
    unsafe { SetWindowDisplayAffinity(HWND(hwnd), WDA_EXCLUDEFROMCAPTURE) }.map_err(|error| error.to_string())
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
