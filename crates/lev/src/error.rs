//! The typed refusals a Lev door returns.
//!
//! Every refusal names a condition a caller can act on. None of them is a
//! generic failure, and none of them is a number invented to fill a field.

use serde::{Deserialize, Serialize};

/// Why a request was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// The request fails contract validation, before any call.
    InvalidRequest,
    /// A Choice names more options than the contract admits.
    TooManyOptions,
    /// Apple Intelligence is off, the device is ineligible, or the model is
    /// still preparing.
    ModelUnavailable,
    /// No fitted calibration map covers this question family, and the caller
    /// asked for probabilities.
    Uncalibrated,
    /// The state and the question exceed the runtime's context window.
    BranchTooLong,
    /// Apple's guardrails blocked generation, or the model refused.
    Guardrail,
    /// The runtime rejected the compiled schema.
    UnsupportedGuide,
    /// The runtime could not decode its own constrained output.
    DecodingFailure,
    /// The attached adapter does not match the running base.
    AdapterIncompatible,
    /// The runtime rate limited or hit a concurrency limit.
    Busy,
    /// The bridge failed in a way that is ours, not the runtime's.
    BridgeError,
}

impl RefusalCode {
    /// The HTTP status a door answers with.
    #[must_use]
    pub const fn status(self) -> u16 {
        match self {
            Self::InvalidRequest | Self::TooManyOptions => 422,
            Self::Uncalibrated | Self::AdapterIncompatible => 409,
            Self::BranchTooLong => 413,
            Self::Guardrail => 451,
            Self::Busy => 429,
            Self::ModelUnavailable => 503,
            Self::UnsupportedGuide | Self::DecodingFailure | Self::BridgeError => 500,
        }
    }

    /// The stable wire label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::TooManyOptions => "too_many_options",
            Self::ModelUnavailable => "model_unavailable",
            Self::Uncalibrated => "uncalibrated",
            Self::BranchTooLong => "branch_too_long",
            Self::Guardrail => "guardrail",
            Self::UnsupportedGuide => "unsupported_guide",
            Self::DecodingFailure => "decoding_failure",
            Self::AdapterIncompatible => "adapter_incompatible",
            Self::Busy => "busy",
            Self::BridgeError => "bridge_error",
        }
    }
}

/// A refusal, with the question it came from when one question caused it.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct Refusal {
    /// What kind of refusal this is.
    pub code: RefusalCode,
    /// What a caller should read.
    pub message: String,
    /// The question id, when one question caused the refusal.
    pub question: Option<String>,
}

impl Refusal {
    /// Builds a refusal with no question attached.
    #[must_use]
    pub fn new(code: RefusalCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), question: None }
    }

    /// Builds a refusal that names the question that caused it.
    #[must_use]
    pub fn question(
        code: RefusalCode,
        question: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            question: Some(question.into()),
        }
    }

    /// Maps a bridge error code onto a refusal.
    #[must_use]
    pub fn from_bridge(code: &str, message: impl Into<String>) -> Self {
        let mapped = match code {
            "exceeded_context_window_size" => RefusalCode::BranchTooLong,
            "guardrail_violation" | "refusal" => RefusalCode::Guardrail,
            "unsupported_guide" | "invalid_generation_schema" => RefusalCode::UnsupportedGuide,
            "decoding_failure" => RefusalCode::DecodingFailure,
            "rate_limited" | "concurrent_requests" => RefusalCode::Busy,
            "assets_unavailable" => RefusalCode::ModelUnavailable,
            "adapter_not_found" | "adapter_incompatible" => RefusalCode::AdapterIncompatible,
            "invalid_request" | "unsupported_language_or_locale" => RefusalCode::InvalidRequest,
            _ => RefusalCode::BridgeError,
        };
        Self::new(mapped, message)
    }
}

/// The result of anything that can refuse.
pub type Result<T> = std::result::Result<T, Refusal>;
