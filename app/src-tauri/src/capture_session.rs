//! 后台识别会话：定时抓取 → 识别 → 把新出现的对方消息推给取景框面板。
//!
//! 这里不翻译：用户在面板上点哪句才翻译哪句。一屏刚识别出十几句时，如果全部自动送去本地模型
//! （每句约 1.9 s），会排很长的队，而用户往往只关心其中一两句。
//! 怎么抓、怎么解析在 `trans-application` 的读取管道里；这里只决定"什么时候抓"（见 [`RefreshControl`]）
//! 和"抓到之后推给谁"。

use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use trans_application::capture::WindowInfo;
use trans_application::capture_pipeline::{CapturePipeline, CaptureSource, CaptureTarget};
use trans_domain::chat_lines::ParsedMessage;
use trans_domain::conversation::Speaker;
use trans_domain::geometry::Rect;

use crate::error::CommandError;
use crate::refresh::RefreshControl;

pub const CAPTURE_LINES_EVENT: &str = "capture-lines";
pub const CAPTURE_STATUS_EVENT: &str = "capture-status";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaptureInfo {
    /// 界面上显示的目标名字：取景框显示尺寸，跟随窗口显示窗口标题。
    pub label: String,
    pub preset_id: String,
    pub preset_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct CaptureStatus {
    /// 正在识别的目标；为空表示没在识别。
    pub running: Option<CaptureInfo>,
    /// 最近一次抓取的错误（例如窗口最小化），给界面显示用。
    pub last_error: Option<String>,
    /// 最近一次抓取识别到的消息条数。
    pub last_seen: usize,
}

pub struct CaptureSession {
    app: AppHandle,
    pipeline: Option<Arc<CapturePipeline>>,
    task: Mutex<Option<JoinHandle<()>>>,
    status: RwLock<CaptureStatus>,
    /// 当前识别目标。后台循环每一轮都读一次，所以识别中挪动取景框、改大小都能立即生效。
    target: Arc<RwLock<Option<CaptureTarget>>>,
    /// 刷新节奏。不随停止/开始重置，用户选了手动就一直是手动。
    refresh: Arc<RefreshControl>,
}

impl CaptureSession {
    pub fn new(app: AppHandle, pipeline: Option<Arc<CapturePipeline>>) -> Self {
        Self {
            app,
            pipeline,
            task: Mutex::new(None),
            status: RwLock::new(CaptureStatus::default()),
            target: Arc::new(RwLock::new(None)),
            refresh: Arc::new(RefreshControl::default()),
        }
    }

    /// 当前平台是否支持识别（非 Windows 上没有截图和 OCR 适配器）。
    pub fn is_supported(&self) -> bool {
        self.pipeline.is_some()
    }

    pub fn status(&self) -> CaptureStatus {
        self.status.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, CommandError> {
        self.pipeline()?.list_windows().map_err(|error| CommandError::new("capture_failed", error.to_string()))
    }

    pub async fn start(&self, target: CaptureTarget, info: CaptureInfo) -> Result<CaptureStatus, CommandError> {
        let pipeline = self.pipeline()?;
        self.stop().await;
        // 换目标就清空判重历史，否则新位置上和之前相同的话会被当成重复丢掉。
        pipeline.reset();

        *self.target.write().unwrap_or_else(PoisonError::into_inner) = Some(target);
        let (app, shared, refresh) = (self.app.clone(), self.target.clone(), self.refresh.clone());
        let handle = tauri::async_runtime::spawn(async move { run_loop(app, pipeline, shared, refresh).await });
        *self.task.lock().await = Some(handle);
        self.update_status(|status| {
            status.running = Some(info);
            status.last_error = None;
            status.last_seen = 0;
        });
        Ok(self.status())
    }

    /// 识别中挪动取景框或改变布局后调用：只换识别区域，不清空判重历史，
    /// 所以框里没变的句子不会在面板里重复出现。
    pub fn update_region(&self, rect: Rect) -> Result<CaptureStatus, CommandError> {
        {
            let mut target = self.target.write().unwrap_or_else(PoisonError::into_inner);
            match target.as_mut().map(|t| &mut t.source) {
                Some(CaptureSource::Screen { rect: current }) => *current = rect,
                Some(CaptureSource::Window { .. }) => {
                    return Err(CommandError::new("not_lens", "当前不是取景框模式，不能更新区域"));
                }
                None => return Err(CommandError::new("not_running", "当前没有在识别")),
            }
        }
        self.update_status(|status| {
            if let Some(info) = status.running.as_mut() {
                info.label = format!("取景框 {}×{}", rect.width, rect.height);
            }
        });
        Ok(self.status())
    }

    /// `None` 为手动模式：开始识别时读一次，之后只在用户点刷新时读。
    pub fn set_interval(&self, interval: Option<Duration>) {
        self.refresh.set_interval(interval);
    }

    pub fn refresh_now(&self) -> Result<(), CommandError> {
        if self.status().running.is_none() {
            return Err(CommandError::new("not_running", "当前没有在识别"));
        }
        self.refresh.request_refresh();
        Ok(())
    }

    pub async fn stop(&self) -> CaptureStatus {
        if let Some(handle) = self.task.lock().await.take() {
            handle.abort();
        }
        if let Some(pipeline) = &self.pipeline {
            // 常驻截图会话只在识别时需要，停了就关掉。
            pipeline.release();
        }
        *self.target.write().unwrap_or_else(PoisonError::into_inner) = None;
        self.update_status(|status| status.running = None);
        self.status()
    }

    pub fn pipeline(&self) -> Result<Arc<CapturePipeline>, CommandError> {
        self.pipeline.clone().ok_or_else(|| CommandError::new("capture_unsupported", "当前平台不支持识别"))
    }

    /// 状态真的变了才推送。识别每轮都会写一次状态，画面没变时内容相同，
    /// 照推会让面板每轮整体重渲染一次。
    fn update_status(&self, change: impl FnOnce(&mut CaptureStatus)) {
        let snapshot = {
            let mut status = self.status.write().unwrap_or_else(PoisonError::into_inner);
            let before = status.clone();
            change(&mut status);
            if *status == before {
                return;
            }
            status.clone()
        };
        if let Err(error) = self.app.emit(CAPTURE_STATUS_EVENT, snapshot) {
            tracing::warn!(%error, "failed to emit capture status");
        }
    }
}

async fn run_loop(
    app: AppHandle,
    pipeline: Arc<CapturePipeline>,
    shared: Arc<RwLock<Option<CaptureTarget>>>,
    refresh: Arc<RefreshControl>,
) {
    loop {
        let started = Instant::now();
        // 每一轮重新读取目标：识别中更新的区域在下一轮生效。
        let Some(target) = shared.read().unwrap_or_else(PoisonError::into_inner).clone() else { return };
        let poll = {
            let pipeline = pipeline.clone();
            // 抓取和 OCR 都是阻塞调用，放到阻塞线程池，别占住异步运行时。
            tauri::async_runtime::spawn_blocking(move || pipeline.poll(&target)).await
        };

        match poll {
            Ok(Ok(outcome)) => {
                publish_status(&app, |status| {
                    status.last_error = None;
                    status.last_seen = outcome.all.len();
                });
                let theirs: Vec<ParsedMessage> =
                    outcome.fresh.into_iter().filter(|message| message.speaker == Speaker::Them).collect();
                if !theirs.is_empty() {
                    if let Err(error) = app.emit(CAPTURE_LINES_EVENT, theirs) {
                        tracing::warn!(%error, "failed to emit captured lines");
                    }
                }
            }
            Ok(Err(error)) => publish_status(&app, |status| status.last_error = Some(error.to_string())),
            Err(error) => publish_status(&app, |status| status.last_error = Some(error.to_string())),
        }

        refresh.wait_next_round(started).await;
    }
}

fn publish_status(app: &AppHandle, change: impl FnOnce(&mut CaptureStatus)) {
    app.state::<crate::state::AppState>().capture.update_status(change);
}
