//! Windows 平台适配器，实现 `trans-application` 定义的截图、OCR、凭据端口。

#![cfg(windows)]

mod capture;
mod credential;
mod ocr;

pub use capture::GraphicsCapture;
pub use credential::CredentialStore;
pub use ocr::WindowsOcr;
