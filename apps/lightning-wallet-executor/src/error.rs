use serde::Serialize;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("invalid config for {field}: {message}")]
pub struct WalletExecutorConfigError {
    pub field: String,
    pub message: String,
}

impl WalletExecutorConfigError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Error)]
#[error("secret load failed ({provider}:{secret_ref}): {message}")]
pub struct SecretLoadError {
    pub provider: String,
    pub secret_ref: String,
    pub message: String,
}

impl SecretLoadError {
    pub fn new(
        provider: impl Into<String>,
        secret_ref: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            secret_ref: secret_ref.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDenialCode {
    HostNotAllowed,
    RequestCapExceeded,
    QuotedAmountExceedsCap,
    WindowCapExceeded,
}

impl PolicyDenialCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HostNotAllowed => "host_not_allowed",
            Self::RequestCapExceeded => "request_cap_exceeded",
            Self::QuotedAmountExceedsCap => "quoted_amount_exceeds_cap",
            Self::WindowCapExceeded => "window_cap_exceeded",
        }
    }
}

impl fmt::Display for PolicyDenialCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Error)]
#[error("policy denied ({code}): {message}")]
pub struct PolicyDeniedError {
    pub code: PolicyDenialCode,
    pub message: String,
    pub host: Option<String>,
    pub max_allowed_msats: Option<u64>,
    pub quoted_amount_msats: Option<u64>,
    pub window_spend_msats: Option<u64>,
    pub window_cap_msats: Option<u64>,
}

impl PolicyDeniedError {
    pub fn new(code: PolicyDenialCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            host: None,
            max_allowed_msats: None,
            quoted_amount_msats: None,
            window_spend_msats: None,
            window_cap_msats: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SparkGatewayErrorCode {
    ApiKeyMissing,
    MnemonicMissing,
    MnemonicInvalid,
    ConnectFailed,
    PrepareFailed,
    SendFailed,
    PaymentPending,
    PaymentFailed,
    PaymentMissingPreimage,
    UnsupportedPaymentMethod,
}

impl SparkGatewayErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKeyMissing => "api_key_missing",
            Self::MnemonicMissing => "mnemonic_missing",
            Self::MnemonicInvalid => "mnemonic_invalid",
            Self::ConnectFailed => "connect_failed",
            Self::PrepareFailed => "prepare_failed",
            Self::SendFailed => "send_failed",
            Self::PaymentPending => "payment_pending",
            Self::PaymentFailed => "payment_failed",
            Self::PaymentMissingPreimage => "payment_missing_preimage",
            Self::UnsupportedPaymentMethod => "unsupported_payment_method",
        }
    }
}

impl fmt::Display for SparkGatewayErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Error)]
#[error("spark gateway error ({code}): {message}")]
pub struct SparkGatewayError {
    pub code: SparkGatewayErrorCode,
    pub message: String,
}

impl SparkGatewayError {
    pub fn new(code: SparkGatewayErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Error)]
#[error("invalid request: {message}")]
pub struct HttpRequestDecodeError {
    pub message: String,
}

impl HttpRequestDecodeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
