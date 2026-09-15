use std::path::PathBuf;

use tauri::{AppHandle, State};
use trans_providers::model_discovery::{discover_gguf, search_roots, ModelFile};

use crate::engines::EngineStatus;
use crate::error::CommandError;
use crate::state::{spawn_apply_engines, AppState};

#[tauri::command]
pub fn engine_status(state: State<'_, AppState>) -> EngineStatus {
    state.engines.status()
}

#[tauri::command]
pub fn restart_engines(app: AppHandle) {
    spawn_apply_engines(app);
}

/// 在 HF_HOME（与其他项目共用的模型目录）和默认 HF 缓存里找 GGUF 模型。只读本地文件，不联网。
#[tauri::command]
pub async fn discover_local_models() -> Result<Vec<ModelFile>, CommandError> {
    let hf_home = std::env::var("HF_HOME").ok();
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    let roots = search_roots(hf_home.as_deref(), home.as_deref());
    tauri::async_runtime::spawn_blocking(move || discover_gguf(&roots)).await.map_err(CommandError::internal)
}

#[tauri::command]
pub fn take_startup_warnings(state: State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut *state.startup_warnings.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}
