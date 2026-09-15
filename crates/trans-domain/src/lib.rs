//! 领域核心：值类型与纯函数。
//!
//! 依赖方向的最内层——不允许引入 IO、async、网络或任何平台 crate。
//! 这里的每个函数都应能在没有模型、没有窗口、没有网络的环境下单测。

pub mod conversation;
pub mod geometry;
pub mod glossary;
pub mod lang;
pub mod ocr_text;
pub mod output;
pub mod prompt;
pub mod region;
pub mod validate;
