//! Tauri 壳。职责只有两件：组装依赖（组合根）、把前端命令转发给应用层。业务逻辑都在 `crates/` 里。

mod capture_session;
mod commands;
mod engines;
mod error;
mod settings_store;
mod state;

use tauri::Manager;

pub fn run() {
    tracing_subscriber::fmt().with_target(false).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::AppState::initialize(app.handle())?;
            app.manage(state);
            state::spawn_apply_engines(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::has_online_api_key,
            commands::settings::set_online_api_key,
            commands::settings::clear_online_api_key,
            commands::engines::engine_status,
            commands::engines::restart_engines,
            commands::engines::discover_local_models,
            commands::engines::take_startup_warnings,
            commands::presets::list_presets,
            commands::capture::capture_supported,
            commands::capture::open_lens,
            commands::capture::close_lens,
            commands::capture::capture_status,
            commands::capture::list_capture_windows,
            commands::capture::preview_capture,
            commands::capture::start_capture,
            commands::capture::stop_capture,
            commands::translate::translate_text,
            commands::translate::calibrate_translation,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the Tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // 正常退出时关掉本地 llama-server，不留后台进程。
                let state = app.state::<state::AppState>();
                tauri::async_runtime::block_on(state.engines.shutdown());
            }
        });
}
