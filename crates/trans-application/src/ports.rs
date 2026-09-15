//! 翻译引擎与凭据存储的端口。

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use trans_domain::prompt::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    Local,
    Online,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionOptions {
    pub temperature: f32,
    pub max_tokens: u32,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub text: String,
    pub model: String,
    pub elapsed: Duration,
}

/// 引擎错误按"用户能做什么"分类，而不是按 HTTP 细节分类。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    #[error("翻译引擎暂不可用：{0}")]
    Unavailable(String),
    #[error("认证失败，请检查 API Key")]
    Unauthorized,
    #[error("在线服务账户余额不足")]
    InsufficientBalance,
    #[error("请求过于频繁，请稍后再试")]
    RateLimited,
    #[error("请求超时")]
    Timeout,
    #[error("服务返回错误（HTTP {status}）")]
    Http { status: u16, body: String },
    #[error("引擎响应格式无法解析：{0}")]
    InvalidResponse(String),
}

impl ProviderError {
    /// 余额不足、认证失败重试也没用，应该直接提示用户（M0-02 两个在线账户都遇到过）。
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Unavailable(_) | Self::RateLimited | Self::Timeout => true,
            Self::Http { status, .. } => *status >= 500,
            Self::Unauthorized | Self::InsufficientBalance | Self::InvalidResponse(_) => false,
        }
    }
}

/// 一个"能完成对话"的引擎。本地 llama-server 和在线 DeepSeek 都实现它；提示词在领域层构造，引擎不关心。
#[async_trait]
pub trait ChatEngine: Send + Sync {
    fn kind(&self) -> EngineKind;
    fn model(&self) -> &str;
    async fn complete(
        &self,
        messages: &[ChatMessage],
        options: &CompletionOptions,
    ) -> Result<Completion, ProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("凭据存储失败：{0}")]
pub struct SecretError(pub String);

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_errors_are_retryable() {
        assert!(ProviderError::Timeout.is_retryable());
        assert!(ProviderError::Http { status: 503, body: String::new() }.is_retryable());
        assert!(!ProviderError::Http { status: 400, body: String::new() }.is_retryable());
        assert!(!ProviderError::InsufficientBalance.is_retryable());
        assert!(!ProviderError::Unauthorized.is_retryable());
    }
}
