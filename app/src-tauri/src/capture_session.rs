//! 后台识别会话：定时抓取目标窗口 → 识别 → 把新出现的对方消息翻译后推给界面。
//!
//! 只在这里决定"多久抓一次"和"抓到之后做什么"；怎么抓、怎么解析在 `trans-application` 的读取管道里。

use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use trans_application::capture::WindowInfo;
use trans_application::capture_pipeline::{CapturePipeline, CaptureTarget};
use trans_application::service::{TranslateRequest, TranslationOutcome, TranslationService};
use trans_domain::chat_lines::ParsedMessage;
use trans_domain::conversation::Speaker;
use trans_domain::glossary::GlossaryEntry;
use trans_domain::lang::{Direction, Tone};

use crate::error::CommandError;

pub const CAPTURE_MESSAGE_EVENT: &str = "capture-message";
pub const CAPTURE_STATUS_EVENT: &str = "capture-status";

/// 抓取间隔。M0-01 实测单次识别 16～42 ms，间隔主要是为了不让 CPU 一直忙，
/// 也给翻译留出时间（本地 4B 每句约 1.9 s）。
const POLL_INTERVAL: Duration = Duration::from_millis(1200);

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

/// 识别到并翻译好的一条消息；翻译失败时 `translation` 为空、`error` 有值。
#[derive(Debug, Clone, Serialize)]
pub struct CapturedMessage {
    pub source: String,
    pub sender: Option<String>,
    pub translation: Option<TranslationOutcome>,
    pub error: Option<CommandError>,
}

pub struct CaptureSession {
    app: AppHandle,
    pipeline: Option<Arc<CapturePipeline>>,
    task: Mutex<Option<JoinHandle<()>>>,
    status: RwLock<CaptureStatus>,
}

impl CaptureSession {
    pub fn new(app: AppHandle, pipeline: Option<Arc<CapturePipeline>>) -> Self {
        Self { app, pipeline, task: Mutex::new(None), status: RwLock::new(CaptureStatus::default()) }
    }

    /// 当前平台是否支持窗口识别（非 Windows 上没有截图和 OCR 适配器）。
    pub fn is_supported(&self) -> bool {
        self.pipeline.is_some()
    }

    pub fn status(&self) -> CaptureStatus {
        self.status.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, CommandError> {
        self.pipeline()?.list_windows().map_err(|error| CommandError::new("capture_failed", error.to_string()))
    }

    pub async fn start(
        &self,
        target: CaptureTarget,
        info: CaptureInfo,
        glossary: Vec<GlossaryEntry>,
        service: Arc<TranslationService>,
    ) -> Result<CaptureStatus, CommandError> {
        let pipeline = self.pipeline()?;
        self.stop().await;
        // 换目标就要清空判重历史，否则新会话里和上一个窗口相同的话会被当成重复丢掉。
        pipeline.reset();

        let app = self.app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            run_loop(app, pipeline, target, glossary, service).await;
        });
        *self.task.lock().await = Some(handle);
        self.update_status(|status| {
            status.running = Some(info);
            status.last_error = None;
            status.last_seen = 0;
        });
        Ok(self.status())
    }

    pub async fn stop(&self) -> CaptureStatus {
        if let Some(handle) = self.task.lock().await.take() {
            handle.abort();
        }
        self.update_status(|status| status.running = None);
        self.status()
    }

    pub fn pipeline(&self) -> Result<Arc<CapturePipeline>, CommandError> {
        self.pipeline.clone().ok_or_else(|| CommandError::new("capture_unsupported", "当前平台不支持窗口识别"))
    }

    fn update_status(&self, change: impl FnOnce(&mut CaptureStatus)) {
        let snapshot = {
            let mut status = self.status.write().unwrap_or_else(PoisonError::into_inner);
            change(&mut status);
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
    target: CaptureTarget,
    glossary: Vec<GlossaryEntry>,
    service: Arc<TranslationService>,
) {
    loop {
        let started = Instant::now();
        let poll = {
            let pipeline = pipeline.clone();
            let target = target.clone();
            // 抓取和 OCR 都是阻塞调用，放到阻塞线程池，别占住异步运行时。
            tauri::async_runtime::spawn_blocking(move || pipeline.poll(&target)).await
        };

        match poll {
            Ok(Ok(outcome)) => {
                publish_status(&app, |status| {
                    status.last_error = None;
                    status.last_seen = outcome.all.len();
                });
                for message in outcome.fresh.into_iter().filter(|m| m.speaker == Speaker::Them) {
                    translate_and_emit(&app, &service, &glossary, message).await;
                }
            }
            Ok(Err(error)) => publish_status(&app, |status| status.last_error = Some(error.to_string())),
            Err(error) => publish_status(&app, |status| status.last_error = Some(error.to_string())),
        }

        // 间隔从"这一轮开始"算起：识别和翻译花掉的时间算在里面，不会越积越慢。
        let elapsed = started.elapsed();
        tokio::time::sleep(POLL_INTERVAL.saturating_sub(elapsed)).await;
    }
}

async fn translate_and_emit(
    app: &AppHandle,
    service: &TranslationService,
    glossary: &[GlossaryEntry],
    message: ParsedMessage,
) {
    let request = TranslateRequest {
        direction: Direction::Incoming,
        text: message.text.clone(),
        tone: Tone::Casual,
        glossary: glossary.to_vec(),
    };
    let captured = match service.translate(request).await {
        Ok(outcome) => {
            CapturedMessage { source: message.text, sender: message.sender, translation: Some(outcome), error: None }
        }
        Err(error) => CapturedMessage {
            source: message.text,
            sender: message.sender,
            translation: None,
            error: Some(error.into()),
        },
    };
    if let Err(error) = app.emit(CAPTURE_MESSAGE_EVENT, captured) {
        tracing::warn!(%error, "failed to emit captured message");
    }
}

fn publish_status(app: &AppHandle, change: impl FnOnce(&mut CaptureStatus)) {
    use tauri::Manager;
    let session = app.state::<crate::state::AppState>();
    session.capture.update_status(change);
}
