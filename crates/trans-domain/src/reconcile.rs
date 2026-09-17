//! 跨帧去重：同一屏内容会被反复识别，只有"这次新出现的消息"才需要翻译。
//!
//! 两步判断：
//! 1. **按顺序对齐上一帧**：聊天是按时间往下追加的，上一帧末尾的若干条会原样出现在这一帧里，
//!    对齐之后排在它们后面的就是新消息——哪怕和以前说过的话一模一样（对方又发一次 "ok"）。
//! 2. **对不齐时退回历史比对**（例如窗口被别的程序挡住后再露出来、聊天大幅滚动）：
//!    和最近见过的消息相似就当作旧的。
//!
//! 最初只有第 2 步：任何和最近 60 条里某条相似度 ≥0.85 的消息都被丢掉，
//! 实测（单元测试）对方重复发 "ok"、"price is 40 dollars" 后又发 "price is 45 dollars" 都不会显示。
//!
//! OCR 每帧结果会有细微抖动（M0-01 实测：同一句里的 item 会被认成 tem），
//! 所以同一条消息的判断用编辑距离相似度，不能用精确相等。

use std::collections::VecDeque;

use crate::chat_lines::ParsedMessage;
use crate::conversation::Speaker;

/// 相似度超过这个值就认为是同一条消息。
const DEFAULT_THRESHOLD: f64 = 0.85;

type Fingerprint = (Speaker, String);

#[derive(Debug)]
pub struct Reconciler {
    /// 说话人必须精确相等，只有正文做模糊比较：否则对方和我自己的同一句话会互相判成重复。
    recent: VecDeque<Fingerprint>,
    /// 上一帧识别到的消息，按屏幕上从上到下的顺序。
    previous: Vec<Fingerprint>,
    capacity: usize,
    threshold: f64,
}

impl Reconciler {
    pub fn new(capacity: usize) -> Self {
        Self { recent: VecDeque::new(), previous: Vec::new(), capacity: capacity.max(1), threshold: DEFAULT_THRESHOLD }
    }

    pub fn with_threshold(capacity: usize, threshold: f64) -> Self {
        Self { threshold, ..Self::new(capacity) }
    }

    /// 返回这一帧里新出现的消息，并把它们记入历史。
    pub fn ingest(&mut self, messages: Vec<ParsedMessage>) -> Vec<ParsedMessage> {
        let messages: Vec<(Fingerprint, ParsedMessage)> = messages
            .into_iter()
            .map(|message| ((message.speaker, fingerprint(&message)), message))
            .filter(|((_, text), _)| !text.is_empty())
            .collect();
        let current: Vec<Fingerprint> = messages.iter().map(|(print, _)| print.clone()).collect();

        let is_new = match self.align(&current) {
            // 对齐段之前的是屏幕上方的内容，可能是上一帧 OCR 漏掉的，按历史判断；对齐段之后的一律是新的。
            Some(Alignment { start, len }) => {
                let recent = &self.recent;
                let threshold = self.threshold;
                current
                    .iter()
                    .enumerate()
                    .map(|(i, print)| if i < start { !seen_in(recent, print, threshold) } else { i >= start + len })
                    .collect::<Vec<_>>()
            }
            None => current.iter().map(|print| !seen_in(&self.recent, print, self.threshold)).collect(),
        };

        let mut fresh = Vec::new();
        for ((print, message), new) in messages.into_iter().zip(is_new) {
            if !new {
                continue;
            }
            if self.recent.len() == self.capacity {
                self.recent.pop_front();
            }
            self.recent.push_back(print);
            fresh.push(message);
        }
        self.previous = current;
        fresh
    }

    /// 换目标窗口或换预设时调用，避免把旧会话的内容当成重复。
    pub fn reset(&mut self) {
        self.recent.clear();
        self.previous.clear();
    }

    /// 找上一帧末尾最长的一段在这一帧里的位置。长度相同取最靠上的位置，
    /// 这样 [x, ok] → [x, ok, ok] 会对齐到开头，末尾那条 ok 算新的。
    fn align(&self, current: &[Fingerprint]) -> Option<Alignment> {
        let previous = &self.previous;
        for len in (1..=previous.len().min(current.len())).rev() {
            let tail = &previous[previous.len() - len..];
            for start in 0..=current.len() - len {
                let matches = tail.iter().zip(&current[start..start + len]).all(|(a, b)| same(a, b, self.threshold));
                if matches {
                    return Some(Alignment { start, len });
                }
            }
        }
        None
    }
}

struct Alignment {
    start: usize,
    len: usize,
}

fn same(a: &Fingerprint, b: &Fingerprint, threshold: f64) -> bool {
    a.0 == b.0 && similarity(&a.1, &b.1) >= threshold
}

fn seen_in(recent: &VecDeque<Fingerprint>, print: &Fingerprint, threshold: f64) -> bool {
    recent.iter().any(|seen| same(seen, print, threshold))
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
    fn a_repeated_short_reply_at_the_bottom_is_a_new_message() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("are you still there?"), message("ok")]);
        let fresh = reconciler.ingest(vec![message("are you still there?"), message("ok"), message("ok")]);
        assert_eq!(texts(fresh), ["ok"]);
    }

    #[test]
    fn a_new_line_that_only_differs_a_little_from_an_old_one_is_still_new() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("price is 40 dollars")]);
        let fresh = reconciler.ingest(vec![message("price is 40 dollars"), message("price is 45 dollars")]);
        assert_eq!(texts(fresh), ["price is 45 dollars"]);
    }

    #[test]
    fn a_line_ocr_missed_in_the_previous_frame_is_reported_when_it_shows_up() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("first message"), message("third message")]);
        let fresh =
            reconciler.ingest(vec![message("first message"), message("second message"), message("third message")]);
        assert_eq!(texts(fresh), ["second message"]);
    }

    #[test]
    fn after_the_chat_was_covered_by_another_window_only_the_really_new_line_is_reported() {
        let mut reconciler = Reconciler::new(20);
        reconciler.ingest(vec![message("hello friend"), message("want to trade?")]);
        // 用户切到别的程序，挡住了聊天区域：识别到的是别的内容。
        reconciler.ingest(vec![message("some unrelated editor text"), message("another unrelated line")]);
        let fresh = reconciler.ingest(vec![
            message("hello friend"),
            message("want to trade?"),
            message("i can pay 40 dollars"),
        ]);
        assert_eq!(texts(fresh), ["i can pay 40 dollars"]);
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
