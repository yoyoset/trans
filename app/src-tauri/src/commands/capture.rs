use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use trans_application::capture::{WindowId, WindowInfo};
use trans_application::capture_pipeline::{CaptureSource, CaptureTarget};
use trans_domain::chat_lines::ParsedMessage;
use trans_domain::geometry::Rect;
use trans_domain::lang::Lang;

use crate::capture_session::{CaptureInfo, CaptureStatus};
use crate::error::CommandError;
use crate::overlay_windows;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CaptureSourceArgs {
    /// 取景框：屏幕绝对坐标，物理像素。
    Lens { x: i32, y: i32, width: u32, height: u32 },
    /// 跟随窗口：区域来自预设，随窗口尺寸换算。
    Window { window: i64 },
}

#[derive(Debug, Deserialize)]
pub struct CaptureArgs {
    pub source: CaptureSourceArgs,
    pub preset_id: String,
    /// 识别语言标签（如 zh-hans、en）。为空时用设置里的"对方语言"。
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CapturePreview {
    /// 实际识别的区域，界面可以显示出来让用户判断框选是否合理。
    pub region: Rect,
    pub messages: Vec<ParsedMessage>,
}

#[tauri::command]
pub fn capture_supported(state: State<'_, AppState>) -> bool {
    state.capture.is_supported()
}

#[tauri::command]
pub fn capture_status(state: State<'_, AppState>) -> CaptureStatus {
    state.capture.status()
}

/// 打开取景框（排除出截图后再显示）。
#[tauri::command]
pub fn open_lens(app: AppHandle) -> Result<(), CommandError> {
    overlay_windows::open_lens(&app)?;
    refresh_on_reopen(&app);
    Ok(())
}

/// 只隐藏取景框，识别照常在后台进行。
#[tauri::command]
pub fn close_lens(app: AppHandle) -> Result<(), CommandError> {
    tracing::info!(capturing = state_is_running(&app), "lens hidden");
    overlay_windows::close_lens(&app)
}

/// 悬浮按钮点击：开关取景框，返回切换后是否可见。
#[tauri::command]
pub fn toggle_lens(app: AppHandle) -> Result<bool, CommandError> {
    let visible = overlay_windows::toggle_lens(&app)?;
    if visible {
        refresh_on_reopen(&app);
    } else {
        tracing::info!(capturing = state_is_running(&app), "lens hidden from floating button");
    }
    Ok(visible)
}

/// 系统里装了哪些 OCR 语言。没装对应语言时识别不出任何文字，所以要让用户能选。
#[tauri::command]
pub fn list_ocr_languages(state: State<'_, AppState>) -> Result<Vec<String>, CommandError> {
    let pipeline = state.capture.pipeline()?;
    let langs = pipeline.ocr_languages().map_err(|error| CommandError::new("ocr_failed", error.to_string()))?;
    Ok(langs.into_iter().map(|lang| lang.as_str().to_owned()).collect())
}

#[tauri::command]
pub async fn list_capture_windows(state: State<'_, AppState>) -> Result<Vec<WindowInfo>, CommandError> {
    let pipeline = state.capture.pipeline()?;
    tauri::async_runtime::spawn_blocking(move || pipeline.list_windows())
        .await
        .map_err(CommandError::internal)?
        .map_err(|error| CommandError::new("capture_failed", error.to_string()))
}

/// 试识别一次，用来确认框选对没对准。不写入判重历史，预览多少次都不会让正式识别漏消息。
#[tauri::command]
pub async fn preview_capture(request: CaptureArgs, state: State<'_, AppState>) -> Result<CapturePreview, CommandError> {
    let (target, _) = build_target(&state, &request).await?;
    let pipeline = state.capture.pipeline()?;
    let outcome = tauri::async_runtime::spawn_blocking(move || pipeline.preview(&target))
        .await
        .map_err(CommandError::internal)?
        .map_err(|error| CommandError::new("capture_failed", error.to_string()))?;
    Ok(CapturePreview { region: outcome.region, messages: outcome.all })
}

#[tauri::command]
pub async fn start_capture(request: CaptureArgs, state: State<'_, AppState>) -> Result<CaptureStatus, CommandError> {
    let (target, info) = build_target(&state, &request).await?;
    state.capture.start(target, info).await
}

/// 识别中挪动取景框、改大小、切换面板左右时调用，区域在下一轮识别生效，已识别的句子不会重复出现。
#[tauri::command]
pub fn update_capture_region(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    state: State<'_, AppState>,
) -> Result<CaptureStatus, CommandError> {
    state.capture.update_region(Rect { x, y, width, height })
}

/// 设置刷新节奏：`interval_ms` 为空是手动模式。改完立即生效，不打断正在进行的识别。
#[tauri::command]
pub fn set_capture_interval(interval_ms: Option<u64>, state: State<'_, AppState>) {
    state.capture.set_interval(interval_ms.map(Duration::from_millis));
}

/// 马上识别一轮（手动模式靠它读取新消息，定时模式下也可以不等间隔）。
#[tauri::command]
pub fn refresh_capture(state: State<'_, AppState>) -> Result<(), CommandError> {
    state.capture.refresh_now()
}

#[tauri::command]
pub async fn stop_capture(state: State<'_, AppState>) -> Result<CaptureStatus, CommandError> {
    Ok(state.capture.stop().await)
}

/// 取景框重新出现时立即读一轮：离开期间来的新消息马上显示，不管刷新节奏是不是手动。
fn refresh_on_reopen(app: &AppHandle) {
    let refreshed = app.state::<AppState>().capture.refresh_if_running();
    tracing::info!(capturing = refreshed, "lens opened");
}

fn state_is_running(app: &AppHandle) -> bool {
    app.state::<AppState>().capture.status().running.is_some()
}

async fn build_target(state: &AppState, request: &CaptureArgs) -> Result<(CaptureTarget, CaptureInfo), CommandError> {
    let preset = state
        .presets
        .iter()
        .find(|preset| preset.id == request.preset_id)
        .ok_or_else(|| CommandError::new("preset_not_found", format!("找不到预设 {}", request.preset_id)))?;

    let (source, label) = match request.source {
        CaptureSourceArgs::Lens { x, y, width, height } => {
            let rect = Rect { x, y, width, height };
            (CaptureSource::Screen { rect }, format!("取景框 {width}×{height}"))
        }
        CaptureSourceArgs::Window { window } => {
            let window = WindowId(window);
            let label = window_title(state, window).unwrap_or_else(|| "窗口".into());
            (CaptureSource::Window { window, region: preset.region }, label)
        }
    };

    // 默认按"对方的语言"识别；用户在取景框里选了别的语言就用选的。
    let lang = match request.lang.as_deref().map(str::trim).filter(|tag| !tag.is_empty()) {
        Some(tag) => Lang::new(tag),
        None => state.settings.read().await.languages.foreign.clone(),
    };
    let target = CaptureTarget { source, parser: preset.parser.clone(), scale: preset.preprocess.scale, lang };
    let info = CaptureInfo { label, preset_id: preset.id.clone(), preset_name: preset.name.clone() };
    Ok((target, info))
}

fn window_title(state: &AppState, window: WindowId) -> Option<String> {
    let windows = state.capture.list_windows().ok()?;
    windows.into_iter().find(|w: &WindowInfo| w.id == window).map(|w| w.title)
}
