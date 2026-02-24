use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::Config;

const DEFAULT_DEVICE_ID: &str = "device:unknown";
const DEFAULT_CLIENT_NAME: &str = "web";

#[derive(Clone)]
pub struct AuthService {
    provider: Arc<dyn IdentityProvider>,
    state: Arc<RwLock<AuthState>>,
    store: AuthStateStore,
    challenge_ttl: Duration,
    access_ttl: Duration,
    refresh_ttl: Duration,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
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
    personal_access_tokens: HashMap<String, PersonalAccessTokenRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingChallenge {
    email: String,
    pending_workos_user_id: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RevokedRefreshTokenRecord {
    refresh_token_id: String,
    session_id: String,
    user_id: String,
    device_id: String,
    revoked_at: DateTime<Utc>,
    reason: RefreshTokenRevocationReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserRecord {
    id: String,
    email: String,
    name: String,
    workos_user_id: String,
    memberships: Vec<OrgMembership>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl SessionRevocationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserRequested => "user_requested",
            Self::AdminRevoked => "admin_revoked",
            Self::TokenReplay => "token_replay",
            Self::DeviceReplaced => "device_replaced",
            Self::SecurityPolicy => "security_policy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct ApiRegisterResult {
    pub user: AuthUser,
    pub created: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersonalAccessTokenRecord {
    token_id: String,
    user_id: String,
    name: String,
    token: String,
    scopes: Vec<String>,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonalAccessTokenView {
    pub token_id: String,
    pub name: String,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct PersonalAccessTokenIssueResult {
    pub token_id: String,
    pub plain_text_token: String,
    pub token: PersonalAccessTokenView,
}

#[derive(Debug, Clone)]
struct AuthStateStore {
    path: Option<PathBuf>,
}

impl AuthStateStore {
    fn from_config(config: &Config) -> Self {
        Self {
            path: config.auth_store_path.clone(),
        }
    }

    fn load_state(&self) -> AuthState {
        let Some(path) = self.path.as_ref() else {
            return AuthState::default();
        };

        let raw = match std::fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return AuthState::default();
            }
            Err(error) => {
                tracing::warn!(
                    target: "openagents.auth",
                    path = %path.display(),
                    error = %error,
                    "failed to read auth store; booting with empty auth state",
                );
                return AuthState::default();
            }
        };

        match serde_json::from_str::<AuthState>(&raw) {
            Ok(state) => state,
            Err(error) => {
                tracing::warn!(
                    target: "openagents.auth",
                    path = %path.display(),
                    error = %error,
                    "failed to parse auth store; booting with empty auth state",
                );
                AuthState::default()
            }
        }
    }

    async fn persist_state(&self, state: &AuthState) -> Result<(), AuthError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| AuthError::Provider {
                    message: format!("failed to prepare auth store directory: {error}"),
                })?;
        }

        let payload = serde_json::to_vec(state).map_err(|error| AuthError::Provider {
            message: format!("failed to encode auth store payload: {error}"),
        })?;
        let temp_path = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));

        tokio::fs::write(&temp_path, payload)
            .await
            .map_err(|error| AuthError::Provider {
                message: format!("failed to write auth store payload: {error}"),
            })?;

        tokio::fs::rename(&temp_path, path)
            .await
            .map_err(|error| AuthError::Provider {
                message: format!("failed to finalize auth store payload: {error}"),
            })?;

        Ok(())
    }
}

impl AuthService {
    pub fn from_config(config: &Config) -> Self {
        let provider = provider_from_config(config);
        let store = AuthStateStore::from_config(config);
        let loaded_state = store.load_state();

        Self {
            provider,
            state: Arc::new(RwLock::new(loaded_state)),
            store,
            challenge_ttl: Duration::seconds(config.auth_challenge_ttl_seconds as i64),
            access_ttl: Duration::seconds(config.auth_access_ttl_seconds as i64),
            refresh_ttl: Duration::seconds(config.auth_refresh_ttl_seconds as i64),
        }
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    async fn persist_state_snapshot(&self, snapshot: AuthState) -> Result<(), AuthError> {
        self.store.persist_state(&snapshot).await
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

        let snapshot = {
            let mut state = self.state.write().await;
            state.challenges.insert(challenge_id.clone(), challenge);
            state.clone()
        };
        self.persist_state_snapshot(snapshot).await?;

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

        let (result, snapshot) = {
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

            let result = VerifyResult {
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
            };

            (result, state.clone())
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(result)
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
            let snapshot = state.clone();
            drop(state);
            let _ = self.persist_state_snapshot(snapshot).await;
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
            let snapshot = state.clone();
            drop(state);
            let _ = self.persist_state_snapshot(snapshot).await;
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

    pub async fn session_or_pat_from_access_token(
        &self,
        access_token: &str,
    ) -> Result<SessionBundle, AuthError> {
        if let Ok(session_bundle) = self.session_from_access_token(access_token).await {
            return Ok(session_bundle);
        }

        let (bundle, snapshot) = {
            let mut state = self.state.write().await;
            let now = Utc::now();

            let (token_id, token_name, token_created_at, token_expires_at, user_id) = {
                let token = state
                    .personal_access_tokens
                    .values_mut()
                    .find(|record| record.token == access_token)
                    .ok_or_else(|| AuthError::Unauthorized {
                        message: "Unauthenticated.".to_string(),
                    })?;

                if token.revoked_at.is_some() {
                    return Err(AuthError::Unauthorized {
                        message: "Unauthenticated.".to_string(),
                    });
                }

                if token
                    .expires_at
                    .map(|expires_at| expires_at <= now)
                    .unwrap_or(false)
                {
                    return Err(AuthError::Unauthorized {
                        message: "Unauthenticated.".to_string(),
                    });
                }

                token.last_used_at = Some(now);

                (
                    token.token_id.clone(),
                    token.name.clone(),
                    token.created_at,
                    token.expires_at.unwrap_or(now + self.refresh_ttl),
                    token.user_id.clone(),
                )
            };

            let (user, memberships) = {
                let existing = state.users_by_id.get(&user_id).cloned().ok_or_else(|| {
                    AuthError::Unauthorized {
                        message: "Unauthenticated.".to_string(),
                    }
                })?;

                let memberships = ensure_default_memberships(
                    &existing.id,
                    &existing.email,
                    existing.memberships.clone(),
                );
                if memberships != existing.memberships {
                    if let Some(record) = state.users_by_id.get_mut(&user_id) {
                        record.memberships = memberships.clone();
                    }
                }

                (existing, memberships)
            };

            let active_org_id = memberships
                .iter()
                .find(|membership| membership.default_org)
                .map(|membership| membership.org_id.clone())
                .or_else(|| {
                    memberships
                        .first()
                        .map(|membership| membership.org_id.clone())
                })
                .unwrap_or_else(|| format!("user:{}", user.id));

            let bundle = SessionBundle {
                session: SessionView {
                    session_id: format!("pat:{token_id}"),
                    user_id: user.id.clone(),
                    email: user.email.clone(),
                    device_id: format!("pat:{token_id}"),
                    token_name,
                    active_org_id,
                    issued_at: token_created_at,
                    access_expires_at: token_expires_at,
                    refresh_expires_at: token_expires_at,
                    status: SessionStatus::Active,
                    reauth_required: false,
                    last_refreshed_at: None,
                    revoked_at: None,
                    revoked_reason: None,
                },
                user: AuthUser {
                    id: user.id.clone(),
                    email: user.email.clone(),
                    name: user.name.clone(),
                    workos_user_id: user.workos_user_id.clone(),
                },
                memberships,
            };

            (bundle, state.clone())
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(bundle)
    }

    pub async fn user_by_id(&self, user_id: &str) -> Option<AuthUser> {
        let state = self.state.read().await;
        let record = state.users_by_id.get(user_id)?;
        Some(AuthUser {
            id: record.id.clone(),
            email: record.email.clone(),
            name: record.name.clone(),
            workos_user_id: record.workos_user_id.clone(),
        })
    }

    pub async fn user_by_id_or_handle(&self, value: &str) -> Option<AuthUser> {
        let needle = value.trim();
        if needle.is_empty() {
            return None;
        }
        let normalized = needle.to_ascii_lowercase();

        let state = self.state.read().await;
        if let Some(record) = state.users_by_id.get(needle) {
            return Some(AuthUser {
                id: record.id.clone(),
                email: record.email.clone(),
                name: record.name.clone(),
                workos_user_id: record.workos_user_id.clone(),
            });
        }

        let record = state
            .users_by_id
            .values()
            .find(|candidate| user_handle_from_email(&candidate.email) == normalized)?;
        Some(AuthUser {
            id: record.id.clone(),
            email: record.email.clone(),
            name: record.name.clone(),
            workos_user_id: record.workos_user_id.clone(),
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

            let snapshot = state.clone();
            drop(state);
            let _ = self.persist_state_snapshot(snapshot).await;
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
            let snapshot = state.clone();
            drop(state);
            let _ = self.persist_state_snapshot(snapshot).await;
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
            let snapshot = state.clone();
            drop(state);
            let _ = self.persist_state_snapshot(snapshot).await;
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
        let snapshot = state.clone();
        drop(state);
        self.persist_state_snapshot(snapshot).await?;

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
        let revoked = revoke_session(
            &mut state,
            &session_id,
            SessionRevocationReason::UserRequested,
            revoked_at,
        )
        .ok_or_else(|| AuthError::Unauthorized {
            message: "Unauthenticated.".to_string(),
        })?;
        let snapshot = state.clone();
        drop(state);
        self.persist_state_snapshot(snapshot).await?;
        Ok(RevocationResult {
            session_id,
            device_id: revoked.device_id,
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
        let mut revoked_device_ids = Vec::new();
        let mut revoked_refresh_token_ids = Vec::new();

        for session_id in candidate_ids {
            if let Some(revoked) =
                revoke_session(&mut state, &session_id, request.reason, revoked_at)
            {
                revoked_session_ids.push(revoked.session_id);
                revoked_device_ids.push(revoked.device_id);
                revoked_refresh_token_ids.push(revoked.refresh_token_id);
            }
        }

        revoked_session_ids.sort();
        revoked_device_ids.sort();
        revoked_device_ids.dedup();
        revoked_refresh_token_ids.sort();
        let snapshot = state.clone();
        drop(state);
        self.persist_state_snapshot(snapshot).await?;

        Ok(SessionBatchRevocationResult {
            revoked_session_ids,
            revoked_device_ids,
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
        let snapshot = state.clone();
        drop(state);
        self.persist_state_snapshot(snapshot).await?;

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

    pub async fn update_profile_name(
        &self,
        user_id: &str,
        name: String,
    ) -> Result<AuthUser, AuthError> {
        let normalized_name = non_empty(name).ok_or_else(|| AuthError::Validation {
            field: "name",
            message: "The name field is required.".to_string(),
        })?;

        let (updated_user, snapshot) = {
            let mut state = self.state.write().await;
            let user =
                state
                    .users_by_id
                    .get_mut(user_id)
                    .ok_or_else(|| AuthError::Unauthorized {
                        message: "Unauthenticated.".to_string(),
                    })?;

            user.name = normalized_name.clone();
            let updated_user = AuthUser {
                id: user.id.clone(),
                email: user.email.clone(),
                name: user.name.clone(),
                workos_user_id: user.workos_user_id.clone(),
            };

            (updated_user, state.clone())
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(updated_user)
    }

    pub async fn delete_profile(&self, user_id: &str) -> Result<AuthUser, AuthError> {
        let now = Utc::now();
        let (deleted_user, snapshot) = {
            let mut state = self.state.write().await;
            let user =
                state
                    .users_by_id
                    .remove(user_id)
                    .ok_or_else(|| AuthError::Unauthorized {
                        message: "Unauthenticated.".to_string(),
                    })?;

            state.users_by_email.remove(&user.email);
            if !user.workos_user_id.trim().is_empty() {
                state.users_by_workos_id.remove(&user.workos_user_id);
            }

            state
                .challenges
                .retain(|_, challenge| challenge.email != user.email);

            let session_ids = state
                .sessions
                .values()
                .filter(|session| session.user_id == user_id)
                .map(|session| session.session_id.clone())
                .collect::<Vec<_>>();

            for session_id in session_ids {
                if let Some(session) = state.sessions.remove(&session_id) {
                    state.access_index.remove(&session.access_token);
                    state.refresh_index.remove(&session.refresh_token);
                    record_revoked_refresh_token(
                        &mut state,
                        session_id,
                        session.user_id,
                        session.device_id,
                        session.refresh_token_id,
                        session.refresh_token,
                        now,
                        RefreshTokenRevocationReason::SessionRevoked,
                    );
                }
            }

            state
                .revoked_refresh_tokens
                .retain(|_, revoked| revoked.user_id != user_id);
            state
                .revoked_refresh_token_ids
                .retain(|_, revoked| revoked.user_id != user_id);
            state
                .personal_access_tokens
                .retain(|_, token| token.user_id != user_id);

            let deleted_user = AuthUser {
                id: user.id,
                email: user.email,
                name: user.name,
                workos_user_id: user.workos_user_id,
            };

            (deleted_user, state.clone())
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(deleted_user)
    }

    pub async fn register_api_user(
        &self,
        email: String,
        name: Option<String>,
    ) -> Result<ApiRegisterResult, AuthError> {
        let normalized_email = normalize_email(&email)?;
        let requested_name = name.and_then(non_empty).map(|value| truncate_name(&value));
        let generated_workos_id = bootstrap_workos_id_for_email(&normalized_email);

        let (user, created, snapshot) = {
            let mut state = self.state.write().await;

            if let Some(existing_user_id) = state.users_by_email.get(&normalized_email).cloned() {
                let (updated_user, user_id, workos_user_id) = {
                    let existing_user =
                        state
                            .users_by_id
                            .get_mut(&existing_user_id)
                            .ok_or_else(|| AuthError::Unauthorized {
                                message: "Unauthenticated.".to_string(),
                            })?;

                    existing_user.email = normalized_email.clone();
                    existing_user.name = requested_name
                        .clone()
                        .unwrap_or_else(|| default_name_from_email(&normalized_email));
                    if existing_user.workos_user_id.trim().is_empty() {
                        existing_user.workos_user_id = generated_workos_id.clone();
                    }
                    existing_user.memberships = ensure_default_memberships(
                        &existing_user.id,
                        &normalized_email,
                        existing_user.memberships.clone(),
                    );

                    (
                        existing_user.clone(),
                        existing_user.id.clone(),
                        existing_user.workos_user_id.clone(),
                    )
                };

                state
                    .users_by_workos_id
                    .insert(workos_user_id, user_id.clone());
                state
                    .users_by_email
                    .insert(normalized_email.clone(), user_id);

                (updated_user, false, state.clone())
            } else {
                let user_id = format!("user_{}", Uuid::new_v4().simple());
                let created_user = UserRecord {
                    id: user_id.clone(),
                    email: normalized_email.clone(),
                    name: requested_name
                        .clone()
                        .unwrap_or_else(|| default_name_from_email(&normalized_email)),
                    workos_user_id: generated_workos_id.clone(),
                    memberships: ensure_default_memberships(
                        &user_id,
                        &normalized_email,
                        Vec::new(),
                    ),
                };

                state
                    .users_by_email
                    .insert(normalized_email.clone(), created_user.id.clone());
                state
                    .users_by_workos_id
                    .insert(generated_workos_id, created_user.id.clone());
                state
                    .users_by_id
                    .insert(created_user.id.clone(), created_user.clone());

                (created_user, true, state.clone())
            }
        };

        self.persist_state_snapshot(snapshot).await?;

        Ok(ApiRegisterResult {
            user: AuthUser {
                id: user.id,
                email: user.email,
                name: user.name,
                workos_user_id: user.workos_user_id,
            },
            created,
        })
    }

    pub async fn local_test_sign_in(
        &self,
        email: String,
        name: Option<String>,
        client_name: Option<&str>,
        requested_device_id: Option<&str>,
    ) -> Result<VerifyResult, AuthError> {
        let normalized_email = normalize_email(&email)?;
        let requested_name = name.and_then(non_empty).map(|value| truncate_name(&value));
        let local_workos_id = local_test_workos_id_for_email(&normalized_email);
        let token_name = token_name_for_client(client_name);
        let device_id = normalize_device_id(requested_device_id, &token_name)?;
        let now = Utc::now();

        let (result, snapshot) = {
            let mut state = self.state.write().await;

            let (user, new_user) = if let Some(existing_user_id) =
                state.users_by_email.get(&normalized_email).cloned()
            {
                let (updated_user, user_id, previous_workos_id) = {
                    let existing_user =
                        state
                            .users_by_id
                            .get_mut(&existing_user_id)
                            .ok_or_else(|| AuthError::Unauthorized {
                                message: "Unauthenticated.".to_string(),
                            })?;
                    let previous_workos_id = existing_user.workos_user_id.clone();

                    existing_user.email = normalized_email.clone();
                    existing_user.name = requested_name
                        .clone()
                        .unwrap_or_else(|| default_name_from_email(&normalized_email));
                    existing_user.workos_user_id = local_workos_id.clone();
                    existing_user.memberships = ensure_default_memberships(
                        &existing_user.id,
                        &normalized_email,
                        existing_user.memberships.clone(),
                    );

                    (
                        existing_user.clone(),
                        existing_user.id.clone(),
                        previous_workos_id,
                    )
                };

                if !previous_workos_id.trim().is_empty() && previous_workos_id != local_workos_id {
                    state.users_by_workos_id.remove(&previous_workos_id);
                }
                state
                    .users_by_workos_id
                    .insert(local_workos_id.clone(), user_id.clone());
                state
                    .users_by_email
                    .insert(normalized_email.clone(), user_id);

                (updated_user, false)
            } else {
                let user_id = format!("user_{}", Uuid::new_v4().simple());
                let created_user = UserRecord {
                    id: user_id.clone(),
                    email: normalized_email.clone(),
                    name: requested_name
                        .clone()
                        .unwrap_or_else(|| default_name_from_email(&normalized_email)),
                    workos_user_id: local_workos_id.clone(),
                    memberships: ensure_default_memberships(
                        &user_id,
                        &normalized_email,
                        Vec::new(),
                    ),
                };

                state
                    .users_by_email
                    .insert(normalized_email.clone(), created_user.id.clone());
                state
                    .users_by_workos_id
                    .insert(local_workos_id.clone(), created_user.id.clone());
                state
                    .users_by_id
                    .insert(created_user.id.clone(), created_user.clone());

                (created_user, true)
            };

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

            let result = VerifyResult {
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
            };

            (result, state.clone())
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(result)
    }

    pub async fn list_personal_access_tokens(
        &self,
        user_id: &str,
    ) -> Result<Vec<PersonalAccessTokenView>, AuthError> {
        let state = self.state.read().await;
        if !state.users_by_id.contains_key(user_id) {
            return Err(AuthError::Unauthorized {
                message: "Unauthenticated.".to_string(),
            });
        }

        let mut tokens: Vec<PersonalAccessTokenView> = state
            .personal_access_tokens
            .values()
            .filter(|token| token.user_id == user_id)
            .map(PersonalAccessTokenView::from_record)
            .collect();
        tokens.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(tokens)
    }

    pub async fn user_by_email(&self, email: &str) -> Result<Option<AuthUser>, AuthError> {
        let normalized_email = normalize_email(email)?;
        let state = self.state.read().await;
        let Some(user_id) = state.users_by_email.get(&normalized_email) else {
            return Ok(None);
        };
        let Some(user) = state.users_by_id.get(user_id) else {
            return Ok(None);
        };
        Ok(Some(AuthUser {
            id: user.id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            workos_user_id: user.workos_user_id.clone(),
        }))
    }

    pub async fn issue_personal_access_token(
        &self,
        user_id: &str,
        name: String,
        scopes: Vec<String>,
        ttl_seconds: Option<u64>,
    ) -> Result<PersonalAccessTokenIssueResult, AuthError> {
        let normalized_name = non_empty(name).ok_or_else(|| AuthError::Validation {
            field: "name",
            message: "Token name is required.".to_string(),
        })?;

        let mut normalized_scopes: Vec<String> = scopes
            .into_iter()
            .filter_map(non_empty)
            .collect::<HashSet<String>>()
            .into_iter()
            .collect();
        normalized_scopes.sort();

        let now = Utc::now();
        let expires_at = ttl_seconds.map(|seconds| now + Duration::seconds(seconds as i64));
        let token_id = format!("pat_{}", Uuid::new_v4().simple());
        let plain_text_token = format!("oa_pat_{}", Uuid::new_v4().simple());
        let record = PersonalAccessTokenRecord {
            token_id: token_id.clone(),
            user_id: user_id.to_string(),
            name: normalized_name.clone(),
            token: plain_text_token.clone(),
            scopes: normalized_scopes.clone(),
            created_at: now,
            last_used_at: None,
            expires_at,
            revoked_at: None,
        };

        let snapshot = {
            let mut state = self.state.write().await;
            if !state.users_by_id.contains_key(user_id) {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }

            state
                .personal_access_tokens
                .insert(token_id.clone(), record.clone());
            state.clone()
        };
        self.persist_state_snapshot(snapshot).await?;

        Ok(PersonalAccessTokenIssueResult {
            token_id,
            plain_text_token,
            token: PersonalAccessTokenView::from_record(&record),
        })
    }

    pub async fn revoke_personal_access_token(
        &self,
        user_id: &str,
        token_id: &str,
    ) -> Result<bool, AuthError> {
        let snapshot = {
            let mut state = self.state.write().await;
            if !state.users_by_id.contains_key(user_id) {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }

            let Some(token) = state.personal_access_tokens.get_mut(token_id) else {
                return Ok(false);
            };

            if token.user_id != user_id {
                return Err(AuthError::Forbidden {
                    message: "Requested token is not owned by current user.".to_string(),
                });
            }

            if token.revoked_at.is_none() {
                token.revoked_at = Some(Utc::now());
            }

            state.clone()
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(true)
    }

    pub async fn revoke_all_personal_access_tokens(
        &self,
        user_id: &str,
    ) -> Result<usize, AuthError> {
        let now = Utc::now();
        let (deleted_count, snapshot) = {
            let mut state = self.state.write().await;
            if !state.users_by_id.contains_key(user_id) {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }

            let mut deleted_count = 0usize;
            for token in state.personal_access_tokens.values_mut() {
                if token.user_id == user_id && token.revoked_at.is_none() {
                    token.revoked_at = Some(now);
                    deleted_count = deleted_count.saturating_add(1);
                }
            }

            (deleted_count, state.clone())
        };

        self.persist_state_snapshot(snapshot).await?;
        Ok(deleted_count)
    }

    pub async fn current_personal_access_token_id(
        &self,
        user_id: &str,
        plain_text_token: &str,
    ) -> Option<String> {
        let now = Utc::now();
        let state = self.state.read().await;

        state
            .personal_access_tokens
            .values()
            .find(|record| {
                record.user_id == user_id
                    && record.token == plain_text_token
                    && record.revoked_at.is_none()
                    && record
                        .expires_at
                        .map(|expires_at| expires_at > now)
                        .unwrap_or(true)
            })
            .map(|record| record.token_id.clone())
    }

    pub async fn evaluate_policy_by_access_token(
        &self,
        access_token: &str,
        request: PolicyCheckRequest,
    ) -> Result<PolicyDecision, AuthError> {
        let bundle = self.session_or_pat_from_access_token(access_token).await?;
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
    pub device_id: String,
    pub revoked_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SessionRevocationOutcome {
    session_id: String,
    device_id: String,
    refresh_token_id: String,
}

#[derive(Debug, Clone)]
pub struct SessionBatchRevocationResult {
    pub revoked_session_ids: Vec<String>,
    pub revoked_device_ids: Vec<String>,
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

impl PersonalAccessTokenView {
    fn from_record(record: &PersonalAccessTokenRecord) -> Self {
        Self {
            token_id: record.token_id.clone(),
            name: record.name.clone(),
            scopes: record.scopes.clone(),
            created_at: record.created_at,
            last_used_at: record.last_used_at,
            expires_at: record.expires_at,
            revoked_at: record.revoked_at,
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

fn default_name_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or_default();
    let normalized = local.replace(['.', '-', '_'], " ");
    let title = normalized
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ");

    let candidate = if title.trim().is_empty() {
        "API User".to_string()
    } else {
        title
    };
    truncate_name(&candidate)
}

fn user_handle_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or_default();
    let mut output = String::with_capacity(local.len().min(64));
    let mut previous_dash = false;
    for character in local.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            output.push(normalized);
            previous_dash = false;
            continue;
        }

        if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }

    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "user".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

fn truncate_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "API User".to_string();
    }
    trimmed.chars().take(120).collect()
}

fn bootstrap_workos_id_for_email(email: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(email.trim().to_lowercase().as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }

    let suffix: String = hex.chars().take(32).collect();
    format!("api_bootstrap_{suffix}")
}

fn local_test_workos_id_for_email(email: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(email.trim().to_lowercase().as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }

    let suffix: String = hex.chars().take(24).collect();
    format!("test_local_{suffix}")
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
        device_id: existing.device_id,
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

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::PathBuf;

    use super::AuthService;
    use crate::config::Config;

    fn test_config(store_path: Option<PathBuf>) -> Config {
        Config {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            log_filter: "debug".to_string(),
            static_dir: std::env::temp_dir(),
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
            auth_store_path: store_path,
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
            route_split_rust_routes: vec!["/".to_string()],
            route_split_cohort_percentage: 0,
            route_split_salt: "route-split-test-salt".to_string(),
            route_split_force_legacy: false,
            route_split_legacy_base_url: Some("https://legacy.openagents.test".to_string()),
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
            liquidity_stats_pool_ids: vec!["llp-main".to_string()],
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
            google_gmail_api_base_url: "https://gmail.googleapis.com".to_string(),
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
    fn token_name_for_client_supports_desktop_canonical_and_legacy_aliases() {
        assert_eq!(
            super::token_name_for_client(Some("autopilot-desktop")),
            "desktop:autopilot-desktop"
        );
        assert_eq!(
            super::token_name_for_client(Some("openagents-desktop")),
            "desktop:openagents-desktop"
        );
        assert_eq!(
            super::token_name_for_client(Some("openagents-expo")),
            "mobile:openagents-expo"
        );
    }

    #[tokio::test]
    async fn persists_session_state_when_store_path_configured() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store_path = temp.path().join("auth-store.json");
        let config = test_config(Some(store_path.clone()));
        let auth = AuthService::from_config(&config);

        let challenge = auth
            .start_challenge("persisted@openagents.com".to_string())
            .await
            .expect("start challenge");
        let verified = auth
            .verify_challenge(
                &challenge.challenge_id,
                "123456".to_string(),
                Some("autopilot-ios"),
                Some("ios:persisted"),
                "127.0.0.1",
                "test-agent",
            )
            .await
            .expect("verify challenge");

        let restored = AuthService::from_config(&config);
        let restored_session = restored
            .session_from_access_token(&verified.access_token)
            .await
            .expect("restored session");
        assert_eq!(restored_session.user.email, "persisted@openagents.com");
        assert_eq!(restored_session.session.device_id, "ios:persisted");
        assert!(store_path.is_file());
    }

    #[tokio::test]
    async fn personal_access_token_lifecycle_is_persisted() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store_path = temp.path().join("auth-store.json");
        let config = test_config(Some(store_path.clone()));
        let auth = AuthService::from_config(&config);

        let challenge = auth
            .start_challenge("token-owner@openagents.com".to_string())
            .await
            .expect("start challenge");
        let verified = auth
            .verify_challenge(
                &challenge.challenge_id,
                "123456".to_string(),
                Some("autopilot-ios"),
                Some("ios:token-owner"),
                "127.0.0.1",
                "test-agent",
            )
            .await
            .expect("verify challenge");

        let issued = auth
            .issue_personal_access_token(
                &verified.user.id,
                "CI token".to_string(),
                vec!["runtime.read".to_string(), "runtime.read".to_string()],
                Some(600),
            )
            .await
            .expect("issue personal token");
        assert!(issued.plain_text_token.starts_with("oa_pat_"));

        let listed = auth
            .list_personal_access_tokens(&verified.user.id)
            .await
            .expect("list personal tokens");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].token_id, issued.token_id);
        assert_eq!(listed[0].scopes, vec!["runtime.read".to_string()]);

        let revoked = auth
            .revoke_personal_access_token(&verified.user.id, &issued.token_id)
            .await
            .expect("revoke token");
        assert!(revoked);

        let restored = AuthService::from_config(&config);
        let restored_tokens = restored
            .list_personal_access_tokens(&verified.user.id)
            .await
            .expect("list restored tokens");
        assert_eq!(restored_tokens.len(), 1);
        assert!(restored_tokens[0].revoked_at.is_some());
        assert!(store_path.is_file());
    }

    #[tokio::test]
    async fn user_by_email_returns_hydrated_user() {
        let auth = AuthService::from_config(&test_config(None));

        let challenge = auth
            .start_challenge("lookup-user@openagents.com".to_string())
            .await
            .expect("start challenge");
        let verified = auth
            .verify_challenge(
                &challenge.challenge_id,
                "123456".to_string(),
                Some("openagents-web"),
                Some("browser:lookup"),
                "127.0.0.1",
                "test-agent",
            )
            .await
            .expect("verify challenge");

        let resolved = auth
            .user_by_email("lookup-user@openagents.com")
            .await
            .expect("lookup by email")
            .expect("user should exist");
        assert_eq!(resolved.id, verified.user.id);
        assert_eq!(resolved.email, "lookup-user@openagents.com");
    }

    #[tokio::test]
    async fn api_register_user_creates_or_updates_user_record() {
        let auth = AuthService::from_config(&test_config(None));

        let first = auth
            .register_api_user(
                "api-user@staging.openagents.com".to_string(),
                Some("API User".to_string()),
            )
            .await
            .expect("first register");
        assert!(first.created);
        assert_eq!(first.user.email, "api-user@staging.openagents.com");
        assert!(first.user.workos_user_id.starts_with("api_bootstrap_"));

        let second = auth
            .register_api_user(
                "api-user@staging.openagents.com".to_string(),
                Some("Updated Name".to_string()),
            )
            .await
            .expect("second register");
        assert!(!second.created);
        assert_eq!(second.user.id, first.user.id);
        assert_eq!(second.user.name, "Updated Name");
    }

    #[tokio::test]
    async fn local_test_sign_in_creates_or_updates_local_test_user() {
        let auth = AuthService::from_config(&test_config(None));

        let first = auth
            .local_test_sign_in(
                "tester@openagents.com".to_string(),
                Some("Maintenance Tester".to_string()),
                Some("openagents-web"),
                Some("browser:local-test"),
            )
            .await
            .expect("first local test sign-in");
        assert!(first.new_user);
        assert_eq!(first.user.email, "tester@openagents.com");
        assert!(first.user.workos_user_id.starts_with("test_local_"));
        assert!(!first.access_token.is_empty());
        assert!(!first.refresh_token.is_empty());

        let second = auth
            .local_test_sign_in(
                "tester@openagents.com".to_string(),
                Some("Updated Tester".to_string()),
                Some("openagents-web"),
                Some("browser:local-test"),
            )
            .await
            .expect("second local test sign-in");
        assert!(!second.new_user);
        assert_eq!(second.user.id, first.user.id);
        assert_eq!(second.user.name, "Updated Tester");
        assert!(second.user.workos_user_id.starts_with("test_local_"));
    }
}
