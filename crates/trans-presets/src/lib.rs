//! 区域预设。新增一个场景 = 新增一个 JSON 文件并在 `BUILTIN` 注册一行，不改任何逻辑代码。

use serde::{Deserialize, Serialize};
use thiserror::Error;
use trans_domain::chat_lines::LineParser;
use trans_domain::glossary::GlossaryEntry;
use trans_domain::region::{AnchoredPoint, AnchoredRegion};

pub const PRESET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionPreset {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    /// 进程名匹配时自动推荐这个预设，例如 `D2R.exe`。
    #[serde(default)]
    pub match_process: Option<String>,
    pub region: AnchoredRegion,
    /// 发送时点击的输入框位置；只复制不发送的场景可以不填。
    #[serde(default)]
    pub input_point: Option<AnchoredPoint>,
    pub parser: LineParser,
    #[serde(default)]
    pub preprocess: Preprocess,
    pub delivery: Delivery,
    #[serde(default)]
    pub glossary: Vec<GlossaryEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preprocess {
    /// OCR 前的放大倍数。M0-01 实测放大效果不单调，所以按预设调，不写死。
    pub scale: f32,
}

impl Default for Preprocess {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// 点击输入点 → 粘贴 → 发送键（网页聊天）。
    ClickPaste,
    /// 打开聊天键 → 粘贴 → 回车（游戏；反作弊风险未实测前默认不用）。
    GameChat,
    /// 只复制到剪贴板，由用户自己粘贴发送。
    CopyOnly,
}

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("预设 {name} 不是合法 JSON：{source}")]
    Parse {
        name: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("预设 {id} 无效：{}", problems.join("；"))]
    Invalid { id: String, problems: Vec<String> },
}

const BUILTIN: &[(&str, &str)] = &[
    // 第一个是界面上的默认选项：先能读出东西，再谈分辨谁说的。
    ("generic", include_str!("../presets/generic.json")),
    ("messenger-web", include_str!("../presets/messenger-web.json")),
    ("d2r-chat", include_str!("../presets/d2r-chat.json")),
];

pub fn builtin_presets() -> Result<Vec<RegionPreset>, PresetError> {
    BUILTIN.iter().map(|(name, json)| parse_preset(name, json)).collect()
}

pub fn parse_preset(name: &str, json: &str) -> Result<RegionPreset, PresetError> {
    let preset: RegionPreset =
        serde_json::from_str(json).map_err(|source| PresetError::Parse { name: name.to_owned(), source })?;
    let problems = validate(&preset);
    if problems.is_empty() {
        Ok(preset)
    } else {
        Err(PresetError::Invalid { id: preset.id, problems })
    }
}

pub fn validate(preset: &RegionPreset) -> Vec<String> {
    let mut problems = Vec::new();
    if preset.schema_version > PRESET_SCHEMA_VERSION {
        problems.push(format!("由更新版本创建（schema {}），请升级软件", preset.schema_version));
    }
    if preset.id.trim().is_empty() {
        problems.push("id 不能为空".into());
    }
    if preset.name.trim().is_empty() {
        problems.push("name 不能为空".into());
    }
    if !(1.0..=4.0).contains(&preset.preprocess.scale) {
        problems.push(format!("放大倍数 {} 超出 1～4 的范围", preset.preprocess.scale));
    }
    if preset.delivery == Delivery::ClickPaste && preset.input_point.is_none() {
        problems.push("点击粘贴发送需要设置输入框位置".into());
    }
    for entry in &preset.glossary {
        if entry.foreign.trim().is_empty() || entry.user.trim().is_empty() {
            problems.push(format!("术语表条目不完整：{} = {}", entry.foreign, entry.user));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use trans_domain::geometry::Size;

    use super::*;

    #[test]
    fn all_builtin_presets_parse_validate_and_resolve_at_common_sizes() {
        let presets = builtin_presets().unwrap();
        assert_eq!(presets.len(), BUILTIN.len());
        for preset in &presets {
            for size in [
                Size { width: 1280, height: 720 },
                Size { width: 1920, height: 1080 },
                Size { width: 2560, height: 1440 },
            ] {
                let rect = preset.region.resolve(size).unwrap_or_else(|e| panic!("{}: {e}", preset.id));
                assert!(rect.width >= 100 && rect.height >= 100, "{} too small at {size:?}: {rect:?}", preset.id);
            }
        }
    }

    #[test]
    fn games_default_to_copy_only_until_anti_cheat_risk_is_tested() {
        let d2r = builtin_presets().unwrap().into_iter().find(|p| p.id == "d2r-chat").unwrap();
        assert_eq!(d2r.delivery, Delivery::CopyOnly);
        assert!(matches!(d2r.parser, LineParser::Prefix { .. }));
    }

    #[test]
    fn rejects_out_of_range_scale_and_missing_input_point() {
        let mut preset = builtin_presets().unwrap().into_iter().find(|p| p.id == "messenger-web").unwrap();
        preset.preprocess.scale = 8.0;
        preset.input_point = None;
        let problems = validate(&preset);
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn reports_which_builtin_failed_to_parse() {
        let err = parse_preset("broken", "{").unwrap_err();
        assert!(err.to_string().starts_with("预设 broken 不是合法 JSON"));
    }
}
