//! 本地 llama-server 子进程管理。
//!
//! 回收：正常退出靠 `kill_on_drop`；Windows 上另外绑定 Job Object，本进程被强杀或崩溃时也不会残留。
//!
//! 日志、Job Object、启动失败时摘日志末尾：见 `child_process`，和 opencode serve 共用。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::process::Command;
use trans_application::ports::ProviderError;

use crate::child_process::ManagedChild;

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

pub struct LlamaServer {
    process: ManagedChild,
    config: LlamaServerConfig,
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
        command.args(config.args());
        let process = ManagedChild::spawn(command, "llama-server", log_path)?;
        tracing::info!(model = %config.model_path.display(), port = config.port, threads = config.threads, "llama-server starting");
        let mut server = Self { process, config };
        server.wait_ready(ready_timeout).await?;
        tracing::info!("llama-server ready");
        Ok(server)
    }

    async fn wait_ready(&mut self, timeout: Duration) -> Result<(), ProviderError> {
        let http = reqwest::Client::new();
        let url = format!("{}/health", self.config.base_url());
        let deadline = Instant::now() + timeout;
        loop {
            let hint = format!("请检查模型文件是否完整、端口 {} 是否被占用", self.config.port);
            if let Some(report) = self.process.exit_report(&hint)? {
                return Err(ProviderError::Unavailable(format!("llama-server {report}")));
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

    pub async fn stop(self) {
        self.process.stop("llama-server").await;
    }
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

    #[tokio::test]
    async fn missing_files_are_reported_before_spawning() {
        let mut cfg = config();
        cfg.server_path = PathBuf::from(r"Z:\definitely\missing\llama-server.exe");
        let err = LlamaServer::start(cfg, None, Duration::from_secs(1)).await.err().unwrap();
        assert!(matches!(err, ProviderError::Unavailable(msg) if msg.contains("llama-server 程序不存在")));
    }
}
