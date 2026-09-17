use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use trans_application::capture::{WindowId, WindowInfo};
use trans_application::capture_pipeline::{CaptureSource, CaptureTarget};
use trans_domain::chat_lines::ParsedMessage;
use trans_domain::geometry::Rect;

use crate::capture_session::{CaptureInfo, CaptureStatus};
use crate::error::CommandError;
use crate::state::AppState;

/// 取景框窗口的 label，和 tauri.conf.json 里一致。
const LENS_WINDOW: &str = "lens";

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

/// 打开取景框，并立刻把它排除出截图——它浮在内容上面，不排除就只会拍到自己。
#[tauri::command]
pub fn open_lens(app: AppHandle) -> Result<(), CommandError> {
    let window =
        app.get_webview_window(LENS_WINDOW).ok_or_else(|| CommandError::new("lens_missing", "取景框窗口不存在"))?;
    #[cfg(windows)]
    {
        let hwnd = window.hwnd().map_err(CommandError::internal)?;
        trans_win::exclude_from_capture(hwnd.0)
            .map_err(|error| CommandError::new("lens_not_excluded", format!("无法把取景框排除出截图：{error}")))?;
    }
    tracing::info!(capturing = state_is_running(&app), "lens opened");
    window.show().map_err(CommandError::internal)?;
    window.set_focus().map_err(CommandError::internal)?;
    Ok(())
}

#[tauri::command]
pub fn close_lens(app: AppHandle) -> Result<(), CommandError> {
    if let Some(window) = app.get_webview_window(LENS_WINDOW) {
        // 只隐藏窗口，识别照常在后台进行；重新打开时面板里会有隐藏期间识别到的消息。
        tracing::info!(capturing = state_is_running(&app), "lens hidden");
        window.hide().map_err(CommandError::internal)?;
    }
    Ok(())
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

    // OCR 语言用"对方的语言"：识别的是对方发来的消息。
    let lang = state.settings.read().await.languages.foreign.clone();
    let target = CaptureTarget { source, parser: preset.parser.clone(), scale: preset.preprocess.scale, lang };
    let info = CaptureInfo { label, preset_id: preset.id.clone(), preset_name: preset.name.clone() };
    Ok((target, info))
}

fn window_title(state: &AppState, window: WindowId) -> Option<String> {
    let windows = state.capture.list_windows().ok()?;
    windows.into_iter().find(|w: &WindowInfo| w.id == window).map(|w| w.title)
}
