//! A small account service over real tenancy stores, for local fixtures
//! and tests: the routes the web server reads a session through
//! (`/v1/session`, `/v1/account`, `/v1/workspaces/{id}`), sign-out, and the
//! GitHub sign-in and link routes. Production serves the same GitHub
//! routes from the gateway (`crates/gateway/src/accounts.rs`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};
use tenancy::sessions::{SessionBook, SessionKind, SessionState, Sessions};
use tenancy::{Accounts, MemberStatus};

use crate::AuthError;
use crate::github::Github;
use crate::service::{self, CodeRequest};

struct Inner {
    dir: PathBuf,
    github: Github,
    tenant: String,
}

/// The local account service.
#[derive(Clone)]
pub struct LocalService(Arc<Inner>);

impl LocalService {
    /// Create fresh stores in `dir` (which must not hold any) and serve
    /// GitHub sign-in onto `tenant`.
    pub fn install(
        dir: &Path,
        github: Github,
        tenant: &str,
        session_ttl: u64,
    ) -> Result<Self, String> {
        Accounts::install(dir).map_err(|e| e.to_string())?;
        Sessions::install(dir, SessionBook::new(session_ttl, 3600)).map_err(|e| e.to_string())?;
        Ok(Self(Arc::new(Inner {
            dir: dir.to_path_buf(),
            github,
            tenant: tenant.into(),
        })))
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.0.dir
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/v1/sessions/github", post(github_sign_in))
            .route("/v1/account/identities/github", post(github_link))
            .route("/v1/session", get(session).delete(sign_out))
            .route("/v1/account", get(account))
            .route("/v1/workspaces/{id}", get(workspace))
            .with_state(self.clone())
    }

    /// Serve on `127.0.0.1:0`; answers the origin.
    pub async fn spawn(&self) -> std::io::Result<String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let router = self.router();
        tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });
        Ok(origin)
    }
}

fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        axum::Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

fn auth_refused(error: AuthError) -> Response {
    refused(
        StatusCode::from_u16(error.status()).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        error.code(),
        &error.to_string(),
    )
}

fn unauthenticated() -> Response {
    refused(
        StatusCode::UNAUTHORIZED,
        "unauthenticated",
        "Sign in again.",
    )
}

/// The active user session behind the bearer token, as (session id, account).
fn caller(state: &LocalService, headers: &HeaderMap) -> Result<(String, String), Response> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|t| t.starts_with("sess_"))
        .ok_or_else(unauthenticated)?;
    let store = Sessions::open(&state.0.dir)
        .and_then(|s| s.store())
        .map_err(|_| {
            refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "sessions_unavailable",
                "Try again later.",
            )
        })?;
    let session = store
        .book
        .session_of_token(token)
        .ok_or_else(unauthenticated)?;
    if session.kind != SessionKind::User || session.standing(now()) != SessionState::Active {
        return Err(unauthenticated());
    }
    Ok((
        session.id.as_str().to_string(),
        session.user.as_str().to_string(),
    ))
}

fn body(value: Value) -> Response {
    let mut value = value;
    value["v"] = json!("openagents.accounts.v1");
    axum::Json(value).into_response()
}

async fn github_sign_in(
    State(state): State<LocalService>,
    request: Result<axum::Json<CodeRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(axum::Json(request)) = request else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send a code and a code_verifier.",
        );
    };
    match service::sign_in(&state.0.dir, &state.0.github, &state.0.tenant, &request).await {
        Ok(signed) => axum::Json(service::signed_in_body(&signed)).into_response(),
        Err(error) => auth_refused(error),
    }
}

async fn github_link(
    State(state): State<LocalService>,
    headers: HeaderMap,
    request: Result<axum::Json<CodeRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (_, account) = match caller(&state, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(axum::Json(request)) = request else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send a code and a code_verifier.",
        );
    };
    match service::link(&state.0.dir, &state.0.github, &account, &request).await {
        Ok(identity) => body(
            json!({"identity": {"provider": "github", "login": identity.profile.login, "account": identity.account}}),
        ),
        Err(error) => auth_refused(error),
    }
}

async fn session(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    let (id, account) = match caller(&state, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(store) = Sessions::open(&state.0.dir).and_then(|s| s.store()) else {
        return refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "sessions_unavailable",
            "Try again later.",
        );
    };
    let Some(record) = store.book.session(&id.as_str().into()) else {
        return unauthenticated();
    };
    body(json!({"session": {
        "id": id, "kind": "user", "account": account,
        "created_at": record.created_at, "expires_at": record.expires_at,
        "state": record.standing(now()).to_string(),
    }}))
}

async fn sign_out(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    let (id, account) = match caller(&state, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(sessions) = Sessions::open(&state.0.dir) else {
        return refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "sessions_unavailable",
            "Try again later.",
        );
    };
    let ended = sessions.mutate(|book, access, now| {
        let session = book.logout(&id.as_str().into(), now)?;
        tenancy::sessions::push_access(
            access,
            tenancy::sessions::Access {
                at: now,
                actor: account.clone(),
                action: "logout".into(),
                workspace: None,
                session: Some(session.id.as_str().to_string()),
                detail: None,
            },
        );
        Ok(session)
    });
    match ended {
        Ok(_) => body(json!({"session": {"id": id, "state": "revoked"}})),
        Err(_) => unauthenticated(),
    }
}

async fn account(State(state): State<LocalService>, headers: HeaderMap) -> Response {
    let (_, account) = match caller(&state, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(store) = Accounts::open(&state.0.dir).and_then(|a| a.store()) else {
        return refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "accounts_unavailable",
            "Try again later.",
        );
    };
    let Some(record) = store.accounts.get(&account) else {
        return refused(StatusCode::NOT_FOUND, "unknown_account", "No such account.");
    };
    let workspaces: Vec<Value> = store
        .workspaces
        .values()
        .filter_map(|w| {
            let m = w.members.get(&account)?;
            (m.status == MemberStatus::Active).then(|| {
                json!({"id": w.id, "name": w.name, "kind": w.kind, "tenant": w.tenant, "role": m.role})
            })
        })
        .collect();
    body(json!({
        "account": {"id": record.id, "label": record.label, "principals": record.principals, "created": record.created},
        "workspaces": workspaces,
    }))
}

async fn workspace(
    State(state): State<LocalService>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    let (_, account) = match caller(&state, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(accounts) = Accounts::open(&state.0.dir) else {
        return refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "accounts_unavailable",
            "Try again later.",
        );
    };
    match (accounts.authorize(&id, &account), accounts.workspace(&id)) {
        (Ok(member), Ok(Some(ws))) => body(json!({
            "workspace": {"id": ws.id, "name": ws.name, "tenant": ws.tenant, "members_epoch": ws.members_epoch},
            "role": member.role,
        })),
        _ => refused(
            StatusCode::FORBIDDEN,
            "not_member",
            "You are not a member of this workspace.",
        ),
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
