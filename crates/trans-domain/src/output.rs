//! 模型输出解析。输出必须是约定的 JSON，解析失败就是失败，绝不把原始文本当译文。

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelOutput {
    pub translation: String,
    /// 在线校对返回的修改说明；实时翻译没有这个字段。
    #[serde(default)]
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OutputError {
    #[error("模型输出不是合法 JSON")]
    NotJson { raw: String },
    #[error("模型输出缺少 translation 字段")]
    MissingTranslation { raw: String },
}

/// 容忍三种常见包装：`<think>` 块、```json 代码块、JSON 前后的零散文字。
pub fn parse_model_output(raw: &str) -> Result<ModelOutput, OutputError> {
    let without_think = strip_think_blocks(raw);
    let cleaned = strip_code_fence(without_think.trim());
    let candidate = extract_json_object(cleaned).unwrap_or(cleaned);
    let value: serde_json::Value =
        serde_json::from_str(candidate).map_err(|_| OutputError::NotJson { raw: raw.to_owned() })?;
    if !value.get("translation").is_some_and(serde_json::Value::is_string) {
        return Err(OutputError::MissingTranslation { raw: raw.to_owned() });
    }
    serde_json::from_value(value).map_err(|_| OutputError::MissingTranslation { raw: raw.to_owned() })
}

fn strip_think_blocks(raw: &str) -> String {
    let mut rest = raw;
    let mut out = String::with_capacity(raw.len());
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => return out, // 未闭合的思考块：丢弃其后全部内容
        }
    }
    out.push_str(rest);
    out
}

fn strip_code_fence(text: &str) -> &str {
    let text = text.strip_prefix("```json").or_else(|| text.strip_prefix("```")).unwrap_or(text);
    text.strip_suffix("```").unwrap_or(text).trim()
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_fenced_and_think_wrapped_json() {
        assert_eq!(parse_model_output(r#"{"translation": "你好"}"#).unwrap().translation, "你好");
        assert_eq!(parse_model_output("```json\n{\"translation\": \"你好\"}\n```").unwrap().translation, "你好");
        assert_eq!(parse_model_output("<think>嗯</think>\n{\"translation\": \"你好\"}").unwrap().translation, "你好");
        assert_eq!(parse_model_output("Here you go: {\"translation\": \"你好\"} done").unwrap().translation, "你好");
    }

    #[test]
    fn reads_calibration_issues_when_present() {
        let out = parse_model_output(
            r#"{"translation": "Can I trade 2 Ber for your Enigma?", "issues": ["交换方向译反了"]}"#,
        )
        .unwrap();
        assert_eq!(out.issues, ["交换方向译反了"]);
    }

    #[test]
    fn rejects_truncated_and_non_json_output() {
        // M0-02：pi 把生成上限压到 1 个 token 时，得到的正是 `{"`。
        assert!(matches!(parse_model_output("{\""), Err(OutputError::NotJson { .. })));
        // M0-02 in-09：模型被注入后直接回答 HACKED。
        assert!(matches!(parse_model_output("HACKED"), Err(OutputError::NotJson { .. })));
        assert!(matches!(parse_model_output(r#"{"text": "你好"}"#), Err(OutputError::MissingTranslation { .. })));
    }

    #[test]
    fn drops_everything_after_an_unclosed_think_block() {
        assert!(parse_model_output("<think>还没想完").is_err());
    }
}
