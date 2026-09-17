//! 通过本机 opencode 程序使用它已登录的订阅（例如 OpenCode Go），不需要在本软件里填 API Key。
//!
//! 做法：后台常驻一个 `opencode serve`，每次翻译/校对建一个临时会话、发一条消息、取回文字后删除会话。
//!
//! 实测依据（opencode 1.18.18，2026-09-17，都没有调用模型、不产生费用）：
//! - `serve` 从启动到接口可用约 3.3 s → 必须常驻，不能每句启动一次。
//! - 不设 `OPENCODE_SERVER_PASSWORD` 时它会警告"server is unsecured"：本机任何程序都能借它调用订阅和
//!   bash 等工具。所以每次启动生成随机密码，走 HTTP Basic（用户名 `opencode`）；不带密码访问返回 401。
//! - 模型不存在时返回 HTTP 500 `UnknownError`，不会走到计费。
//! - M0-02：每次请求会额外带约 4k token 的 agent 系统提示词（走缓存）；所有工具按请求显式关闭。

use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;
use trans_application::ports::{ChatEngine, Completion, CompletionOptions, EngineKind, ProviderError};
use trans_domain::prompt::{ChatMessage, Role};

use crate::child_process::ManagedChild;

const USERNAME: &str = "opencode";
const EXECUTABLE: &str = "opencode.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpencodeServerConfig {
    pub program: PathBuf,
    /// 服务的工作目录。会话的 directory 字段指向这里，放在应用数据目录下，不碰用户文件。
    pub work_dir: PathBuf,
}

/// 运行中的 `opencode serve`。
pub struct OpencodeServer {
    process: ManagedChild,
    config: OpencodeServerConfig,
    base_url: String,
    password: String,
}

impl OpencodeServer {
    pub async fn start(
        config: OpencodeServerConfig,
        log_path: Option<&Path>,
        ready_timeout: Duration,
    ) -> Result<Self, ProviderError> {
        if !config.program.is_file() {
            return Err(ProviderError::Unavailable(format!("opencode 程序不存在：{}", config.program.display())));
        }
        std::fs::create_dir_all(&config.work_dir)
            .map_err(|e| ProviderError::Unavailable(format!("无法创建 opencode 工作目录：{e}")))?;
        let port = free_port()?;
        let password = random_token();

        let mut command = Command::new(&config.program);
        command
            .args(["serve", "--hostname", "127.0.0.1", "--port", &port.to_string()])
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .env("OPENCODE_SERVER_USERNAME", USERNAME)
            .current_dir(&config.work_dir);
        let process = ManagedChild::spawn(command, "opencode", log_path)?;
        tracing::info!(program = %config.program.display(), port, "opencode serve starting");

        let mut server = Self { process, config, base_url: format!("http://127.0.0.1:{port}"), password };
        server.wait_ready(ready_timeout).await?;
        tracing::info!("opencode serve ready");
        Ok(server)
    }

    async fn wait_ready(&mut self, timeout: Duration) -> Result<(), ProviderError> {
        let http = reqwest::Client::new();
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(report) = self.process.exit_report("请确认 opencode 能在命令行正常运行")? {
                return Err(ProviderError::Unavailable(format!("opencode {report}")));
            }
            let response = http
                .get(format!("{}/experimental/tool/ids", self.base_url))
                .basic_auth(USERNAME, Some(&self.password))
                .timeout(Duration::from_secs(2))
                .send()
                .await;
            if response.is_ok_and(|r| r.status().is_success()) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProviderError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    pub fn config(&self) -> &OpencodeServerConfig {
        &self.config
    }

    /// 为指定模型创建引擎。会先取一次工具列表，之后每个请求都把它们全部关闭。
    pub async fn engine(&self, model: &str, variant: Option<String>) -> Result<OpencodeEngine, ProviderError> {
        let target = ModelRef::parse(model)?;
        let client = Client { http: http_client()?, base_url: self.base_url.clone(), password: self.password.clone() };
        let tool_ids: Vec<String> =
            client.send_json(reqwest::Method::GET, "/experimental/tool/ids", None, None).await?;
        let tools_off = tool_ids.into_iter().map(|id| (id, false)).collect();
        Ok(OpencodeEngine { client, model: model.to_owned(), target, variant, tools_off })
    }

    pub async fn stop(self) {
        self.process.stop("opencode").await;
    }
}

/// 找 opencode 程序：先找 PATH 里的 `opencode.exe`，再找 npm 全局安装位置。
/// npm 装的 `opencode.cmd` 只是转发到 `node_modules\opencode-ai\bin\opencode.exe`，直接启动 exe 少一层 cmd。
pub fn find_opencode(path_var: Option<&str>, appdata: Option<&Path>) -> Option<PathBuf> {
    let on_path = path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(EXECUTABLE))
        .find(|candidate| candidate.is_file());
    on_path.or_else(|| {
        let npm = appdata?.join("npm").join("node_modules").join("opencode-ai").join("bin").join(EXECUTABLE);
        npm.is_file().then_some(npm)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelRef {
    provider_id: String,
    model_id: String,
}

impl ModelRef {
    /// `opencode-go/deepseek-v4-flash` → 提供方 + 模型。模型名本身可能还带斜杠，只在第一个斜杠处切开。
    fn parse(model: &str) -> Result<Self, ProviderError> {
        match model.trim().split_once('/') {
            Some((provider, id)) if !provider.is_empty() && !id.is_empty() => {
                Ok(Self { provider_id: provider.to_owned(), model_id: id.to_owned() })
            }
            _ => Err(ProviderError::Unavailable(format!(
                "opencode 模型名应为「提供方/模型」，例如 opencode-go/deepseek-v4-flash，当前是「{model}」"
            ))),
        }
    }
}

pub struct OpencodeEngine {
    client: Client,
    model: String,
    target: ModelRef,
    variant: Option<String>,
    tools_off: BTreeMap<String, bool>,
}

#[async_trait]
impl ChatEngine for OpencodeEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Online
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(
        &self,
        messages: &[ChatMessage],
        options: &CompletionOptions,
    ) -> Result<Completion, ProviderError> {
        let started = Instant::now();
        let session: SessionInfo = self
            .client
            .send_json(reqwest::Method::POST, "/session", Some(serde_json::json!({ "title": "trans" })), None)
            .await?;
        let body = message_body(messages, &self.target, self.variant.as_deref(), &self.tools_off);
        let reply: Result<serde_json::Value, ProviderError> = self
            .client
            .send_json(
                reqwest::Method::POST,
                &format!("/session/{}/message", session.id),
                Some(body),
                Some(options.timeout),
            )
            .await;
        // 不管成功失败都删会话，否则 opencode 的会话列表会越积越多。
        let deleted =
            self.client.send_raw(reqwest::Method::DELETE, &format!("/session/{}", session.id), None, None).await;
        if let Err(error) = deleted {
            tracing::warn!(%error, "failed to delete opencode session");
        }
        let text = reply_text(&reply?)?;
        Ok(Completion { text, model: self.model.clone(), elapsed: started.elapsed() })
    }
}

/// 我们的提示词固定是"系统 + 用户"两条；opencode 的接口是一个 system 字段加若干文本片段。
fn message_body(
    messages: &[ChatMessage],
    target: &ModelRef,
    variant: Option<&str>,
    tools_off: &BTreeMap<String, bool>,
) -> serde_json::Value {
    let join = |role: Role| {
        messages.iter().filter(|m| m.role == role).map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n\n")
    };
    let parts: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| serde_json::json!({ "type": "text", "text": m.content }))
        .collect();
    let mut body = serde_json::json!({
        "model": { "providerID": target.provider_id, "modelID": target.model_id },
        "system": join(Role::System),
        "tools": tools_off,
        "parts": parts,
    });
    if let Some(variant) = variant.filter(|v| !v.trim().is_empty()) {
        body["variant"] = serde_json::Value::String(variant.to_owned());
    }
    body
}

#[derive(Deserialize)]
struct SessionInfo {
    id: String,
}

/// 回复里的文字片段拼起来；没有文字但带了错误时按错误分类。
fn reply_text(reply: &serde_json::Value) -> Result<String, ProviderError> {
    let text: String = reply["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| part["type"] == "text")
        .filter_map(|part| part["text"].as_str())
        .collect();
    if !text.trim().is_empty() {
        return Ok(text);
    }
    match reply["info"].get("error") {
        Some(error) if !error.is_null() => Err(classify_reply_error(error)),
        _ => Err(ProviderError::InvalidResponse("opencode 回复里没有文字".into())),
    }
}

/// opencode 把服务商的错误包在 `info.error` 里：`{name, data: {message, statusCode?}}`。
fn classify_reply_error(error: &serde_json::Value) -> ProviderError {
    let name = error["name"].as_str().unwrap_or_default();
    let message = error["data"]["message"].as_str().unwrap_or(name);
    let status = error["data"]["statusCode"].as_u64().and_then(|s| u16::try_from(s).ok());
    if name.contains("Auth") {
        return ProviderError::Unauthorized;
    }
    match status {
        Some(status) => crate::openai_compat::classify_status(status, message),
        None => {
            let lower = message.to_ascii_lowercase();
            if ["credits", "insufficient balance", "quota"].iter().any(|m| lower.contains(m)) {
                ProviderError::InsufficientBalance
            } else {
                ProviderError::Unavailable(format!("opencode：{message}"))
            }
        }
    }
}

struct Client {
    http: reqwest::Client,
    base_url: String,
    password: String,
}

impl Client {
    async fn send_raw(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<String, ProviderError> {
        let mut request = self
            .http
            .request(method, format!("{}{route}", self.base_url))
            .basic_auth(USERNAME, Some(&self.password))
            .timeout(timeout.unwrap_or(Duration::from_secs(10)));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                ProviderError::Timeout
            } else {
                ProviderError::Unavailable(format!("无法连接 opencode：{error}"))
            }
        })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        match status {
            200..=299 => Ok(text),
            // 401 来自 opencode 服务本身（我们的访问密码不对），不是订阅的认证问题。
            401 => Err(ProviderError::Unavailable("opencode 服务拒绝访问，请重启引擎".into())),
            _ => Err(ProviderError::Http { status, body: text.chars().take(500).collect() }),
        }
    }

    async fn send_json<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<T, ProviderError> {
        let text = self.send_raw(method, route, body, timeout).await?;
        serde_json::from_str(&text).map_err(|e| ProviderError::InvalidResponse(format!("opencode {route}：{e}")))
    }
}

fn http_client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder().build().map_err(|e| ProviderError::Unavailable(format!("无法创建 HTTP 客户端：{e}")))
}

/// 让系统分配一个空闲端口。释放后到 opencode 绑定之间有极短的空窗，被抢占时启动会失败并报出日志。
fn free_port() -> Result<u16, ProviderError> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|e| ProviderError::Unavailable(format!("找不到可用端口：{e}")))
}

/// 本次运行的访问密码。`RandomState` 的密钥来自系统随机源，两个 64 位拼成 128 位，足够挡住本机其它程序猜测。
fn random_token() -> String {
    let (a, b) = (RandomState::new().hash_one(0u8), RandomState::new().hash_one(1u8));
    format!("{a:016x}{b:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage { role: Role::System, content: "你是翻译引擎".into() },
            ChatMessage { role: Role::User, content: "<source>hello</source>".into() },
        ]
    }

    #[test]
    fn model_ref_splits_at_the_first_slash_only() {
        let model = ModelRef::parse("openrouter/deepseek/deepseek-v4-flash").unwrap();
        assert_eq!(model.provider_id, "openrouter");
        assert_eq!(model.model_id, "deepseek/deepseek-v4-flash");
        assert!(ModelRef::parse("deepseek-v4-flash").is_err());
        assert!(ModelRef::parse("/x").is_err());
    }

    #[test]
    fn message_body_puts_the_system_prompt_aside_and_disables_every_tool() {
        let tools = BTreeMap::from([("bash".to_owned(), false), ("edit".to_owned(), false)]);
        let target = ModelRef::parse("opencode-go/deepseek-v4-flash").unwrap();
        let body = message_body(&messages(), &target, Some("low"), &tools);
        assert_eq!(body["model"]["providerID"], "opencode-go");
        assert_eq!(body["model"]["modelID"], "deepseek-v4-flash");
        assert_eq!(body["system"], "你是翻译引擎");
        assert_eq!(body["parts"], serde_json::json!([{ "type": "text", "text": "<source>hello</source>" }]));
        assert_eq!(body["tools"], serde_json::json!({ "bash": false, "edit": false }));
        assert_eq!(body["variant"], "low");
        assert!(message_body(&messages(), &target, Some(" "), &tools).get("variant").is_none());
    }

    #[test]
    fn reply_text_joins_text_parts_and_ignores_the_rest() {
        let reply = serde_json::json!({
            "info": {},
            "parts": [
                { "type": "step-start" },
                { "type": "text", "text": "{\"translation\":" },
                { "type": "text", "text": " \"你好\"}" },
            ]
        });
        assert_eq!(reply_text(&reply).unwrap(), "{\"translation\": \"你好\"}");
    }

    #[test]
    fn reply_errors_become_actionable_provider_errors() {
        let reply = |error: serde_json::Value| serde_json::json!({ "info": { "error": error }, "parts": [] });
        assert_eq!(
            reply_text(&reply(serde_json::json!({ "name": "ProviderAuthError", "data": { "message": "bad" } }))),
            Err(ProviderError::Unauthorized)
        );
        // M0-02 实测过的 OpenCode Zen 余额不足。
        assert_eq!(
            reply_text(&reply(serde_json::json!({
                "name": "APIError",
                "data": { "message": "{\"type\":\"CreditsError\",\"message\":\"Insufficient balance\"}", "statusCode": 401 }
            }))),
            Err(ProviderError::InsufficientBalance)
        );
        assert_eq!(
            reply_text(&reply(
                serde_json::json!({ "name": "APIError", "data": { "message": "slow", "statusCode": 429 } })
            )),
            Err(ProviderError::RateLimited)
        );
        assert!(matches!(reply_text(&serde_json::json!({ "parts": [] })), Err(ProviderError::InvalidResponse(_))));
    }

    #[test]
    fn finds_opencode_on_path_before_the_npm_location() {
        let dir = tempfile::tempdir().unwrap();
        let on_path = dir.path().join("bin");
        let npm_bin = dir.path().join("appdata").join("npm").join("node_modules").join("opencode-ai").join("bin");
        for folder in [&on_path, &npm_bin] {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(folder.join(EXECUTABLE), b"").unwrap();
        }
        let appdata = dir.path().join("appdata");
        let path_var = std::env::join_paths([dir.path().join("empty"), on_path.clone()]).unwrap();

        assert_eq!(find_opencode(path_var.to_str(), Some(&appdata)), Some(on_path.join(EXECUTABLE)));
        assert_eq!(find_opencode(None, Some(&appdata)), Some(npm_bin.join(EXECUTABLE)));
        assert_eq!(find_opencode(None, Some(&dir.path().join("nowhere"))), None);
    }

    #[test]
    fn access_tokens_are_long_and_differ_between_runs() {
        let (a, b) = (random_token(), random_token());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
