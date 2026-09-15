//! 翻译引擎适配器。
//!
//! 本地和在线引擎走同一个 OpenAI 兼容客户端：本地是 `http://127.0.0.1:<port>` 上的 llama-server，
//! 在线是 DeepSeek 等服务。换模型、换服务商都只是换配置。

pub mod llama_server;
pub mod model_discovery;
pub mod openai_compat;
