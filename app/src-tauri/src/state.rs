//! 组合根：创建适配器并注入应用层。

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use trans_application::ports::SecretStore;
use trans_application::router::EngineSet;
use trans_application::service::{ServiceConfig, TranslationService};
use trans_application::settings::AppSettings;
use trans_presets::RegionPreset;

use crate::capture_session::CaptureSession;
use crate::engines::EngineManager;
use crate::settings_store::SettingsStore;

/// 凭据管理器里的名字前缀。
const APP_ID: &str = "trans";

pub struct AppState {
    pub settings: RwLock<AppSettings>,
    pub settings_store: SettingsStore,
    pub secrets: Arc<dyn SecretStore>,
    pub service: Arc<TranslationService>,
    pub engines: EngineManager,
    pub capture: CaptureSession,
    pub presets: Vec<RegionPreset>,
    pub startup_warnings: Mutex<Vec<String>>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let store = SettingsStore::new(app.path().app_config_dir()?.join("settings.json"));
        let mut warnings = Vec::new();
        let settings = match store.load() {
            Ok(settings) => settings,
            Err(error) => {
                tracing::error!(%error, path = %store.path().display(), "settings unreadable, falling back to defaults");
                warnings.push(format!("设置文件无法读取，已改用默认设置：{error}"));
                if let Some(backup) = store.quarantine() {
                    warnings.push(format!("原设置文件已备份为 {}", backup.display()));
                }
                AppSettings::default()
            }
        };

        let service = Arc::new(TranslationService::new(EngineSet::default(), ServiceConfig::from(&settings)));
        Ok(Self {
            settings: RwLock::new(settings),
            settings_store: store,
            secrets: platform_secret_store(),
            service,
            engines: EngineManager::new(app.clone()),
            capture: CaptureSession::new(app.clone(), capture_pipeline()),
            presets: trans_presets::builtin_presets()?,
            startup_warnings: Mutex::new(warnings),
        })
    }
}

/// 在后台按当前设置（重新）应用引擎。多次触发时串行执行，且每次都读取执行那一刻的最新设置，
/// 避免"先发起的旧设置最后生效"。
pub fn spawn_apply_engines(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _serialized = state.engines.lock_apply().await;
        let settings = state.settings.read().await.clone();
        state.engines.apply(&settings, state.secrets.as_ref(), &state.service).await;
    });
}

/// 截图和 OCR 只有 Windows 适配器；其他平台上窗口识别不可用，但翻译照常工作。
#[cfg(windows)]
fn capture_pipeline() -> Option<Arc<trans_application::capture_pipeline::CapturePipeline>> {
    Some(Arc::new(trans_application::capture_pipeline::CapturePipeline::new(
        Arc::new(trans_win::GraphicsCapture),
        Arc::new(trans_win::WindowsOcr),
    )))
}

#[cfg(not(windows))]
fn capture_pipeline() -> Option<Arc<trans_application::capture_pipeline::CapturePipeline>> {
    None
}

#[cfg(windows)]
fn platform_secret_store() -> Arc<dyn SecretStore> {
    Arc::new(trans_win::CredentialStore::new(APP_ID))
}

#[cfg(not(windows))]
fn platform_secret_store() -> Arc<dyn SecretStore> {
    struct Unsupported;
    impl SecretStore for Unsupported {
        fn get(&self, _: &str) -> Result<Option<String>, trans_application::ports::SecretError> {
            Ok(None)
        }
        fn set(&self, _: &str, _: &str) -> Result<(), trans_application::ports::SecretError> {
            Err(trans_application::ports::SecretError("当前平台不支持凭据存储".into()))
        }
        fn delete(&self, _: &str) -> Result<(), trans_application::ports::SecretError> {
            Ok(())
        }
    }
    let _ = APP_ID;
    Arc::new(Unsupported)
}
