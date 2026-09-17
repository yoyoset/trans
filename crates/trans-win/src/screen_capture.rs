//! 取景框用的屏幕区域截图：常驻截图会话，只拷贝取景框那一小块。
//!
//! 为什么常驻：每次新建 Windows.Graphics.Capture 会话再取首帧，实测单次 499～802 ms
//! （800×600 区域连续 5 轮），而同一帧 OCR 只要 35～65 ms——截图才是持续识别的瓶颈。
//!
//! 做法：按"显示器 + 区域"建一个会话，系统在画面变化时推帧，回调里只把这块区域从显存
//! 拷出来留作最新一帧；识别时直接取这份拷贝。画面没变系统就不推帧，留着的那份仍然是准的。
//! 取景框挪动或改大小后区域变了，才重建会话（用户拖框是低频操作）。

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use trans_application::capture::{CaptureError, Frame, ScreenCapture};
use trans_domain::geometry::Rect;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as CaptureFrame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};

/// 新会话等首帧的上限。首帧总会推送，超时说明会话本身出了问题。
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(3);

type HandlerError = Box<dyn std::error::Error + Send + Sync>;

/// 取景框截图。会话在 [`ScreenCapture::release`] 时关闭（识别停止时调用）。
#[derive(Default)]
pub struct MonitorCapture {
    live: Mutex<Option<LiveSession>>,
}

impl ScreenCapture for MonitorCapture {
    fn grab_region(&self, rect: Rect) -> Result<Frame, CaptureError> {
        if rect.width == 0 || rect.height == 0 {
            return Err(CaptureError::Backend("取景框大小为 0".into()));
        }
        let (monitor, local) = locate(rect)?;
        let key = SessionKey { monitor: monitor.0 as isize, region: local };

        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        if live.as_ref().is_none_or(|session| session.key != key || session.control.is_finished()) {
            if let Some(old) = live.take() {
                old.close();
            }
            *live = Some(LiveSession::start(monitor, key)?);
        }
        let session = live.as_ref().expect("session was just ensured");
        match session.latest.wait_for_frame(FIRST_FRAME_TIMEOUT) {
            Ok(frame) => Ok(frame),
            Err(error) => {
                // 会话已经不可用（显示器变了、区域越界等）：关掉，下一轮重建。
                if let Some(broken) = live.take() {
                    broken.close();
                }
                Err(error)
            }
        }
    }

    fn release(&self) {
        if let Some(session) = self.live.lock().unwrap_or_else(PoisonError::into_inner).take() {
            session.close();
        }
    }
}

impl Drop for MonitorCapture {
    fn drop(&mut self) {
        self.release();
    }
}

/// 找出区域所在显示器，并把屏幕坐标换算成显示器内坐标。
fn locate(rect: Rect) -> Result<(HMONITOR, Rect), CaptureError> {
    let center = POINT { x: rect.x + rect.width as i32 / 2, y: rect.y + rect.height as i32 / 2 };
    // SAFETY: MonitorFromPoint 只读系统状态；MONITOR_DEFAULTTONEAREST 保证返回有效句柄。
    let monitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: monitor 有效，info 已按要求填好 cbSize。
    if !unsafe { GetMonitorInfoW(monitor, &raw mut info) }.as_bool() {
        return Err(CaptureError::Backend("读取显示器信息失败".into()));
    }
    let local = Rect {
        x: rect.x - info.rcMonitor.left,
        y: rect.y - info.rcMonitor.top,
        width: rect.width,
        height: rect.height,
    };
    let (monitor_width, monitor_height) =
        (info.rcMonitor.right - info.rcMonitor.left, info.rcMonitor.bottom - info.rcMonitor.top);
    let inside = local.x >= 0
        && local.y >= 0
        && local.x + local.width as i32 <= monitor_width
        && local.y + local.height as i32 <= monitor_height;
    if !inside {
        return Err(CaptureError::Backend("取景框超出显示器范围，请把框完整拖进一块屏幕".into()));
    }
    Ok((monitor, local))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SessionKey {
    monitor: isize,
    region: Rect,
}

struct LiveSession {
    key: SessionKey,
    latest: Arc<Latest>,
    control: CaptureControl<RegionHandler, HandlerError>,
}

impl LiveSession {
    fn start(monitor: HMONITOR, key: SessionKey) -> Result<Self, CaptureError> {
        let latest = Arc::new(Latest::default());
        let settings = Settings::new(
            Monitor::from_raw_hmonitor(monitor.0),
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            (latest.clone(), key.region),
        );
        let control =
            RegionHandler::start_free_threaded(settings).map_err(|error| CaptureError::Backend(error.to_string()))?;
        Ok(Self { key, latest, control })
    }

    fn close(self) {
        if let Err(error) = self.control.stop() {
            tracing::warn!(%error, "failed to stop screen capture session");
        }
    }
}

/// 回调线程写、识别线程读的最新一帧。
#[derive(Default)]
struct Latest {
    slot: Mutex<Slot>,
    arrived: Condvar,
}

#[derive(Default)]
struct Slot {
    frame: Option<Frame>,
    error: Option<String>,
}

impl Latest {
    fn publish(&self, result: Result<Frame, String>) {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        match result {
            Ok(frame) => slot.frame = Some(frame),
            Err(error) => slot.error = Some(error),
        }
        self.arrived.notify_all();
    }

    fn wait_for_frame(&self, timeout: Duration) -> Result<Frame, CaptureError> {
        let slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        let (slot, _) = self
            .arrived
            .wait_timeout_while(slot, timeout, |slot| slot.frame.is_none() && slot.error.is_none())
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = &slot.error {
            return Err(CaptureError::Backend(error.clone()));
        }
        slot.frame.clone().ok_or(CaptureError::NoFrame { timeout_ms: timeout.as_millis() as u64 })
    }
}

struct RegionHandler {
    latest: Arc<Latest>,
    region: Rect,
    scratch: Vec<u8>,
}

impl GraphicsCaptureApiHandler for RegionHandler {
    type Flags = (Arc<Latest>, Rect);
    type Error = HandlerError;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (latest, region) = ctx.flags;
        Ok(Self { latest, region, scratch: Vec::new() })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut CaptureFrame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let Rect { x, y, width, height } = self.region;
        let (x, y) = (x as u32, y as u32);
        if x + width > frame.width() || y + height > frame.height() {
            // 显示器分辨率变了：报给识别线程，由它关掉会话重建。
            self.latest.publish(Err("显示器分辨率已变化，取景框超出画面".into()));
            return Ok(());
        }
        let buffer = frame.buffer_crop(x, y, x + width, y + height)?;
        let bgra = buffer.as_nopadding_buffer(&mut self.scratch).to_vec();
        self.latest.publish(Frame::new(width, height, bgra).map_err(|error| error.to_string()));
        Ok(())
    }
}
