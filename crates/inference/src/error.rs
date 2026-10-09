//! Errors: the HTTP error body, the streaming `error` event's payload, and
//! the error object inside a failed response.
//!
//! One shape serves both APIs: `{"error": {"type", "code", "param",
//! "message"}}`. Chat Completions clients read the same fields.

use serde::{Deserialize, Serialize};

use crate::wire::{Extra, open_enum};

open_enum! {
    /// The error's category. The first five are the spec's; the rest are
    /// the gateway's, in the same shape.
    pub enum ErrorType {
        InvalidRequest = "invalid_request",
        NotFound = "not_found",
        TooManyRequests = "too_many_requests",
        ServerError = "server_error",
        ModelError = "model_error",
        Unauthorized = "unauthorized",
        InsufficientBalance = "insufficient_balance",
        LimitReached = "limit_reached",
        UpstreamFailed = "upstream_failed",
        NoRoute = "no_route",
    }
}

impl ErrorType {
    /// The HTTP status this category is served with.
    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            Self::InvalidRequest => 400,
            Self::Unauthorized => 401,
            Self::InsufficientBalance => 402,
            Self::LimitReached => 403,
            Self::NotFound => 404,
            Self::TooManyRequests => 429,
            Self::ServerError | Self::ModelError | Self::Other(_) => 500,
            Self::UpstreamFailed => 502,
            Self::NoRoute => 503,
        }
    }
}

/// An error: what the HTTP body's `error` holds, and the payload of a
/// streaming `error` event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    #[serde(rename = "type")]
    pub kind: ErrorType,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    pub message: String,
    /// Response headers sent with the error (streaming payload only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl ApiError {
    /// An error of this category.
    #[must_use]
    pub fn new(kind: ErrorType, message: impl Into<String>) -> Self {
        Self {
            kind,
            code: None,
            param: None,
            message: message.into(),
            headers: None,
            extra: Extra::new(),
        }
    }

    /// A `400 invalid_request` naming the offending parameter.
    #[must_use]
    pub fn invalid_request(param: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            param: Some(param.into()),
            ..Self::new(ErrorType::InvalidRequest, message)
        }
    }

    /// Sets `code`.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// The HTTP status to serve this error with.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.kind.status()
    }

    /// The HTTP body: `{"error": {...}}`.
    #[must_use]
    pub fn body(&self) -> ErrorBody {
        ErrorBody {
            error: self.clone(),
        }
    }

    /// The error object a failed response carries (`code`, `message`). The
    /// code falls back to the category when none is set.
    #[must_use]
    pub fn response_error(&self) -> ResponseError {
        ResponseError {
            code: self
                .code
                .clone()
                .unwrap_or_else(|| self.kind.as_str().to_owned()),
            message: self.message.clone(),
            extra: Extra::new(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ApiError {}

/// The HTTP error body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: ApiError,
}

/// The `error` object inside a response (`status: "failed"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
    #[serde(flatten)]
    pub extra: Extra,
}
