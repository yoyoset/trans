//! 对话上下文。在线校对时把最近几轮对话作为数据带给模型。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    Me,
    Them,
}

impl Speaker {
    pub fn as_str(self) -> &'static str {
        match self {
            Speaker::Me => "me",
            Speaker::Them => "them",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub speaker: Speaker,
    pub text: String,
}

/// 取最后 `max` 轮；上下文越长越慢越贵，由设置决定上限。
pub fn recent_turns(turns: &[Turn], max: usize) -> &[Turn] {
    &turns[turns.len().saturating_sub(max)..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(text: &str) -> Turn {
        Turn { speaker: Speaker::Them, text: text.into() }
    }

    #[test]
    fn keeps_only_the_latest_turns() {
        let turns = vec![turn("a"), turn("b"), turn("c")];
        let recent: Vec<_> = recent_turns(&turns, 2).iter().map(|t| t.text.as_str()).collect();
        assert_eq!(recent, ["b", "c"]);
        assert_eq!(recent_turns(&turns, 10).len(), 3);
        assert!(recent_turns(&turns, 0).is_empty());
    }
}
