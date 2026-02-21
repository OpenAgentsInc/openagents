use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::Config;

const DEFAULT_DEVICE_ID: &str = "device:unknown";
const DEFAULT_ORG_ID: &str = "user:1";
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
    access_token: String,
    refresh_token: String,
    refresh_token_id: String,
    issued_at: DateTime<Utc>,
    access_expires_at: DateTime<Utc>,
    refresh_expires_at: DateTime<Utc>,
    status: SessionStatus,
    reauth_required: bool,
}

#[derive(Debug, Clone)]
struct UserRecord {
    id: String,
    email: String,
    name: String,
    workos_user_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    ReauthRequired,
    Expired,
    Revoked,
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
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub workos_user_id: String,
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
        let now = Utc::now();

        let mut state = self.state.write().await;

        let (user, new_user) = upsert_user(&mut state, verified)?;

        revoke_existing_sessions_for_token_name(&mut state, &user.id, &token_name);

        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        let access_token = format!("oa_at_{}", Uuid::new_v4().simple());
        let refresh_token = format!("oa_rt_{}", Uuid::new_v4().simple());
        let refresh_token_id = format!("rtid_{}", Uuid::new_v4().simple());

        let session = SessionRecord {
            session_id: session_id.clone(),
            user_id: user.id.clone(),
            email: user.email.clone(),
            device_id: token_name.clone(),
            token_name: token_name.clone(),
            access_token: access_token.clone(),
            refresh_token: refresh_token.clone(),
            refresh_token_id: refresh_token_id.clone(),
            issued_at: now,
            access_expires_at: now + self.access_ttl,
            refresh_expires_at: now + self.refresh_ttl,
            status: SessionStatus::Active,
            reauth_required: false,
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

        if session.status != SessionStatus::Active || session.access_expires_at <= Utc::now() {
            if let Some(stale_session) = state.sessions.get_mut(&session_id) {
                stale_session.status = SessionStatus::Expired;
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
        })
    }

    pub async fn refresh_session(
        &self,
        refresh_token: &str,
        rotate_refresh_token: bool,
    ) -> Result<RefreshResult, AuthError> {
        let mut state = self.state.write().await;
        let session_id = match state.refresh_index.get(refresh_token) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Invalid refresh token.".to_string(),
                });
            }
        };

        let existing = match state.sessions.get(&session_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Invalid refresh token.".to_string(),
                });
            }
        };

        if existing.status != SessionStatus::Active || existing.refresh_expires_at <= Utc::now() {
            if let Some(stale_session) = state.sessions.get_mut(&session_id) {
                stale_session.status = SessionStatus::Expired;
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
        let mut updated = existing;
        updated.access_token = new_access.clone();
        updated.access_expires_at = Utc::now() + self.access_ttl;

        state.access_index.remove(&old_access);
        state
            .access_index
            .insert(new_access.clone(), session_id.clone());

        let replaced_refresh_token_id = if rotate_refresh_token {
            let new_refresh = format!("oa_rt_{}", Uuid::new_v4().simple());
            let new_refresh_token_id = format!("rtid_{}", Uuid::new_v4().simple());
            state.refresh_index.remove(&old_refresh);
            state
                .refresh_index
                .insert(new_refresh.clone(), session_id.clone());
            updated.refresh_token = new_refresh;
            updated.refresh_token_id = new_refresh_token_id;
            updated.refresh_expires_at = Utc::now() + self.refresh_ttl;
            Some(old_refresh_token_id)
        } else {
            None
        };

        let access_token_out = updated.access_token.clone();
        let refresh_token_out = updated.refresh_token.clone();
        let refresh_token_id_out = updated.refresh_token_id.clone();
        let updated_view = SessionView::from_session(&updated);
        state.sessions.insert(session_id, updated);

        Ok(RefreshResult {
            token_type: "Bearer",
            access_token: access_token_out,
            refresh_token: refresh_token_out,
            refresh_token_id: refresh_token_id_out,
            replaced_refresh_token_id,
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

        let existing = match state.sessions.get(&session_id) {
            Some(value) => value.clone(),
            None => {
                return Err(AuthError::Unauthorized {
                    message: "Unauthenticated.".to_string(),
                });
            }
        };

        state.access_index.remove(&existing.access_token);
        state.refresh_index.remove(&existing.refresh_token);

        let Some(session) = state.sessions.get_mut(&session_id) else {
            return Err(AuthError::Unauthorized {
                message: "Unauthenticated.".to_string(),
            });
        };

        session.status = SessionStatus::Revoked;

        Ok(RevocationResult {
            session_id,
            revoked_at: Utc::now(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SessionBundle {
    pub session: SessionView,
    pub user: AuthUser,
}

#[derive(Debug, Clone)]
pub struct RevocationResult {
    pub session_id: String,
    pub revoked_at: DateTime<Utc>,
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
            active_org_id: DEFAULT_ORG_ID.to_string(),
        }
    }
}

fn provider_from_config(config: &Config) -> Arc<dyn IdentityProvider> {
    match config.auth_provider_mode.as_str() {
        "mock" => Arc::new(MockIdentityProvider {
            code: config.mock_magic_code.clone(),
        }),
        "workos" => {
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
                Arc::new(MockIdentityProvider {
                    code: config.mock_magic_code.clone(),
                })
            }
        }
        _ => {
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
                Arc::new(MockIdentityProvider {
                    code: config.mock_magic_code.clone(),
                })
            }
        }
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
            state.users_by_email.insert(email, user_id.clone());
            state.users_by_workos_id.insert(workos_user_id, user_id);
            return Ok((user.clone(), false));
        }
    }

    let new_user = UserRecord {
        id: format!("user_{}", Uuid::new_v4().simple()),
        email: email.clone(),
        name: derived_name(
            &email,
            verified.first_name.as_deref(),
            verified.last_name.as_deref(),
        ),
        workos_user_id: workos_user_id.clone(),
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

fn revoke_existing_sessions_for_token_name(state: &mut AuthState, user_id: &str, token_name: &str) {
    let session_ids: Vec<String> = state
        .sessions
        .values()
        .filter(|session| session.user_id == user_id && session.token_name == token_name)
        .map(|session| session.session_id.clone())
        .collect();

    for session_id in session_ids {
        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.status = SessionStatus::Revoked;
            state.access_index.remove(&session.access_token);
            state.refresh_index.remove(&session.refresh_token);
        }
    }
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
