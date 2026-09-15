//! OpenAI 兼容 `/chat/completions` 客户端。

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;
use trans_application::ports::{ChatEngine, Completion, CompletionOptions, EngineKind, ProviderError};
use trans_domain::prompt::ChatMessage;

#[derive(Clone)]
pub struct OpenAiCompatConfig {
    pub kind: EngineKind,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub reasoning_effort: Option<String>,
}

/// 手写 Debug：API Key 绝不能出现在日志里。
impl std::fmt::Debug for OpenAiCompatConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatConfig")
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("reasoning_effort", &self.reasoning_effort)
            .finish()
    }
}

pub struct OpenAiCompatEngine {
    config: OpenAiCompatConfig,
    http: reqwest::Client,
}

impl OpenAiCompatEngine {
    pub fn new(config: OpenAiCompatConfig) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| ProviderError::Unavailable(format!("无法创建 HTTP 客户端：{e}")))?;
        Ok(Self { config, http })
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

#[async_trait]
impl ChatEngine for OpenAiCompatEngine {
    fn kind(&self) -> EngineKind {
        self.config.kind
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn complete(
        &self,
        messages: &[ChatMessage],
        options: &CompletionOptions,
    ) -> Result<Completion, ProviderError> {
        let started = Instant::now();
        let mut body = serde_json::json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": options.temperature,
            "max_tokens": options.max_tokens,
            "stream": false,
        });
        if let Some(effort) = &self.config.reasoning_effort {
            body["reasoning_effort"] = serde_json::Value::String(effort.clone());
        }

        let mut request =
            self.http.post(chat_completions_url(&self.config.base_url)).timeout(options.timeout).json(&body);
        if let Some(key) = &self.config.api_key {
            request = request.bearer_auth(key);
        }
        let response = request.send().await.map_err(transport_error)?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(transport_error)?;
        if !(200..300).contains(&status) {
            return Err(classify_status(status, &text));
        }

        let parsed: ChatResponse =
            serde_json::from_str(&text).map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| ProviderError::InvalidResponse("响应中没有 choices[0].message.content".into()))?;
        Ok(Completion {
            text: content,
            model: parsed.model.unwrap_or_else(|| self.config.model.clone()),
            elapsed: started.elapsed(),
        })
    }
}

/// 兼容两种填法：`https://api.deepseek.com` 和 `.../v1/chat/completions`。
pub fn chat_completions_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_owned()
    } else {
        format!("{base}/chat/completions")
    }
}

fn transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else if error.is_connect() {
        ProviderError::Unavailable(format!("无法连接到引擎：{error}"))
    } else {
        ProviderError::Unavailable(error.to_string())
    }
}

/// 状态码 → 用户可理解的错误。有的服务余额不足时返回 401 而不是 402（M0-02：OpenCode Zen 返回 401 `CreditsError`）。
pub fn classify_status(status: u16, body: &str) -> ProviderError {
    let lower = body.to_ascii_lowercase();
    let balance = ["insufficient balance", "creditserror", "insufficient_quota"].iter().any(|m| lower.contains(m));
    match status {
        402 => ProviderError::InsufficientBalance,
        401 | 403 | 429 if balance => ProviderError::InsufficientBalance,
        401 | 403 => ProviderError::Unauthorized,
        429 => ProviderError::RateLimited,
        _ => ProviderError::Http { status, body: body.chars().take(500).collect() },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use trans_domain::prompt::Role;

    use super::*;

    #[test]
    fn classifies_real_error_bodies_seen_in_m0() {
        let zen = r#"{"type":"CreditsError","message":"Insufficient balance. Manage your billing here"}"#;
        assert_eq!(classify_status(401, zen), ProviderError::InsufficientBalance);
        assert_eq!(
            classify_status(402, r#"{"error":{"message":"Insufficient Balance"}}"#),
            ProviderError::InsufficientBalance
        );
        assert_eq!(classify_status(401, r#"{"error":"invalid api key"}"#), ProviderError::Unauthorized);
        assert_eq!(classify_status(429, "slow down"), ProviderError::RateLimited);
        assert!(matches!(classify_status(503, "busy"), ProviderError::Http { status: 503, .. }));
    }

    #[test]
    fn builds_the_endpoint_from_either_base_url_style() {
        assert_eq!(chat_completions_url("https://api.deepseek.com/"), "https://api.deepseek.com/chat/completions");
        assert_eq!(chat_completions_url("http://127.0.0.1:8765/v1"), "http://127.0.0.1:8765/v1/chat/completions");
        assert_eq!(chat_completions_url("https://x.ai/v1/chat/completions"), "https://x.ai/v1/chat/completions");
    }

    #[test]
    fn debug_output_redacts_the_api_key() {
        let config = OpenAiCompatConfig {
            kind: EngineKind::Online,
            base_url: "https://api.deepseek.com".into(),
            model: "m".into(),
            api_key: Some("sk-secret-value".into()),
            reasoning_effort: None,
        };
        let printed = format!("{config:?}");
        assert!(!printed.contains("sk-secret-value") && printed.contains("<redacted>"));
    }

    /// 起一个只应答一次的本地 HTTP 服务，返回固定的响应，并把收到的请求交还给测试。
    async fn one_shot_server(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_http_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (url, handle)
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = socket.read(&mut chunk).await.unwrap();
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf);
            if let Some(header_end) = text.find("\r\n\r\n") {
                let length = text[..header_end]
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if buf.len() >= header_end + 4 + length {
                    return String::from_utf8(buf).unwrap();
                }
            }
            if n == 0 {
                return String::from_utf8_lossy(&buf).into_owned();
            }
        }
    }

    fn engine(base_url: String, api_key: Option<&str>) -> OpenAiCompatEngine {
        OpenAiCompatEngine::new(OpenAiCompatConfig {
            kind: EngineKind::Local,
            base_url,
            model: "qwen3-4b".into(),
            api_key: api_key.map(str::to_owned),
            reasoning_effort: Some("low".into()),
        })
        .unwrap()
    }

    fn options() -> CompletionOptions {
        CompletionOptions { temperature: 0.0, max_tokens: 64, timeout: Duration::from_secs(5) }
    }

    fn messages() -> Vec<ChatMessage> {
        vec![ChatMessage { role: Role::User, content: "hi".into() }]
    }

    #[tokio::test]
    async fn sends_an_openai_request_and_reads_the_first_choice() {
        let (url, server) = one_shot_server(
            "200 OK",
            r#"{"model":"F:\\hf_cache\\Qwen3-4B.gguf","choices":[{"message":{"role":"assistant","content":"{\"translation\": \"嗨\"}"}}]}"#,
        )
        .await;
        let completion = engine(url, Some("sk-test")).complete(&messages(), &options()).await.unwrap();
        assert_eq!(completion.text, r#"{"translation": "嗨"}"#);
        assert_eq!(completion.model, r"F:\hf_cache\Qwen3-4B.gguf");

        let request = server.await.unwrap();
        assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
        assert!(request.to_ascii_lowercase().contains("authorization: bearer sk-test"));
        assert!(request.contains(r#""reasoning_effort":"low""#));
        assert!(request.contains(r#""role":"user""#));
    }

    #[tokio::test]
    async fn maps_http_errors_through_classify_status() {
        let (url, _server) = one_shot_server("402 Payment Required", r#"{"error":"Insufficient Balance"}"#).await;
        let err = engine(url, None).complete(&messages(), &options()).await.unwrap_err();
        assert_eq!(err, ProviderError::InsufficientBalance);
    }

    #[tokio::test]
    async fn an_unreachable_engine_is_reported_as_unavailable() {
        // 先绑定再释放端口，确保这个端口上没有服务。
        let port = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
        let err = engine(format!("http://127.0.0.1:{port}"), None).complete(&messages(), &options()).await.unwrap_err();
        assert!(matches!(err, ProviderError::Unavailable(_)), "{err:?}");
    }
}
