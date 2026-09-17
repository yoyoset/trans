//! 翻译引擎适配器。
//!
//! 本地和在线（API Key 方式）走同一个 OpenAI 兼容客户端：本地是 `http://127.0.0.1:<port>` 上的 llama-server，
//! 在线是 DeepSeek 等服务。在线也可以走 opencode 订阅（`opencode`）。换模型、换服务商都只是换配置。

pub mod child_process;
#[cfg(windows)]
mod job_object;
pub mod llama_server;
pub mod model_discovery;
pub mod openai_compat;
pub mod opencode;
