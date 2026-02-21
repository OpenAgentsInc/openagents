use std::collections::HashSet;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, errors::ErrorKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct SyncAuthConfig {
    pub signing_key: String,
    pub issuer: String,
    pub audience: String,
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
    pub oa_sync_scopes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPrincipal {
    pub user_id: Option<u64>,
    pub org_id: Option<String>,
    pub device_id: Option<String>,
    pub scopes: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizedKhalaTopic {
    WorkerLifecycle { worker_id: String },
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
    #[error("sync token revoked")]
    TokenRevoked,
    #[error("forbidden topic {topic}")]
    ForbiddenTopic { topic: String },
    #[error("missing scope for topic {topic}")]
    MissingScope {
        topic: String,
        required_scopes: Vec<String>,
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
            Self::TokenRevoked => "token_revoked",
            Self::ForbiddenTopic { .. } => "forbidden_topic",
            Self::MissingScope { .. } => "missing_scope",
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
                | Self::TokenRevoked
        )
    }
}

#[derive(Clone)]
pub struct SyncAuthorizer {
    decoding_key: DecodingKey,
    validation: Validation,
    revoked_jtis: HashSet<String>,
}

impl SyncAuthorizer {
    #[must_use]
    pub fn from_config(config: SyncAuthConfig) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[config.issuer.as_str()]);
        validation.set_audience(&[config.audience.as_str()]);
        validation.leeway = 0;
        Self {
            decoding_key: DecodingKey::from_secret(config.signing_key.as_bytes()),
            validation,
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
        let claims = match decode::<SyncTokenClaims>(token, &self.decoding_key, &self.validation) {
            Ok(decoded) => decoded.claims,
            Err(error) => match error.kind() {
                ErrorKind::ExpiredSignature => return Err(SyncAuthError::TokenExpired),
                _ => return Err(SyncAuthError::InvalidToken),
            },
        };

        if !claims.jti.is_empty() && self.revoked_jtis.contains(&claims.jti) {
            return Err(SyncAuthError::TokenRevoked);
        }

        Ok(SyncPrincipal {
            user_id: claims.oa_user_id,
            org_id: claims.oa_org_id,
            device_id: claims.oa_device_id,
            scopes: claims.oa_sync_scopes.into_iter().collect(),
        })
    }

    pub fn authorize_topic(
        &self,
        principal: &SyncPrincipal,
        topic: &str,
    ) -> Result<AuthorizedKhalaTopic, SyncAuthError> {
        let normalized_topic = topic.trim();
        let authorized_topic = parse_topic(normalized_topic)?;
        match &authorized_topic {
            AuthorizedKhalaTopic::CodexWorkerEvents => {
                ensure_scope(
                    principal,
                    normalized_topic,
                    &[
                        "runtime.codex_worker_events",
                        "runtime.worker_lifecycle_events",
                    ],
                )?;
            }
            AuthorizedKhalaTopic::WorkerLifecycle { .. } => {
                ensure_scope(
                    principal,
                    normalized_topic,
                    &[
                        "runtime.codex_worker_events",
                        "runtime.worker_lifecycle_events",
                    ],
                )?;
            }
            AuthorizedKhalaTopic::RunEvents { .. } => {
                ensure_scope(principal, normalized_topic, &["runtime.run_events"])?;
            }
        }
        Ok(authorized_topic)
    }
}

fn parse_topic(topic: &str) -> Result<AuthorizedKhalaTopic, SyncAuthError> {
    if topic.is_empty() {
        return Err(SyncAuthError::ForbiddenTopic {
            topic: topic.to_string(),
        });
    }

    if topic == "runtime.codex_worker_events" {
        return Ok(AuthorizedKhalaTopic::CodexWorkerEvents);
    }

    if let Some(rest) = topic.strip_prefix("worker:") {
        if let Some(worker_id) = rest.strip_suffix(":lifecycle") {
            if !worker_id.trim().is_empty() {
                return Ok(AuthorizedKhalaTopic::WorkerLifecycle {
                    worker_id: worker_id.to_string(),
                });
            }
        }
    }

    if let Some(rest) = topic.strip_prefix("run:") {
        if let Some(run_id) = rest.strip_suffix(":events") {
            if !run_id.trim().is_empty() {
                return Ok(AuthorizedKhalaTopic::RunEvents {
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
        AuthorizedKhalaTopic, SyncAuthConfig, SyncAuthError, SyncAuthorizer, SyncTokenClaims,
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
    fn revoked_jti_is_rejected() {
        let key = "sync-test-key";
        let authorizer = SyncAuthorizer::from_config(SyncAuthConfig {
            signing_key: key.to_string(),
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
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
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
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
            issuer: "https://openagents.com".to_string(),
            audience: "openagents-sync".to_string(),
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
            oa_sync_scopes: vec!["runtime.codex_worker_events".to_string()],
        };
        let token = make_token(claims, key);
        let principal = authorizer.authenticate(&token).expect("principal");

        let allowed = authorizer
            .authorize_topic(&principal, "worker:desktopw:shared:lifecycle")
            .expect("worker topic should be allowed");
        assert_eq!(
            allowed,
            AuthorizedKhalaTopic::WorkerLifecycle {
                worker_id: "desktopw:shared".to_string(),
            }
        );

        let denied = authorizer
            .authorize_topic(&principal, "run:019c7f93:events")
            .expect_err("run topic should require runtime.run_events scope");
        match denied {
            SyncAuthError::MissingScope { .. } => {}
            other => panic!("expected missing scope, got {other:?}"),
        }
    }
}
