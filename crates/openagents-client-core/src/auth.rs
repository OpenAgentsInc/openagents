use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const DEFAULT_CONTROL_BASE_URL: &str = "http://127.0.0.1:8787";
pub const ENV_CONTROL_BASE_URL: &str = "OPENAGENTS_CONTROL_BASE_URL";
pub const ENV_CONTROL_BASE_URL_LEGACY: &str = "OPENAGENTS_AUTH_BASE_URL";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthInputError {
    #[error("base url must not be empty")]
    EmptyBaseUrl,
    #[error("base url must use http:// or https:// and include a host")]
    InvalidBaseUrl,
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

pub fn resolve_control_base_url() -> Result<(String, &'static str), AuthInputError> {
    if let Some(base_url) = env_non_empty(ENV_CONTROL_BASE_URL) {
        return normalize_base_url(&base_url).map(|normalized| (normalized, ENV_CONTROL_BASE_URL));
    }
    if let Some(base_url) = env_non_empty(ENV_CONTROL_BASE_URL_LEGACY) {
        return normalize_base_url(&base_url)
            .map(|normalized| (normalized, ENV_CONTROL_BASE_URL_LEGACY));
    }
    normalize_base_url(DEFAULT_CONTROL_BASE_URL).map(|normalized| (normalized, "default_local"))
}

pub fn normalize_base_url(raw: &str) -> Result<String, AuthInputError> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(AuthInputError::EmptyBaseUrl);
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(AuthInputError::InvalidBaseUrl);
    }
    let Some((_, remainder)) = trimmed.split_once("://") else {
        return Err(AuthInputError::InvalidBaseUrl);
    };
    if remainder.trim().is_empty() || remainder.starts_with('/') {
        return Err(AuthInputError::InvalidBaseUrl);
    }
    Ok(trimmed.to_string())
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
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
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_env<T>(primary: Option<&str>, legacy: Option<&str>, test: impl FnOnce() -> T) -> T {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let previous_primary = std::env::var(ENV_CONTROL_BASE_URL).ok();
        let previous_legacy = std::env::var(ENV_CONTROL_BASE_URL_LEGACY).ok();

        if let Some(value) = primary {
            unsafe { std::env::set_var(ENV_CONTROL_BASE_URL, value) };
        } else {
            unsafe { std::env::remove_var(ENV_CONTROL_BASE_URL) };
        }

        if let Some(value) = legacy {
            unsafe { std::env::set_var(ENV_CONTROL_BASE_URL_LEGACY, value) };
        } else {
            unsafe { std::env::remove_var(ENV_CONTROL_BASE_URL_LEGACY) };
        }

        let result = test();

        if let Some(value) = previous_primary {
            unsafe { std::env::set_var(ENV_CONTROL_BASE_URL, value) };
        } else {
            unsafe { std::env::remove_var(ENV_CONTROL_BASE_URL) };
        }
        if let Some(value) = previous_legacy {
            unsafe { std::env::set_var(ENV_CONTROL_BASE_URL_LEGACY, value) };
        } else {
            unsafe { std::env::remove_var(ENV_CONTROL_BASE_URL_LEGACY) };
        }

        result
    }

    #[test]
    fn normalize_base_url_trims_and_drops_trailing_slash() {
        let normalized = normalize_base_url(" https://openagents.com/ ").expect("valid base url");
        assert_eq!(normalized, "https://openagents.com");
    }

    #[test]
    fn normalize_base_url_requires_http_scheme() {
        let error = normalize_base_url("openagents.com").expect_err("expected invalid url");
        assert_eq!(error, AuthInputError::InvalidBaseUrl);
    }

    #[test]
    fn resolve_control_base_url_defaults_local() {
        with_env(None, None, || {
            let (resolved, source) = resolve_control_base_url().expect("default local url");
            assert_eq!(resolved, DEFAULT_CONTROL_BASE_URL);
            assert_eq!(source, "default_local");
        });
    }

    #[test]
    fn resolve_control_base_url_prefers_primary_env() {
        with_env(
            Some("https://staging.openagents.com/"),
            Some("https://legacy.example.com"),
            || {
                let (resolved, source) = resolve_control_base_url().expect("env url");
                assert_eq!(resolved, "https://staging.openagents.com");
                assert_eq!(source, ENV_CONTROL_BASE_URL);
            },
        );
    }

    #[test]
    fn resolve_control_base_url_uses_legacy_env_when_primary_missing() {
        with_env(None, Some("https://legacy.example.com/"), || {
            let (resolved, source) = resolve_control_base_url().expect("legacy env url");
            assert_eq!(resolved, "https://legacy.example.com");
            assert_eq!(source, ENV_CONTROL_BASE_URL_LEGACY);
        });
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
