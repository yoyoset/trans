//! Windows.Graphics.Capture 按窗口截图。
//!
//! 当前实现每次调用都新建截图会话、取一帧后结束。M0-01 实测建立会话到首帧要 594 ms，
//! 持续识别时必须改成常驻会话（已列入 architecture.md「已知问题」）。

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use trans_application::capture::{CaptureError, Frame, WindowCapture, WindowId, WindowInfo};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as CaptureFrame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
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

        let slot: Slot = Arc::new(Mutex::new(None));
        let settings = Settings::new(
            window,
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
}

type Slot = Arc<Mutex<Option<Frame>>>;

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
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(Frame { width, height, bgra });
        control.stop();
        Ok(())
    }
}
