//! 截图与 OCR 的端口。两者都是阻塞调用，调用方负责放到阻塞线程池里执行。

use serde::{Deserialize, Serialize};
use thiserror::Error;
use trans_domain::geometry::Rect;
use trans_domain::lang::Lang;

pub use trans_domain::chat_lines::OcrLine;
pub use trans_domain::frame::{Frame, FrameError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub process_name: String,
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

/// 按屏幕绝对坐标截取一块区域，供"取景框"使用：用户把框拖到哪里就识别哪里，
/// 不关心底下是哪个窗口。实现必须把本程序自己的窗口排除在外，否则只会拍到取景框自己。
pub trait ScreenCapture: Send + Sync {
    fn grab_region(&self, rect: Rect) -> Result<Frame, CaptureError>;

    /// 识别停止时调用：实现可以借此关闭常驻截图会话等资源。下次 `grab_region` 时按需重建。
    fn release(&self) {}
}

pub trait OcrEngine: Send + Sync {
    fn recognize(&self, frame: &Frame, lang: &Lang) -> Result<Vec<OcrLine>, OcrError>;
}
