//! Enterprise sign-in routes (REV-50): a workspace's reviewed OpenID
//! Connect provider, admin subject links, token sign-in, and the scoped
//! audit export. Verification is RS256 against the reviewed keys only;
//! no discovery document or JWKS fetch happens on this path, so a provider
//! outage refuses and never widens anything.
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{MethodRouter, get, post};
use serde_json::{Value, json};
use tenancy::accounts::sso::{Jwk, Terms, Verifier, b64url};
use tenancy::sessions::{self};
use tenancy::workspaces::UserId;

use crate::accounts::{
    accounts_refusal, accounts_store, answered, member, member_account, principal, refused,
    sessions_refusal, sessions_store,
};
use crate::serve::ServeState;

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        (
            "/v1/workspaces/{workspace}/sso",
            get(provider_read).put(provider_put),
        ),
        ("/v1/workspaces/{workspace}/sso/links", post(link)),
        ("/v1/workspaces/{workspace}/sso/unlink", post(unlink)),
        ("/v1/workspaces/{workspace}/sso/sign-in", post(sign_in)),
        ("/v1/workspaces/{workspace}/sso/audit", get(audit)),
    ]
}

/// RS256 over the reviewed key. Other algorithms refuse.
pub struct Rs256;
impl Verifier for Rs256 {
    fn verify(&self, alg: &str, key: &Jwk, signing_input: &[u8], signature: &[u8]) -> bool {
        if alg != "RS256" {
            return false;
        }
        let (Ok(n), Ok(e)) = (b64url(&key.n), b64url(&key.e)) else {
            return false;
        };
        let public = ring::signature::RsaPublicKeyComponents { n: &n, e: &e };
        public
            .verify(
                &ring::signature::RSA_PKCS1_2048_8192_SHA256,
                signing_input,
                signature,
            )
            .is_ok()
    }
}

fn field<'a>(body: &'a Value, name: &str) -> Result<&'a str, Response> {
    body.get(name).and_then(Value::as_str).ok_or_else(|| {
        refused(
            StatusCode::BAD_REQUEST,
            "empty_field",
            format!("`{name}` is required"),
        )
    })
}

fn actor(state: &ServeState, headers: &HeaderMap, workspace: &str) -> Result<String, Response> {
    let principal = principal(state, headers)?;
    let account = member_account(&principal)?.to_string();
    member(state, &account, workspace)?;
    Ok(account)
}

/// `GET /v1/workspaces/{ws}/sso` — the current provider terms for a member.
async fn provider_read(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
) -> Response {
    if let Err(response) = actor(&state, &headers, &workspace) {
        return response;
    }
    let accounts = match accounts_store(&state) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match accounts.sso_current(&workspace) {
        Ok(Some(revision)) => answered(StatusCode::OK, json!({"provider": revision})),
        Ok(None) => answered(StatusCode::OK, json!({"provider": Value::Null})),
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `PUT /v1/workspaces/{ws}/sso` — record reviewed terms; owner only.
async fn provider_put(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let account = match actor(&state, &headers, &workspace) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let reviewer = match field(&body, "reviewer") {
        Ok(r) => r.to_string(),
        Err(r) => return r,
    };
    let terms: Terms = match body
        .get("terms")
        .cloned()
        .and_then(|t| serde_json::from_value(t).ok())
    {
        Some(t) => t,
        None => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_terms",
                "`terms` must be a complete SSO provider record",
            );
        }
    };
    let accounts = match accounts_store(&state) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match accounts.sso_configure(&account, &workspace, &reviewer, terms) {
        Ok(revision) => answered(StatusCode::OK, json!({"provider": revision})),
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `POST /v1/workspaces/{ws}/sso/links` — bind a subject to a member; admin or owner.
async fn link(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let actor = match actor(&state, &headers, &workspace) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let (Ok(account), Ok(sub)) = (field(&body, "account"), field(&body, "sub")) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "empty_field",
            "`account` and `sub` are required",
        );
    };
    let accounts = match accounts_store(&state) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match accounts.sso_link(&actor, &workspace, account, sub) {
        Ok(link) => answered(StatusCode::OK, json!({"link": link})),
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `POST /v1/workspaces/{ws}/sso/unlink` — remove a subject's binding.
async fn unlink(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let actor = match actor(&state, &headers, &workspace) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let sub = match field(&body, "sub") {
        Ok(s) => s.to_string(),
        Err(r) => return r,
    };
    let accounts = match accounts_store(&state) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match accounts.sso_unlink(&actor, &workspace, &sub) {
        Ok(link) => answered(StatusCode::OK, json!({"unlinked": link})),
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `POST /v1/workspaces/{ws}/sso/sign-in` — `{"id_token": "..."}` with
/// no `Authorization` header. A verified, admitted token mints a user
/// session for the linked member through the session book.
async fn sign_in(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    if headers.get("authorization").is_some() {
        return refused(
            StatusCode::BAD_REQUEST,
            "already_signed_in",
            "SSO sign-in takes an ID token in the body and no `Authorization` header.",
        );
    }
    let token = match field(&body, "id_token") {
        Ok(t) => t.to_string(),
        Err(r) => return r,
    };
    let accounts = match accounts_store(&state) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let admitted = match accounts.sso_sign_in(&workspace, &token, &Rs256) {
        Ok(a) => a,
        Err(refusal) => return accounts_refusal(refusal),
    };
    let sessions = match sessions_store(&state) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let account = admitted.account.clone();
    let issued = match sessions.mutate(|book, access, now| {
        let issued = book.issue(UserId::from(account.as_str()), now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: account.clone(),
                action: "sso-sign-in".to_string(),
                workspace: Some(admitted.workspace.clone()),
                session: Some(issued.session.id.as_str().to_string()),
                detail: Some(format!("provider:{}", admitted.provider_digest)),
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
                "workspace": admitted.workspace,
                "role": admitted.role.to_string(),
                "created_at": issued.session.created_at,
                "expires_at": issued.session.expires_at,
            },
            "token": issued.once,
        }),
    )
}

/// `GET /v1/workspaces/{ws}/sso/audit?since=UNIX` — admin or owner.
async fn audit(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(workspace): Path<String>,
    axum::extract::Query(query): axum::extract::Query<std::collections::BTreeMap<String, String>>,
) -> Response {
    let actor = match actor(&state, &headers, &workspace) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let since = query
        .get("since")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let accounts = match accounts_store(&state) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match accounts.sso_audit(&actor, &workspace, since) {
        Ok(rows) => answered(StatusCode::OK, json!({"audit": rows})),
        Err(refusal) => accounts_refusal(refusal),
    }
}
