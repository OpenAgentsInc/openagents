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
        Config {
            bind_addr: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            log_filter: "debug".to_string(),
            static_dir: std::path::PathBuf::from("."),
            auth_provider_mode: "mock".to_string(),
            workos_client_id: None,
            workos_api_key: None,
            workos_api_base_url: "https://api.workos.com".to_string(),
            mock_magic_code: "123456".to_string(),
            auth_local_test_login_enabled: false,
            auth_local_test_login_allowed_emails: vec![],
            auth_local_test_login_signing_key: None,
            auth_api_signup_enabled: false,
            auth_api_signup_allowed_domains: vec![],
            auth_api_signup_default_token_name: "api-bootstrap".to_string(),
            admin_emails: vec![],
            khala_token_enabled: true,
            khala_token_signing_key: Some("khala-test-signing-key".to_string()),
            khala_token_issuer: "https://openagents.test".to_string(),
            khala_token_audience: "openagents-khala-test".to_string(),
            khala_token_subject_prefix: "user".to_string(),
            khala_token_key_id: "khala-auth-test-v1".to_string(),
            khala_token_claims_version: "oa_khala_claims_v1".to_string(),
            khala_token_ttl_seconds: 300,
            khala_token_min_ttl_seconds: 60,
            khala_token_max_ttl_seconds: 900,
            auth_store_path: None,
            auth_challenge_ttl_seconds: 600,
            auth_access_ttl_seconds: 3600,
            auth_refresh_ttl_seconds: 2592000,
            sync_token_enabled: true,
            sync_token_signing_key: Some("sync-test-signing-key".to_string()),
            sync_token_issuer: "https://openagents.test".to_string(),
            sync_token_audience: "openagents-sync-test".to_string(),
            sync_token_key_id: "sync-auth-test-v1".to_string(),
            sync_token_claims_version: "oa_sync_claims_v1".to_string(),
            sync_token_ttl_seconds: 300,
            sync_token_min_ttl_seconds: 60,
            sync_token_max_ttl_seconds: 900,
            sync_token_allowed_scopes: vec![
                "runtime.codex_worker_events".to_string(),
                "runtime.codex_worker_summaries".to_string(),
                "runtime.run_summaries".to_string(),
            ],
            sync_token_default_scopes: vec!["runtime.codex_worker_events".to_string()],
            route_split_enabled: true,
            route_split_mode: "rust".to_string(),
            route_split_rust_routes: vec!["/".to_string()],
            route_split_cohort_percentage: 100,
            route_split_salt: "route-split-test-salt".to_string(),
            route_split_force_legacy: false,
            route_split_legacy_base_url: None,
            runtime_sync_revoke_base_url: None,
            runtime_sync_revoke_path: "/internal/v1/sync/sessions/revoke".to_string(),
            runtime_signature_secret: None,
            runtime_signature_ttl_seconds: 60,
            runtime_internal_shared_secret: None,
            runtime_internal_key_id: "runtime-internal-v1".to_string(),
            runtime_internal_signature_ttl_seconds: 60,
            runtime_internal_secret_fetch_path: "/api/internal/runtime/integrations/secrets/fetch"
                .to_string(),
            runtime_internal_secret_cache_ttl_ms: 60_000,
            runtime_elixir_base_url: None,
            runtime_signing_key: None,
            runtime_signing_key_id: "runtime-v1".to_string(),
            runtime_comms_delivery_ingest_path: "/internal/v1/comms/delivery-events".to_string(),
            runtime_comms_delivery_timeout_ms: 10_000,
            runtime_comms_delivery_max_retries: 2,
            runtime_comms_delivery_retry_backoff_ms: 200,
            smoke_stream_secret: None,
            resend_webhook_secret: None,
            resend_webhook_tolerance_seconds: 300,
            google_oauth_client_id: None,
            google_oauth_client_secret: None,
            google_oauth_redirect_uri: None,
            google_oauth_scopes: "https://www.googleapis.com/auth/gmail.readonly".to_string(),
            google_oauth_token_url: "https://oauth2.googleapis.com/token".to_string(),
            runtime_driver: "legacy".to_string(),
            runtime_force_driver: None,
            runtime_force_legacy: false,
            runtime_canary_user_percent: 0,
            runtime_canary_autopilot_percent: 0,
            runtime_canary_seed: "runtime-canary-v1".to_string(),
            runtime_overrides_enabled: true,
            runtime_shadow_enabled: false,
            runtime_shadow_sample_rate: 1.0,
            runtime_shadow_max_capture_bytes: 200_000,
            codex_thread_store_path: None,
            domain_store_path: None,
            maintenance_mode_enabled: false,
            maintenance_bypass_token: None,
            maintenance_bypass_cookie_name: "oa_maintenance_bypass".to_string(),
            maintenance_bypass_cookie_ttl_seconds: 900,
            maintenance_allowed_paths: vec!["/healthz".to_string(), "/readyz".to_string()],
            compat_control_enforced: false,
            compat_control_protocol_version: "openagents.control.v1".to_string(),
            compat_control_min_client_build_id: "00000000T000000Z".to_string(),
            compat_control_max_client_build_id: None,
            compat_control_min_schema_version: 1,
            compat_control_max_schema_version: 1,
        }
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
