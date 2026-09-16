use serde::{Deserialize, Serialize};
use tauri::State;
use trans_application::capture::{WindowId, WindowInfo};
use trans_application::capture_pipeline::CaptureTarget;
use trans_domain::chat_lines::ParsedMessage;
use trans_domain::geometry::Rect;
use trans_domain::glossary::GlossaryEntry;

use crate::capture_session::{CaptureInfo, CaptureStatus};
use crate::error::CommandError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CaptureArgs {
    /// 目标窗口句柄，来自 `list_capture_windows`。
    pub window: i64,
    pub preset_id: String,
}

#[derive(Debug, Serialize)]
pub struct CapturePreview {
    /// 实际裁剪的区域，界面可以显示出来让用户判断框选是否合理。
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

#[tauri::command]
pub async fn list_capture_windows(state: State<'_, AppState>) -> Result<Vec<WindowInfo>, CommandError> {
    let pipeline = state.capture.pipeline()?;
    let windows = tauri::async_runtime::spawn_blocking(move || pipeline.list_windows())
        .await
        .map_err(CommandError::internal)?
        .map_err(|error| CommandError::new("capture_failed", error.to_string()))?;
    Ok(windows)
}

/// 试识别一次，用来确认框选对没对准。不写入判重历史，预览多少次都不会让正式识别漏消息。
#[tauri::command]
pub async fn preview_capture(request: CaptureArgs, state: State<'_, AppState>) -> Result<CapturePreview, CommandError> {
    let (target, _, _) = build_target(&state, &request).await?;
    let pipeline = state.capture.pipeline()?;
    let outcome = tauri::async_runtime::spawn_blocking(move || pipeline.preview(&target))
        .await
        .map_err(CommandError::internal)?
        .map_err(|error| CommandError::new("capture_failed", error.to_string()))?;
    Ok(CapturePreview { region: outcome.region, messages: outcome.all })
}

#[tauri::command]
pub async fn start_capture(request: CaptureArgs, state: State<'_, AppState>) -> Result<CaptureStatus, CommandError> {
    let (target, info, glossary) = build_target(&state, &request).await?;
    state.capture.start(target, info, glossary, state.service.clone()).await
}

#[tauri::command]
pub async fn stop_capture(state: State<'_, AppState>) -> Result<CaptureStatus, CommandError> {
    Ok(state.capture.stop().await)
}

async fn build_target(
    state: &AppState,
    request: &CaptureArgs,
) -> Result<(CaptureTarget, CaptureInfo, Vec<GlossaryEntry>), CommandError> {
    let preset = state
        .presets
        .iter()
        .find(|preset| preset.id == request.preset_id)
        .ok_or_else(|| CommandError::new("preset_not_found", format!("找不到预设 {}", request.preset_id)))?;

    let window = WindowId(request.window);
    // OCR 语言用"对方的语言"：识别的是对方发来的消息。
    let lang = state.settings.read().await.languages.foreign.clone();
    let target = CaptureTarget {
        window,
        region: preset.region,
        parser: preset.parser.clone(),
        scale: preset.preprocess.scale,
        lang,
    };
    let info = CaptureInfo {
        window,
        window_title: window_title(state, window).unwrap_or_default(),
        preset_id: preset.id.clone(),
        preset_name: preset.name.clone(),
    };
    Ok((target, info, preset.glossary.clone()))
}

fn window_title(state: &AppState, window: WindowId) -> Option<String> {
    let windows = state.capture.list_windows().ok()?;
    windows.into_iter().find(|w: &WindowInfo| w.id == window).map(|w| w.title)
}
