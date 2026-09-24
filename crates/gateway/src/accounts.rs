//! The account surface: the HTTP adapter in front of `accounts.json`,
//! `sessions.json`, and `keys.json`.
//!
//! The stores keep the records; this module is the transport. A caller
//! presents an `oak_` key or a `sess_` token, the adapter resolves the
//! account it belongs to, and every workspace operation authorizes the
//! acting account's membership before it touches state. Secrets cross
//! the boundary the way the stores keep them: an `oak_` key, an `inv_`
//! token, an `rcv_` token, and a `sess_` token are each returned
//! exactly once, in the response that minted them.
//!
//! The session token is the browser surface — `Authorization: Bearer
//! sess_<hex>` on every management route, and on `POST /v1/systemone`
//! under the rules `serve::authenticate` documents. There are no
//! cookies: a bearer token carries no ambient authority, so there is
//! no cross-site forgery surface to defend.
//!
//! Every route answers the shared envelope: `{"error": {"code", ..}}`
//! on a refusal, and a `openagents.accounts.v1` document on success.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, delete, get, patch, post};
use serde_json::{Value, json};

use tenancy::sessions::{self, SessionId, SessionKind};
use tenancy::workspaces::UserId;
use tenancy::{Accounts, MemberRef, Registry, Role, keys};

use crate::serve::ServeState;

/// The schema tag every account-surface response carries.
const SCHEMA: &str = "openagents.accounts.v1";

/// The anonymous budget's id — the one funded lane a deployment runs.
pub(crate) const ANONYMOUS_BUDGET: &str = "onb_public";

/// The default invitation lifetime: seven days. An invitation is a
/// credential in transit — it should expire if it was never delivered.
const INVITE_TTL_SECS: u64 = 604_800;

/// The longest invitation lifetime the adapter accepts — a month.
const INVITE_TTL_MAX: u64 = 2_592_000;

/// The management routes the account surface mounts — present only
/// when `accounts` is configured, like `/v1/balance` under `money`.
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/sessions", post(sign_in)),
        ("/v1/session", get(session_status).delete(logout)),
        ("/v1/accounts", post(sign_up)),
        ("/v1/account", get(account_view)),
        ("/v1/account/access", get(own_access)),
        ("/v1/invitations/accept", post(invitation_accept)),
        ("/v1/recovery/redeem", post(recovery_redeem)),
        ("/v1/workspaces", post(workspace_create)),
        (
            "/v1/workspaces/{workspace}",
            get(workspace_view).patch(workspace_update),
        ),
        (
            "/v1/workspaces/{workspace}/invitations",
            post(invitation_issue),
        ),
        (
            "/v1/workspaces/{workspace}/invitations/{invitation}",
            delete(invitation_revoke),
        ),
        (
            "/v1/workspaces/{workspace}/members/{account}",
            patch(member_role).delete(member_remove),
        ),
        ("/v1/workspaces/{workspace}/transfer", post(transfer)),
        ("/v1/workspaces/{workspace}/recovery", post(recovery_issue)),
        ("/v1/workspaces/{workspace}/access", get(workspace_access)),
        (
            "/v1/workspaces/{workspace}/keys",
            get(keys_list).post(key_issue),
        ),
        ("/v1/workspaces/{workspace}/keys/{key}/copy", post(key_copy)),
        (
            "/v1/workspaces/{workspace}/keys/{key}/pause",
            post(key_pause),
        ),
        (
            "/v1/workspaces/{workspace}/keys/{key}/resume",
            post(key_resume),
        ),
        (
            "/v1/workspaces/{workspace}/keys/{key}/rotate",
            post(key_rotate),
        ),
        ("/v1/workspaces/{workspace}/keys/{key}", delete(key_revoke)),
    ]
}

/// Who a management call runs as.
pub(crate) enum Principal {
    /// An account reached by session token or by `oak_` key — the
    /// credential kind is the audit detail, not a different authority.
    Account {
        account: String,
        /// The session's digest id, when a `sess_` token resolved.
        session: Option<String>,
    },
    /// The funded public lane — an anonymous session holding no
    /// account. It answers its own session routes and nothing else.
    Anonymous { session: String },
}

impl Principal {
    /// The account id, when the principal holds one.
    pub(crate) fn account(&self) -> Option<&str> {
        match self {
            Self::Account { account, .. } => Some(account),
            Self::Anonymous { .. } => None,
        }
    }

    /// The actor name the access log records.
    pub(crate) fn actor(&self) -> &str {
        match self {
            Self::Account { account, .. } => account,
            Self::Anonymous { .. } => "anonymous",
        }
    }

    /// The session id the call ran under, for the access log.
    pub(crate) fn session(&self) -> Option<&str> {
        match self {
            Self::Account { session, .. } => session.as_deref(),
            Self::Anonymous { session } => Some(session),
        }
    }
}

/// The response the account surface's refusals take: the shared error
/// envelope with a stable code.
pub(crate) fn refused(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(json!({"error": {"code": code, "message": message.into()}})),
    )
        .into_response()
}

/// A success document under the surface's schema tag.
fn answered(status: StatusCode, fields: Value) -> Response {
    let mut body = fields;
    body["v"] = json!(SCHEMA);
    (status, Json(body)).into_response()
}

fn unavailable(name: &'static str, message: impl Into<String>) -> Response {
    refused(StatusCode::SERVICE_UNAVAILABLE, name, message)
}

/// The bearer token of the request — `oak_`, `sess_`, or refused.
fn bearer(headers: &HeaderMap) -> Result<String, Response> {
    let header = headers.get("authorization").ok_or_else(|| {
        refused(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Sign in first. Send an `oak_` API key or a `sess_` session token in the `Authorization: Bearer` header.",
        )
    })?;
    let header = header.to_str().map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "malformed",
            "The `Authorization` header contains characters that aren't valid text.",
        )
    })?;
    header
        .strip_prefix("Bearer ")
        .map(str::to_string)
        .ok_or_else(|| {
            refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "The `Authorization` header must start with `Bearer `.",
            )
        })
}

/// Open the accounts store — the membership read every management call
/// makes.
pub(crate) fn accounts_store(state: &ServeState) -> Result<Accounts, Response> {
    Accounts::open(&state.dir).map_err(|trouble| {
        unavailable(
            "accounts_unavailable",
            format!(
                "The service can't read accounts right now. Try again later. Details: {trouble}"
            ),
        )
    })
}

/// Open the sessions store.
fn sessions_store(state: &ServeState) -> Result<sessions::Sessions, Response> {
    sessions::Sessions::open(&state.dir).map_err(|trouble| {
        unavailable(
            "sessions_unavailable",
            format!(
                "The service can't read sessions right now. Try again later. Details: {trouble}"
            ),
        )
    })
}

/// The registry — needed wherever a key authenticates or a tenant
/// resolves.
fn registry(state: &ServeState) -> Result<Registry, Response> {
    Registry::open(&state.dir).map_err(|trouble| {
        unavailable(
            "registry_unavailable",
            format!("The service can't read its model settings right now. Try again later. Details: {trouble}"),
        )
    })
}

/// Resolve the caller to a [`Principal`].
///
/// A `sess_` token resolves through the session book: an active
/// user-kind session names its account, an anonymous-kind one names
/// the funded lane, and a closed or unknown one is `unauthenticated`.
/// Any other bearer resolves as an `oak_` key through `keys.rs`, then
/// the `key:<id>` principal to its account — a key bound to no account
/// is a valid credential with no account surface, which is a distinct
/// answer from a bad one.
pub(crate) fn principal(state: &ServeState, headers: &HeaderMap) -> Result<Principal, Response> {
    let token = bearer(headers)?;
    if token.starts_with("sess_") {
        let sessions = sessions_store(state)?;
        let store = sessions
            .store()
            .map_err(|t| unavailable("sessions_unavailable", t.to_string()))?;
        let session = store.book.session_of_token(&token).ok_or_else(|| {
            refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Your session token isn't recognized. Sign in again.",
            )
        })?;
        if session.standing(unix_now()) != sessions::SessionState::Active {
            return Err(refused(
                StatusCode::UNAUTHORIZED,
                "session_closed",
                format!(
                    "Your session is {}. Sign in again.",
                    session.standing(unix_now())
                ),
            ));
        }
        return Ok(match session.kind {
            SessionKind::Anonymous => Principal::Anonymous {
                session: session.id.as_str().to_string(),
            },
            SessionKind::User => Principal::Account {
                account: session.user.as_str().to_string(),
                session: Some(session.id.as_str().to_string()),
            },
        });
    }
    let registry = registry(state)?;
    let authenticated =
        keys::authenticate(&state.dir, registry.manifest(), &token).map_err(|cause| {
            refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                format!("Your API key was rejected: {cause}"),
            )
        })?;
    if authenticated
        .scopes
        .as_ref()
        .is_some_and(|scopes| !scopes.permits_action("accounts"))
    {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "out_of_scope",
            "Your API key's scope doesn't allow account actions.",
        ));
    }
    let accounts = accounts_store(state)?;
    let account = accounts
        .account_of_principal(&format!("key:{}", authenticated.key_id))
        .map_err(|t| unavailable("accounts_unavailable", t.to_string()))?
        .ok_or_else(|| {
            refused(
                StatusCode::FORBIDDEN,
                "no_account",
                "This API key is valid but isn't linked to an account. Use an API \
                 key that a workspace member created.",
            )
        })?;
    Ok(Principal::Account {
        account,
        session: None,
    })
}

/// The account the call runs as — an anonymous principal has none.
pub(crate) fn member_account(principal: &Principal) -> Result<&str, Response> {
    principal.account().ok_or_else(|| {
        refused(
            StatusCode::FORBIDDEN,
            "membership_required",
            "An anonymous session has no account. Sign in, or use a workspace \
             member's API key.",
        )
    })
}

/// Authorize the account's membership in `workspace`, mapping the
/// store's refusals onto the shared envelope.
pub(crate) fn member(
    state: &ServeState,
    account: &str,
    workspace: &str,
) -> Result<MemberRef, Response> {
    accounts_store(state)?
        .authorize(workspace, account)
        .map_err(accounts_refusal)
}

/// The accounts-store refusal as an HTTP answer.
fn accounts_refusal(refusal: tenancy::accounts::Refusal) -> Response {
    use tenancy::accounts::Refusal as R;
    let (status, code) = match &refusal {
        R::Store(_) => (StatusCode::SERVICE_UNAVAILABLE, "accounts_unavailable"),
        R::Authentication(_) => (StatusCode::UNAUTHORIZED, "unauthenticated"),
        R::UnknownAccount(_) => (StatusCode::NOT_FOUND, "unknown_account"),
        R::UnknownWorkspace(_) => (StatusCode::NOT_FOUND, "unknown_workspace"),
        R::UnknownPrincipal(_) => (StatusCode::FORBIDDEN, "unknown_principal"),
        R::PrincipalTaken { .. } => (StatusCode::CONFLICT, "principal_taken"),
        R::NotMember { .. } => (StatusCode::FORBIDDEN, "not_member"),
        R::Revoked { .. } => (StatusCode::FORBIDDEN, "membership_revoked"),
        R::AlreadyMember { .. } => (StatusCode::CONFLICT, "already_member"),
        R::Forbidden { .. } => (StatusCode::FORBIDDEN, "forbidden"),
        R::PersonalWorkspace(_) => (StatusCode::BAD_REQUEST, "personal_workspace"),
        R::SeatLimit { .. } => (StatusCode::CONFLICT, "seat_limit"),
        R::SeatsBelowMembers { .. } => (StatusCode::CONFLICT, "seats_below_members"),
        R::LastOwner { .. } => (StatusCode::CONFLICT, "last_owner"),
        R::OwnerByInvitation { .. } => (StatusCode::BAD_REQUEST, "owner_by_invitation"),
        R::OwnershipByTransfer { .. } => (StatusCode::BAD_REQUEST, "ownership_by_transfer"),
        R::MalformedInvitation => (StatusCode::BAD_REQUEST, "malformed_invitation"),
        R::UnknownInvitation(_) | R::WrongSecret(_) => {
            (StatusCode::FORBIDDEN, "invalid_invitation")
        }
        R::InvitationClosed { .. } => (StatusCode::CONFLICT, "invitation_closed"),
        R::InvitationExpired(_) => (StatusCode::FORBIDDEN, "invitation_expired"),
        R::TenantMismatch => (StatusCode::FORBIDDEN, "tenant_mismatch"),
        R::EmptyField(field) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "empty_field",
                format!("`{field}` is required"),
            );
        }
    };
    refused(status, code, refusal.to_string())
}

/// The session-store refusal as an HTTP answer.
fn sessions_refusal(refusal: sessions::Refusal) -> Response {
    use sessions::Refusal as R;
    let (status, code) = match &refusal {
        R::Store(_) | R::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "sessions_unavailable"),
        R::SignInDenied => (StatusCode::UNAUTHORIZED, "sign_in_denied"),
        R::MalformedDigest(_) => (StatusCode::BAD_REQUEST, "malformed_digest"),
        R::UnknownUser(_) => (StatusCode::NOT_FOUND, "unknown_user"),
        R::UnknownSession(_) => (StatusCode::UNAUTHORIZED, "unauthenticated"),
        R::SessionClosed { .. } => (StatusCode::UNAUTHORIZED, "session_closed"),
        R::AnonymousSession => (StatusCode::FORBIDDEN, "membership_required"),
        R::UnknownRecovery => (StatusCode::FORBIDDEN, "invalid_recovery"),
        R::RecoveryClosed { .. } => (StatusCode::CONFLICT, "recovery_closed"),
        R::RecoveryExpired => (StatusCode::FORBIDDEN, "recovery_expired"),
        R::DuplicateBudget { .. } => (StatusCode::CONFLICT, "duplicate_budget"),
        R::UnknownBudget { .. } => (StatusCode::NOT_FOUND, "unknown_budget"),
        R::AnonymousBudgetExpired { .. } => (StatusCode::FORBIDDEN, "budget_expired"),
        R::AnonymousBudgetSpent { .. } => (StatusCode::FORBIDDEN, "budget_spent"),
        R::AnonymousSessionCapped { .. } => (StatusCode::FORBIDDEN, "session_capped"),
        R::Membership(_) => (StatusCode::FORBIDDEN, "forbidden"),
    };
    refused(status, code, refusal.to_string())
}

/// The key-store failure as an HTTP answer.
fn keys_refusal(trouble: keys::KeyTrouble) -> Response {
    let (status, code) = match &trouble {
        keys::KeyTrouble::UnknownKey(_) => (StatusCode::NOT_FOUND, "unknown_key"),
        keys::KeyTrouble::UnknownTenant(_) => (StatusCode::NOT_FOUND, "unknown_tenant"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "keys_unavailable"),
    };
    refused(status, code, trouble.to_string())
}

/// Append an access event — best effort: an audit write that cannot
/// land must not strand the operation it describes, and the refusal it
/// would return says the store is down, which the operation's own
/// answer will have said already.
pub(crate) fn record(
    state: &ServeState,
    principal: &Principal,
    action: &str,
    workspace: Option<&str>,
    detail: Option<String>,
) {
    if let Ok(sessions) = sessions_store(state) {
        sessions
            .record(
                principal.actor(),
                action,
                workspace,
                principal.session(),
                detail,
            )
            .ok();
    }
}

/// The current time as Unix seconds.
pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// A required string field of a JSON body.
pub(crate) fn field<'a>(body: &'a Value, name: &str) -> Result<&'a str, Response> {
    body.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            refused(
                StatusCode::BAD_REQUEST,
                "empty_field",
                format!("`{name}` is required"),
            )
        })
}

/// The configured accounts block — present by construction on every
/// mounted route.
fn accounts_config(state: &ServeState) -> &crate::config::Accounts {
    state
        .config
        .accounts
        .as_ref()
        .expect("account routes mount only under `accounts` config")
}

/// `POST /v1/sessions` — sign in.
///
/// With an `oak_` bearer, mints a session for the account the key is
/// bound to. With no credential, mints an anonymous session against
/// the operator-funded budget — the bounded public lane, refused when
/// the deployment funds none.
async fn sign_in(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if headers.get("authorization").is_none() {
        return anonymous_sign_in(state).await;
    }
    let token = match bearer(&headers) {
        Ok(token) => token,
        Err(response) => return response,
    };
    if token.starts_with("sess_") {
        return refused(
            StatusCode::BAD_REQUEST,
            "already_signed_in",
            "You're already signed in with a session token. To sign in again, \
             send an `oak_` API key or no `Authorization` header.",
        );
    }
    let registry = match registry(&state) {
        Ok(registry) => registry,
        Err(response) => return response,
    };
    let authenticated = match keys::authenticate(&state.dir, registry.manifest(), &token) {
        Ok(authenticated) => authenticated,
        Err(cause) => {
            return refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                format!("Your API key was rejected: {cause}"),
            );
        }
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let account = match accounts
        .account_of_principal(&format!("key:{}", authenticated.key_id))
        .map_err(|t| unavailable("accounts_unavailable", t.to_string()))
    {
        Ok(Some(account)) => account,
        Ok(None) => {
            return refused(
                StatusCode::FORBIDDEN,
                "no_account",
                "This API key is valid but isn't linked to an account.",
            );
        }
        Err(response) => return response,
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    // The credential record tracks which key secret last authenticated
    // the account: a rotated key's digest differs, and recording the
    // change ends the sessions the old secret minted.
    let key_digest = match keys::load(&state.dir).ok().and_then(|store| {
        store
            .keys
            .get(&authenticated.key_id)
            .map(|key| key.digest.clone())
    }) {
        Some(digest) => digest,
        None => {
            return unavailable(
                "keys_unavailable",
                "The service can't read the record for this API key.",
            );
        }
    };
    let issued = match sessions.mutate(|book, access, now| {
        if book
            .credentials
            .get(&UserId::from(account.as_str()))
            .is_none_or(|credential| credential.digest != key_digest)
        {
            book.set_credential(UserId::from(account.as_str()), key_digest.clone(), now)?;
        }
        let issued = book.issue(UserId::from(account.as_str()), now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: account.clone(),
                action: "sign-in".to_string(),
                workspace: None,
                session: Some(issued.session.id.as_str().to_string()),
                detail: Some(format!("key:{}", authenticated.key_id)),
            },
        );
        Ok(issued)
    }) {
        Ok(issued) => issued,
        Err(refusal) => return sessions_refusal(refusal),
    };
    answered(
        StatusCode::OK,
        json!({
            "session": {
                "id": issued.session.id.as_str(),
                "kind": "user",
                "account": account,
                "created_at": issued.session.created_at,
                "expires_at": issued.session.expires_at,
            },
            "token": issued.once,
        }),
    )
}

/// The anonymous half of sign-in: a session against the funded budget.
async fn anonymous_sign_in(state: Arc<ServeState>) -> Response {
    let Some(anonymous) = accounts_config(&state).anonymous.as_ref() else {
        return refused(
            StatusCode::FORBIDDEN,
            "anonymous_disabled",
            "This service doesn't offer free anonymous use. Sign in with an \
             `oak_` API key, or create an account.",
        );
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    let (bound, cap, ttl, workspace) = (
        anonymous.bound,
        anonymous.session_cap,
        anonymous.ttl_secs,
        anonymous.workspace.clone(),
    );
    let issued = match sessions.mutate(|book, access, now| {
        if !book.onboarding.contains_key(ANONYMOUS_BUDGET) {
            book.fund_anonymous(sessions::Onboarding {
                id: ANONYMOUS_BUDGET.to_string(),
                workspace: tenancy::workspaces::WorkspaceId::from(workspace.as_str()),
                funded_by: "operator".to_string(),
                bound,
                spent: 0,
                session_cap: cap,
                sessions: Default::default(),
                funded_at: now,
                expires_at: now + ttl,
            })?;
        }
        let issued = book.issue_anonymous(now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: "anonymous".to_string(),
                action: "sign-in".to_string(),
                workspace: Some(workspace.clone()),
                session: Some(issued.session.id.as_str().to_string()),
                detail: Some(format!("budget:{ANONYMOUS_BUDGET}")),
            },
        );
        Ok(issued)
    }) {
        Ok(issued) => issued,
        Err(refusal) => return sessions_refusal(refusal),
    };
    answered(
        StatusCode::OK,
        json!({
            "session": {
                "id": issued.session.id.as_str(),
                "kind": "anonymous",
                "created_at": issued.session.created_at,
                "expires_at": issued.session.expires_at,
            },
            "token": issued.once,
        }),
    )
}

/// `GET /v1/session` — the session the bearer token names: its kind,
/// its account when it holds one, and its deadline. The id is the
/// token's digest — the token itself is never stored or returned.
async fn session_status(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let Some(session_id) = principal.session() else {
        return refused(
            StatusCode::BAD_REQUEST,
            "not_a_session",
            "You sent an `oak_` API key, not a session token, so there's no \
             session to show.",
        );
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    let store = match sessions.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("sessions_unavailable", trouble.to_string()),
    };
    let session = store
        .book
        .session(&SessionId(session_id.to_string()))
        .expect("a resolved principal's session stands in the book");
    let mut body = json!({
        "session": {
            "id": session.id.as_str(),
            "kind": session.kind,
            "created_at": session.created_at,
            "expires_at": session.expires_at,
            "state": session.standing(unix_now()).to_string(),
        }
    });
    if let Some(account) = principal.account() {
        body["session"]["account"] = json!(account);
    }
    if let Some(budget) = store.book.onboarding.get(ANONYMOUS_BUDGET) {
        body["budget"] = json!({
            "id": budget.id,
            "remaining": budget.remaining(),
            "session_drawn": budget.sessions.get(session.id.as_str()).copied().unwrap_or(0),
            "session_cap": budget.session_cap,
            "expires_at": budget.expires_at,
        });
    }
    answered(StatusCode::OK, body)
}

/// `DELETE /v1/session` — logout. Ending ends the session the bearer
/// names; an `oak_` key has no session to end.
async fn logout(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let Some(session_id) = principal.session().map(str::to_string) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "not_a_session",
            "You sent an `oak_` API key, not a session token, so there's no \
             session to sign out of. To stop the key from working, revoke it.",
        );
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    match sessions.mutate(|book, access, now| {
        let session = book.logout(&SessionId(session_id.clone()), now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: principal.actor().to_string(),
                action: "logout".to_string(),
                workspace: None,
                session: Some(session.id.as_str().to_string()),
                detail: None,
            },
        );
        Ok(session)
    }) {
        Ok(session) => answered(
            StatusCode::OK,
            json!({"session": {"id": session.id.as_str(), "state": "revoked"}}),
        ),
        Err(refusal) => sessions_refusal(refusal),
    }
}

/// `POST /v1/accounts` — self-serve sign-up: the account, its personal
/// workspace bound to the configured sign-up tenant, its first `oak_`
/// key, and its first session, in one call. The key secret and the
/// session token exist only in this response.
async fn sign_up(State(state): State<Arc<ServeState>>, Json(body): Json<Value>) -> Response {
    let Some(tenant) = accounts_config(&state).signup_tenant.clone() else {
        return refused(
            StatusCode::FORBIDDEN,
            "signup_disabled",
            "This service doesn't offer sign-up. Ask the operator to create \
             an account for you.",
        );
    };
    let label = match field(&body, "label") {
        Ok(label) => label.to_string(),
        Err(response) => return response,
    };
    if label.len() > 256 {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "`label` is bounded at 256 characters",
        );
    }
    let registry = match registry(&state) {
        Ok(registry) => registry,
        Err(response) => return response,
    };
    if !registry.manifest().tenants.contains_key(&tenant) {
        return unavailable(
            "signup_unavailable",
            format!(
                "Sign-up isn't available: the sign-up account `{tenant}` isn't set up. Contact the operator."
            ),
        );
    }
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let account = match accounts.create_account(&label, &[]) {
        Ok(account) => account,
        Err(refusal) => return accounts_refusal(refusal),
    };
    let workspace = match body.get("workspace").and_then(Value::as_str) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => label.clone(),
    };
    let workspace = match accounts.create_workspace(
        &account.id,
        &workspace,
        tenancy::WorkspaceKind::Personal,
        &tenant,
        None,
    ) {
        Ok(workspace) => workspace,
        Err(refusal) => return accounts_refusal(refusal),
    };
    let issued = match keys::issue_scoped(
        &state.dir,
        registry.manifest(),
        &tenant,
        Some("default"),
        None,
    ) {
        Ok(issued) => issued,
        Err(trouble) => return keys_refusal(trouble),
    };
    let principal_ref = format!("key:{}", issued.key.id);
    if let Err(refusal) =
        accounts.update_principals(&account.id, std::slice::from_ref(&principal_ref))
    {
        return accounts_refusal(refusal);
    }
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    let session = match sessions.mutate(|book, access, now| {
        book.set_credential(
            UserId::from(account.id.as_str()),
            issued.key.digest.clone(),
            now,
        )?;
        let issued_session = book.issue(UserId::from(account.id.as_str()), now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: account.id.clone(),
                action: "sign-up".to_string(),
                workspace: Some(workspace.id.clone()),
                session: Some(issued_session.session.id.as_str().to_string()),
                detail: Some(format!("tenant:{tenant} key:{}", issued.key.id)),
            },
        );
        Ok(issued_session)
    }) {
        Ok(session) => session,
        Err(refusal) => return sessions_refusal(refusal),
    };
    answered(
        StatusCode::CREATED,
        json!({
            "account": {"id": account.id, "label": account.label},
            "workspace": {
                "id": workspace.id,
                "name": workspace.name,
                "kind": "personal",
                "tenant": workspace.tenant,
                "role": "owner",
            },
            "key": {
                "id": issued.key.id,
                "name": issued.key.name,
                "tenant": issued.key.tenant,
            },
            "key_token": issued.token,
            "session": {
                "id": session.session.id.as_str(),
                "kind": "user",
                "expires_at": session.session.expires_at,
            },
            "session_token": session.once,
        }),
    )
}

/// `GET /v1/account` — the caller's account and every workspace it
/// belongs to. Workspace switching is the client choosing one of these
/// ids for `X-Workspace-Id`; this read is the menu it chooses from.
async fn account_view(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    let record = match store.accounts.get(&account) {
        Some(record) => record,
        None => {
            return refused(
                StatusCode::NOT_FOUND,
                "unknown_account",
                "No account matches your API key or session.",
            );
        }
    };
    let workspaces: Vec<Value> = store
        .workspaces
        .values()
        .filter_map(|workspace| {
            let membership = workspace.members.get(&account)?;
            (membership.status == tenancy::MemberStatus::Active).then(|| {
                json!({
                    "id": workspace.id,
                    "name": workspace.name,
                    "kind": workspace.kind,
                    "tenant": workspace.tenant,
                    "role": membership.role,
                    "seats": workspace.seats,
                    "members": workspace.members.values()
                        .filter(|m| m.status == tenancy::MemberStatus::Active)
                        .count(),
                })
            })
        })
        .collect();
    answered(
        StatusCode::OK,
        json!({
            "account": {
                "id": record.id,
                "label": record.label,
                "principals": record.principals,
                "created": record.created,
            },
            "workspaces": workspaces,
        }),
    )
}

/// `GET /v1/account/access` — the caller's own access history.
async fn own_access(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    let store = match sessions.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("sessions_unavailable", trouble.to_string()),
    };
    let events: Vec<&sessions::Access> = store
        .access
        .iter()
        .filter(|event| event.actor == account)
        .collect();
    answered(StatusCode::OK, json!({"access": events}))
}

/// `POST /v1/workspaces` — `{name, seats?}` opens an organization
/// workspace on the sign-up tenant, owned by the caller. Personal
/// workspaces come only from sign-up — the collaboration surface is
/// the kind a member may mint.
async fn workspace_create(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let Some(tenant) = accounts_config(&state).signup_tenant.clone() else {
        return refused(
            StatusCode::FORBIDDEN,
            "signup_disabled",
            "This service doesn't let you create organization workspaces. \
             Ask the operator to create one for you.",
        );
    };
    let name = match field(&body, "name") {
        Ok(name) => name.to_string(),
        Err(response) => return response,
    };
    let seats = match body.get("seats") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_u64().and_then(|seats| u32::try_from(seats).ok()) {
            Some(seats) => Some(seats),
            None => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "`seats` must be a non-negative integer or null",
                );
            }
        },
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.create_workspace(
        &account,
        &name,
        tenancy::WorkspaceKind::Organization,
        &tenant,
        seats,
    ) {
        Ok(workspace) => {
            record(
                &state,
                &principal,
                "workspace-create",
                Some(&workspace.id),
                Some(format!("tenant:{tenant}")),
            );
            answered(
                StatusCode::CREATED,
                json!({
                    "workspace": {
                        "id": workspace.id,
                        "name": workspace.name,
                        "kind": "organization",
                        "tenant": workspace.tenant,
                        "seats": workspace.seats,
                        "role": "owner",
                    }
                }),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `GET /v1/workspaces/{ws}` — the workspace as the member may see it:
/// members and seats for everyone, live invitations for admins.
async fn workspace_view(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let membership = match member(&state, &account, &workspace) {
        Ok(membership) => membership,
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    let ws = store
        .workspaces
        .get(&workspace)
        .expect("authorize read the workspace");
    let members: Vec<Value> = ws
        .members
        .values()
        .map(|membership| {
            json!({
                "account": membership.account,
                "role": membership.role,
                "status": membership.status,
                "granted": membership.granted,
            })
        })
        .collect();
    let mut body = json!({
        "workspace": {
            "id": ws.id,
            "name": ws.name,
            "kind": ws.kind,
            "tenant": ws.tenant,
            "seats": ws.seats,
            "members_epoch": ws.members_epoch,
        },
        "members": members,
        "role": membership.role,
    });
    if membership.role >= Role::Admin {
        let invitations: Vec<Value> = store
            .invitations
            .values()
            .filter(|invitation| invitation.workspace == workspace)
            .map(|invitation| {
                json!({
                    "id": invitation.id,
                    "role": invitation.role,
                    "status": invitation.status,
                    "invited_by": invitation.invited_by,
                    "created": invitation.created,
                    "expires_unix": invitation.expires_unix,
                    "accepted_by": invitation.accepted_by,
                })
            })
            .collect();
        body["invitations"] = json!(invitations);
    }
    answered(StatusCode::OK, body)
}

/// `PATCH /v1/workspaces/{ws}` — `{name?, seats?}` for the owner.
async fn workspace_update(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let mut changed = Vec::new();
    if let Some(name) = body.get("name") {
        let Some(name) = name.as_str().filter(|name| !name.is_empty()) else {
            return refused(
                StatusCode::BAD_REQUEST,
                "empty_field",
                "`name` must be a non-empty string",
            );
        };
        match accounts.rename(&account, &workspace, name) {
            Ok(_) => changed.push("name"),
            Err(refusal) => return accounts_refusal(refusal),
        }
    }
    if let Some(seats) = body.get("seats") {
        let seats = if seats.is_null() {
            None
        } else {
            match seats.as_u64().and_then(|seats| u32::try_from(seats).ok()) {
                Some(seats) => Some(seats),
                None => {
                    return refused(
                        StatusCode::BAD_REQUEST,
                        "invalid_request",
                        "`seats` must be a non-negative integer or null",
                    );
                }
            }
        };
        match accounts.set_seats(&account, &workspace, seats) {
            Ok(_) => changed.push("seats"),
            Err(refusal) => return accounts_refusal(refusal),
        }
    }
    if changed.is_empty() {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Set `name`, `seats`, or both.",
        );
    }
    record(
        &state,
        &principal,
        "workspace-update",
        Some(&workspace),
        Some(changed.join(",")),
    );
    let ws = match accounts.workspace(&workspace) {
        Ok(Some(ws)) => ws,
        Ok(None) => {
            return refused(
                StatusCode::NOT_FOUND,
                "unknown_workspace",
                "This workspace no longer exists.",
            );
        }
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    answered(
        StatusCode::OK,
        json!({
            "workspace": {
                "id": ws.id, "name": ws.name, "kind": ws.kind,
                "tenant": ws.tenant, "seats": ws.seats,
            },
            "changed": changed,
        }),
    )
}

/// `POST /v1/workspaces/{ws}/invitations` — `{role, ttl_secs?}` issues
/// an invitation token. The token is returned once; delivery is the
/// inviter's, out of band.
async fn invitation_issue(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let role = match field(&body, "role").and_then(|role| match role {
        "admin" => Ok(Role::Admin),
        "member" => Ok(Role::Member),
        "owner" => Err(refused(
            StatusCode::BAD_REQUEST,
            "owner_by_invitation",
            "An invitation can make someone an `admin` or a `member`. To make \
             someone the owner, transfer ownership instead.",
        )),
        _ => Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "`role` must be `admin` or `member`",
        )),
    }) {
        Ok(role) => role,
        Err(response) => return response,
    };
    let ttl = match body.get("ttl_secs") {
        None | Some(Value::Null) => INVITE_TTL_SECS,
        Some(value) => match value.as_u64() {
            Some(ttl) if ttl > 0 && ttl <= INVITE_TTL_MAX => ttl,
            _ => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    format!("`ttl_secs` must be between 1 and {INVITE_TTL_MAX}"),
                );
            }
        },
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.invite(&account, &workspace, role, ttl) {
        Ok(invited) => {
            record(
                &state,
                &principal,
                "invite",
                Some(&workspace),
                Some(format!("{} role:{role}", invited.invitation.id)),
            );
            answered(
                StatusCode::CREATED,
                json!({
                    "invitation": {
                        "id": invited.invitation.id,
                        "workspace": invited.invitation.workspace,
                        "role": invited.invitation.role,
                        "expires_unix": invited.invitation.expires_unix,
                    },
                    "token": invited.token,
                }),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `DELETE /v1/workspaces/{ws}/invitations/{inv}` — withdraw a pending
/// invitation. The record stays; the state is the answer.
async fn invitation_revoke(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, invitation)): Path<(String, String)>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.revoke_invitation(&account, &workspace, &invitation) {
        Ok(record_invitation) => {
            record(
                &state,
                &principal,
                "invite-revoke",
                Some(&workspace),
                Some(invitation),
            );
            answered(
                StatusCode::OK,
                json!({"invitation": {"id": record_invitation.id, "status": record_invitation.status}}),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `POST /v1/invitations/accept` — `{token}` joins the caller's account
/// to the workspace the token names.
async fn invitation_accept(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let token = match field(&body, "token") {
        Ok(token) => token.to_string(),
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    // The token's id names the invitation — and the workspace — the
    // membership lands in.
    let invitation_id = token
        .strip_prefix("inv_")
        .and_then(|body| body.split('.').next())
        .unwrap_or_default()
        .to_string();
    match accounts.accept(&account, &token) {
        Ok(membership) => {
            let store = accounts.store().ok();
            let joined = store
                .as_ref()
                .and_then(|store| store.invitations.get(&invitation_id))
                .map(|invitation| invitation.workspace.clone())
                .unwrap_or_default();
            record(
                &state,
                &principal,
                "invite-accept",
                Some(&joined),
                Some(format!("role:{}", membership.role)),
            );
            answered(
                StatusCode::OK,
                json!({
                    "membership": {
                        "workspace": joined,
                        "account": membership.account,
                        "role": membership.role,
                        "granted": membership.granted,
                    }
                }),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `PATCH /v1/workspaces/{ws}/members/{account}` — `{role}` moves a
/// member between `admin` and `member`. Ownership never passes through
/// this route.
async fn member_role(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, target)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let actor = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let role = match field(&body, "role").and_then(|role| match role {
        "admin" => Ok(Role::Admin),
        "member" => Ok(Role::Member),
        _ => Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "`role` must be `admin` or `member` — ownership moves only \
             through `POST .../transfer`",
        )),
    }) {
        Ok(role) => role,
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.set_role(&actor, &workspace, &target, role) {
        Ok(membership) => {
            record(
                &state,
                &principal,
                "set-role",
                Some(&workspace),
                Some(format!("{target} role:{role}")),
            );
            answered(
                StatusCode::OK,
                json!({
                    "membership": {
                        "account": membership.account,
                        "role": membership.role,
                        "status": membership.status,
                    }
                }),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `DELETE /v1/workspaces/{ws}/members/{account}` — remove a member.
///
/// The membership record stays as `revoked`, the member's sessions end
/// in the same call, and their keys refuse on the very next
/// authentication — access is a state, never a deletion.
async fn member_remove(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, target)): Path<(String, String)>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let actor = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.remove_member(&actor, &workspace, &target) {
        Ok(membership) => {
            if let Ok(sessions) = sessions_store(&state) {
                sessions
                    .mutate(|book, access, now| {
                        book.revoke_all(&UserId::from(target.as_str()), now);
                        sessions::push_access(
                            access,
                            sessions::Access {
                                at: now,
                                actor: actor.clone(),
                                action: "member-remove".to_string(),
                                workspace: Some(workspace.clone()),
                                session: principal.session().map(str::to_string),
                                detail: Some(target.clone()),
                            },
                        );
                        Ok(())
                    })
                    .ok();
            }
            answered(
                StatusCode::OK,
                json!({
                    "membership": {
                        "account": membership.account,
                        "role": membership.role,
                        "status": membership.status,
                    }
                }),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `POST /v1/workspaces/{ws}/transfer` — `{account}` hands ownership
/// to another active member; the caller becomes an admin in the same
/// revision.
async fn transfer(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let actor = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let target = match field(&body, "account") {
        Ok(target) => target.to_string(),
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.transfer_ownership(&actor, &workspace, &target) {
        Ok(()) => {
            record(
                &state,
                &principal,
                "transfer-ownership",
                Some(&workspace),
                Some(target.clone()),
            );
            answered(
                StatusCode::OK,
                json!({
                    "workspace": workspace,
                    "owner": target,
                }),
            )
        }
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `POST /v1/workspaces/{ws}/recovery` — `{account}` issues a recovery
/// token for a member. The admin delivers it out of band; the member
/// redeems it at `/v1/recovery/redeem`.
async fn recovery_issue(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let actor = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let target = match field(&body, "account") {
        Ok(target) => target.to_string(),
        Err(response) => return response,
    };
    let membership = match member(&state, &actor, &workspace) {
        Ok(membership) => membership,
        Err(response) => return response,
    };
    if membership.role < Role::Admin {
        return refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Only a workspace admin or the owner can create a recovery token.",
        );
    }
    // The member must hold an active membership and a bound key —
    // recovery rotates the key, so a member without one has nothing
    // to recover.
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    let Some(ws) = store.workspaces.get(&workspace) else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_workspace",
            "This workspace no longer exists.",
        );
    };
    match ws.members.get(&target) {
        Some(membership) if membership.status == tenancy::MemberStatus::Active => {}
        Some(_) => {
            return refused(
                StatusCode::FORBIDDEN,
                "membership_revoked",
                "This member was removed from the workspace, so there's no access to recover.",
            );
        }
        None => {
            return refused(
                StatusCode::FORBIDDEN,
                "not_member",
                "This account isn't a member of this workspace.",
            );
        }
    }
    let Some(account_record) = store.accounts.get(&target) else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_account",
            "This account no longer exists.",
        );
    };
    let Some(key_id) = account_record
        .principals
        .iter()
        .find_map(|principal| principal.strip_prefix("key:"))
    else {
        return refused(
            StatusCode::CONFLICT,
            "no_credential",
            "This member has no API key, so there's no key to replace. \
             Recovery works only for members who have a key.",
        );
    };
    let key_digest = match keys::load(&state.dir)
        .ok()
        .and_then(|store| store.keys.get(key_id).map(|key| key.digest.clone()))
    {
        Some(digest) => digest,
        None => {
            return refused(
                StatusCode::CONFLICT,
                "no_credential",
                "The service can't read the record for this member's API key.",
            );
        }
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    match sessions.mutate(|book, access, now| {
        let user = UserId::from(target.as_str());
        if !book.credentials.contains_key(&user) {
            book.set_credential(user.clone(), key_digest.clone(), now)?;
        }
        let issued = book.issue_recovery(&user, now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: actor.clone(),
                action: "recovery-issue".to_string(),
                workspace: Some(workspace.clone()),
                session: principal.session().map(str::to_string),
                detail: Some(target.clone()),
            },
        );
        Ok(issued)
    }) {
        Ok(issued) => answered(
            StatusCode::CREATED,
            json!({
                "recovery": {
                    "user": issued.recovery.user.as_str(),
                    "issued_at": issued.recovery.issued_at,
                    "expires_at": issued.recovery.expires_at,
                },
                "token": issued.once,
            }),
        ),
        Err(refusal) => sessions_refusal(refusal),
    }
}

/// `POST /v1/recovery/redeem` — `{token}` consumes a recovery token:
/// the member's bound key rotates, every session the old credential
/// minted ends, and the new key secret leaves in this response only.
async fn recovery_redeem(
    State(state): State<Arc<ServeState>>,
    Json(body): Json<Value>,
) -> Response {
    let token = match field(&body, "token") {
        Ok(token) => token.to_string(),
        Err(response) => return response,
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    // The token's standing is checked before the key rotates — a spent
    // or expired token must not move the credential.
    let user = {
        let store = match sessions.store() {
            Ok(store) => store,
            Err(trouble) => return unavailable("sessions_unavailable", trouble.to_string()),
        };
        let now = unix_now();
        match store.book.recovery_of_token(&token) {
            Some(recovery)
                if recovery.state == sessions::RecoveryState::Pending
                    && recovery.expires_at > now =>
            {
                recovery.user.clone()
            }
            Some(recovery) if recovery.expires_at <= now => {
                return refused(
                    StatusCode::FORBIDDEN,
                    "recovery_expired",
                    "This recovery token has expired. Ask a workspace admin for a new one.",
                );
            }
            Some(recovery) => {
                return refused(
                    StatusCode::CONFLICT,
                    "recovery_closed",
                    format!(
                        "This recovery token is already {}. Each recovery token works only once.",
                        recovery.state
                    ),
                );
            }
            None => {
                return refused(
                    StatusCode::FORBIDDEN,
                    "invalid_recovery",
                    "This recovery token isn't recognized.",
                );
            }
        }
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let account_record = match accounts.store() {
        Ok(store) => store.accounts.get(user.as_str()).cloned(),
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    let Some(account_record) = account_record else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_account",
            "The account this recovery token is for no longer exists.",
        );
    };
    let Some(key_id) = account_record
        .principals
        .iter()
        .find_map(|principal| principal.strip_prefix("key:"))
        .map(str::to_string)
    else {
        return refused(
            StatusCode::CONFLICT,
            "no_credential",
            "This account has no API key to replace.",
        );
    };
    let issued = match keys::rotate(&state.dir, &key_id) {
        Ok(issued) => issued,
        Err(trouble) => return keys_refusal(trouble),
    };
    if let Err(refusal) =
        accounts.update_principals(user.as_str(), &[format!("key:{}", issued.key.id)])
    {
        return accounts_refusal(refusal);
    }
    match sessions.mutate(|book, access, now| {
        let consumed = book.redeem_recovery(&token, now)?;
        book.set_credential(consumed.clone(), issued.key.digest.clone(), now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: consumed.as_str().to_string(),
                action: "recovery-redeem".to_string(),
                workspace: None,
                session: None,
                detail: Some(format!("key:{}", issued.key.id)),
            },
        );
        Ok(())
    }) {
        Ok(()) => answered(
            StatusCode::OK,
            json!({
                "account": account_record.id,
                "key": {"id": issued.key.id, "tenant": issued.key.tenant},
                "key_token": issued.token,
            }),
        ),
        Err(refusal) => sessions_refusal(refusal),
    }
}

/// `GET /v1/workspaces/{ws}/access` — the workspace's access history.
/// Members read their own events; admins read the workspace's.
async fn workspace_access(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let membership = match member(&state, &account, &workspace) {
        Ok(membership) => membership,
        Err(response) => return response,
    };
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    let store = match sessions.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("sessions_unavailable", trouble.to_string()),
    };
    let admin = membership.role >= Role::Admin;
    let events: Vec<&sessions::Access> = store
        .access
        .iter()
        .filter(|event| {
            event.workspace.as_deref() == Some(workspace.as_str())
                && (admin || event.actor == account)
        })
        .collect();
    answered(StatusCode::OK, json!({"access": events}))
}

/// `GET /v1/workspaces/{ws}/keys` — the workspace's keys: a member
/// reads their own, an admin reads all of them with attribution. No
/// secret field exists to leak — the records are ids, digests, and
/// states.
async fn keys_list(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    let membership = match member(&state, &account, &workspace) {
        Ok(membership) => membership,
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    let Some(ws) = store.workspaces.get(&workspace) else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_workspace",
            "This workspace no longer exists.",
        );
    };
    let key_store = match keys::load(&state.dir) {
        Ok(store) => store,
        Err(trouble) => return keys_refusal(trouble),
    };
    let admin = membership.role >= Role::Admin;
    let mut keys_out = Vec::new();
    for key in key_store.keys.values() {
        if key.tenant != ws.tenant {
            continue;
        }
        let owner = store
            .accounts
            .values()
            .find(|account| {
                account
                    .principals
                    .iter()
                    .any(|principal| principal == &format!("key:{}", key.id))
            })
            .map(|account| account.id.clone());
        if !admin && owner.as_deref() != Some(account.as_str()) {
            continue;
        }
        keys_out.push(json!({
            "id": key.id,
            "name": key.name,
            "status": key.status,
            "scopes": key.scopes,
            "tenant": key.tenant,
            "created": key.created,
            "rotated_from": key.rotated_from,
            "copied_from": key.copied_from,
            "account": owner,
        }));
    }
    answered(StatusCode::OK, json!({"keys": keys_out}))
}

/// `POST /v1/workspaces/{ws}/keys` — `{name?, scopes?}` issues a key
/// on the workspace's tenant, bound to the caller's account. The
/// secret leaves in this response only.
async fn key_issue(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    if let Err(response) = member(&state, &account, &workspace) {
        return response;
    }
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => return unavailable("accounts_unavailable", trouble.to_string()),
    };
    let Some(ws) = store.workspaces.get(&workspace) else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_workspace",
            "This workspace no longer exists.",
        );
    };
    let name = match body.get("name") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_str().filter(|name| !name.is_empty()) {
            Some(name) => Some(name.to_string()),
            None => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "empty_field",
                    "`name` must be a non-empty string",
                );
            }
        },
    };
    let scopes = match body.get("scopes") {
        None | Some(Value::Null) => None,
        Some(value) => match serde_json::from_value::<keys::Scopes>(value.clone()) {
            Ok(scopes) => Some(scopes),
            Err(error) => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    format!("`scopes` must name `models` and `actions` lists: {error}"),
                );
            }
        },
    };
    let registry = match registry(&state) {
        Ok(registry) => registry,
        Err(response) => return response,
    };
    let issued = match keys::issue_scoped(
        &state.dir,
        registry.manifest(),
        &ws.tenant,
        name.as_deref(),
        scopes,
    ) {
        Ok(issued) => issued,
        Err(trouble) => return keys_refusal(trouble),
    };
    let mut principals = store
        .accounts
        .get(&account)
        .map(|record| record.principals.clone())
        .unwrap_or_default();
    principals.push(format!("key:{}", issued.key.id));
    if let Err(refusal) = accounts.update_principals(&account, &principals) {
        return accounts_refusal(refusal);
    }
    record(
        &state,
        &principal,
        "key-issue",
        Some(&workspace),
        Some(format!(
            "key:{} name:{}",
            issued.key.id,
            name.unwrap_or_default()
        )),
    );
    answered(
        StatusCode::CREATED,
        json!({
            "key": {
                "id": issued.key.id,
                "name": issued.key.name,
                "tenant": issued.key.tenant,
                "scopes": issued.key.scopes,
                "status": issued.key.status,
            },
            "key_token": issued.token,
        }),
    )
}

/// Everything a single-key operation resolves before it runs: the
/// caller's principal and account, their membership in the named
/// workspace, and the key record — checked to belong to the
/// workspace's tenant and to be one the caller may touch.
struct KeyContext {
    principal: Principal,
    account: String,
    /// The key's bound owner — the account the `key:<id>` principal
    /// resolves to, when it resolves. An operator-issued key may name
    /// no account.
    owner: Option<String>,
}

/// Resolve the shared preconditions of a single-key operation.
fn key_context(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
    key_id: &str,
) -> Result<KeyContext, Response> {
    let principal = principal(state, headers)?;
    let account = member_account(&principal)?.to_string();
    let membership = member(state, &account, workspace)?;
    let accounts = accounts_store(state)?;
    let store = accounts
        .store()
        .map_err(|t| unavailable("accounts_unavailable", t.to_string()))?;
    let Some(ws) = store.workspaces.get(workspace) else {
        return Err(refused(
            StatusCode::NOT_FOUND,
            "unknown_workspace",
            "This workspace no longer exists.",
        ));
    };
    let key_store = keys::load(&state.dir).map_err(keys_refusal)?;
    let key = key_store.keys.get(key_id).ok_or_else(|| {
        refused(
            StatusCode::NOT_FOUND,
            "unknown_key",
            "No API key has this ID.",
        )
    })?;
    if key.tenant != ws.tenant {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "tenant_mismatch",
            "This API key doesn't belong to this workspace.",
        ));
    }
    let owner = accounts
        .account_of_principal(&format!("key:{key_id}"))
        .map_err(|t| unavailable("accounts_unavailable", t.to_string()))?;
    let own = owner.as_deref() == Some(account.as_str());
    if !own && membership.role < Role::Admin {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This API key belongs to another member. Only a workspace admin \
             or the owner can manage it.",
        ));
    }
    Ok(KeyContext {
        principal,
        account,
        owner,
    })
}

/// A key record as the surface answers it — ids, names, and states,
/// never a secret.
fn key_view(key: &keys::Key) -> Value {
    json!({
        "id": key.id,
        "name": key.name,
        "tenant": key.tenant,
        "scopes": key.scopes,
        "status": key.status,
        "created": key.created,
        "rotated_from": key.rotated_from,
        "copied_from": key.copied_from,
    })
}

/// `POST .../keys/{key}/copy` — a fresh credential under the same
/// reach, bound to the caller's account. The secret leaves once.
async fn key_copy(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key_id)): Path<(String, String)>,
) -> Response {
    let context = match key_context(&state, &headers, &workspace, &key_id) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let issued = match keys::copy(&state.dir, &key_id) {
        Ok(issued) => issued,
        Err(trouble) => return keys_refusal(trouble),
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let mut principals = accounts
        .store()
        .ok()
        .and_then(|store| {
            store
                .accounts
                .get(&context.account)
                .map(|a| a.principals.clone())
        })
        .unwrap_or_default();
    principals.push(format!("key:{}", issued.key.id));
    if let Err(refusal) = accounts.update_principals(&context.account, &principals) {
        return accounts_refusal(refusal);
    }
    record(
        &state,
        &context.principal,
        "key-copy",
        Some(&workspace),
        Some(format!("key:{}", issued.key.id)),
    );
    answered(
        StatusCode::CREATED,
        json!({"key": key_view(&issued.key), "key_token": issued.token}),
    )
}

/// The shared shape of pause, resume, and revoke: resolve, run the
/// status write, answer the record.
fn key_status_op(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
    key_id: &str,
    action: &str,
    run: impl FnOnce() -> Result<(), keys::KeyTrouble>,
) -> Response {
    let context = match key_context(state, headers, workspace, key_id) {
        Ok(context) => context,
        Err(response) => return response,
    };
    if let Err(trouble) = run() {
        return keys_refusal(trouble);
    }
    let key = match keys::load(&state.dir)
        .ok()
        .and_then(|store| store.keys.get(key_id).cloned())
    {
        Some(key) => key,
        None => {
            return refused(
                StatusCode::NOT_FOUND,
                "unknown_key",
                "No API key has this ID.",
            );
        }
    };
    record(
        state,
        &context.principal,
        action,
        Some(workspace),
        Some(format!("key:{key_id}")),
    );
    answered(StatusCode::OK, json!({"key": key_view(&key)}))
}

/// `POST .../keys/{key}/pause` — hold the key without ending it.
async fn key_pause(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key_id)): Path<(String, String)>,
) -> Response {
    key_status_op(&state, &headers, &workspace, &key_id, "key-pause", || {
        keys::pause(&state.dir, &key_id)
    })
}

/// `POST .../keys/{key}/resume` — release a held key.
async fn key_resume(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key_id)): Path<(String, String)>,
) -> Response {
    key_status_op(&state, &headers, &workspace, &key_id, "key-resume", || {
        keys::resume(&state.dir, &key_id)
    })
}

/// `DELETE .../keys/{key}` — revoke the key. The record stays; the
/// state is the answer.
async fn key_revoke(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key_id)): Path<(String, String)>,
) -> Response {
    key_status_op(&state, &headers, &workspace, &key_id, "key-revoke", || {
        keys::revoke(&state.dir, &key_id)
    })
}

/// `POST .../keys/{key}/rotate` — a fresh id and secret for the same
/// reach. The new credential lands on the key owner's account before
/// the answer returns — rotation must never strand the membership the
/// old id resolved — and the new secret leaves in this response only.
async fn key_rotate(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path((workspace, key_id)): Path<(String, String)>,
) -> Response {
    let context = match key_context(&state, &headers, &workspace, &key_id) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let issued = match keys::rotate(&state.dir, &key_id) {
        Ok(issued) => issued,
        Err(trouble) => return keys_refusal(trouble),
    };
    if let Some(owner) = &context.owner {
        let accounts = match accounts_store(&state) {
            Ok(accounts) => accounts,
            Err(response) => return response,
        };
        let mut principals = accounts
            .store()
            .ok()
            .and_then(|store| store.accounts.get(owner).map(|a| a.principals.clone()))
            .unwrap_or_default();
        let old = format!("key:{key_id}");
        if let Some(position) = principals.iter().position(|p| p == &old) {
            principals[position] = format!("key:{}", issued.key.id);
        } else {
            principals.push(format!("key:{}", issued.key.id));
        }
        if let Err(refusal) = accounts.update_principals(owner, &principals) {
            return accounts_refusal(refusal);
        }
    }
    record(
        &state,
        &context.principal,
        "key-rotate",
        Some(&workspace),
        Some(format!("key:{}", issued.key.id)),
    );
    answered(
        StatusCode::OK,
        json!({"key": key_view(&issued.key), "key_token": issued.token}),
    )
}
