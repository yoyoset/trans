//! 用户设置模型。所有字段都有默认值，旧版本配置缺字段时自动补齐；模型、引擎全部是数据，换模型不改代码。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use trans_domain::lang::{Direction, LanguagePair};

use crate::ports::EngineKind;

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u32,
    pub languages: LanguagePair,
    pub engines: EngineSettings,
    pub hotkeys: HotkeySettings,
}

/// 全局热键。写法如 `ctrl+alt+t`；留空表示不注册这个热键。
/// 系统里同一个组合只能有一个主人，被别的软件占了就得换一个，所以必须可改。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeySettings {
    /// 打开/收起取景框。
    pub toggle_lens: String,
    /// 取景框让开鼠标 / 恢复。
    pub click_through: String,
    /// 立即识别一次。
    pub refresh: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            toggle_lens: "ctrl+alt+t".into(),
            click_through: "ctrl+alt+shift+t".into(),
            refresh: "ctrl+alt+shift+r".into(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            languages: LanguagePair::default(),
            engines: EngineSettings::default(),
            hotkeys: HotkeySettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    pub local: LocalEngineSettings,
    pub online: OnlineEngineSettings,
    pub routing: RoutingSettings,
    pub calibration: CalibrationSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalEngineSettings {
    /// llama-server 可执行文件。可以复用其他项目自带的那份。
    pub server_path: Option<PathBuf>,
    /// GGUF 模型文件。换模型 = 换这个路径。
    pub model_path: Option<PathBuf>,
    /// 推理线程数。和游戏同时运行时不宜占满全部核心。
    pub threads: u16,
    pub context_size: u32,
    pub port: u16,
}

impl Default for LocalEngineSettings {
    fn default() -> Self {
        Self { server_path: None, model_path: None, threads: 8, context_size: 4096, port: 8765 }
    }
}

/// 在线引擎怎么接入。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnlineChannel {
    /// 直连 OpenAI 兼容接口，API Key 存凭据管理器。
    #[default]
    ApiKey,
    /// 通过本机 opencode 程序，用它已经登录的订阅（例如 OpenCode Go），不需要在本软件里填 Key。
    Opencode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnlineEngineSettings {
    pub enabled: bool,
    pub channel: OnlineChannel,
    /// 界面上显示的名字。
    pub display_name: String,
    /// OpenAI 兼容接口地址。换服务商 = 换地址和模型名。
    pub base_url: String,
    pub model: String,
    /// 推理强度（如 `low`）。M0-02：推理模型默认强度下 p90 达 18 s。
    pub reasoning_effort: Option<String>,
    /// API Key 在凭据管理器里的键名；Key 本身永远不写进设置文件。
    pub api_key_secret: String,
    pub opencode: OpencodeSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpencodeSettings {
    /// opencode 程序；为空时自动查找（PATH 里的 opencode.exe，或 npm 全局安装位置）。
    pub program: Option<PathBuf>,
    /// `提供方/模型`，和 `opencode models` 列出的一致。
    pub model: String,
    /// 推理强度变体。M0-02：OpenRouter 上的 DeepSeek V4 Flash 默认推理 p90 18 s，low 降到 7.7 s。
    pub variant: Option<String>,
}

impl Default for OpencodeSettings {
    fn default() -> Self {
        Self { program: None, model: "opencode-go/deepseek-v4-flash".into(), variant: Some("low".into()) }
    }
}

impl Default for OnlineEngineSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channel: OnlineChannel::ApiKey,
            display_name: "DeepSeek".into(),
            base_url: "https://api.deepseek.com".into(),
            model: "deepseek-v4-flash".into(),
            reasoning_effort: None,
            api_key_secret: "online-api-key".into(),
            opencode: OpencodeSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnginePreference {
    #[default]
    Local,
    Online,
}

impl EnginePreference {
    pub fn kind(self) -> EngineKind {
        match self {
            Self::Local => EngineKind::Local,
            Self::Online => EngineKind::Online,
        }
    }
}

/// 两个方向分别设置：对方消息 → 用户语言、用户输入 → 外语。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingSettings {
    pub incoming: EnginePreference,
    pub outgoing: EnginePreference,
}

impl RoutingSettings {
    pub fn preference(&self, direction: Direction) -> EnginePreference {
        match direction {
            Direction::Incoming => self.incoming,
            Direction::Outgoing => self.outgoing,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalibrationSettings {
    /// 在线校对时附带的最近对话轮数。
    pub context_turns: u8,
}

impl Default for CalibrationSettings {
    fn default() -> Self {
        Self { context_turns: 6 }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("设置文件格式错误：{0}")]
    Malformed(#[from] serde_json::Error),
    #[error("设置文件由更新版本（schema {found}）创建，当前版本只支持到 {supported}，请升级软件")]
    NewerSchema { found: u32, supported: u32 },
}

/// 读取任意版本的设置 JSON。新增 schema 版本时在这里加迁移分支，调用方不需要知道。
pub fn load_settings(json: &str) -> Result<AppSettings, SettingsError> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let found = value.get("schema_version").and_then(serde_json::Value::as_u64).unwrap_or(1);
    let found = u32::try_from(found).unwrap_or(u32::MAX);
    if found > SETTINGS_SCHEMA_VERSION {
        return Err(SettingsError::NewerSchema { found, supported: SETTINGS_SCHEMA_VERSION });
    }
    let mut settings: AppSettings = serde_json::from_value(value)?;
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_json() {
        let json = serde_json::to_string(&AppSettings::default()).unwrap();
        assert_eq!(load_settings(&json).unwrap(), AppSettings::default());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults_and_unknown_fields_are_ignored() {
        let settings = load_settings(r#"{"engines": {"local": {"threads": 4}, "future_field": true}}"#).unwrap();
        assert_eq!(settings.engines.local.threads, 4);
        assert_eq!(settings.engines.local.port, 8765);
        assert_eq!(settings.engines.routing.outgoing, EnginePreference::Local);
    }

    #[test]
    fn settings_saved_before_channels_existed_keep_using_the_api_key() {
        let settings =
            load_settings(r#"{"engines": {"online": {"enabled": true, "model": "deepseek-chat"}}}"#).unwrap();
        assert_eq!(settings.engines.online.channel, OnlineChannel::ApiKey);
        assert_eq!(settings.engines.online.opencode.model, "opencode-go/deepseek-v4-flash");
    }

    #[test]
    fn refuses_settings_written_by_a_newer_version() {
        assert!(matches!(
            load_settings(r#"{"schema_version": 99}"#),
            Err(SettingsError::NewerSchema { found: 99, .. })
        ));
    }

    #[test]
    fn api_key_is_never_part_of_serialized_settings() {
        let json = serde_json::to_string(&AppSettings::default()).unwrap();
        assert!(json.contains("\"api_key_secret\":\"online-api-key\""));
        assert!(!json.contains("sk-"));
    }
}
