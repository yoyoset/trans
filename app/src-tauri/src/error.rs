//! 返回给前端的错误：`code` 供界面判断下一步动作，`message` 是给人看的摘要，`detail` 是可展开的技术细节。

use serde::Serialize;
use trans_application::ports::ProviderError;
use trans_application::router::RouteError;
use trans_application::service::TranslateError;
use trans_domain::output::OutputError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), detail: None }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "internal command error");
        Self { code: "internal", message: "内部错误，请查看日志".into(), detail: Some(error.to_string()) }
    }
}

impl From<TranslateError> for CommandError {
    fn from(error: TranslateError) -> Self {
        let message = error.to_string();
        let (code, detail) = match &error {
            TranslateError::EmptyInput => ("empty_input", None),
            TranslateError::Route(RouteError::NoEngine) => ("no_engine", None),
            TranslateError::Route(RouteError::OnlineNotConfigured) => ("online_not_configured", None),
            TranslateError::Provider(provider) => provider_code(provider),
            TranslateError::Output(OutputError::NotJson { raw }) => ("output_not_json", Some(raw.clone())),
            TranslateError::Output(OutputError::MissingTranslation { raw }) => {
                ("output_missing_translation", Some(raw.clone()))
            }
        };
        Self { code, message, detail }
    }
}

fn provider_code(error: &ProviderError) -> (&'static str, Option<String>) {
    match error {
        ProviderError::Unavailable(reason) => ("engine_unavailable", Some(reason.clone())),
        ProviderError::Unauthorized => ("unauthorized", None),
        ProviderError::InsufficientBalance => ("insufficient_balance", None),
        ProviderError::RateLimited => ("rate_limited", None),
        ProviderError::Timeout => ("timeout", None),
        ProviderError::Http { status, body } => ("http_error", Some(format!("HTTP {status}: {body}"))),
        ProviderError::InvalidResponse(reason) => ("invalid_response", Some(reason.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_model_output_goes_to_detail_not_to_the_summary() {
        let error: CommandError = TranslateError::Output(OutputError::NotJson { raw: "HACKED".into() }).into();
        assert_eq!(error.code, "output_not_json");
        assert_eq!(error.message, "模型输出不是合法 JSON");
        assert_eq!(error.detail.as_deref(), Some("HACKED"));
    }

    #[test]
    fn actionable_codes_for_account_and_configuration_problems() {
        let balance: CommandError = TranslateError::Provider(ProviderError::InsufficientBalance).into();
        let no_online: CommandError = TranslateError::Route(RouteError::OnlineNotConfigured).into();
        assert_eq!((balance.code, no_online.code), ("insufficient_balance", "online_not_configured"));
    }

    #[test]
    fn serializes_without_empty_detail() {
        let json = serde_json::to_string(&CommandError::new("empty_input", "没有需要翻译的内容")).unwrap();
        assert_eq!(json, r#"{"code":"empty_input","message":"没有需要翻译的内容"}"#);
    }
}
