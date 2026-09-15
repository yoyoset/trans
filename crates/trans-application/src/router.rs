//! 按"方向 + 用途"选择引擎。
//!
//! 产品决策（2026-09-15）：实时翻译默认走本地，结果立刻出；在线只在用户点"在线校对"时调用。

use std::sync::Arc;

use thiserror::Error;
use trans_domain::lang::Direction;

use crate::ports::{ChatEngine, EngineKind};
use crate::settings::RoutingSettings;

#[derive(Clone, Default)]
pub struct EngineSet {
    pub local: Option<Arc<dyn ChatEngine>>,
    pub online: Option<Arc<dyn ChatEngine>>,
}

impl EngineSet {
    pub fn get(&self, kind: EngineKind) -> Option<&Arc<dyn ChatEngine>> {
        match kind {
            EngineKind::Local => self.local.as_ref(),
            EngineKind::Online => self.online.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// 实时翻译：按方向偏好选，偏好的引擎没配置时退回另一个。
    Realtime,
    /// 用户点击的在线校对：只能走在线引擎，不能悄悄换成本地。
    Calibration,
}

pub struct Route {
    pub engine: Arc<dyn ChatEngine>,
    /// 偏好的引擎不可用而改用了另一个，界面需要提示。
    pub fell_back: bool,
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Route")
            .field("kind", &self.engine.kind())
            .field("model", &self.engine.model())
            .field("fell_back", &self.fell_back)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteError {
    #[error("没有可用的翻译引擎，请在设置中配置本地模型或在线引擎")]
    NoEngine,
    #[error("在线校对需要先在设置中启用在线引擎并填写 API Key")]
    OnlineNotConfigured,
}

pub fn route(
    engines: &EngineSet,
    routing: &RoutingSettings,
    direction: Direction,
    purpose: Purpose,
) -> Result<Route, RouteError> {
    match purpose {
        Purpose::Calibration => engines
            .online
            .clone()
            .map(|engine| Route { engine, fell_back: false })
            .ok_or(RouteError::OnlineNotConfigured),
        Purpose::Realtime => {
            let preferred = routing.preference(direction).kind();
            let other = match preferred {
                EngineKind::Local => EngineKind::Online,
                EngineKind::Online => EngineKind::Local,
            };
            if let Some(engine) = engines.get(preferred) {
                Ok(Route { engine: engine.clone(), fell_back: false })
            } else if let Some(engine) = engines.get(other) {
                Ok(Route { engine: engine.clone(), fell_back: true })
            } else {
                Err(RouteError::NoEngine)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use trans_domain::prompt::ChatMessage;

    use super::*;
    use crate::ports::{Completion, CompletionOptions, ProviderError};
    use crate::settings::EnginePreference;

    struct Stub(EngineKind);

    #[async_trait]
    impl ChatEngine for Stub {
        fn kind(&self) -> EngineKind {
            self.0
        }
        fn model(&self) -> &str {
            "stub"
        }
        async fn complete(&self, _: &[ChatMessage], _: &CompletionOptions) -> Result<Completion, ProviderError> {
            unreachable!("router tests never call the engine")
        }
    }

    fn engines(local: bool, online: bool) -> EngineSet {
        EngineSet {
            local: local.then(|| Arc::new(Stub(EngineKind::Local)) as Arc<dyn ChatEngine>),
            online: online.then(|| Arc::new(Stub(EngineKind::Online)) as Arc<dyn ChatEngine>),
        }
    }

    #[test]
    fn realtime_uses_the_per_direction_preference() {
        let routing = RoutingSettings { incoming: EnginePreference::Local, outgoing: EnginePreference::Online };
        let both = engines(true, true);
        let incoming = route(&both, &routing, Direction::Incoming, Purpose::Realtime).unwrap();
        let outgoing = route(&both, &routing, Direction::Outgoing, Purpose::Realtime).unwrap();
        assert_eq!((incoming.engine.kind(), incoming.fell_back), (EngineKind::Local, false));
        assert_eq!((outgoing.engine.kind(), outgoing.fell_back), (EngineKind::Online, false));
    }

    #[test]
    fn realtime_falls_back_and_says_so() {
        let routing = RoutingSettings::default();
        let only_online = engines(false, true);
        let r = route(&only_online, &routing, Direction::Incoming, Purpose::Realtime).unwrap();
        assert_eq!((r.engine.kind(), r.fell_back), (EngineKind::Online, true));
        assert_eq!(
            route(&engines(false, false), &routing, Direction::Incoming, Purpose::Realtime).unwrap_err(),
            RouteError::NoEngine
        );
    }

    #[test]
    fn calibration_never_silently_uses_the_local_engine() {
        let err = route(&engines(true, false), &RoutingSettings::default(), Direction::Outgoing, Purpose::Calibration)
            .unwrap_err();
        assert_eq!(err, RouteError::OnlineNotConfigured);
    }
}
