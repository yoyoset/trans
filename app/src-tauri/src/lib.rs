//! Tauri 壳。职责只有两件：组装依赖（组合根）、把前端命令转发给应用层。业务逻辑都在 `crates/` 里。

mod capture_session;
mod commands;
mod engines;
mod error;
mod logging;
mod overlay_windows;
mod refresh;
mod settings_store;
mod state;
mod window_geometry;

use tauri::Manager;

pub fn run() {
    let logging = logging::Logging::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Err(error) = overlay_windows::show_main(app) {
                tracing::warn!(?error, "failed to bring the main window back");
            }
        }))
        .on_window_event(|window, event| {
            // 关主窗口不退出：收成悬浮按钮，识别和本地引擎都继续运行。
            if let tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) = event {
                if let Some(webview) = window.app_handle().get_webview_window(window.label()) {
                    overlay_windows::remember_geometry(&webview);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 三个窗口都不能真的关掉：关掉就再也打不开了（Alt+F4、任务栏关闭都会走到这里）。
                api.prevent_close();
                if let Err(error) = overlay_windows::handle_close(window.app_handle(), window.label()) {
                    tracing::warn!(?error, label = window.label(), "failed to handle window close");
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            match app.path().app_log_dir() {
                Ok(dir) => match logging.attach_file(&dir) {
                    Ok(path) => tracing::info!(path = %path.display(), "logging to file"),
                    Err(error) => tracing::warn!(%error, "file logging unavailable"),
                },
                Err(error) => tracing::warn!(%error, "no log directory; file logging unavailable"),
            }
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
            commands::capture::toggle_lens,
            commands::app_windows::show_main_window,
            commands::app_windows::quit_app,
            commands::capture::capture_status,
            commands::capture::list_capture_windows,
            commands::capture::preview_capture,
            commands::capture::start_capture,
            commands::capture::update_capture_region,
            commands::capture::set_capture_interval,
            commands::capture::refresh_capture,
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
