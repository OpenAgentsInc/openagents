use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const DEFAULT_CONTROL_BASE_URL: &str = "https://openagents.com";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthInputError {
    #[error("base url must not be empty")]
    EmptyBaseUrl,
    #[error("email must not be empty")]
    EmptyEmail,
    #[error("verification code must not be empty")]
    EmptyVerificationCode,
    #[error("verification code must contain letters or digits")]
    InvalidVerificationCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthSessionState {
    pub base_url: String,
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthChallengeRequest {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthVerifyRequest {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_id: Option<String>,
}

pub trait AuthStateStore {
    type Error;

    fn load_auth_state(&self) -> Result<Option<AuthSessionState>, Self::Error>;
    fn persist_auth_state(&self, state: &AuthSessionState) -> Result<(), Self::Error>;
    fn clear_auth_state(&self) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait AuthApiTransport {
    type Error;

    async fn send_auth_challenge(&self, request: AuthChallengeRequest) -> Result<(), Self::Error>;
    async fn verify_auth_code(
        &self,
        request: AuthVerifyRequest,
    ) -> Result<AuthSessionState, Self::Error>;
    async fn refresh_session(&self, refresh_token: &str) -> Result<AuthSessionState, Self::Error>;
    async fn logout(&self, access_token: &str) -> Result<(), Self::Error>;
}

pub fn normalize_base_url(raw: &str) -> Result<String, AuthInputError> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(AuthInputError::EmptyBaseUrl);
    }
    Ok(trimmed.to_string())
}

pub fn normalize_email(raw: &str) -> Result<String, AuthInputError> {
    let normalized = raw.trim().to_lowercase();
    if normalized.is_empty() {
        return Err(AuthInputError::EmptyEmail);
    }
    Ok(normalized)
}

pub fn normalize_verification_code(raw: &str) -> Result<String, AuthInputError> {
    let collapsed = raw.split_whitespace().collect::<String>();
    if collapsed.is_empty() {
        return Err(AuthInputError::EmptyVerificationCode);
    }

    let digits_only = collapsed
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits_only.len() == 6 {
        return Ok(digits_only);
    }

    let alnum = collapsed
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if alnum.is_empty() {
        return Err(AuthInputError::InvalidVerificationCode);
    }

    Ok(alnum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_url_trims_and_drops_trailing_slash() {
        let normalized = normalize_base_url(" https://openagents.com/ ").expect("valid base url");
        assert_eq!(normalized, "https://openagents.com");
    }

    #[test]
    fn normalize_email_lowercases_and_trims() {
        let normalized = normalize_email("  ChrIS@OpenAgents.com ").expect("valid email");
        assert_eq!(normalized, "chris@openagents.com");
    }

    #[test]
    fn normalize_verification_code_prefers_six_digit_extraction() {
        let normalized = normalize_verification_code("Code: 123 456.").expect("valid code");
        assert_eq!(normalized, "123456");
    }

    #[test]
    fn normalize_verification_code_supports_alnum_codes() {
        let normalized = normalize_verification_code(" abCD-12 ").expect("valid code");
        assert_eq!(normalized, "abCD12");
    }

    #[test]
    fn normalize_verification_code_rejects_empty_input() {
        let error = normalize_verification_code("   ").expect_err("expected error");
        assert_eq!(error, AuthInputError::EmptyVerificationCode);
    }
}
