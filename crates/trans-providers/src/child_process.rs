//! 后台子进程（llama-server、opencode serve）的共同做法：不弹控制台窗口、输出写日志文件、
//! 生死绑在本进程上、启动失败时从日志末尾摘出原因。

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::{Child, Command};
use trans_application::ports::ProviderError;

/// 启动失败时，错误信息里最多带日志末尾几行。
const FAILURE_LOG_LINES: usize = 6;
/// 单行过长（例如一整段张量信息）时截断，避免错误提示刷屏。
const FAILURE_LOG_LINE_CHARS: usize = 200;

/// 运行中的后台子进程。drop 时结束进程（`kill_on_drop`），本进程被强杀时由 Job Object 兜底。
pub struct ManagedChild {
    pub child: Child,
    log_path: Option<PathBuf>,
    /// 放在 `child` 之后：drop 时先由 `kill_on_drop` 正常结束，再关闭 Job 句柄。
    #[cfg(windows)]
    _job: Option<crate::job_object::KillOnCloseJob>,
}

impl ManagedChild {
    /// `name` 只用于日志和错误信息；`log_path` 为空时丢弃输出（测试用）。
    pub fn spawn(mut command: Command, name: &str, log_path: Option<&Path>) -> Result<Self, ProviderError> {
        let (stdout, stderr) = log_output(name, log_path);
        command.stdin(Stdio::null()).stdout(stdout).stderr(stderr).kill_on_drop(true);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command.spawn().map_err(|e| ProviderError::Unavailable(format!("启动 {name} 失败：{e}")))?;
        #[cfg(windows)]
        let _job = bind_to_this_process(&child, name);
        Ok(Self {
            child,
            log_path: log_path.map(Path::to_path_buf),
            #[cfg(windows)]
            _job,
        })
    }

    /// 进程已经退出时返回带日志末尾的说明；还在运行返回 `None`。
    pub fn exit_report(&mut self, hint: &str) -> Result<Option<String>, ProviderError> {
        let Some(status) = self.child.try_wait().map_err(|e| ProviderError::Unavailable(e.to_string()))? else {
            return Ok(None);
        };
        let mut message = format!("启动后退出（{status}），{hint}");
        if let Some(path) = &self.log_path {
            let log = std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
            if let Some(tail) = log.ok().as_deref().and_then(log_tail) {
                message.push_str(&format!("\n日志末尾：\n{tail}"));
            }
            message.push_str(&format!("\n完整日志：{}", path.display()));
        }
        Ok(Some(message))
    }

    pub async fn stop(mut self, name: &str) {
        if let Err(error) = self.child.kill().await {
            tracing::warn!(%error, name, "failed to stop child process");
        }
    }
}

/// 打开日志文件作为子进程的 stdout 和 stderr（每次启动覆盖）；打不开就丢弃输出，不影响启动。
fn log_output(name: &str, log_path: Option<&Path>) -> (Stdio, Stdio) {
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
            tracing::warn!(%error, name, "cannot open child process log; its output will be discarded");
            (Stdio::null(), Stdio::null())
        }
        None => (Stdio::null(), Stdio::null()),
    }
}

/// 日志末尾几行非空内容，过长的行截断。日志为空时返回 `None`。
pub fn log_tail(log: &str) -> Option<String> {
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
        .join("\n");
    Some(tail)
}

/// 绑定失败不影响使用，只是回到"强杀时可能残留"的状态，所以记日志而不是报错。
#[cfg(windows)]
fn bind_to_this_process(child: &Child, name: &str) -> Option<crate::job_object::KillOnCloseJob> {
    let handle = child.raw_handle()?;
    crate::job_object::KillOnCloseJob::adopt(handle.cast())
        .inspect_err(|error| tracing::warn!(%error, name, "child process not bound to a job object"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_tail_keeps_last_non_empty_lines_and_truncates_long_ones() {
        let long = "x".repeat(FAILURE_LOG_LINE_CHARS + 50);
        let log = format!("a\nb\n\nc\nd\ne\nf\n{long}\ngguf_init: failed to open model\n\n");
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
        assert_eq!(log_tail("\n  \n"), None);
    }
}
