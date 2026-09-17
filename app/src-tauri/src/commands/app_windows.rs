use tauri::AppHandle;

use crate::error::CommandError;
use crate::overlay_windows;

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), CommandError> {
    overlay_windows::show_main(&app)
}

/// 真正退出（主窗口的关闭按钮只是收成悬浮按钮）。退出时会一并关掉本地 llama-server。
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    tracing::info!("quit requested from floating button");
    app.exit(0);
}
