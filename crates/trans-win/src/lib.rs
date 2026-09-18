//! Windows 平台适配器，实现 `trans-application` 定义的截图、OCR、凭据端口。

#![cfg(windows)]

mod capture;
mod credential;
mod ocr;
mod screen_capture;
mod send_text;

pub use capture::{clip_to_circle, exclude_from_capture, GraphicsCapture};
pub use credential::CredentialStore;
pub use ocr::WindowsOcr;
pub use screen_capture::MonitorCapture;
pub use send_text::{paste_at, set_clipboard_text};
