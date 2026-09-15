//! 截图与 OCR 的端口。都是阻塞调用，调用方负责放到阻塞线程池。

use serde::{Deserialize, Serialize};
use thiserror::Error;
use trans_domain::geometry::Rect;
use trans_domain::lang::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub process_name: String,
}

/// 连续 BGRA8 像素，无行填充。
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame").field("width", &self.width).field("height", &self.height).finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OcrLine {
    pub text: String,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CaptureError {
    #[error("找不到目标窗口，它可能已经关闭")]
    WindowNotFound,
    #[error("{timeout_ms} ms 内没有收到画面，目标窗口可能已最小化")]
    NoFrame { timeout_ms: u64 },
    #[error("截图失败：{0}")]
    Backend(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OcrError {
    #[error("系统没有安装 {0} 的 OCR 语言包")]
    LanguageNotInstalled(String),
    #[error("文字识别失败：{0}")]
    Backend(String),
}

pub trait WindowCapture: Send + Sync {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, CaptureError>;
    fn grab_frame(&self, window: WindowId) -> Result<Frame, CaptureError>;
}

pub trait OcrEngine: Send + Sync {
    fn recognize(&self, frame: &Frame, lang: &Lang) -> Result<Vec<OcrLine>, OcrError>;
}
