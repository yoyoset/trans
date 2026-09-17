//! 把子进程的生死绑在本进程上：本进程怎么退出（包括被任务管理器强杀、崩溃），子进程都跟着结束。
//!
//! 原理：子进程加入一个设了 `KILL_ON_JOB_CLOSE` 的 Job Object，句柄只由本进程持有。
//! 本进程一结束，系统关闭句柄，Job 里的进程被一并终止。`kill_on_drop` 只覆盖正常退出，覆盖不了强杀。

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// 持有期间子进程受约束；drop 时关闭句柄，Job 里的进程随之结束。
#[derive(Debug)]
pub struct KillOnCloseJob {
    handle: HANDLE,
}

// SAFETY: Job Object 句柄是内核对象句柄，可以在线程间转移和共享使用。
unsafe impl Send for KillOnCloseJob {}
unsafe impl Sync for KillOnCloseJob {}

impl KillOnCloseJob {
    /// `process` 是子进程的原始句柄（例如 `tokio::process::Child::raw_handle`）。
    pub fn adopt(process: *mut std::ffi::c_void) -> windows::core::Result<Self> {
        // SAFETY: 不带名字和安全属性创建匿名 Job，失败时返回错误。
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }?;
        let job = Self { handle };

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: limits 是按该信息类要求的结构体，长度与之一致。
        unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }?;
        // SAFETY: process 由调用方保证是存活子进程的有效句柄。
        unsafe { AssignProcessToJobObject(job.handle, HANDLE(process)) }?;
        Ok(job)
    }
}

impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        // SAFETY: handle 由 CreateJobObjectW 创建，只在这里关闭一次。
        if let Err(error) = unsafe { CloseHandle(self.handle) } {
            tracing::warn!(%error, "failed to close job object handle");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::io::AsRawHandle;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;

    /// 关掉 Job 句柄（等同于本进程被强杀时系统做的事）后，子进程必须结束。
    #[test]
    fn child_dies_when_job_handle_closes() {
        let mut child = Command::new("cmd")
            .args(["/c", "ping -n 30 127.0.0.1 >nul"])
            .stdout(Stdio::null())
            .spawn()
            .expect("cmd is available on Windows");
        let job = KillOnCloseJob::adopt(child.as_raw_handle().cast()).expect("job assignment works");
        assert!(child.try_wait().unwrap().is_none(), "child should still be running while the job is held");

        drop(job);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "child survived closing the job handle");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
