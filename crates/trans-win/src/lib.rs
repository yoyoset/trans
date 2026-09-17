//! Windows 平台适配器，实现 `trans-application` 定义的截图、OCR、凭据端口。

#![cfg(windows)]

mod capture;
mod credential;
mod ocr;
mod screen_capture;

pub use capture::{exclude_from_capture, GraphicsCapture};
pub use credential::CredentialStore;
pub use ocr::WindowsOcr;
pub use screen_capture::MonitorCapture;
