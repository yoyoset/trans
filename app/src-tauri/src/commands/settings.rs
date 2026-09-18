use tauri::{AppHandle, State};
use trans_application::service::ServiceConfig;
use trans_application::settings::AppSettings;

use crate::error::CommandError;
use crate::state::{spawn_apply_engines, AppState};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    Ok(state.settings.read().await.clone())
}

/// 先落盘再生效：写文件失败时内存里的设置保持不变，不会出现"界面显示已保存、重启后丢失"。
/// 引擎在后台重建（加载本地模型可能要几十秒），进度通过 `engine-status` 事件推送。
#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state.settings_store.save(&settings).map_err(CommandError::internal)?;
    state.service.update_config(ServiceConfig::from(&settings));
    // 热键改了要重新注册：系统里同一个组合只能有一个主人。
    let problems = crate::hotkeys::apply(&app, &settings.hotkeys);
    *state.settings.write().await = settings;
    spawn_apply_engines(app);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(CommandError::new("hotkey_unavailable", problems.join("；")))
    }
}

#[tauri::command]
pub async fn has_online_api_key(state: State<'_, AppState>) -> Result<bool, CommandError> {
    let name = state.settings.read().await.engines.online.api_key_secret.clone();
    let key = state.secrets.get(&name).map_err(CommandError::internal)?;
    Ok(key.is_some_and(|k| !k.trim().is_empty()))
}

#[tauri::command]
pub async fn set_online_api_key(key: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(CommandError::new("empty_api_key", "API Key 不能为空"));
    }
    let name = state.settings.read().await.engines.online.api_key_secret.clone();
    state.secrets.set(&name, key).map_err(CommandError::internal)?;
    spawn_apply_engines(app);
    Ok(())
}

#[tauri::command]
pub async fn clear_online_api_key(app: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError> {
    let name = state.settings.read().await.engines.online.api_key_secret.clone();
    state.secrets.delete(&name).map_err(CommandError::internal)?;
    spawn_apply_engines(app);
    Ok(())
}
