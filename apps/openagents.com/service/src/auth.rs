use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::Config;

const DEFAULT_DEVICE_ID: &str = "device:unknown";
const DEFAULT_CLIENT_NAME: &str = "web";

#[derive(Clone)]
pub struct AuthService {
    provider: Arc<dyn IdentityProvider>,
    state: Arc<RwLock<AuthState>>,
    challenge_ttl: Duration,
    access_ttl: Duration,
    refresh_ttl: Duration,
}

#[derive(Debug, Default)]
struct AuthState {
    challenges: HashMap<String, PendingChallenge>,
    sessions: HashMap<String, SessionRecord>,
    access_index: HashMap<String, String>,
    refresh_index: HashMap<String, String>,
    revoked_refresh_tokens: HashMap<String, RevokedRefreshTokenRecord>,
    revoked_refresh_token_ids: HashMap<String, RevokedRefreshTokenRecord>,
    users_by_id: HashMap<String, UserRecord>,
    users_by_email: HashMap<String, String>,
    users_by_workos_id: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct PendingChallenge {
    email: String,
    pending_workos_user_id: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    session_id: String,
    user_id: String,
    email: String,
    device_id: String,
    token_name: String,
    active_org_id: String,
    access_token: String,
    refresh_token: String,
    refresh_token_id: String,
    issued_at: DateTime<Utc>,
    access_expires_at: DateTime<Utc>,
    refresh_expires_at: DateTime<Utc>,
    status: SessionStatus,
    reauth_required: bool,
    last_refreshed_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    revoked_reason: Option<SessionRevocationReason>,
}

#[derive(Debug, Clone)]
struct RevokedRefreshTokenRecord {
    refresh_token_id: String,
    session_id: String,
    user_id: String,
    device_id: String,
    revoked_at: DateTime<Utc>,
    reason: RefreshTokenRevocationReason,
}

#[derive(Debug, Clone)]
struct UserRecord {
    id: String,
    email: String,
    name: String,
    workos_user_id: String,
    memberships: Vec<OrgMembership>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    ReauthRequired,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRevocationReason {
    UserRequested,
    AdminRevoked,
    TokenReplay,
    DeviceReplaced,
    SecurityPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshTokenRevocationReason {
    Rotated,
    SessionRevoked,
    ReplayDetected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgMembership {
    pub org_id: String,
    pub org_slug: String,
    pub role: OrgRole,
    pub role_scopes: Vec<String>,
    pub default_org: bool,
}

#[derive(Debug, Clone)]
pub struct ChallengeResult {
    pub challenge_id: String,
    pub email: String,
    pub pending_workos_user_id: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub user: AuthUser,
    pub token_type: &'static str,
    pub access_token: String,
    pub refresh_token: String,
    pub token_name: String,
    pub session: SessionView,
    pub new_user: bool,
}

#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub token_type: &'static str,
    pub access_token: String,
    pub refresh_token: String,
    pub refresh_token_id: String,
    pub replaced_refresh_token_id: Option<String>,
    pub session: SessionView,
}

#[derive(Debug, Clone)]
pub struct SessionView {
    pub session_id: String,
    pub user_id: String,
    pub email: String,
    pub device_id: String,
    pub status: SessionStatus,
    pub token_name: String,
    pub issued_at: DateTime<Utc>,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub reauth_required: bool,
    pub active_org_id: String,
    pub last_refreshed_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_reason: Option<SessionRevocationReason>,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub workos_user_id: String,
}

#[derive(Debug, Clone)]
pub struct SessionRevocationRequest {
    pub target: SessionRevocationTarget,
    pub include_current: bool,
    pub reason: SessionRevocationReason,
}

#[derive(Debug, Clone)]
pub enum SessionRevocationTarget {
    SessionId(String),
    DeviceId(String),
    AllSessions,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionAuditView {
    pub session_id: String,
    pub user_id: String,
    pub email: String,
    pub device_id: String,
    pub token_name: String,
    pub status: SessionStatus,
    pub issued_at: DateTime<Utc>,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub active_org_id: String,
    pub reauth_required: bool,
    pub last_refreshed_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_reason: Option<SessionRevocationReason>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("{message}")]
    Validation {
        field: &'static str,
        message: String,
    },
    #[error("{message}")]
    Unauthorized { message: String },
    #[error("{message}")]
    Forbidden { message: String },
    #[error("{message}")]
    Conflict { message: String },
    #[error("{message}")]
    Provider { message: String },
}

#[derive(Debug, Clone)]
struct StartMagicAuthResult {
    pending_workos_user_id: String,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct VerifyMagicAuthResult {
    workos_user_id: String,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    _provider_access_token: String,
    _provider_refresh_token: String,
}

#[async_trait]
trait IdentityProvider: Send + Sync {
    async fn start_magic_auth(&self, email: &str) -> Result<StartMagicAuthResult, AuthError>;

    async fn verify_magic_auth(
        &self,
        code: &str,
        pending_workos_user_id: &str,
        email: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<VerifyMagicAuthResult, AuthError>;

    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
struct MockIdentityProvider {
    code: String,
}

#[derive(Debug, Clone)]
struct WorkosIdentityProvider {
    api_key: String,
    client_id: String,
    base_url: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone)]
struct UnavailableIdentityProvider {
    message: String,
}

impl AuthService {
    pub fn from_config(config: &Config) -> Self {
        let provider = provider_from_config(config);

        Self {
            provider,
            state: Arc::new(RwLock::new(AuthState::default())),
            challenge_ttl: Duration::seconds(config.auth_challenge_ttl_seconds as i64),
            access_ttl: Duration::seconds(config.auth_access_ttl_seconds as i64),
            refresh_ttl: Duration::seconds(config.auth_refresh_ttl_seconds as i64),
        }
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    pub async fn start_challenge(&self, email: String) -> Result<ChallengeResult, AuthError> {
        let normalized_email = normalize_email(&email)?;

        let provider = self.provider.start_magic_auth(&normalized_email).await?;
        let challenge_id = format!("challenge_{}", Uuid::new_v4().simple());
        let expires_at = provider
            .expires_at
            .unwrap_or_else(|| Utc::now() + self.challenge_ttl);

        let challenge = PendingChallenge {
            email: normalized_email.clone(),
            pending_workos_user_id: provider.pending_workos_user_id.clone(),
            expires_at,
        };

        let mut state = self.state.write().await;
        state.challenges.insert(challenge_id.clone(), challenge);

        Ok(ChallengeResult {
            challenge_id,
            email: normalized_email,
            pending_workos_user_id: provider.pending_workos_user_id,
            expires_at,
        })
    }

    pub async fn verify_challenge(
        &self,
        challenge_id: &str,
        code: String,
        client_name: Option<&str>,
        requested_device_id: Option<&str>,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<VerifyResult, AuthError> {
        let normalized_code = code.trim().to_string();
        if normalized_code.is_empty() {
            return Err(AuthError::Validation {
                field: "code",
                message: "That code is invalid or expired. Request a new code.".to_string(),
            });
        }

        let pending = {
            let mut state = self.state.write().await;
            match state.challenges.remove(challenge_id) {
                Some(challenge) => challenge,
                None => {
                    return Err(AuthError::Validation {
                        field: "code",
                        message: "Your sign-in code expired. Request a new code.".to_string(),
                    });
                }
            }
        };

        if pending.expires_at <= Utc::now() {
            return Err(AuthError::Validation {
                field: "code",
                message: "Your sign-in code expired. Request a new code.".to_string(),
            });
        }

        let verified = self
            .provider
            .verify_magic_auth(
                &normalized_code,
                &pending.pending_workos_user_id,
                &pending.email,
                ip_address,
                user_agent,
            )
            .await?;

        let token_name = token_name_for_client(client_name);
        let device_id = normalize_device_id(requested_device_id, &token_name)?;
        let now = Utc::now();

        let mut state = self.state.write().await;

        let (user, new_user) = upsert_user(&mut state, verified)?;
        let active_org_id = user
            .memberships
            .iter()
            .find(|membership| membership.default_org)
            .map(|membership| membership.org_id.clone())
            .or_else(|| {
                user.memberships
                    .first()
                    .map(|membership| membership.org_id.clone())
            })
            .unwrap_or_else(|| format!("user:{}", user.id));

        revoke_existing_sessions_for_device(
            &mut state,
            &user.id,
            &device_id,
            SessionRevocationReason::DeviceReplaced,
            now,
        );

        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        let access_token = format!("oa_at_{}", Uuid::new_v4().simple());
        let refresh_token = format!("oa_rt_{}", Uuid::new_v4().simple());
        let refresh_token_id = format!("rtid_{}", Uuid::new_v4().simple());

        let session = SessionRecord {
            session_id: session_id.clone(),
            user_id: user.id.clone(),
            email: user.email.clone(),
            device_id: device_id.clone(),
            token_name: token_name.clone(),
            active_org_id,
            access_token: access_token.clone(),
            refresh_token: refresh_token.clone(),
            refresh_token_id: refresh_token_id.clone(),
            issued_at: now,
            access_expires_at: now + self.access_ttl,
            refresh_expires_at: now + self.refresh_ttl,
            status: SessionStatus::Active,
            reauth_required: false,
            last_refreshed_at: None,
            revoked_at: None,
            revoked_reason: None,
        };

        state
            .access_index
            .insert(access_token.clone(), session_id.clone());
        state
            .refresh_index
            .insert(refresh_token.clone(), session_id.clone());
        state.sessions.insert(session_id.clone(), session.clone());

        Ok(VerifyResult {
            user: AuthUser {
                id: user.id,
                email: user.email,
                name: user.name,
                workos_user_id: user.workos_user_id,
            },
            token_type: "Bearer",
            access_token,
            refresh_token,
            token_name,
            session: SessionView::from_session(&session),
            new_user,
        })
    }

    pub async fn session_from_access_token(
        &self,
        access_token: &str,
    ) -> Result<SessionBundle, AuthError> {
        let mut state = self.state.write().await;
        let session_id = match state.access_index.get(access_token) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        let session = match state.sessions.get(&session_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        if session.status != SessionStatus::Active {
            state.access_index.remove(access_token);
            return Err(AuthError::Unauthorized {
                message: auth_denied_message(session.status),
            });
        }

        if session.access_expires_at <= Utc::now() {
            if let Some(stale_session) = state.sessions.get_mut(&session_id) {
                stale_session.status = SessionStatus::Expired;
                stale_session.revoked_reason = None;
                stale_session.revoked_at = None;
            }
            state.access_index.remove(access_token);
            return Err(AuthError::Unauthorized {
                message: "Unauthenticated.".to_string(),
            });
        }

        let user = match state.users_by_id.get(&session.user_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        Ok(SessionBundle {
            session: SessionView::from_session(&session),
            user: AuthUser {
                id: user.id,
                email: user.email,
                name: user.name,
                workos_user_id: user.workos_user_id,
            },
            memberships: user.memberships,
        })
    }

    pub async fn refresh_session(
        &self,
        refresh_token: &str,
        requested_device_id: Option<&str>,
        rotate_refresh_token: bool,
    ) -> Result<RefreshResult, AuthError> {
        if !rotate_refresh_token {
            return Err(AuthError::Validation {
                field: "rotate_refresh_token",
                message: "Refresh token rotation is required.".to_string(),
            });
        }

        let mut state = self.state.write().await;

        if let Some(revoked) = state.revoked_refresh_tokens.get(refresh_token).cloned() {
            let replay_detected_at = revoked.revoked_at;
            let replay_reason = revoked.reason;
            if let Some(replayed_session) = state.sessions.get(&revoked.session_id).cloned() {
                if replayed_session.status == SessionStatus::Active {
                    revoke_session(
                        &mut state,
                        &replayed_session.session_id,
                        SessionRevocationReason::TokenReplay,
                        replay_detected_at,
                    );
                }
            }

            if replay_reason != RefreshTokenRevocationReason::ReplayDetected {
                record_revoked_refresh_token(
                    &mut state,
                    revoked.session_id,
                    revoked.user_id,
                    revoked.device_id,
                    revoked.refresh_token_id,
                    refresh_token.to_string(),
                    Utc::now(),
                    RefreshTokenRevocationReason::ReplayDetected,
                );
            }

            return Err(AuthError::Unauthorized {
                message: "Refresh token was already rotated or revoked.".to_string(),
            });
        }

        let session_id = match state.refresh_index.get(refresh_token) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Invalid refresh token.".to_string(),
                });
            }
        };

        let mut existing = match state.sessions.get(&session_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Invalid refresh token.".to_string(),
                });
            }
        };

        if let Some(device_id) = requested_device_id {
            let normalized_device_id = normalize_device_id(Some(device_id), &existing.token_name)?;
            if normalized_device_id != existing.device_id {
                return Err(AuthError::Forbidden {
                    message: "Refresh token does not belong to the requested device.".to_string(),
                });
            }
        }

        if existing.status != SessionStatus::Active {
            state.refresh_index.remove(refresh_token);
            return Err(AuthError::Unauthorized {
                message: auth_denied_message(existing.status),
            });
        }

        if existing.refresh_expires_at <= Utc::now() {
            if let Some(stale_session) = state.sessions.get_mut(&session_id) {
                stale_session.status = SessionStatus::Expired;
                stale_session.revoked_reason = None;
                stale_session.revoked_at = None;
            }
            state.refresh_index.remove(refresh_token);
            return Err(AuthError::Unauthorized {
                message: "Refresh session expired.".to_string(),
            });
        }

        let old_access = existing.access_token.clone();
        let old_refresh = existing.refresh_token.clone();
        let old_refresh_token_id = existing.refresh_token_id.clone();

        let new_access = format!("oa_at_{}", Uuid::new_v4().simple());
        existing.access_token = new_access.clone();
        existing.access_expires_at = Utc::now() + self.access_ttl;
        existing.last_refreshed_at = Some(Utc::now());

        state.access_index.remove(&old_access);
        state
            .access_index
            .insert(new_access.clone(), session_id.clone());

        let new_refresh = format!("oa_rt_{}", Uuid::new_v4().simple());
        let new_refresh_token_id = format!("rtid_{}", Uuid::new_v4().simple());
        state.refresh_index.remove(&old_refresh);
        state
            .refresh_index
            .insert(new_refresh.clone(), session_id.clone());
        existing.refresh_token = new_refresh;
        existing.refresh_token_id = new_refresh_token_id;
        existing.refresh_expires_at = Utc::now() + self.refresh_ttl;

        record_revoked_refresh_token(
            &mut state,
            existing.session_id.clone(),
            existing.user_id.clone(),
            existing.device_id.clone(),
            old_refresh_token_id.clone(),
            old_refresh,
            Utc::now(),
            RefreshTokenRevocationReason::Rotated,
        );

        let access_token_out = existing.access_token.clone();
        let refresh_token_out = existing.refresh_token.clone();
        let refresh_token_id_out = existing.refresh_token_id.clone();
        let updated_view = SessionView::from_session(&existing);
        state.sessions.insert(session_id, existing);

        Ok(RefreshResult {
            token_type: "Bearer",
            access_token: access_token_out,
            refresh_token: refresh_token_out,
            refresh_token_id: refresh_token_id_out,
            replaced_refresh_token_id: Some(old_refresh_token_id),
            session: updated_view,
        })
    }

    pub async fn revoke_session_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<RevocationResult, AuthError> {
        let mut state = self.state.write().await;
        let session_id = match state.access_index.get(access_token) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        if !state.sessions.contains_key(&session_id) {
            return Err(AuthError::Unauthorized {
                message: "Unauthenticated.".to_string(),
            });
        };

        let revoked_at = Utc::now();
        let _ = revoke_session(
            &mut state,
            &session_id,
            SessionRevocationReason::UserRequested,
            revoked_at,
        );
        Ok(RevocationResult {
            session_id,
            revoked_at,
        })
    }

    pub async fn list_user_sessions(
        &self,
        user_id: &str,
        device_id_filter: Option<&str>,
    ) -> Result<Vec<SessionAuditView>, AuthError> {
        let normalized_filter = if let Some(value) = device_id_filter {
            Some(normalize_device_id(Some(value), DEFAULT_DEVICE_ID)?)
        } else {
            None
        };

        let state = self.state.read().await;
        let mut sessions: Vec<SessionAuditView> = state
            .sessions
            .values()
            .filter(|session| session.user_id == user_id)
            .filter(|session| {
                normalized_filter
                    .as_ref()
                    .map(|filter| session.device_id == *filter)
                    .unwrap_or(true)
            })
            .map(SessionAuditView::from_session)
            .collect();
        sessions.sort_by(|left, right| right.issued_at.cmp(&left.issued_at));
        Ok(sessions)
    }

    pub async fn revoke_user_sessions(
        &self,
        user_id: &str,
        current_session_id: &str,
        request: SessionRevocationRequest,
    ) -> Result<SessionBatchRevocationResult, AuthError> {
        let mut state = self.state.write().await;
        let mut candidate_ids: HashSet<String> = match request.target {
            SessionRevocationTarget::SessionId(session_id) => {
                let session =
                    state
                        .sessions
                        .get(&session_id)
                        .ok_or_else(|| AuthError::Validation {
                            field: "session_id",
                            message: "Requested session does not exist.".to_string(),
                        })?;
                if session.user_id != user_id {
                    return Err(AuthError::Forbidden {
                        message: "Requested session is not owned by current user.".to_string(),
                    });
                }
                HashSet::from([session_id])
            }
            SessionRevocationTarget::DeviceId(device_id) => {
                let normalized_device =
                    normalize_device_id(Some(device_id.as_str()), DEFAULT_DEVICE_ID)?;
                state
                    .sessions
                    .values()
                    .filter(|session| {
                        session.user_id == user_id && session.device_id == normalized_device
                    })
                    .map(|session| session.session_id.clone())
                    .collect()
            }
            SessionRevocationTarget::AllSessions => state
                .sessions
                .values()
                .filter(|session| session.user_id == user_id)
                .map(|session| session.session_id.clone())
                .collect(),
        };

        if !request.include_current {
            candidate_ids.remove(current_session_id);
        }

        let revoked_at = Utc::now();
        let mut revoked_session_ids = Vec::new();
        let mut revoked_refresh_token_ids = Vec::new();

        for session_id in candidate_ids {
            if let Some(revoked) =
                revoke_session(&mut state, &session_id, request.reason, revoked_at)
            {
                revoked_session_ids.push(revoked.session_id);
                revoked_refresh_token_ids.push(revoked.refresh_token_id);
            }
        }

        revoked_session_ids.sort();
        revoked_refresh_token_ids.sort();

        Ok(SessionBatchRevocationResult {
            revoked_session_ids,
            revoked_refresh_token_ids,
            reason: request.reason,
            revoked_at,
        })
    }

    pub async fn set_active_org_by_access_token(
        &self,
        access_token: &str,
        org_id: &str,
    ) -> Result<SessionBundle, AuthError> {
        let mut state = self.state.write().await;
        let session_id = match state.access_index.get(access_token) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        let existing = match state.sessions.get(&session_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        if existing.status != SessionStatus::Active {
            return Err(AuthError::Unauthorized {
                message: auth_denied_message(existing.status),
            });
        }

        if existing.access_expires_at <= Utc::now() {
            return Err(AuthError::Unauthorized {
                message: "Unauthenticated.".to_string(),
            });
        }

        let user = match state.users_by_id.get(&existing.user_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        if !user
            .memberships
            .iter()
            .any(|membership| membership.org_id == org_id)
        {
            return Err(AuthError::Forbidden {
                message: "Requested organization is not available for this user.".to_string(),
            });
        }

        let mut updated = existing;
        updated.active_org_id = org_id.to_string();
        state.sessions.insert(session_id, updated.clone());

        Ok(SessionBundle {
            session: SessionView::from_session(&updated),
            user: AuthUser {
                id: user.id.clone(),
                email: user.email.clone(),
                name: user.name.clone(),
                workos_user_id: user.workos_user_id.clone(),
            },
            memberships: user.memberships.clone(),
        })
    }

    pub async fn evaluate_policy_by_access_token(
        &self,
        access_token: &str,
        request: PolicyCheckRequest,
    ) -> Result<PolicyDecision, AuthError> {
        let bundle = self.session_from_access_token(access_token).await?;
        let resolved_org_id = request
            .org_id
            .and_then(|value| non_empty(value.trim().to_string()))
            .unwrap_or_else(|| bundle.session.active_org_id.clone());

        let Some(membership) = bundle
            .memberships
            .iter()
            .find(|membership| membership.org_id == resolved_org_id)
        else {
            return Ok(PolicyDecision {
                allowed: false,
                resolved_org_id,
                granted_scopes: Vec::new(),
                denied_reasons: vec!["org_scope_denied".to_string()],
            });
        };

        let mut denied_reasons = Vec::new();
        let mut granted_scopes = Vec::new();

        for scope in request.required_scopes {
            let normalized_scope = scope.trim().to_string();
            if normalized_scope.is_empty() {
                continue;
            }

            if scope_allowed(membership, &normalized_scope) {
                granted_scopes.push(normalized_scope);
            } else {
                denied_reasons.push(format!("scope_denied:{normalized_scope}"));
            }
        }

        for topic in request.requested_topics {
            let normalized_topic = topic.trim().to_string();
            if normalized_topic.is_empty() {
                continue;
            }

            if !topic_allowed(&normalized_topic, &bundle.user.id, &resolved_org_id) {
                denied_reasons.push(format!("topic_denied:{normalized_topic}"));
            }
        }

        Ok(PolicyDecision {
            allowed: denied_reasons.is_empty(),
            resolved_org_id,
            granted_scopes,
            denied_reasons,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SessionBundle {
    pub session: SessionView,
    pub user: AuthUser,
    pub memberships: Vec<OrgMembership>,
}

#[derive(Debug, Clone)]
pub struct RevocationResult {
    pub session_id: String,
    pub revoked_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SessionRevocationOutcome {
    session_id: String,
    refresh_token_id: String,
}

#[derive(Debug, Clone)]
pub struct SessionBatchRevocationResult {
    pub revoked_session_ids: Vec<String>,
    pub revoked_refresh_token_ids: Vec<String>,
    pub reason: SessionRevocationReason,
    pub revoked_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PolicyCheckRequest {
    pub org_id: Option<String>,
    pub required_scopes: Vec<String>,
    pub requested_topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub resolved_org_id: String,
    pub granted_scopes: Vec<String>,
    pub denied_reasons: Vec<String>,
}

impl SessionView {
    fn from_session(session: &SessionRecord) -> Self {
        Self {
            session_id: session.session_id.clone(),
            user_id: session.user_id.clone(),
            email: session.email.clone(),
            device_id: session.device_id.clone(),
            status: session.status,
            token_name: session.token_name.clone(),
            issued_at: session.issued_at,
            access_expires_at: session.access_expires_at,
            refresh_expires_at: session.refresh_expires_at,
            reauth_required: session.reauth_required,
            active_org_id: session.active_org_id.clone(),
            last_refreshed_at: session.last_refreshed_at,
            revoked_at: session.revoked_at,
            revoked_reason: session.revoked_reason,
        }
    }
}

impl SessionAuditView {
    fn from_session(session: &SessionRecord) -> Self {
        Self {
            session_id: session.session_id.clone(),
            user_id: session.user_id.clone(),
            email: session.email.clone(),
            device_id: session.device_id.clone(),
            token_name: session.token_name.clone(),
            status: session.status,
            issued_at: session.issued_at,
            access_expires_at: session.access_expires_at,
            refresh_expires_at: session.refresh_expires_at,
            active_org_id: session.active_org_id.clone(),
            reauth_required: session.reauth_required,
            last_refreshed_at: session.last_refreshed_at,
            revoked_at: session.revoked_at,
            revoked_reason: session.revoked_reason,
        }
    }
}

fn provider_from_config(config: &Config) -> Arc<dyn IdentityProvider> {
    match config.auth_provider_mode.as_str() {
        "mock" => Arc::new(MockIdentityProvider {
            code: config.mock_magic_code.clone(),
        }),
        "workos" | "auto" => workos_or_unavailable(config),
        _ => workos_or_unavailable(config),
    }
}

fn workos_or_unavailable(config: &Config) -> Arc<dyn IdentityProvider> {
    if let (Some(client_id), Some(api_key)) = (
        config.workos_client_id.clone(),
        config.workos_api_key.clone(),
    ) {
        Arc::new(WorkosIdentityProvider::new(
            client_id,
            api_key,
            config.workos_api_base_url.clone(),
        ))
    } else {
        Arc::new(UnavailableIdentityProvider {
            message:
                "WorkOS identity provider is required. Configure WORKOS_CLIENT_ID and WORKOS_API_KEY or use OA_AUTH_PROVIDER_MODE=mock only for local/testing."
                    .to_string(),
        })
    }
}

fn normalize_email(raw_email: &str) -> Result<String, AuthError> {
    let email = raw_email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') || email.len() > 255 {
        return Err(AuthError::Validation {
            field: "email",
            message: "Enter a valid email address first.".to_string(),
        });
    }

    Ok(email)
}

fn token_name_for_client(client_name: Option<&str>) -> String {
    let normalized = client_name
        .map(str::trim)
        .map(str::to_lowercase)
        .unwrap_or_else(|| DEFAULT_CLIENT_NAME.to_string());

    match normalized.as_str() {
        "autopilot-ios" | "openagents-expo" => format!("mobile:{normalized}"),
        "autopilot-desktop" | "openagents-desktop" => format!("desktop:{normalized}"),
        _ => DEFAULT_DEVICE_ID.to_string(),
    }
}

fn upsert_user(
    state: &mut AuthState,
    verified: VerifyMagicAuthResult,
) -> Result<(UserRecord, bool), AuthError> {
    let email = normalize_email(&verified.email)?;
    let workos_user_id = verified.workos_user_id.trim().to_string();

    let mut selected_user_id = None;

    if let Some(existing_by_email) = state.users_by_email.get(&email) {
        selected_user_id = Some(existing_by_email.clone());
    }

    if selected_user_id.is_none() {
        if let Some(existing_by_workos) = state.users_by_workos_id.get(&workos_user_id) {
            selected_user_id = Some(existing_by_workos.clone());
        }
    }

    if let Some(user_id) = selected_user_id {
        if let Some(user) = state.users_by_id.get_mut(&user_id) {
            user.email = email.clone();
            user.workos_user_id = workos_user_id.clone();
            user.name = derived_name(
                &email,
                verified.first_name.as_deref(),
                verified.last_name.as_deref(),
            );
            user.memberships =
                ensure_default_memberships(&user.id, &email, user.memberships.clone());
            state.users_by_email.insert(email, user_id.clone());
            state.users_by_workos_id.insert(workos_user_id, user_id);
            return Ok((user.clone(), false));
        }
    }

    let user_id = format!("user_{}", Uuid::new_v4().simple());
    let new_user = UserRecord {
        id: user_id.clone(),
        email: email.clone(),
        name: derived_name(
            &email,
            verified.first_name.as_deref(),
            verified.last_name.as_deref(),
        ),
        workos_user_id: workos_user_id.clone(),
        memberships: ensure_default_memberships(&user_id, &email, Vec::new()),
    };

    state.users_by_email.insert(email, new_user.id.clone());
    state
        .users_by_workos_id
        .insert(workos_user_id, new_user.id.clone());
    state
        .users_by_id
        .insert(new_user.id.clone(), new_user.clone());

    Ok((new_user, true))
}

fn revoke_existing_sessions_for_device(
    state: &mut AuthState,
    user_id: &str,
    device_id: &str,
    reason: SessionRevocationReason,
    revoked_at: DateTime<Utc>,
) {
    let session_ids: Vec<String> = state
        .sessions
        .values()
        .filter(|session| session.user_id == user_id && session.device_id == device_id)
        .map(|session| session.session_id.clone())
        .collect();

    for session_id in session_ids {
        let _ = revoke_session(state, &session_id, reason, revoked_at);
    }
}

fn revoke_session(
    state: &mut AuthState,
    session_id: &str,
    reason: SessionRevocationReason,
    revoked_at: DateTime<Utc>,
) -> Option<SessionRevocationOutcome> {
    let existing = state.sessions.get(session_id)?.clone();

    if matches!(
        existing.status,
        SessionStatus::Revoked | SessionStatus::Expired
    ) {
        return None;
    }

    state.access_index.remove(&existing.access_token);
    state.refresh_index.remove(&existing.refresh_token);
    record_revoked_refresh_token(
        state,
        existing.session_id.clone(),
        existing.user_id.clone(),
        existing.device_id.clone(),
        existing.refresh_token_id.clone(),
        existing.refresh_token.clone(),
        revoked_at,
        RefreshTokenRevocationReason::SessionRevoked,
    );

    let reauth_required = matches!(
        reason,
        SessionRevocationReason::TokenReplay | SessionRevocationReason::SecurityPolicy
    );

    if let Some(session) = state.sessions.get_mut(session_id) {
        session.status = SessionStatus::Revoked;
        session.reauth_required = reauth_required;
        session.revoked_at = Some(revoked_at);
        session.revoked_reason = Some(reason);
    }

    Some(SessionRevocationOutcome {
        session_id: existing.session_id,
        refresh_token_id: existing.refresh_token_id,
    })
}

fn record_revoked_refresh_token(
    state: &mut AuthState,
    session_id: String,
    user_id: String,
    device_id: String,
    refresh_token_id: String,
    refresh_token: String,
    revoked_at: DateTime<Utc>,
    reason: RefreshTokenRevocationReason,
) {
    let record = RevokedRefreshTokenRecord {
        refresh_token_id: refresh_token_id.clone(),
        session_id,
        user_id,
        device_id,
        revoked_at,
        reason,
    };
    state
        .revoked_refresh_token_ids
        .insert(refresh_token_id, record.clone());
    state.revoked_refresh_tokens.insert(refresh_token, record);
}

fn normalize_device_id(
    requested_device_id: Option<&str>,
    fallback: &str,
) -> Result<String, AuthError> {
    let candidate = requested_device_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback);

    if candidate.len() > 160 {
        return Err(AuthError::Validation {
            field: "device_id",
            message: "Device id exceeds maximum length.".to_string(),
        });
    }

    if !candidate.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, ':' | '-' | '_' | '.')
    }) {
        return Err(AuthError::Validation {
            field: "device_id",
            message: "Device id contains unsupported characters.".to_string(),
        });
    }

    Ok(candidate.to_lowercase())
}

fn derived_name(email: &str, first_name: Option<&str>, last_name: Option<&str>) -> String {
    let first = first_name.unwrap_or_default().trim();
    let last = last_name.unwrap_or_default().trim();
    let candidate = format!("{first} {last}").trim().to_string();

    if candidate.is_empty() {
        return email.to_string();
    }

    candidate
}

fn ensure_default_memberships(
    user_id: &str,
    email: &str,
    existing: Vec<OrgMembership>,
) -> Vec<OrgMembership> {
    let mut by_org: HashMap<String, OrgMembership> = existing
        .into_iter()
        .map(|membership| (membership.org_id.clone(), membership))
        .collect();

    let personal_org_id = format!("user:{user_id}");
    by_org
        .entry(personal_org_id.clone())
        .or_insert_with(|| OrgMembership {
            org_id: personal_org_id.clone(),
            org_slug: format!("user-{user_id}"),
            role: OrgRole::Owner,
            role_scopes: owner_role_scopes(),
            default_org: true,
        });

    if email.ends_with("@openagents.com") {
        by_org
            .entry("org:openagents".to_string())
            .or_insert_with(|| OrgMembership {
                org_id: "org:openagents".to_string(),
                org_slug: "openagents".to_string(),
                role: OrgRole::Member,
                role_scopes: member_role_scopes(),
                default_org: false,
            });
    }

    let mut memberships: Vec<OrgMembership> = by_org.into_values().collect();
    memberships.sort_by(|left, right| left.org_id.cmp(&right.org_id));

    if !memberships.iter().any(|membership| membership.default_org) {
        if let Some(first) = memberships.first_mut() {
            first.default_org = true;
        }
    }

    memberships
}

fn owner_role_scopes() -> Vec<String> {
    vec![
        "runtime.read".to_string(),
        "runtime.write".to_string(),
        "sync.subscribe".to_string(),
        "policy.evaluate".to_string(),
        "org.membership.read".to_string(),
        "org.membership.write".to_string(),
    ]
}

fn member_role_scopes() -> Vec<String> {
    vec![
        "runtime.read".to_string(),
        "sync.subscribe".to_string(),
        "policy.evaluate".to_string(),
        "org.membership.read".to_string(),
    ]
}

fn scope_allowed(membership: &OrgMembership, required_scope: &str) -> bool {
    match membership.role {
        OrgRole::Owner | OrgRole::Admin => true,
        OrgRole::Member | OrgRole::Viewer => membership
            .role_scopes
            .iter()
            .any(|scope| scope == required_scope),
    }
}

fn topic_allowed(topic: &str, user_id: &str, org_id: &str) -> bool {
    if topic.starts_with(&format!("user:{user_id}:")) {
        return true;
    }

    let org_prefix = if org_id.starts_with("org:") {
        format!("{org_id}:")
    } else {
        format!("org:{org_id}:")
    };

    if topic.starts_with(&org_prefix) {
        return true;
    }

    if topic.starts_with("run:") {
        return true;
    }

    false
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn auth_denied_message(status: SessionStatus) -> String {
    match status {
        SessionStatus::Active => "Unauthenticated.".to_string(),
        SessionStatus::ReauthRequired => "Session requires reauthentication.".to_string(),
        SessionStatus::Expired => "Refresh session expired.".to_string(),
        SessionStatus::Revoked => "Session was revoked.".to_string(),
    }
}

#[async_trait]
impl IdentityProvider for MockIdentityProvider {
    async fn start_magic_auth(&self, email: &str) -> Result<StartMagicAuthResult, AuthError> {
        Ok(StartMagicAuthResult {
            pending_workos_user_id: format!(
                "mock_workos_{}",
                Uuid::new_v5(&Uuid::NAMESPACE_DNS, email.as_bytes()).simple()
            ),
            expires_at: Some(Utc::now() + Duration::minutes(10)),
        })
    }

    async fn verify_magic_auth(
        &self,
        code: &str,
        pending_workos_user_id: &str,
        email: &str,
        _ip_address: &str,
        _user_agent: &str,
    ) -> Result<VerifyMagicAuthResult, AuthError> {
        if code.trim() != self.code {
            return Err(AuthError::Validation {
                field: "code",
                message: "That code is invalid or expired. Request a new code.".to_string(),
            });
        }

        let local = email.split('@').next().unwrap_or("User");
        Ok(VerifyMagicAuthResult {
            workos_user_id: pending_workos_user_id.to_string(),
            email: email.to_string(),
            first_name: Some(local.to_string()),
            last_name: Some("Mock".to_string()),
            _provider_access_token: format!("workos_access_{}", Uuid::new_v4().simple()),
            _provider_refresh_token: format!("workos_refresh_{}", Uuid::new_v4().simple()),
        })
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

impl WorkosIdentityProvider {
    fn new(client_id: String, api_key: String, base_url: String) -> Self {
        Self {
            client_id,
            api_key,
            base_url,
            http: reqwest::Client::new(),
        }
    }

    async fn post_form<T: serde::de::DeserializeOwned, P: Serialize>(
        &self,
        path: &str,
        payload: &P,
    ) -> Result<T, AuthError> {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );

        let response = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(payload)
            .send()
            .await
            .map_err(|error| AuthError::Provider {
                message: format!("Unable to contact WorkOS: {error}"),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 400 || status.as_u16() == 401 || status.as_u16() == 422 {
                return Err(AuthError::Validation {
                    field: "code",
                    message: "That code is invalid or expired. Request a new code.".to_string(),
                });
            }

            return Err(AuthError::Provider {
                message: format!("WorkOS request failed ({status}): {body}"),
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|error| AuthError::Provider {
                message: format!("Invalid WorkOS response payload: {error}"),
            })
    }
}

#[async_trait]
impl IdentityProvider for UnavailableIdentityProvider {
    async fn start_magic_auth(&self, _email: &str) -> Result<StartMagicAuthResult, AuthError> {
        Err(AuthError::Provider {
            message: self.message.clone(),
        })
    }

    async fn verify_magic_auth(
        &self,
        _code: &str,
        _pending_workos_user_id: &str,
        _email: &str,
        _ip_address: &str,
        _user_agent: &str,
    ) -> Result<VerifyMagicAuthResult, AuthError> {
        Err(AuthError::Provider {
            message: self.message.clone(),
        })
    }

    fn name(&self) -> &'static str {
        "workos"
    }
}

#[async_trait]
impl IdentityProvider for WorkosIdentityProvider {
    async fn start_magic_auth(&self, email: &str) -> Result<StartMagicAuthResult, AuthError> {
        let payload = serde_json::json!({
            "email": email,
        });

        let result: WorkosMagicAuthResponse = self
            .post_form("user_management/magic_auth", &payload)
            .await?;

        let pending_workos_user_id = result.user_id.unwrap_or_default().trim().to_string();

        if pending_workos_user_id.is_empty() {
            return Err(AuthError::Validation {
                field: "email",
                message: "Sign-in provider response was invalid. Please try again.".to_string(),
            });
        }

        Ok(StartMagicAuthResult {
            pending_workos_user_id,
            expires_at: result.expires_at.and_then(parse_timestamp),
        })
    }

    async fn verify_magic_auth(
        &self,
        code: &str,
        pending_workos_user_id: &str,
        email: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<VerifyMagicAuthResult, AuthError> {
        let email_payload = serde_json::json!({
            "client_id": self.client_id,
            "client_secret": self.api_key,
            "grant_type": "urn:workos:oauth:grant-type:magic-auth:code",
            "email": email,
            "code": code,
            "ip_address": empty_to_none(ip_address),
            "user_agent": empty_to_none(user_agent),
        });

        let auth_response = match self
            .post_form::<WorkosAuthenticateResponse, _>(
                "user_management/authenticate",
                &email_payload,
            )
            .await
        {
            Ok(response) => response,
            Err(_) => {
                let user_payload = serde_json::json!({
                    "client_id": self.client_id,
                    "client_secret": self.api_key,
                    "grant_type": "urn:workos:oauth:grant-type:magic-auth:code",
                    "user_id": pending_workos_user_id,
                    "code": code,
                    "ip_address": empty_to_none(ip_address),
                    "user_agent": empty_to_none(user_agent),
                });

                self.post_form::<WorkosAuthenticateResponse, _>(
                    "user_management/authenticate",
                    &user_payload,
                )
                .await?
            }
        };

        let user = auth_response.user;
        let workos_user_id = user.id.unwrap_or_default().trim().to_string();
        let resolved_email = user.email.unwrap_or_default().trim().to_lowercase();

        if workos_user_id.is_empty() || resolved_email.is_empty() {
            return Err(AuthError::Validation {
                field: "code",
                message: "Sign-in provider user payload was invalid. Please try again.".to_string(),
            });
        }

        let access_token = auth_response
            .access_token
            .unwrap_or_default()
            .trim()
            .to_string();
        let refresh_token = auth_response
            .refresh_token
            .unwrap_or_default()
            .trim()
            .to_string();

        if access_token.is_empty() || refresh_token.is_empty() {
            return Err(AuthError::Validation {
                field: "code",
                message: "Sign-in provider response was incomplete. Please try again.".to_string(),
            });
        }

        Ok(VerifyMagicAuthResult {
            workos_user_id,
            email: resolved_email,
            first_name: user.first_name,
            last_name: user.last_name,
            _provider_access_token: access_token,
            _provider_refresh_token: refresh_token,
        })
    }

    fn name(&self) -> &'static str {
        "workos"
    }
}

#[derive(Debug, Deserialize)]
struct WorkosMagicAuthResponse {
    #[serde(alias = "user_id", alias = "userId")]
    user_id: Option<String>,
    #[serde(alias = "expires_at", alias = "expiresAt")]
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkosAuthenticateResponse {
    #[serde(alias = "access_token", alias = "accessToken")]
    access_token: Option<String>,
    #[serde(alias = "refresh_token", alias = "refreshToken")]
    refresh_token: Option<String>,
    user: WorkosUser,
}

#[derive(Debug, Deserialize)]
struct WorkosUser {
    id: Option<String>,
    email: Option<String>,
    #[serde(alias = "first_name", alias = "firstName")]
    first_name: Option<String>,
    #[serde(alias = "last_name", alias = "lastName")]
    last_name: Option<String>,
}

fn parse_timestamp(raw: String) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw.trim())
        .map(|value| value.with_timezone(&Utc))
        .ok()
}

fn empty_to_none(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
