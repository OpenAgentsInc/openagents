use crate::contract::{AppleFmErrorCode, AppleFmErrorDetail, AppleFmToolCallError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Typed Foundation Models error surfaced by the reusable Rust lane.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Error)]
#[error("{message}")]
pub struct AppleFmFoundationModelsError {
    /// Typed error family aligned to the Python SDK contract.
    pub kind: AppleFmErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Optional platform failure reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Optional platform recovery suggestion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_suggestion: Option<String>,
    /// Optional lower-level debug detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_description: Option<String>,
    /// Optional refusal explanation returned by Apple FM safety handling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_explanation: Option<String>,
    /// Optional failed tool name for tool-call failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Optional underlying tool/runtime detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underlying_error: Option<String>,
}

impl AppleFmFoundationModelsError {
    /// Returns whether callers may reasonably retry this error class.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            AppleFmErrorCode::AssetsUnavailable
                | AppleFmErrorCode::RateLimited
                | AppleFmErrorCode::ConcurrentRequests
                | AppleFmErrorCode::ServerError
                | AppleFmErrorCode::Unknown
        )
    }

    /// Returns the typed tool-call failure if this is a tool-call error.
    #[must_use]
    pub fn tool_call_error(&self) -> Option<AppleFmToolCallError> {
        if self.kind != AppleFmErrorCode::ToolCallFailed {
            return None;
        }
        Some(AppleFmToolCallError::new(
            self.tool_name
                .clone()
                .unwrap_or_else(|| "unknown_tool".to_string()),
            self.underlying_error
                .clone()
                .unwrap_or_else(|| self.message.clone()),
        ))
    }
}

impl From<AppleFmErrorDetail> for AppleFmFoundationModelsError {
    fn from(detail: AppleFmErrorDetail) -> Self {
        let kind = detail.kind();
        Self {
            kind,
            message: detail.message,
            failure_reason: detail.failure_reason,
            recovery_suggestion: detail.recovery_suggestion,
            debug_description: detail.debug_description,
            refusal_explanation: detail.refusal_explanation,
            tool_name: detail.tool_name,
            underlying_error: detail.underlying_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppleFmFoundationModelsError;
    use crate::contract::{AppleFmErrorCode, AppleFmErrorDetail};

    #[test]
    fn foundation_models_error_maps_python_family_codes() {
        let cases = [
            (
                "exceeded_context_window_size",
                AppleFmErrorCode::ExceededContextWindowSize,
                false,
            ),
            (
                "assets_unavailable",
                AppleFmErrorCode::AssetsUnavailable,
                true,
            ),
            (
                "guardrail_violation",
                AppleFmErrorCode::GuardrailViolation,
                false,
            ),
            (
                "unsupported_guide",
                AppleFmErrorCode::UnsupportedGuide,
                false,
            ),
            (
                "unsupported_language_or_locale",
                AppleFmErrorCode::UnsupportedLanguageOrLocale,
                false,
            ),
            ("decoding_failure", AppleFmErrorCode::DecodingFailure, false),
            ("rate_limited", AppleFmErrorCode::RateLimited, true),
            (
                "concurrent_requests",
                AppleFmErrorCode::ConcurrentRequests,
                true,
            ),
            ("refusal", AppleFmErrorCode::Refusal, false),
            (
                "invalid_generation_schema",
                AppleFmErrorCode::InvalidGenerationSchema,
                false,
            ),
        ];

        for (code, expected_kind, expected_retryable) in cases {
            let error = AppleFmFoundationModelsError::from(AppleFmErrorDetail {
                message: format!("mapped {code}"),
                r#type: code.to_string(),
                code: Some(code.to_string()),
                tool_name: None,
                underlying_error: None,
                failure_reason: Some("failure".to_string()),
                recovery_suggestion: Some("recover".to_string()),
                debug_description: Some("debug".to_string()),
                refusal_explanation: None,
            });
            assert_eq!(error.kind, expected_kind);
            assert_eq!(error.is_retryable(), expected_retryable);
            assert_eq!(error.failure_reason.as_deref(), Some("failure"));
            assert_eq!(error.recovery_suggestion.as_deref(), Some("recover"));
            assert_eq!(error.debug_description.as_deref(), Some("debug"));
        }
    }

    #[test]
    fn foundation_models_error_reconstructs_tool_failures() {
        let error = AppleFmFoundationModelsError::from(AppleFmErrorDetail {
            message: "Tool failed".to_string(),
            r#type: "tool_call_failed".to_string(),
            code: Some("tool_call_failed".to_string()),
            tool_name: Some("lookup_secret_code".to_string()),
            underlying_error: Some("boom".to_string()),
            failure_reason: None,
            recovery_suggestion: None,
            debug_description: None,
            refusal_explanation: None,
        });

        assert_eq!(error.kind, AppleFmErrorCode::ToolCallFailed);
        let tool_error = error.tool_call_error().expect("tool error");
        assert_eq!(tool_error.tool_name, "lookup_secret_code");
        assert_eq!(tool_error.underlying_error, "boom");
    }

    #[test]
    fn foundation_models_error_preserves_refusal_explanation() {
        let error = AppleFmFoundationModelsError::from(AppleFmErrorDetail {
            message: "Model refused".to_string(),
            r#type: "refusal".to_string(),
            code: Some("refusal".to_string()),
            tool_name: None,
            underlying_error: None,
            failure_reason: Some("Safety policy".to_string()),
            recovery_suggestion: Some("Rephrase".to_string()),
            debug_description: Some("debug refusal".to_string()),
            refusal_explanation: Some("I can't help with that.".to_string()),
        });

        assert_eq!(error.kind, AppleFmErrorCode::Refusal);
        assert_eq!(
            error.refusal_explanation.as_deref(),
            Some("I can't help with that.")
        );
    }
}
