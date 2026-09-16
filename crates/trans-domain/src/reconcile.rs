//! 跨帧去重：同一屏内容会被反复识别，只有"这次新出现的消息"才需要翻译。
//!
//! OCR 每帧结果会有细微抖动（M0-01 实测：同一句里的 item 会被认成 tem），
//! 所以判重用编辑距离相似度，不能用精确相等。

use std::collections::VecDeque;

use crate::chat_lines::ParsedMessage;
use crate::conversation::Speaker;

/// 相似度超过这个值就认为是同一条消息。
const DEFAULT_THRESHOLD: f64 = 0.85;

#[derive(Debug)]
pub struct Reconciler {
    /// 说话人必须精确相等，只有正文做模糊比较：否则对方和我自己的同一句话会互相判成重复。
    recent: VecDeque<(Speaker, String)>,
    capacity: usize,
    threshold: f64,
}

impl Reconciler {
    pub fn new(capacity: usize) -> Self {
        Self { recent: VecDeque::new(), capacity: capacity.max(1), threshold: DEFAULT_THRESHOLD }
    }

    pub fn with_threshold(capacity: usize, threshold: f64) -> Self {
        Self { threshold, ..Self::new(capacity) }
    }

    /// 返回这一帧里新出现的消息，并把它们记入历史。
    ///
    /// 已知取舍：对方把同一句话连发两遍时，第二遍会被当成重复丢掉。
    pub fn ingest(&mut self, messages: Vec<ParsedMessage>) -> Vec<ParsedMessage> {
        let mut fresh = Vec::new();
        for message in messages {
            let text = fingerprint(&message);
            let duplicate = self
                .recent
                .iter()
                .any(|(speaker, seen)| *speaker == message.speaker && similarity(seen, &text) >= self.threshold);
            if text.is_empty() || duplicate {
                continue;
            }
            if self.recent.len() == self.capacity {
                self.recent.pop_front();
            }
            self.recent.push_back((message.speaker, text));
            fresh.push(message);
        }
        fresh
    }

    /// 换目标窗口或换预设时调用，避免把旧会话的内容当成重复。
    pub fn reset(&mut self) {
        self.recent.clear();
    }
}

/// 判重用的正文：折叠空白、忽略大小写。
fn fingerprint(message: &ParsedMessage) -> String {
    message.text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// 1.0 表示完全相同，0.0 表示毫无相似。
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    1.0 - levenshtein(&a, &b) as f64 / longest as f64
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (previous[j] + cost).min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::Speaker;

    fn message(text: &str) -> ParsedMessage {
        ParsedMessage { speaker: Speaker::Them, sender: None, text: text.into() }
    }

    fn texts(messages: Vec<ParsedMessage>) -> Vec<String> {
        messages.into_iter().map(|m| m.text).collect()
    }

    #[test]
    fn the_same_screen_captured_twice_yields_new_messages_only_once() {
        let mut reconciler = Reconciler::new(20);
        let frame = || vec![message("Hi there!"), message("Is the item still available?")];
        assert_eq!(reconciler.ingest(frame()).len(), 2);
        assert!(reconciler.ingest(frame()).is_empty());
    }

    #[test]
    fn ocr_jitter_on_the_same_sentence_is_not_a_new_message() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("Yeah, it's still here. Are you interested?")]);
        // M0-01 实测的抖动：漏掉一个 i。
        let jittered = reconciler.ingest(vec![message("Yeah, t's still here. Are you interested?")]);
        assert!(jittered.is_empty(), "{jittered:?}");
    }

    #[test]
    fn scrolling_keeps_old_lines_quiet_and_reports_the_new_one() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("first message"), message("second message")]);
        let fresh = reconciler.ingest(vec![message("second message"), message("third message")]);
        assert_eq!(texts(fresh), ["third message"]);
    }

    #[test]
    fn the_same_text_from_the_other_side_is_a_different_message() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("sounds good to me")]);
        let mine = ParsedMessage { speaker: Speaker::Me, sender: None, text: "sounds good to me".into() };
        assert_eq!(reconciler.ingest(vec![mine]).len(), 1);
    }

    #[test]
    fn history_beyond_capacity_is_forgotten_so_long_chats_do_not_grow_without_bound() {
        let mut reconciler = Reconciler::new(2);
        reconciler.ingest(vec![message("alpha"), message("bravo"), message("charlie")]);
        assert_eq!(texts(reconciler.ingest(vec![message("alpha")])), ["alpha"], "oldest entry was dropped");
    }

    #[test]
    fn reset_forgets_the_previous_conversation() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("hello there")]);
        reconciler.reset();
        assert_eq!(texts(reconciler.ingest(vec![message("hello there")])), ["hello there"]);
    }

    #[test]
    fn similarity_is_symmetric_and_bounded() {
        assert_eq!(similarity("abc", "abc"), 1.0);
        assert_eq!(similarity("", ""), 1.0);
        assert!((similarity("kitten", "sitting") - similarity("sitting", "kitten")).abs() < f64::EPSILON);
        assert!(similarity("kitten", "sitting") < 0.6);
    }
}
