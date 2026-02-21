use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use uuid::Uuid;

use crate::config::Config;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct SyncTokenIssuer {
    enabled: bool,
    signing_key: Option<String>,
    issuer: String,
    audience: String,
    key_id: String,
    claims_version: String,
    ttl_seconds: u32,
    min_ttl_seconds: u32,
    max_ttl_seconds: u32,
    allowed_scopes: Vec<String>,
    default_scopes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncTokenError {
    #[error("{message}")]
    InvalidRequest { message: String },
    #[error("{message}")]
    InvalidScope { message: String },
    #[error("{message}")]
    Unavailable { message: String },
}

#[derive(Debug, Clone)]
pub struct SyncTokenIssueRequest {
    pub user_id: String,
    pub org_id: String,
    pub session_id: String,
    pub device_id: String,
    pub requested_scopes: Vec<String>,
    pub requested_topics: Vec<String>,
    pub requested_ttl_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncTopicGrant {
    pub topic: String,
    pub required_scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncTokenResult {
    pub token: String,
    pub token_type: &'static str,
    pub expires_in: u32,
    pub issued_at: String,
    pub expires_at: String,
    pub issuer: String,
    pub audience: String,
    pub subject: String,
    pub org_id: String,
    pub claims_version: String,
    pub scopes: Vec<String>,
    pub granted_topics: Vec<SyncTopicGrant>,
    pub kid: String,
    pub token_id: String,
    pub session_id: String,
    pub device_id: String,
}

impl SyncTokenIssuer {
    pub fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.sync_token_enabled,
            signing_key: config.sync_token_signing_key.clone(),
            issuer: config.sync_token_issuer.clone(),
            audience: config.sync_token_audience.clone(),
            key_id: config.sync_token_key_id.clone(),
            claims_version: config.sync_token_claims_version.clone(),
            ttl_seconds: config.sync_token_ttl_seconds,
            min_ttl_seconds: config.sync_token_min_ttl_seconds,
            max_ttl_seconds: config.sync_token_max_ttl_seconds,
            allowed_scopes: config.sync_token_allowed_scopes.clone(),
            default_scopes: config.sync_token_default_scopes.clone(),
        }
    }

    pub fn issue(&self, request: SyncTokenIssueRequest) -> Result<SyncTokenResult, SyncTokenError> {
        if !self.enabled {
            return Err(SyncTokenError::Unavailable {
                message: "sync token minting is disabled".to_string(),
            });
        }

        let signing_key = self
            .signing_key
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| SyncTokenError::Unavailable {
                message: "sync token signing key is not configured".to_string(),
            })?;

        if self.issuer.trim().is_empty() || self.audience.trim().is_empty() {
            return Err(SyncTokenError::Unavailable {
                message: "sync token issuer and audience must be configured".to_string(),
            });
        }

        if self.key_id.trim().is_empty() {
            return Err(SyncTokenError::Unavailable {
                message: "sync token key_id must be configured".to_string(),
            });
        }

        if self.claims_version.trim().is_empty() {
            return Err(SyncTokenError::Unavailable {
                message: "sync token claims_version must be configured".to_string(),
            });
        }

        if self.min_ttl_seconds == 0
            || self.max_ttl_seconds == 0
            || self.max_ttl_seconds < self.min_ttl_seconds
        {
            return Err(SyncTokenError::Unavailable {
                message: "sync token ttl bounds are invalid".to_string(),
            });
        }

        let ttl_seconds = request.requested_ttl_seconds.unwrap_or(self.ttl_seconds);
        if ttl_seconds < self.min_ttl_seconds || ttl_seconds > self.max_ttl_seconds {
            return Err(SyncTokenError::InvalidRequest {
                message: "requested ttl is outside configured bounds".to_string(),
            });
        }

        let scopes = self.resolve_scopes(request.requested_scopes)?;
        if scopes.is_empty() {
            return Err(SyncTokenError::Unavailable {
                message: "sync token scopes are not configured".to_string(),
            });
        }

        let granted_topics = derive_topics(
            &scopes,
            &request.requested_topics,
            &request.user_id,
            &request.org_id,
        )?;

        let issued_at = Utc::now();
        let expires_at = issued_at + Duration::seconds(ttl_seconds as i64);
        let subject = format!("user:{}", request.user_id);
        let token_id = format!("sync_{}", Uuid::new_v4().simple());

        let header = serde_json::json!({
            "alg": "HS256",
            "typ": "JWT",
            "kid": self.key_id,
        });

        let claims = serde_json::json!({
            "iss": self.issuer,
            "aud": self.audience,
            "sub": subject,
            "iat": issued_at.timestamp(),
            "nbf": issued_at.timestamp(),
            "exp": expires_at.timestamp(),
            "jti": token_id,
            "oa_user_id": request.user_id,
            "oa_org_id": request.org_id,
            "oa_session_id": request.session_id,
            "oa_device_id": request.device_id,
            "oa_sync_scopes": scopes,
            "oa_sync_topics": granted_topics.iter().map(|topic| topic.topic.clone()).collect::<Vec<_>>(),
            "oa_claims_version": self.claims_version,
        });

        let token = encode_hs256_jwt(&header, &claims, &signing_key)?;

        Ok(SyncTokenResult {
            token,
            token_type: "Bearer",
            expires_in: ttl_seconds,
            issued_at: timestamp(issued_at),
            expires_at: timestamp(expires_at),
            issuer: self.issuer.clone(),
            audience: self.audience.clone(),
            subject,
            org_id: request.org_id,
            claims_version: self.claims_version.clone(),
            scopes,
            granted_topics,
            kid: self.key_id.clone(),
            token_id,
            session_id: request.session_id,
            device_id: request.device_id,
        })
    }

    fn resolve_scopes(&self, requested: Vec<String>) -> Result<Vec<String>, SyncTokenError> {
        let requested = normalize_unique(requested);

        if !requested.is_empty() {
            let unknown: Vec<String> = requested
                .iter()
                .filter(|scope| !self.allowed_scopes.iter().any(|allowed| allowed == *scope))
                .cloned()
                .collect();

            if !unknown.is_empty() {
                return Err(SyncTokenError::InvalidScope {
                    message: "requested sync scopes are not allowed".to_string(),
                });
            }

            return Ok(requested);
        }

        let defaults = normalize_unique(self.default_scopes.clone());
        if !defaults.is_empty() {
            return Ok(defaults);
        }

        Ok(normalize_unique(self.allowed_scopes.clone()))
    }
}

fn derive_topics(
    scopes: &[String],
    requested_topics: &[String],
    user_id: &str,
    org_id: &str,
) -> Result<Vec<SyncTopicGrant>, SyncTokenError> {
    let topics = if !requested_topics.is_empty() {
        requested_topics
            .iter()
            .map(|topic| topic.trim().to_string())
            .filter(|topic| !topic.is_empty())
            .collect::<Vec<_>>()
    } else {
        scopes
            .iter()
            .flat_map(|scope| default_topics_for_scope(scope, user_id, org_id))
            .collect::<Vec<_>>()
    };

    let mut grants = Vec::new();
    let mut seen = BTreeSet::new();

    for topic in topics {
        if seen.contains(&topic) {
            continue;
        }

        let required_scope =
            scope_for_topic(&topic).ok_or_else(|| SyncTokenError::InvalidRequest {
                message: format!("requested topic '{topic}' is not supported"),
            })?;

        if !scopes.iter().any(|scope| scope == &required_scope) {
            return Err(SyncTokenError::InvalidScope {
                message: format!("requested topic '{topic}' requires scope '{required_scope}'"),
            });
        }

        grants.push(SyncTopicGrant {
            topic: topic.clone(),
            required_scope,
        });
        seen.insert(topic);
    }

    Ok(grants)
}

fn default_topics_for_scope(scope: &str, user_id: &str, org_id: &str) -> Vec<String> {
    match scope {
        "runtime.codex_worker_events" => vec![org_topic(org_id, "worker_events")],
        "runtime.codex_worker_summaries" => vec![org_topic(org_id, "workers")],
        "runtime.run_summaries" => vec![format!("user:{user_id}:runs")],
        _ => Vec::new(),
    }
}

fn org_topic(org_id: &str, suffix: &str) -> String {
    if org_id.starts_with("org:") {
        format!("{org_id}:{suffix}")
    } else {
        format!("org:{org_id}:{suffix}")
    }
}

fn scope_for_topic(topic: &str) -> Option<String> {
    if topic.ends_with(":worker_events") {
        return Some("runtime.codex_worker_events".to_string());
    }

    if topic.ends_with(":workers") {
        return Some("runtime.codex_worker_summaries".to_string());
    }

    if topic.ends_with(":runs") {
        return Some("runtime.run_summaries".to_string());
    }

    None
}

fn normalize_unique(values: Vec<String>) -> Vec<String> {
    let mut set = BTreeSet::new();
    for value in values {
        let normalized = value.trim().to_string();
        if !normalized.is_empty() {
            set.insert(normalized);
        }
    }

    set.into_iter().collect()
}

fn encode_hs256_jwt(
    header: &serde_json::Value,
    claims: &serde_json::Value,
    signing_key: &str,
) -> Result<String, SyncTokenError> {
    let header_json = serde_json::to_vec(header).map_err(|error| SyncTokenError::Unavailable {
        message: format!("failed to encode jwt header: {error}"),
    })?;
    let claims_json = serde_json::to_vec(claims).map_err(|error| SyncTokenError::Unavailable {
        message: format!("failed to encode jwt claims: {error}"),
    })?;

    let header_segment = URL_SAFE_NO_PAD.encode(header_json);
    let claims_segment = URL_SAFE_NO_PAD.encode(claims_json);
    let signing_input = format!("{header_segment}.{claims_segment}");

    let mut mac = HmacSha256::new_from_slice(signing_key.as_bytes()).map_err(|error| {
        SyncTokenError::Unavailable {
            message: format!("invalid signing key: {error}"),
        }
    })?;

    mac.update(signing_input.as_bytes());
    let signature = mac.finalize().into_bytes();
    let signature_segment = URL_SAFE_NO_PAD.encode(signature);

    Ok(format!("{signing_input}.{signature_segment}"))
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn issuer() -> SyncTokenIssuer {
        let bind_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));
        let config = Config {
            bind_addr,
            log_filter: "debug".to_string(),
            static_dir: std::env::temp_dir(),
            auth_provider_mode: "mock".to_string(),
            workos_client_id: None,
            workos_api_key: None,
            workos_api_base_url: "https://api.workos.com".to_string(),
            mock_magic_code: "123456".to_string(),
            auth_challenge_ttl_seconds: 600,
            auth_access_ttl_seconds: 3600,
            auth_refresh_ttl_seconds: 86400,
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
            route_split_enabled: false,
            route_split_mode: "legacy".to_string(),
            route_split_rust_routes: vec!["/chat".to_string()],
            route_split_cohort_percentage: 0,
            route_split_salt: "openagents-route-split-v1".to_string(),
            route_split_force_legacy: false,
            route_split_legacy_base_url: Some("https://legacy.openagents.test".to_string()),
        };

        SyncTokenIssuer::from_config(&config)
    }

    fn issue_request() -> SyncTokenIssueRequest {
        SyncTokenIssueRequest {
            user_id: "1".to_string(),
            org_id: "org:openagents".to_string(),
            session_id: "sess_123".to_string(),
            device_id: "mobile:autopilot-ios".to_string(),
            requested_scopes: vec!["runtime.run_summaries".to_string()],
            requested_topics: vec![],
            requested_ttl_seconds: None,
        }
    }

    #[test]
    fn rejects_topic_without_required_scope() {
        let issuer = issuer();
        let mut request = issue_request();
        request.requested_topics = vec!["org:openagents:worker_events".to_string()];

        let result = issuer.issue(request);
        assert!(matches!(result, Err(SyncTokenError::InvalidScope { .. })));
    }

    #[test]
    fn rejects_unsupported_topic_pattern() {
        let issuer = issuer();
        let mut request = issue_request();
        request.requested_scopes = vec!["runtime.codex_worker_events".to_string()];
        request.requested_topics = vec!["org:openagents:unknown".to_string()];

        let result = issuer.issue(request);
        assert!(matches!(result, Err(SyncTokenError::InvalidRequest { .. })));
    }

    #[test]
    fn rejects_ttl_outside_bounds() {
        let issuer = issuer();
        let mut request = issue_request();
        request.requested_ttl_seconds = Some(30);

        let result = issuer.issue(request);
        assert!(matches!(result, Err(SyncTokenError::InvalidRequest { .. })));
    }
}
