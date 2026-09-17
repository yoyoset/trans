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
use trans_application::settings::{
    AppSettings, LocalEngineSettings, OnlineChannel, OnlineEngineSettings, OpencodeSettings,
};
use trans_providers::llama_server::{LlamaServer, LlamaServerConfig};
use trans_providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatEngine};
use trans_providers::opencode::{find_opencode, OpencodeServer, OpencodeServerConfig};

pub const ENGINE_STATUS_EVENT: &str = "engine-status";

/// 4B 模型纯 CPU 加载约 6 s（M0-02）；14B 或多开游戏时会慢得多，留足余量。
const LOCAL_READY_TIMEOUT: Duration = Duration::from_secs(180);

/// opencode serve 实测约 2.5～3.3 s 就绪；首次运行可能要初始化配置，留足余量。
const OPENCODE_READY_TIMEOUT: Duration = Duration::from_secs(60);

/// 引擎子进程用到的目录。拿不到时对应功能降级（不写日志 / 不能用 opencode），不影响其它引擎。
#[derive(Debug, Clone, Default)]
pub struct EnginePaths {
    pub log_dir: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
}

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
    opencode: Mutex<Option<OpencodeServer>>,
    status: RwLock<EngineStatus>,
    paths: EnginePaths,
}

impl EngineManager {
    pub fn new(app: AppHandle, paths: EnginePaths) -> Self {
        Self {
            app,
            apply_lock: Mutex::new(()),
            local: Mutex::new(None),
            opencode: Mutex::new(None),
            status: RwLock::new(EngineStatus::default()),
            paths,
        }
    }

    pub fn status(&self) -> EngineStatus {
        self.status.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub async fn lock_apply(&self) -> MutexGuard<'_, ()> {
        self.apply_lock.lock().await
    }

    pub async fn apply(&self, settings: &AppSettings, secrets: &dyn SecretStore, service: &TranslationService) {
        // 两个引擎互不依赖，并行启动：opencode 约 3 s、llama-server 约 6 s，不必排队。
        let ((online, online_state), (local, local_state, started)) = tokio::join!(
            self.online_engine(&settings.engines.online, secrets),
            self.local_engine(&settings.engines.local)
        );
        self.publish(|status| status.online = online_state);
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
        self.stop_opencode().await;
    }

    async fn stop_opencode(&self) {
        if let Some(server) = self.opencode.lock().await.take() {
            server.stop().await;
        }
    }

    async fn online_engine(
        &self,
        settings: &OnlineEngineSettings,
        secrets: &dyn SecretStore,
    ) -> (Option<Arc<dyn ChatEngine>>, EngineState) {
        if !settings.enabled || settings.channel != OnlineChannel::Opencode {
            // 不用 opencode 时不留着它的后台进程。
            self.stop_opencode().await;
        }
        if !settings.enabled {
            return (None, EngineState::NotConfigured);
        }
        match settings.channel {
            OnlineChannel::ApiKey => api_key_engine(settings, secrets),
            OnlineChannel::Opencode => self.opencode_engine(&settings.opencode).await,
        }
    }

    /// opencode 程序和工作目录没变时复用正在运行的 serve；只换模型不用重启。
    async fn opencode_engine(&self, settings: &OpencodeSettings) -> (Option<Arc<dyn ChatEngine>>, EngineState) {
        let failed = |message: String| (None, EngineState::Failed { message });
        let program = settings.program.clone().or_else(|| {
            let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
            find_opencode(std::env::var("PATH").ok().as_deref(), appdata.as_deref())
        });
        let Some(program) = program else {
            return failed("没有找到 opencode 程序。请先安装 opencode 并登录订阅，或在设置里指定 opencode.exe".into());
        };
        let Some(data_dir) = &self.paths.data_dir else {
            return failed("拿不到应用数据目录，无法启动 opencode".into());
        };
        let config = OpencodeServerConfig { program, work_dir: data_dir.join("opencode-work") };

        let mut running = self.opencode.lock().await;
        if running.as_ref().map(OpencodeServer::config) != Some(&config) {
            if let Some(old) = running.take() {
                old.stop().await;
            }
            self.publish(|status| status.online = EngineState::Starting);
            let log = self.paths.log_dir.as_ref().map(|dir| dir.join("opencode.log"));
            match OpencodeServer::start(config, log.as_deref(), OPENCODE_READY_TIMEOUT).await {
                Ok(server) => *running = Some(server),
                Err(error) => return failed(error.to_string()),
            }
        }
        let server = running.as_ref().expect("server was just ensured");
        let variant = settings.variant.clone().filter(|v| !v.trim().is_empty());
        match server.engine(&settings.model, variant).await {
            Ok(engine) => (
                Some(Arc::new(engine) as Arc<dyn ChatEngine>),
                EngineState::Ready { model: format!("OpenCode · {}", settings.model) },
            ),
            Err(error) => failed(error.to_string()),
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
            match LlamaServer::start(
                config.clone(),
                self.paths.log_dir.as_ref().map(|dir| dir.join("llama-server.log")).as_deref(),
                LOCAL_READY_TIMEOUT,
            )
            .await
            {
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

fn api_key_engine(
    settings: &OnlineEngineSettings,
    secrets: &dyn SecretStore,
) -> (Option<Arc<dyn ChatEngine>>, EngineState) {
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
