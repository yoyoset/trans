//! 应用层。依赖方向：`trans-domain ← trans-application ← 适配器（providers / win）← app 壳`。
//!
//! 这里定义"需要什么能力"（ports），适配器负责"怎么实现"。更换模型或引擎只改设置和适配器，不改这里。

pub mod capture;
pub mod ports;
pub mod router;
pub mod service;
pub mod settings;
