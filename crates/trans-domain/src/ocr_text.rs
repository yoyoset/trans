//! OCR 文本规范化。
//!
//! M0-01 实测：Windows OCR 识别中文时会在每个汉字之间插入空格（`测 试 方 法`、`10 ． 8MM`）。

/// 删除两个"宽字符"之间、以及宽字符与数字之间的空格；中英文之间的空格保留（`Ber 符文`）。
pub fn normalize_ocr_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let prev = out.trim_end_matches(' ').chars().next_back();
            let next = chars[i + 1..].iter().copied().find(|&n| n != ' ');
            if let (Some(p), Some(n)) = (prev, next) {
                if joins_without_space(p, n) {
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

fn joins_without_space(a: char, b: char) -> bool {
    (is_wide(a) && (is_wide(b) || b.is_ascii_digit())) || (a.is_ascii_digit() && is_wide(b))
}

fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF   // CJK 扩展 A
        | 0x4E00..=0x9FFF // CJK 基本区
        | 0x3000..=0x303F // CJK 标点
        | 0xFF00..=0xFFEF // 全角字符
    )
}

#[cfg(test)]
mod tests {
    use super::normalize_ocr_text as n;

    #[test]
    fn removes_spaces_windows_ocr_inserts_between_han_characters() {
        assert_eq!(n("测 试 方 法 ， 把 耗 材 送 进 去"), "测试方法，把耗材送进去");
        assert_eq!(n("你 好  吗"), "你好吗");
    }

    #[test]
    fn removes_spaces_between_digits_and_fullwidth_punctuation() {
        assert_eq!(n("正 常 高 度 是 10 ． 8MM"), "正常高度是10．8MM");
    }

    #[test]
    fn keeps_spaces_between_latin_and_han_and_between_latin_words() {
        assert_eq!(n("我用 2 个 Ber 符文"), "我用2个 Ber 符文");
        assert_eq!(n("Hello world"), "Hello world");
    }
}
