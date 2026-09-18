//! 把 OCR 出来的文本行拼成消息，并判断是谁说的。
//!
//! 两种场景规则完全不同，所以做成两个解析器，由区域预设选用：
//! - 网页聊天：靠气泡在左边还是右边判断。
//! - 游戏聊天：靠行首的「[频道] 名字:」判断。M0-01 实测 OCR 经常把方括号认错，所以必须容错。

use serde::{Deserialize, Serialize};

use crate::conversation::Speaker;
use crate::geometry::Rect;
use crate::ocr_text::normalize_ocr_text;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LineParser {
    /// 通用：框里的每一行都当成对方的内容，不判断谁说的。
    /// 用在没有气泡、也没有名字前缀的地方（网页正文、任意应用窗口）。
    Plain,
    /// 网页聊天：气泡靠左是对方，靠右是自己。
    Bubble,
    /// 游戏聊天：解析行首的发送者名字；名字等于 self_name 的是自己。
    Prefix {
        #[serde(default)]
        self_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedMessage {
    pub speaker: Speaker,
    /// 发送者名字（游戏聊天能解析出来，网页气泡通常没有）。
    pub sender: Option<String>,
    pub text: String,
}

/// 相邻两行的垂直间距小于行高的这个倍数时，算同一条消息的换行。
const SAME_MESSAGE_GAP: f64 = 1.2;

/// 名字长度上限：超过这个长度多半是正文里的冒号，不是发送者。
const MAX_SENDER_CHARS: usize = 24;

pub fn parse_lines(lines: &[OcrLine], region_width: u32, parser: &LineParser) -> Vec<ParsedMessage> {
    let mut sorted: Vec<&OcrLine> = lines.iter().filter(|l| !l.text.trim().is_empty()).collect();
    sorted.sort_by_key(|l| (l.bounds.y, l.bounds.x));
    match parser {
        LineParser::Plain => parse_plain(&sorted),
        LineParser::Bubble => parse_bubbles(&sorted, region_width),
        LineParser::Prefix { self_name } => parse_prefixed(&sorted, self_name.as_deref()),
    }
}

/// 行距接近的相邻行合成一条，其余各自成条；一律算对方说的。
fn parse_plain(lines: &[&OcrLine]) -> Vec<ParsedMessage> {
    let mut messages: Vec<ParsedMessage> = Vec::new();
    let mut previous: Option<&OcrLine> = None;
    for line in lines {
        let text = normalize_ocr_text(line.text.trim());
        let continues = previous.is_some_and(|prev| vertical_gap(prev, line) < SAME_MESSAGE_GAP);
        match messages.last_mut().filter(|_| continues) {
            Some(last) => {
                last.text.push(' ');
                last.text.push_str(&text);
            }
            None => messages.push(ParsedMessage { speaker: Speaker::Them, sender: None, text }),
        }
        previous = Some(line);
    }
    messages
}

fn parse_bubbles(lines: &[&OcrLine], region_width: u32) -> Vec<ParsedMessage> {
    let mut messages: Vec<ParsedMessage> = Vec::new();
    let mut previous: Option<&OcrLine> = None;
    for line in lines {
        let speaker = bubble_speaker(line, region_width);
        let continues = previous.is_some_and(|prev| {
            bubble_speaker(prev, region_width) == speaker && vertical_gap(prev, line) < SAME_MESSAGE_GAP
        });
        let text = normalize_ocr_text(line.text.trim());
        match messages.last_mut().filter(|_| continues) {
            Some(last) => {
                last.text.push(' ');
                last.text.push_str(&text);
            }
            None => messages.push(ParsedMessage { speaker, sender: None, text }),
        }
        previous = Some(line);
    }
    messages
}

/// 气泡中心在右半边算自己，否则算对方。居中判不准时按对方处理：
/// 把自己的消息误判成对方只是多翻一句，反过来会漏掉对方的话。
fn bubble_speaker(line: &OcrLine, region_width: u32) -> Speaker {
    let center = f64::from(line.bounds.x) + f64::from(line.bounds.width) / 2.0;
    if center > f64::from(region_width) * 0.55 {
        Speaker::Me
    } else {
        Speaker::Them
    }
}

fn vertical_gap(previous: &OcrLine, current: &OcrLine) -> f64 {
    let previous_bottom = f64::from(previous.bounds.y) + f64::from(previous.bounds.height);
    let height = f64::from(previous.bounds.height).max(1.0);
    (f64::from(current.bounds.y) - previous_bottom) / height
}

fn parse_prefixed(lines: &[&OcrLine], self_name: Option<&str>) -> Vec<ParsedMessage> {
    let mut messages: Vec<ParsedMessage> = Vec::new();
    for line in lines {
        let text = normalize_ocr_text(line.text.trim());
        match split_sender(&text) {
            Some((sender, body)) => {
                let mine = self_name.is_some_and(|me| me.eq_ignore_ascii_case(&sender));
                let speaker = if mine { Speaker::Me } else { Speaker::Them };
                messages.push(ParsedMessage { speaker, sender: Some(sender), text: body });
            }
            // 没有名字前缀：接在上一条后面（长消息会换行），没有上一条就按对方的话处理。
            None => match messages.last_mut() {
                Some(last) => {
                    last.text.push(' ');
                    last.text.push_str(&text);
                }
                None => messages.push(ParsedMessage { speaker: Speaker::Them, sender: None, text }),
            },
        }
    }
    messages
}

/// 从「[频道] 名字: 内容」里取出名字和内容。
fn split_sender(line: &str) -> Option<(String, String)> {
    const COLONS: [char; 2] = [':', '\u{ff1a}'];
    let without_channel = strip_channel(line);
    let colon = without_channel.find(COLONS)?;
    let separator = without_channel[colon..].chars().next()?.len_utf8();
    let sender = without_channel[..colon].trim();
    let body = without_channel[colon + separator..].trim();
    let looks_like_a_name =
        !sender.is_empty() && sender.chars().count() <= MAX_SENDER_CHARS && !sender.contains(char::is_whitespace);
    (looks_like_a_name && !body.is_empty()).then(|| (sender.to_owned(), body.to_owned()))
}

/// 剥掉行首的频道段。右括号可能被 OCR 认成别的字符（M0-01 实测），所以找不到右括号时退而用空格断开。
fn strip_channel(line: &str) -> &str {
    const CLOSERS: [char; 4] = [']', ')', '}', '\u{3011}'];
    let Some(rest) = line.strip_prefix('[') else { return line };
    match rest.find(CLOSERS).or_else(|| rest.find(' ')) {
        Some(end) => rest[end + 1..].trim_start(),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x: i32, y: i32) -> OcrLine {
        OcrLine { text: text.into(), bounds: Rect { x, y, width: text.chars().count() as u32 * 8, height: 20 } }
    }

    #[test]
    fn plain_reads_every_line_regardless_of_where_it_sits() {
        // 靠右的那行在气泡模式下会算成"自己"而被丢掉；通用模式必须照样读出来。
        let lines = [line("网页上的一段中文", 20, 10), line("Right aligned text", 420, 70)];
        let parsed = parse_lines(&lines, 520, &LineParser::Plain);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().all(|m| m.speaker == Speaker::Them && m.sender.is_none()));
        assert_eq!(parsed[1].text, "Right aligned text");
    }

    #[test]
    fn plain_joins_lines_that_belong_to_the_same_paragraph() {
        let lines = [line("first half of a sentence", 20, 10), line("and its second half", 20, 32)];
        let parsed = parse_lines(&lines, 520, &LineParser::Plain);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "first half of a sentence and its second half");
    }

    #[test]
    fn bubbles_on_the_left_are_them_and_on_the_right_are_me() {
        let lines = [line("Hi there! Is the item still available?", 20, 10), line("Yeah, still here.", 320, 70)];
        let parsed = parse_lines(&lines, 520, &LineParser::Bubble);
        assert_eq!(parsed[0].speaker, Speaker::Them);
        assert_eq!(parsed[1].speaker, Speaker::Me);
        assert_eq!(parsed[0].text, "Hi there! Is the item still available?");
    }

    #[test]
    fn adjacent_lines_of_the_same_bubble_join_into_one_message() {
        let lines =
            [line("Could you do 40 dollars", 20, 10), line("including shipping?", 20, 32), line("lol no", 320, 120)];
        let parsed = parse_lines(&lines, 520, &LineParser::Bubble);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "Could you do 40 dollars including shipping?");
    }

    #[test]
    fn game_lines_keep_the_sender_and_drop_the_channel() {
        let lines = [
            line("[Trade] Stormrage: WTS Enigma 2.5b, pm me", 0, 0),
            line("[Party] Zhan: gg, need 1 more", 0, 25),
            line("MyChar: omw", 0, 50),
        ];
        let parsed = parse_lines(&lines, 640, &LineParser::Prefix { self_name: Some("MyChar".into()) });
        assert_eq!(parsed[0].sender.as_deref(), Some("Stormrage"));
        assert_eq!(parsed[0].text, "WTS Enigma 2.5b, pm me");
        assert_eq!(parsed[0].speaker, Speaker::Them);
        assert_eq!(parsed[2].speaker, Speaker::Me, "my own character is not translated as incoming");
    }

    #[test]
    fn tolerates_the_bracket_errors_ocr_actually_made() {
        // M0-01 实测：Trade 的右括号被认成 l，Guild 的右括号被认成右圆括号。
        let lines = [line("[Tradel Stormrage: WTS Enigma", 0, 0), line("[Guild) Nyx: brb 5 min", 0, 25)];
        let parsed = parse_lines(&lines, 640, &LineParser::Prefix { self_name: None });
        assert_eq!(parsed[0].sender.as_deref(), Some("Stormrage"));
        assert_eq!(parsed[1].sender.as_deref(), Some("Nyx"));
        assert_eq!(parsed[1].text, "brb 5 min");
    }

    #[test]
    fn a_colon_inside_the_body_does_not_look_like_a_sender() {
        let lines = [line("Zhan: can you drop the key at act 5", 0, 0), line("meet me at: the stairs", 0, 25)];
        let parsed = parse_lines(&lines, 640, &LineParser::Prefix { self_name: None });
        assert_eq!(parsed.len(), 1, "second line is a continuation, not a new sender");
        assert_eq!(parsed[0].text, "can you drop the key at act 5 meet me at: the stairs");
    }

    #[test]
    fn chinese_lines_lose_the_spaces_windows_ocr_inserts() {
        let lines = [line("duifang: 你 好 吗", 0, 0)];
        let parsed = parse_lines(&lines, 640, &LineParser::Prefix { self_name: None });
        assert_eq!(parsed[0].text, "你好吗");
    }

    #[test]
    fn empty_lines_are_dropped() {
        assert!(parse_lines(&[line("   ", 0, 0)], 640, &LineParser::Bubble).is_empty());
    }
}
