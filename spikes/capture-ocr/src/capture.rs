//! Windows.Graphics.Capture 按窗口截取单帧。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use crate::image_ops::Bgra;

/// 最小化窗口不产出帧，超时后放弃，而不是无限等待。
const FRAME_TIMEOUT: Duration = Duration::from_secs(3);

type Slot = Arc<Mutex<Option<Bgra>>>;

struct OneShot {
    slot: Slot,
}

impl GraphicsCaptureApiHandler for OneShot {
    type Flags = Slot;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { slot: ctx.flags })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, control: InternalCaptureControl) -> Result<(), Self::Error> {
        let buffer = frame.buffer()?;
        let (width, height) = (buffer.width(), buffer.height());
        let mut scratch = Vec::new();
        let pixels = buffer.as_nopadding_buffer(&mut scratch).to_vec();
        *self.slot.lock().map_err(|_| "slot poisoned")? = Some(Bgra { width, height, pixels });
        control.stop();
        Ok(())
    }
}

pub fn print_windows() -> Result<()> {
    for w in Window::enumerate()? {
        let process = w.process_name().unwrap_or_default();
        let title = w.title().unwrap_or_default();
        if !title.is_empty() {
            println!("{:<24} {}", process, title);
        }
    }
    Ok(())
}

pub fn grab_one_frame(target: &str) -> Result<Bgra> {
    let window = find_window(target)?;
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
    let control = OneShot::start_free_threaded(settings).map_err(|e| anyhow!("启动截图失败：{e}"))?;

    let deadline = Instant::now() + FRAME_TIMEOUT;
    while Instant::now() < deadline && !control.is_finished() {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !control.is_finished() {
        let _ = control.stop();
    }
    let frame = slot.lock().map_err(|_| anyhow!("slot poisoned"))?.take();
    frame.ok_or_else(|| anyhow!("{} 内没有收到帧（窗口是否最小化？）", humantime(FRAME_TIMEOUT)))
}

fn find_window(target: &str) -> Result<Window> {
    let needle = target.to_lowercase();
    let mut matches: Vec<Window> = Window::enumerate()?
        .into_iter()
        .filter(|w| {
            let title = w.title().unwrap_or_default().to_lowercase();
            let process = w.process_name().unwrap_or_default().to_lowercase();
            process == needle || title.contains(&needle)
        })
        .collect();
    match matches.len() {
        0 => bail!("找不到匹配 \"{target}\" 的窗口，先运行 list 看看"),
        1 => Ok(matches.remove(0)),
        n => {
            let names: Vec<String> = matches.iter().map(|w| w.title().unwrap_or_default()).collect();
            bail!("匹配到 {n} 个窗口，请用更精确的关键字：{names:?}")
        }
    }
}

fn humantime(d: Duration) -> String {
    format!("{}s", d.as_secs())
}
