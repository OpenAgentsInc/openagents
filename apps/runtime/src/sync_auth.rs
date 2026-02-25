use std::collections::HashSet;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, errors::ErrorKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct SyncAuthConfig {
    pub signing_key: String,
    pub fallback_signing_keys: Vec<String>,
    pub issuer: String,
    pub audience: String,
    pub require_jti: bool,
    pub max_token_age_seconds: u64,
    pub clock_skew_leeway_seconds: u64,
    pub revoked_jtis: HashSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncTokenClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub exp: usize,
    #[serde(default)]
    pub nbf: usize,
    #[serde(default)]
    pub iat: usize,
    #[serde(default)]
    pub jti: String,
    #[serde(default)]
    pub oa_user_id: Option<u64>,
    #[serde(default)]
    pub oa_org_id: Option<String>,
    #[serde(default)]
    pub oa_device_id: Option<String>,
    #[serde(default)]
    pub oa_client_surface: Option<String>,
    #[serde(default)]
    pub oa_sync_scopes: Vec<String>,
    #[serde(default)]
    pub oa_sync_topics: Vec<String>,
    #[serde(default)]
    pub oa_sync_streams: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPrincipal {
    pub user_id: Option<u64>,
    pub org_id: Option<String>,
    pub device_id: Option<String>,
    pub client_surface: Option<String>,
    pub scopes: HashSet<String>,
    pub allowed_streams: Option<HashSet<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizedSpacetimeTopic {
    WorkerLifecycle { worker_id: String },
    FleetWorkers { user_id: u64 },
    RunEvents { run_id: String },
    CodexWorkerEvents,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SyncAuthError {
    #[error("missing authorization header")]
    MissingAuthorization,
    #[error("invalid authorization scheme")]
    InvalidAuthorizationScheme,
    #[error("invalid sync token")]
    InvalidToken,
    #[error("sync token expired")]
    TokenExpired,
    #[error("sync token is not valid yet")]
    TokenNotYetValid,
    #[error("sync token missing jti")]
    MissingJti,
    #[error("sync token missing iat")]
    MissingIssuedAt,
    #[error("sync token too old")]
    TokenTooOld,
    #[error("sync token revoked")]
    TokenRevoked,
    #[error("forbidden topic {topic}")]
    ForbiddenTopic { topic: String },
    #[error("missing scope for topic {topic}")]
    MissingScope {
        topic: String,
        required_scopes: Vec<String>,
    },
    #[error("client surface {client_surface} is not allowed to access topic {topic}")]
    SurfacePolicyDenied {
        topic: String,
        client_surface: String,
    },
}

impl SyncAuthError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingAuthorization => "missing_authorization",
            Self::InvalidAuthorizationScheme => "invalid_authorization_scheme",
            Self::InvalidToken => "invalid_token",
            Self::TokenExpired => "token_expired",
            Self::TokenNotYetValid => "token_not_yet_valid",
            Self::MissingJti => "missing_jti",
            Self::MissingIssuedAt => "missing_iat",
            Self::TokenTooOld => "token_too_old",
            Self::TokenRevoked => "token_revoked",
            Self::ForbiddenTopic { .. } => "forbidden_topic",
            Self::MissingScope { .. } => "missing_scope",
            Self::SurfacePolicyDenied { .. } => "surface_policy_denied",
        }
    }

    #[must_use]
    pub fn is_unauthorized(&self) -> bool {
        matches!(
            self,
            Self::MissingAuthorization
                | Self::InvalidAuthorizationScheme
                | Self::InvalidToken
                | Self::TokenExpired
                | Self::TokenNotYetValid
                | Self::MissingJti
                | Self::MissingIssuedAt
                | Self::TokenTooOld
                | Self::TokenRevoked
        )
    }
}

#[derive(Clone)]
pub struct SyncAuthorizer {
    decoding_key: DecodingKey,
    fallback_decoding_keys: Vec<DecodingKey>,
    validation: Validation,
    require_jti: bool,
    max_token_age_seconds: u64,
    revoked_jtis: HashSet<String>,
}

impl SyncAuthorizer {
    #[must_use]
    pub fn from_config(config: SyncAuthConfig) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[config.issuer.as_str()]);
        validation.set_audience(&[config.audience.as_str()]);
        validation.validate_nbf = true;
        validation.leeway = config.clock_skew_leeway_seconds;
        Self {
            decoding_key: DecodingKey::from_secret(config.signing_key.as_bytes()),
            fallback_decoding_keys: config
                .fallback_signing_keys
                .into_iter()
                .map(|value| DecodingKey::from_secret(value.as_bytes()))
                .collect(),
            validation,
            require_jti: config.require_jti,
            max_token_age_seconds: config.max_token_age_seconds.max(1),
            revoked_jtis: config.revoked_jtis,
        }
    }

    pub fn extract_bearer_token(header_value: Option<&str>) -> Result<&str, SyncAuthError> {
        let raw = header_value.ok_or(SyncAuthError::MissingAuthorization)?;
        let trimmed = raw.trim();
        let Some(token) = trimmed.strip_prefix("Bearer ") else {
            return Err(SyncAuthError::InvalidAuthorizationScheme);
        };
        let token = token.trim();
        if token.is_empty() {
            return Err(SyncAuthError::InvalidAuthorizationScheme);
        }
        Ok(token)
    }

    pub fn authenticate(&self, token: &str) -> Result<SyncPrincipal, SyncAuthError> {
        let now = chrono::Utc::now().timestamp().max(0) as usize;
        let claims = self.decode_claims(token)?;

        if claims.iat == 0 {
            return Err(SyncAuthError::MissingIssuedAt);
        }
        if now.saturating_sub(claims.iat) > self.max_token_age_seconds as usize {
            return Err(SyncAuthError::TokenTooOld);
        }
        if self.require_jti && claims.jti.trim().is_empty() {
            return Err(SyncAuthError::MissingJti);
        }
        if !claims.jti.is_empty() && self.revoked_jtis.contains(&claims.jti) {
            return Err(SyncAuthError::TokenRevoked);
        }

        Ok(SyncPrincipal {
            user_id: claims.oa_user_id,
            org_id: claims.oa_org_id,
            device_id: claims.oa_device_id,
            client_surface: normalize_client_surface(claims.oa_client_surface.as_deref()),
            scopes: claims.oa_sync_scopes.into_iter().collect(),
            allowed_streams: normalize_stream_grants(claims.oa_sync_topics, claims.oa_sync_streams),
        })
    }

    pub fn authorize_topic(
        &self,
        principal: &SyncPrincipal,
        topic: &str,
    ) -> Result<AuthorizedSpacetimeTopic, SyncAuthError> {
        let normalized_topic = topic.trim();
        let authorized_topic = parse_topic(normalized_topic)?;
        ensure_stream_grant(principal, normalized_topic)?;
        match &authorized_topic {
            AuthorizedSpacetimeTopic::CodexWorkerEvents => {
                ensure_scope(
                    principal,
                    normalized_topic,
                    &[
                        "runtime.codex_worker_events",
                        "runtime.worker_lifecycle_events",
                    ],
                )?;
            }
            AuthorizedSpacetimeTopic::WorkerLifecycle { .. } => {
                ensure_scope(
                    principal,
                    normalized_topic,
                    &[
                        "runtime.codex_worker_events",
                        "runtime.worker_lifecycle_events",
                    ],
                )?;
            }
            AuthorizedSpacetimeTopic::FleetWorkers { user_id } => {
                ensure_scope(
                    principal,
                    normalized_topic,
                    &[
                        "runtime.codex_worker_events",
                        "runtime.worker_lifecycle_events",
                    ],
                )?;
                if principal.user_id != Some(*user_id) {
                    return Err(SyncAuthError::ForbiddenTopic {
                        topic: normalized_topic.to_string(),
                    });
                }
            }
            AuthorizedSpacetimeTopic::RunEvents { .. } => {
                ensure_scope(principal, normalized_topic, &["runtime.run_events"])?;
            }
        }
        Ok(authorized_topic)
    }

    fn decode_claims(&self, token: &str) -> Result<SyncTokenClaims, SyncAuthError> {
        match decode::<SyncTokenClaims>(token, &self.decoding_key, &self.validation) {
            Ok(decoded) => Ok(decoded.claims),
            Err(primary_error) => {
                if is_signature_error(&primary_error) {
                    for key in &self.fallback_decoding_keys {
                        if let Ok(decoded) = decode::<SyncTokenClaims>(token, key, &self.validation)
                        {
                            return Ok(decoded.claims);
                        }
                    }
                }
                Err(map_decode_error(primary_error))
            }
        }
    }
}

fn normalize_stream_grants(topics: Vec<String>, streams: Vec<String>) -> Option<HashSet<String>> {
    let mut grants = HashSet::new();
    for value in topics.into_iter().chain(streams) {
        let normalized = value.trim().to_string();
        if !normalized.is_empty() {
            grants.insert(normalized);
        }
    }
    if grants.is_empty() {
        None
    } else {
        Some(grants)
    }
}

fn ensure_stream_grant(principal: &SyncPrincipal, stream: &str) -> Result<(), SyncAuthError> {
    if let Some(grants) = principal.allowed_streams.as_ref()
        && !grants.contains(stream)
    {
        return Err(SyncAuthError::ForbiddenTopic {
            topic: stream.to_string(),
        });
    }
    Ok(())
}

fn is_signature_error(error: &jsonwebtoken::errors::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::InvalidSignature | ErrorKind::InvalidAlgorithm
    )
}

fn map_decode_error(error: jsonwebtoken::errors::Error) -> SyncAuthError {
    match error.kind() {
        ErrorKind::ExpiredSignature => SyncAuthError::TokenExpired,
        ErrorKind::ImmatureSignature => SyncAuthError::TokenNotYetValid,
        _ => SyncAuthError::InvalidToken,
    }
}

fn normalize_client_surface(client_surface: Option<&str>) -> Option<String> {
    let raw = client_surface?;
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn parse_topic(topic: &str) -> Result<AuthorizedSpacetimeTopic, SyncAuthError> {
    if topic.is_empty() {
        return Err(SyncAuthError::ForbiddenTopic {
            topic: topic.to_string(),
        });
    }

    if topic == "runtime.codex_worker_events" {
        return Ok(AuthorizedSpacetimeTopic::CodexWorkerEvents);
    }

    if let Some(rest) = topic.strip_prefix("worker:") {
        if let Some(worker_id) = rest.strip_suffix(":lifecycle") {
            if !worker_id.trim().is_empty() {
                return Ok(AuthorizedSpacetimeTopic::WorkerLifecycle {
                    worker_id: worker_id.to_string(),
                });
            }
        }
    }

    if let Some(rest) = topic.strip_prefix("fleet:user:") {
        if let Some(user_id) = rest.strip_suffix(":workers") {
            let trimmed = user_id.trim();
            if let Ok(user_id) = trimmed.parse::<u64>() {
                return Ok(AuthorizedSpacetimeTopic::FleetWorkers { user_id });
            }
        }
    }

    if let Some(rest) = topic.strip_prefix("run:") {
        if let Some(run_id) = rest.strip_suffix(":events") {
            if !run_id.trim().is_empty() {
                return Ok(AuthorizedSpacetimeTopic::RunEvents {
                    run_id: run_id.to_string(),
                });
            }
        }
    }

    Err(SyncAuthError::ForbiddenTopic {
        topic: topic.to_string(),
    })
}

fn ensure_scope(
    principal: &SyncPrincipal,
    topic: &str,
    required_scopes: &[&str],
) -> Result<(), SyncAuthError> {
    let has_scope = required_scopes
        .iter()
        .any(|scope| principal.scopes.contains(*scope));
    if has_scope {
        Ok(())
    } else {
        Err(SyncAuthError::MissingScope {
            topic: topic.to_string(),
            required_scopes: required_scopes
                .iter()
                .map(|scope| (*scope).to_string())
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;
    use jsonwebtoken::{EncodingKey, Header, encode};

    use super::{
        AuthorizedSpacetimeTopic, SyncAuthConfig, SyncAuthError, SyncAuthorizer, SyncTokenClaims,
    };

    fn make_token(claims: SyncTokenClaims, key: &str) -> String {
        encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(key.as_bytes()),
        )
        .expect("token should encode")
    }

    #[test]
    fn bearer_extraction_requires_scheme() {
        assert_eq!(
            SyncAuthorizer::extract_bearer_token(None),
            Err(SyncAuthError::MissingAuthorization)
        );
        assert_eq!(
            SyncAuthorizer::extract_bearer_token(Some("token")),
            Err(SyncAuthError::InvalidAuthorizationScheme)
        );
    }

    #[test]
    fn malformed_token_is_rejected() {
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: "sync-test-key".to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::new(),
        });
        let result = authorizer.authenticate("this-is-not-a-jwt");
        assert_eq!(result, Err(SyncAuthError::InvalidToken));
    }

    #[test]
    fn revoked_jti_is_rejected() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::from([String::from("revoked-jti")]),
        });
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: (Utc::now().timestamp() + 60) as usize,
            nbf: Utc::now().timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            jti: "revoked-jti".to_string(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
            oa_sync_topics: vec![],
            oa_sync_streams: vec![],
        };
        let token = make_token(claims, key);
        let result = authorizer.authenticate(&token);
        assert_eq!(result, Err(SyncAuthError::TokenRevoked));
    }

    #[test]
    fn topic_scope_matrix_enforces_expected_scopes() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::new(),
        });
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: (Utc::now().timestamp() + 60) as usize,
            nbf: Utc::now().timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            jti: "ok-jti".to_string(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
            oa_sync_topics: vec![
                "worker:desktopw:shared:lifecycle".to_string(),
                "fleet:user:1:workers".to_string(),
                "fleet:user:2:workers".to_string(),
                "run:019c7f93:events".to_string(),
            ],
            oa_sync_streams: vec![],
        };
        let token = make_token(claims, key);
        let principal = authorizer.authenticate(&token).expect("principal");

        let allowed = authorizer
            .authorize_topic(&principal, "worker:desktopw:shared:lifecycle")
            .expect("worker topic should be allowed");
        assert_eq!(
            allowed,
            AuthorizedSpacetimeTopic::WorkerLifecycle {
                worker_id: "desktopw:shared".to_string(),
            }
        );

        let fleet_allowed = authorizer
            .authorize_topic(&principal, "fleet:user:1:workers")
            .expect("fleet topic should be allowed for bound user");
        assert_eq!(
            fleet_allowed,
            AuthorizedSpacetimeTopic::FleetWorkers { user_id: 1 }
        );

        let fleet_denied = authorizer
            .authorize_topic(&principal, "fleet:user:2:workers")
            .expect_err("fleet topic should be denied for other users");
        match fleet_denied {
            SyncAuthError::ForbiddenTopic { .. } => {}
            other => panic!("expected forbidden topic, got {other:?}"),
        }

        let denied = authorizer
            .authorize_topic(&principal, "run:019c7f93:events")
            .expect_err("run topic should require runtime.run_events scope");
        match denied {
            SyncAuthError::MissingScope { .. } => {}
            other => panic!("expected missing scope, got {other:?}"),
        }
    }

    #[test]
    fn missing_jti_is_rejected_when_required() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::new(),
        });
        let now = Utc::now().timestamp() as usize;
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: now + 60,
            nbf: now,
            iat: now,
            jti: String::new(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.run_events".to_string()],
            oa_sync_topics: vec![],
            oa_sync_streams: vec![],
        };
        let token = make_token(claims, key);
        let result = authorizer.authenticate(&token);
        assert_eq!(result, Err(SyncAuthError::MissingJti));
    }

    #[test]
    fn stale_iat_is_rejected_by_max_age_policy() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 30,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::new(),
        });
        let now = Utc::now().timestamp() as usize;
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: now + 3600,
            nbf: now.saturating_sub(600),
            iat: now.saturating_sub(600),
            jti: "old-token-jti".to_string(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.run_events".to_string()],
            oa_sync_topics: vec![],
            oa_sync_streams: vec![],
        };
        let token = make_token(claims, key);
        let result = authorizer.authenticate(&token);
        assert_eq!(result, Err(SyncAuthError::TokenTooOld));
    }

    #[test]
    fn token_signed_with_fallback_key_is_accepted() {
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: "primary-signing-key".to_string(),
            fallback_signing_keys: vec!["previous-signing-key".to_string()],
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::new(),
        });
        let now = Utc::now().timestamp() as usize;
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: now + 60,
            nbf: now,
            iat: now,
            jti: "fallback-key-token".to_string(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
            oa_sync_topics: vec![],
            oa_sync_streams: vec![],
        };
        let token = make_token(claims, "previous-signing-key");
        let principal = authorizer.authenticate(&token);
        assert!(principal.is_ok());
    }

    #[test]
    fn explicit_stream_grants_are_enforced() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 0,
            revoked_jtis: HashSet::new(),
        });
        let now = Utc::now().timestamp() as usize;
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: now + 60,
            nbf: now,
            iat: now,
            jti: "stream-grant-token".to_string(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
            oa_sync_topics: vec![],
            oa_sync_streams: vec!["worker:desktopw:shared:lifecycle".to_string()],
        };
        let token = make_token(claims, key);
        let principal = authorizer.authenticate(&token).expect("principal");

        let denied = authorizer
            .authorize_topic(&principal, "worker:other-worker:lifecycle")
            .expect_err("worker stream outside grant must be denied");
        assert!(matches!(denied, SyncAuthError::ForbiddenTopic { .. }));
    }

    #[test]
    fn token_not_yet_valid_is_rejected_when_outside_leeway() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            fallback_signing_keys: Vec::new(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
            require_jti: true,
            max_token_age_seconds: 300,
            clock_skew_leeway_seconds: 5,
            revoked_jtis: HashSet::new(),
        });
        let now = Utc::now().timestamp() as usize;
        let claims = SyncTokenClaims {
            iss: "https://openagents.com".to_string(),
            aud: "openagents-sync".to_string(),
            sub: "user:1".to_string(),
            exp: now + 120,
            nbf: now + 30,
            iat: now,
            jti: "immature-token".to_string(),
            oa_user_id: Some(1),
            oa_org_id: Some("user:1".to_string()),
            oa_device_id: Some("ios-device".to_string()),
            oa_client_surface: Some("ios".to_string()),
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
            oa_sync_topics: vec![],
            oa_sync_streams: vec![],
        };
        let token = make_token(claims, key);
        let result = authorizer.authenticate(&token);
        assert_eq!(result, Err(SyncAuthError::TokenNotYetValid));
    }
}
