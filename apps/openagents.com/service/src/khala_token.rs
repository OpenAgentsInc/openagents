use std::collections::HashSet;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use uuid::Uuid;

use crate::config::Config;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct KhalaTokenIssuer {
    enabled: bool,
    signing_key: Option<String>,
    issuer: String,
    audience: String,
    subject_prefix: String,
    key_id: String,
    claims_version: String,
    ttl_seconds: u32,
    min_ttl_seconds: u32,
    max_ttl_seconds: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum KhalaTokenError {
    #[error("{message}")]
    InvalidRequest { message: String },
    #[error("{message}")]
    Unavailable { message: String },
}

#[derive(Debug, Clone)]
pub struct KhalaTokenIssueRequest {
    pub user_id: String,
    pub scope: Vec<String>,
    pub workspace_id: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KhalaTokenResult {
    pub token: String,
    pub token_type: &'static str,
    pub expires_in: u32,
    pub issued_at: String,
    pub expires_at: String,
    pub issuer: String,
    pub audience: String,
    pub subject: String,
    pub claims_version: String,
    pub scope: Vec<String>,
    pub workspace_id: Option<String>,
    pub role: Option<String>,
    pub kid: String,
}

impl KhalaTokenIssuer {
    pub fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.khala_token_enabled,
            signing_key: config.khala_token_signing_key.clone(),
            issuer: config.khala_token_issuer.clone(),
            audience: config.khala_token_audience.clone(),
            subject_prefix: config.khala_token_subject_prefix.clone(),
            key_id: config.khala_token_key_id.clone(),
            claims_version: config.khala_token_claims_version.clone(),
            ttl_seconds: config.khala_token_ttl_seconds,
            min_ttl_seconds: config.khala_token_min_ttl_seconds,
            max_ttl_seconds: config.khala_token_max_ttl_seconds,
        }
    }

    pub fn issue(
        &self,
        request: KhalaTokenIssueRequest,
    ) -> Result<KhalaTokenResult, KhalaTokenError> {
        if !self.enabled {
            return Err(KhalaTokenError::Unavailable {
                message: "khala token minting is disabled".to_string(),
            });
        }

        let signing_key = self
            .signing_key
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| KhalaTokenError::Unavailable {
                message: "khala token signing key is not configured".to_string(),
            })?;

        if self.issuer.trim().is_empty() || self.audience.trim().is_empty() {
            return Err(KhalaTokenError::Unavailable {
                message: "khala token issuer and audience must be configured".to_string(),
            });
        }

        if self.claims_version.trim().is_empty() {
            return Err(KhalaTokenError::Unavailable {
                message: "khala token claims_version must be configured".to_string(),
            });
        }

        if self.min_ttl_seconds == 0
            || self.max_ttl_seconds == 0
            || self.max_ttl_seconds < self.min_ttl_seconds
        {
            return Err(KhalaTokenError::Unavailable {
                message: "khala token ttl bounds are invalid".to_string(),
            });
        }

        if self.ttl_seconds < self.min_ttl_seconds || self.ttl_seconds > self.max_ttl_seconds {
            return Err(KhalaTokenError::Unavailable {
                message: "khala token ttl_seconds is outside configured bounds".to_string(),
            });
        }

        let normalized_scope = normalize_scope(request.scope);
        let workspace_id = request
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let role = request
            .role
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);

        let issued_at = Utc::now();
        let expires_at = issued_at + Duration::seconds(self.ttl_seconds as i64);
        let subject = format!("{}:{}", self.subject_prefix, request.user_id);

        let mut claims = serde_json::json!({
            "iss": self.issuer,
            "aud": self.audience,
            "sub": subject,
            "iat": issued_at.timestamp(),
            "nbf": issued_at.timestamp(),
            "exp": expires_at.timestamp(),
            "jti": format!("khala_{}", Uuid::new_v4().simple()),
            "oa_user_id": request.user_id,
            "oa_claims_version": self.claims_version,
        });

        if !normalized_scope.is_empty() {
            claims["scope"] = serde_json::to_value(normalized_scope.clone()).map_err(|error| {
                KhalaTokenError::Unavailable {
                    message: format!("failed to encode khala scope claim: {error}"),
                }
            })?;
        }

        if let Some(workspace_id) = workspace_id.as_ref() {
            claims["oa_workspace_id"] = serde_json::Value::String(workspace_id.clone());
        }

        if let Some(role) = role.as_ref() {
            claims["oa_role"] = serde_json::Value::String(role.clone());
        }

        let mut header = serde_json::json!({
            "alg": "HS256",
            "typ": "JWT",
        });
        if !self.key_id.trim().is_empty() {
            header["kid"] = serde_json::Value::String(self.key_id.clone());
        }

        let token = encode_hs256_jwt(&header, &claims, &signing_key)?;

        Ok(KhalaTokenResult {
            token,
            token_type: "Bearer",
            expires_in: self.ttl_seconds,
            issued_at: timestamp(issued_at),
            expires_at: timestamp(expires_at),
            issuer: self.issuer.clone(),
            audience: self.audience.clone(),
            subject,
            claims_version: self.claims_version.clone(),
            scope: normalized_scope,
            workspace_id,
            role,
            kid: self.key_id.clone(),
        })
    }
}

fn normalize_scope(scope: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for entry in scope {
        let value = entry.trim().to_string();
        if value.is_empty() {
            continue;
        }

        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }

    normalized
}

fn encode_hs256_jwt(
    header: &serde_json::Value,
    claims: &serde_json::Value,
    signing_key: &str,
) -> Result<String, KhalaTokenError> {
    let header_bytes =
        serde_json::to_vec(header).map_err(|error| KhalaTokenError::Unavailable {
            message: format!("failed to encode khala jwt header: {error}"),
        })?;
    let claims_bytes =
        serde_json::to_vec(claims).map_err(|error| KhalaTokenError::Unavailable {
            message: format!("failed to encode khala jwt claims: {error}"),
        })?;

    let header_segment = URL_SAFE_NO_PAD.encode(header_bytes);
    let claims_segment = URL_SAFE_NO_PAD.encode(claims_bytes);
    let signing_input = format!("{header_segment}.{claims_segment}");

    let mut mac = HmacSha256::new_from_slice(signing_key.as_bytes()).map_err(|error| {
        KhalaTokenError::Unavailable {
            message: format!("failed to initialize khala jwt signer: {error}"),
        }
    })?;
    mac.update(signing_input.as_bytes());
    let signature_segment = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

    Ok(format!("{signing_input}.{signature_segment}"))
}

fn timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        let mut config = Config::for_tests(std::path::PathBuf::from("."));
        config.auth_refresh_ttl_seconds = 2_592_000;
        config.route_split_mode = "rust".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        config.route_split_cohort_percentage = 100;
        config.route_split_legacy_base_url = None;
        config.smoke_stream_secret = None;
        config
    }

    #[test]
    fn khala_token_issue_returns_expected_payload_shape() {
        let issuer = KhalaTokenIssuer::from_config(&test_config());
        let issued = issuer
            .issue(KhalaTokenIssueRequest {
                user_id: "user_123".to_string(),
                scope: vec!["codex:read".to_string(), "codex:write".to_string()],
                workspace_id: Some("workspace_42".to_string()),
                role: Some("admin".to_string()),
            })
            .expect("khala token should issue");

        assert_eq!(issued.token_type, "Bearer");
        assert_eq!(issued.issuer, "https://openagents.test");
        assert_eq!(issued.audience, "openagents-khala-test");
        assert_eq!(issued.subject, "user:user_123");
        assert_eq!(issued.claims_version, "oa_khala_claims_v1");
        assert_eq!(issued.scope, vec!["codex:read", "codex:write"]);
        assert_eq!(issued.workspace_id.as_deref(), Some("workspace_42"));
        assert_eq!(issued.role.as_deref(), Some("admin"));
        assert!(!issued.token.is_empty());
    }

    #[test]
    fn khala_token_issue_requires_signing_key() {
        let mut config = test_config();
        config.khala_token_signing_key = None;
        let issuer = KhalaTokenIssuer::from_config(&config);
        let result = issuer.issue(KhalaTokenIssueRequest {
            user_id: "user_123".to_string(),
            scope: Vec::new(),
            workspace_id: None,
            role: None,
        });

        assert!(matches!(result, Err(KhalaTokenError::Unavailable { .. })));
    }
}
