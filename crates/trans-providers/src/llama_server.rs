//! 本地 llama-server 子进程管理。
//!
//! 回收：正常退出靠 `kill_on_drop`；Windows 上另外绑定 Job Object，本进程被强杀或崩溃时也不会残留。
//!
//! 日志：llama-server 的 stdout/stderr 写入调用方给的日志文件，每次启动覆盖（大小不会无限增长）。
//! 启动失败时从日志末尾摘几行放进错误信息，用户不用自己翻日志就能看到原因。

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::process::{Child, Command};
use trans_application::ports::ProviderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlamaServerConfig {
    pub server_path: PathBuf,
    pub model_path: PathBuf,
    pub threads: u16,
    pub context_size: u32,
    pub port: u16,
}

impl LlamaServerConfig {
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// 强制纯 CPU（`-dev none -ngl 0`）：这台机器日常被多开暗黑2 占满 GPU，不能假设显卡可用。
    /// 单槽位（`-np 1`）：同一时间只翻译一句，且系统提示词前缀可以一直命中缓存。
    pub fn args(&self) -> Vec<String> {
        [
            "-m",
            &self.model_path.to_string_lossy(),
            "-dev",
            "none",
            "-ngl",
            "0",
            "-t",
            &self.threads.to_string(),
            "-c",
            &self.context_size.to_string(),
            "-np",
            "1",
            "--host",
            "127.0.0.1",
            "--port",
            &self.port.to_string(),
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    }
}

/// 启动失败时，错误信息里最多带日志末尾几行。
const FAILURE_LOG_LINES: usize = 6;
/// 单行过长（例如一整段张量信息）时截断，避免错误提示刷屏。
const FAILURE_LOG_LINE_CHARS: usize = 200;

pub struct LlamaServer {
    child: Child,
    config: LlamaServerConfig,
    log_path: Option<PathBuf>,
    /// 和 `child` 同生命周期；放在 `child` 之后，drop 时先由 `kill_on_drop` 正常结束。
    #[cfg(windows)]
    _job: Option<crate::job_object::KillOnCloseJob>,
}

impl LlamaServer {
    /// 启动并等待 `/health` 返回 200。模型加载期间 `/health` 返回 503，属于正常等待。
    /// `log_path` 为空时丢弃输出（测试用）。
    pub async fn start(
        config: LlamaServerConfig,
        log_path: Option<&Path>,
        ready_timeout: Duration,
    ) -> Result<Self, ProviderError> {
        for (label, path) in [("llama-server 程序", &config.server_path), ("模型文件", &config.model_path)] {
            if !path.is_file() {
                return Err(ProviderError::Unavailable(format!("{label}不存在：{}", path.display())));
            }
        }

        let mut command = Command::new(&config.server_path);
        let (stdout, stderr) = log_output(log_path);
        command.args(config.args()).stdin(Stdio::null()).stdout(stdout).stderr(stderr).kill_on_drop(true);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command.spawn().map_err(|e| ProviderError::Unavailable(format!("启动 llama-server 失败：{e}")))?;

        tracing::info!(model = %config.model_path.display(), port = config.port, threads = config.threads, "llama-server starting");
        #[cfg(windows)]
        let _job = bind_to_this_process(&child);
        let mut server = Self {
            child,
            config,
            log_path: log_path.map(Path::to_path_buf),
            #[cfg(windows)]
            _job,
        };
        server.wait_ready(ready_timeout).await?;
        tracing::info!("llama-server ready");
        Ok(server)
    }

    async fn wait_ready(&mut self, timeout: Duration) -> Result<(), ProviderError> {
        let http = reqwest::Client::new();
        let url = format!("{}/health", self.config.base_url());
        let deadline = Instant::now() + timeout;
        loop {
            let exited = self.child.try_wait().map_err(|e| ProviderError::Unavailable(e.to_string()))?;
            if let Some(status) = exited {
                let mut message = format!(
                    "llama-server 启动后退出（{status}），请检查模型文件是否完整、端口 {} 是否被占用",
                    self.config.port
                );
                if let Some(path) = &self.log_path {
                    let log = std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                    if let Some(tail) = log.ok().as_deref().and_then(log_tail) {
                        message.push_str(&format!(
                            "
日志末尾：
{tail}"
                        ));
                    }
                    message.push_str(&format!(
                        "
完整日志：{}",
                        path.display()
                    ));
                }
                return Err(ProviderError::Unavailable(message));
            }
            let healthy = http
                .get(&url)
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success());
            if healthy {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProviderError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    pub fn config(&self) -> &LlamaServerConfig {
        &self.config
    }

    pub async fn stop(mut self) {
        if let Err(error) = self.child.kill().await {
            tracing::warn!(%error, "failed to stop llama-server");
        }
    }
}

/// 打开日志文件作为子进程的 stdout 和 stderr；打不开就丢弃输出，不影响启动。
fn log_output(log_path: Option<&Path>) -> (Stdio, Stdio) {
    let opened = log_path.map(|path| {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = File::create(path)?;
        Ok::<_, std::io::Error>((file.try_clone()?, file))
    });
    match opened {
        Some(Ok((stdout, stderr))) => (stdout.into(), stderr.into()),
        Some(Err(error)) => {
            tracing::warn!(%error, "cannot open llama-server log; its output will be discarded");
            (Stdio::null(), Stdio::null())
        }
        None => (Stdio::null(), Stdio::null()),
    }
}

/// 日志末尾几行非空内容，过长的行截断。日志为空时返回 `None`。
fn log_tail(log: &str) -> Option<String> {
    let lines: Vec<&str> = log.lines().map(str::trim_end).filter(|line| !line.trim().is_empty()).collect();
    if lines.is_empty() {
        return None;
    }
    let tail = lines[lines.len().saturating_sub(FAILURE_LOG_LINES)..]
        .iter()
        .map(|line| match line.char_indices().nth(FAILURE_LOG_LINE_CHARS) {
            Some((cut, _)) => format!("{}…", &line[..cut]),
            None => (*line).to_owned(),
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    Some(tail)
}

/// 绑定失败不影响使用，只是回到"强杀时可能残留"的状态，所以记日志而不是报错。
#[cfg(windows)]
fn bind_to_this_process(child: &Child) -> Option<crate::job_object::KillOnCloseJob> {
    let handle = child.raw_handle()?;
    crate::job_object::KillOnCloseJob::adopt(handle.cast())
        .inspect_err(|error| tracing::warn!(%error, "llama-server not bound to a job object; it may outlive a crash"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LlamaServerConfig {
        LlamaServerConfig {
            server_path: PathBuf::from(r"F:\my_ai\subgen\dist\translator\llama-server.exe"),
            model_path: PathBuf::from(r"F:\hf_cache\Qwen3-4B-Instruct-2507-Q4_K_M.gguf"),
            threads: 8,
            context_size: 4096,
            port: 8765,
        }
    }

    #[test]
    fn args_force_cpu_single_slot_and_localhost() {
        let args = config().args().join(" ");
        assert_eq!(
            args,
            r"-m F:\hf_cache\Qwen3-4B-Instruct-2507-Q4_K_M.gguf -dev none -ngl 0 -t 8 -c 4096 -np 1 --host 127.0.0.1 --port 8765"
        );
        assert_eq!(config().base_url(), "http://127.0.0.1:8765");
    }

    #[test]
    fn log_tail_keeps_last_non_empty_lines_and_truncates_long_ones() {
        let long = "x".repeat(FAILURE_LOG_LINE_CHARS + 50);
        let log = format!(
            "a
b

c
d
e
f
{long}
gguf_init: failed to open model

"
        );
        let tail = log_tail(&log).unwrap();
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), FAILURE_LOG_LINES);
        assert_eq!(lines[0], "c");
        assert!(lines[4].ends_with('…') && lines[4].chars().count() == FAILURE_LOG_LINE_CHARS + 1);
        assert_eq!(lines[5], "gguf_init: failed to open model");
    }

    #[test]
    fn log_tail_of_empty_log_is_none() {
        assert_eq!(log_tail(""), None);
        assert_eq!(
            log_tail(
                "
  
"
            ),
            None
        );
    }

    #[tokio::test]
    async fn missing_files_are_reported_before_spawning() {
        let mut cfg = config();
        cfg.server_path = PathBuf::from(r"Z:\definitely\missing\llama-server.exe");
        let err = LlamaServer::start(cfg, None, Duration::from_secs(1)).await.err().unwrap();
        assert!(matches!(err, ProviderError::Unavailable(msg) if msg.contains("llama-server 程序不存在")));
    }
}
