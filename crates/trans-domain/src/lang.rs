//! 语言、翻译方向与语气。语言用数据表描述，新增语言改表，不改逻辑。

use serde::{Deserialize, Serialize};

/// BCP-47 语言标签（如 `zh`、`en`、`zh-Hans`），统一小写保存。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Lang(String);

impl Lang {
    pub fn new(tag: &str) -> Self {
        Self(tag.trim().to_ascii_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 主语言子标签：`zh-hans` → `zh`。
    pub fn primary(&self) -> &str {
        self.0.split(['-', '_']).next().unwrap_or("")
    }

    pub fn script(&self) -> Script {
        let primary = self.primary();
        if HAN_LANGS.contains(&primary) {
            Script::Han
        } else if KANA_LANGS.contains(&primary) {
            Script::Kana
        } else if HANGUL_LANGS.contains(&primary) {
            Script::Hangul
        } else if LATIN_LANGS.contains(&primary) {
            Script::Latin
        } else {
            Script::Unknown
        }
    }
}

/// 译文校验只关心书写系统，不关心具体语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Script {
    Han,
    Latin,
    /// 日文：假名为主，夹杂汉字。
    Kana,
    /// 朝鲜文：谚文。
    Hangul,
    /// 未收录的语言：跳过书写系统校验，而不是误报。
    Unknown,
}

const HAN_LANGS: &[&str] = &["zh"];
const KANA_LANGS: &[&str] = &["ja"];
const HANGUL_LANGS: &[&str] = &["ko"];
const LATIN_LANGS: &[&str] =
    &["en", "fr", "de", "es", "it", "pt", "nl", "sv", "da", "nb", "no", "fi", "pl", "cs", "tr", "id", "ms", "vi"];

/// `Incoming`：对方的外语消息 → 用户语言；`Outgoing`：用户输入 → 外语。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePair {
    /// 用户自己的语言（默认中文）。
    pub user: Lang,
    /// 对方使用的语言（默认英文）。
    pub foreign: Lang,
}

impl LanguagePair {
    pub fn source(&self, direction: Direction) -> &Lang {
        match direction {
            Direction::Incoming => &self.foreign,
            Direction::Outgoing => &self.user,
        }
    }

    pub fn target(&self, direction: Direction) -> &Lang {
        match direction {
            Direction::Incoming => &self.user,
            Direction::Outgoing => &self.foreign,
        }
    }
}

impl Default for LanguagePair {
    fn default() -> Self {
        Self { user: Lang::new("zh"), foreign: Lang::new("en") }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Casual,
    Formal,
}

impl Tone {
    pub fn as_str(self) -> &'static str {
        match self {
            Tone::Casual => "casual",
            Tone::Formal => "formal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_subtag_and_script() {
        assert_eq!(Lang::new("zh-Hans").primary(), "zh");
        assert_eq!(Lang::new("ZH_CN").script(), Script::Han);
        assert_eq!(Lang::new("en-US").script(), Script::Latin);
        assert_eq!(Lang::new("ja").script(), Script::Kana);
        assert_eq!(Lang::new("ko-KR").script(), Script::Hangul);
        assert_eq!(Lang::new("sw").script(), Script::Unknown, "没收录的语言跳过书写系统校验");
    }

    #[test]
    fn source_and_target_follow_direction() {
        let pair = LanguagePair::default();
        assert_eq!(pair.source(Direction::Incoming).as_str(), "en");
        assert_eq!(pair.target(Direction::Incoming).as_str(), "zh");
        assert_eq!(pair.source(Direction::Outgoing).as_str(), "zh");
        assert_eq!(pair.target(Direction::Outgoing).as_str(), "en");
    }
}
