//! 引擎生命周期：按设置创建本地/在线引擎，替换进翻译服务，并把状态推送给前端。

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, MutexGuard};
use trans_application::ports::{ChatEngine, EngineKind, SecretStore};
use trans_application::router::EngineSet;
use trans_application::service::TranslationService;
use trans_application::settings::{AppSettings, LocalEngineSettings, OnlineEngineSettings};
use trans_providers::llama_server::{LlamaServer, LlamaServerConfig};
use trans_providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatEngine};

pub const ENGINE_STATUS_EVENT: &str = "engine-status";

/// 4B 模型纯 CPU 加载约 6 s（M0-02）；14B 或多开游戏时会慢得多，留足余量。
const LOCAL_READY_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EngineState {
    NotConfigured,
    Starting,
    Ready { model: String },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineStatus {
    pub local: EngineState,
    pub online: EngineState,
}

impl Default for EngineStatus {
    fn default() -> Self {
        Self { local: EngineState::NotConfigured, online: EngineState::NotConfigured }
    }
}

pub struct EngineManager {
    app: AppHandle,
    apply_lock: Mutex<()>,
    local: Mutex<Option<LlamaServer>>,
    status: RwLock<EngineStatus>,
    /// llama-server 输出写到这里，每次启动覆盖。
    local_log: Option<PathBuf>,
}

impl EngineManager {
    pub fn new(app: AppHandle, local_log: Option<PathBuf>) -> Self {
        Self {
            app,
            apply_lock: Mutex::new(()),
            local: Mutex::new(None),
            status: RwLock::new(EngineStatus::default()),
            local_log,
        }
    }

    pub fn status(&self) -> EngineStatus {
        self.status.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub async fn lock_apply(&self) -> MutexGuard<'_, ()> {
        self.apply_lock.lock().await
    }

    pub async fn apply(&self, settings: &AppSettings, secrets: &dyn SecretStore, service: &TranslationService) {
        let (online, online_state) = online_engine(&settings.engines.online, secrets);
        self.publish(|status| status.online = online_state);
        let (local, local_state, started) = self.local_engine(&settings.engines.local).await;
        service.replace_engines(EngineSet { local, online });
        if started {
            // 预热完才报"就绪"，用户的第一句翻译不必替系统提示词预填充多等约 4 s。
            if let Err(error) = service.warm_up_local().await {
                tracing::warn!(%error, "local engine warm-up failed");
            }
        }
        self.publish(|status| status.local = local_state);
    }

    pub async fn shutdown(&self) {
        if let Some(server) = self.local.lock().await.take() {
            server.stop().await;
        }
    }

    /// 本地配置没变时复用正在运行的 llama-server，不重启（重启一次要重新加载模型）。
    /// 返回值第三项表示这次是否新启动了进程（新启动的需要预热）。
    async fn local_engine(&self, settings: &LocalEngineSettings) -> (Option<Arc<dyn ChatEngine>>, EngineState, bool) {
        let mut running = self.local.lock().await;
        let (Some(server_path), Some(model_path)) = (settings.server_path.clone(), settings.model_path.clone()) else {
            if let Some(old) = running.take() {
                old.stop().await;
            }
            return (None, EngineState::NotConfigured, false);
        };
        let config = LlamaServerConfig {
            server_path,
            model_path,
            threads: settings.threads,
            context_size: settings.context_size,
            port: settings.port,
        };

        let started = running.as_ref().map(LlamaServer::config) != Some(&config);
        if started {
            if let Some(old) = running.take() {
                old.stop().await;
            }
            self.publish(|status| status.local = EngineState::Starting);
            match LlamaServer::start(config.clone(), self.local_log.as_deref(), LOCAL_READY_TIMEOUT).await {
                Ok(server) => *running = Some(server),
                Err(error) => return (None, EngineState::Failed { message: error.to_string() }, false),
            }
        }

        let model = model_label(&config.model_path);
        let engine = OpenAiCompatEngine::new(OpenAiCompatConfig {
            kind: EngineKind::Local,
            base_url: format!("{}/v1", config.base_url()),
            model: model.clone(),
            api_key: None,
            reasoning_effort: None,
        });
        match engine {
            Ok(engine) => (Some(Arc::new(engine) as Arc<dyn ChatEngine>), EngineState::Ready { model }, started),
            Err(error) => (None, EngineState::Failed { message: error.to_string() }, false),
        }
    }

    fn publish(&self, update: impl FnOnce(&mut EngineStatus)) {
        let snapshot = {
            let mut status = self.status.write().unwrap_or_else(PoisonError::into_inner);
            update(&mut status);
            status.clone()
        };
        if let Err(error) = self.app.emit(ENGINE_STATUS_EVENT, snapshot) {
            tracing::warn!(%error, "failed to emit engine status");
        }
    }
}

fn online_engine(
    settings: &OnlineEngineSettings,
    secrets: &dyn SecretStore,
) -> (Option<Arc<dyn ChatEngine>>, EngineState) {
    if !settings.enabled {
        return (None, EngineState::NotConfigured);
    }
    let key = match secrets.get(&settings.api_key_secret) {
        Ok(Some(key)) if !key.trim().is_empty() => key,
        Ok(_) => return (None, EngineState::Failed { message: "已启用在线引擎，但还没有填写 API Key".into() }),
        Err(error) => return (None, EngineState::Failed { message: error.to_string() }),
    };
    let engine = OpenAiCompatEngine::new(OpenAiCompatConfig {
        kind: EngineKind::Online,
        base_url: settings.base_url.clone(),
        model: settings.model.clone(),
        api_key: Some(key),
        reasoning_effort: settings.reasoning_effort.clone().filter(|e| !e.trim().is_empty()),
    });
    match engine {
        Ok(engine) => (
            Some(Arc::new(engine) as Arc<dyn ChatEngine>),
            EngineState::Ready { model: format!("{} · {}", settings.display_name, settings.model) },
        ),
        Err(error) => (None, EngineState::Failed { message: error.to_string() }),
    }
}

fn model_label(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}
