//! 术语表：按区域预设加载，只把原文里真正出现的词条带进提示词。
//!
//! M0-02 实测：整张术语表放进系统提示词时，4B 模型把 "baal run" 串进了无关回复。

use serde::{Deserialize, Serialize};

use crate::lang::Direction;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    /// 外语一侧的词，如 `WTS`。
    pub foreign: String,
    /// 用户语言一侧的词，如 `出售`。
    pub user: String,
}

impl GlossaryEntry {
    pub fn source_term(&self, direction: Direction) -> &str {
        match direction {
            Direction::Incoming => &self.foreign,
            Direction::Outgoing => &self.user,
        }
    }

    pub fn target_term(&self, direction: Direction) -> &str {
        match direction {
            Direction::Incoming => &self.user,
            Direction::Outgoing => &self.foreign,
        }
    }
}

/// 外语词按"整词、忽略大小写"匹配（`Ber` 不匹配 `Bernard`）；用户语言词按子串匹配。
pub fn match_glossary<'a>(text: &str, direction: Direction, entries: &'a [GlossaryEntry]) -> Vec<&'a GlossaryEntry> {
    entries
        .iter()
        .filter(|entry| match direction {
            Direction::Incoming => contains_word(text, &entry.foreign),
            Direction::Outgoing => !entry.user.is_empty() && text.contains(entry.user.as_str()),
        })
        .collect()
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.trim().is_empty() {
        return false;
    }
    let hay = haystack.to_lowercase();
    let needle = needle.to_lowercase();
    hay.match_indices(&needle).any(|(start, found)| {
        let before = hay[..start].chars().next_back();
        let after = hay[start + found.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphabetic()) && !after.is_some_and(|c| c.is_ascii_alphabetic())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d2r() -> Vec<GlossaryEntry> {
        [("WTS", "出售"), ("pm", "私聊"), ("Enigma", "谜团"), ("Ber", "Ber 符文"), ("baal run", "刷巴尔")]
            .into_iter()
            .map(|(foreign, user)| GlossaryEntry { foreign: foreign.into(), user: user.into() })
            .collect()
    }

    fn foreign_terms(text: &str, direction: Direction) -> Vec<String> {
        match_glossary(text, direction, &d2r()).into_iter().map(|e| e.foreign.clone()).collect()
    }

    #[test]
    fn english_terms_match_whole_words_case_insensitively() {
        assert_eq!(foreign_terms("[Trade] wts Enigma 2.5b, PM me", Direction::Incoming), ["WTS", "pm", "Enigma"]);
        assert!(foreign_terms("Bernard is here", Direction::Incoming).is_empty());
        assert_eq!(foreign_terms("need 1 more for baal run", Direction::Incoming), ["baal run"]);
    }

    #[test]
    fn digits_count_as_word_boundaries() {
        // 已知取舍："5pm" 会匹配 pm。聊天里 "5pm" 很少见，交易频道的 "pm" 很常见。
        assert_eq!(foreign_terms("5pm", Direction::Incoming), ["pm"]);
    }

    #[test]
    fn user_language_terms_match_as_substrings() {
        assert_eq!(foreign_terms("我用 2 个 Ber 换你的谜团可以吗？", Direction::Outgoing), ["Enigma"]);
        // M0-02 out-05：这句话不含任何术语，不能带上 "baal run"。
        assert!(foreign_terms("谢谢兄弟，下次一起刷。", Direction::Outgoing).is_empty());
    }
}
