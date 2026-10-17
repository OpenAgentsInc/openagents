//! The account service issues job-only Lab worker keys and checks current
//! issuer-session and project authority whenever a worker asks to act.
use crate::{accounts, serve::ServeState};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{MethodRouter, delete, post},
};
use lab_jobs::{
    authority::{Authority, Worker},
    protocol::{Access, Enroll},
};
use lab_kit::SpaceRef;
use serde_json::json;
use std::sync::Arc;
use tenancy::sessions::{SessionId, SessionState};

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/account/lab-workers", post(enroll)),
        ("/v1/account/lab-workers/{id}", delete(revoke)),
        ("/v1/account/lab-worker-access", post(access)),
    ]
}
fn failed() -> Response {
    accounts::refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "worker_unavailable",
        "Lab workers aren't available right now. Try again later.",
    )
}
fn denied() -> Response {
    accounts::refused(
        StatusCode::UNAUTHORIZED,
        "worker_revoked",
        "This Lab worker key no longer works. Add the worker again.",
    )
}
fn issuer_required() -> Response {
    accounts::refused(
        StatusCode::FORBIDDEN,
        "worker_sign_in_required",
        "Sign in with an app or login key to add a Lab worker.",
    )
}
fn authority(state: &ServeState) -> Result<Authority, Response> {
    Ok(Authority::new(Arc::new(
        lab_store::backend::FsBackend::open(state.dir.join("lab-workers")).map_err(|_| failed())?,
    )))
}
fn allowed(state: &ServeState, account: &str, space: &SpaceRef) -> Result<(), Response> {
    let store = accounts::accounts_store(state)?
        .store()
        .map_err(|_| failed())?;
    if !store.accounts.contains_key(account) {
        return Err(denied());
    }
    match space {
        SpaceRef::Personal { account: owner } if owner == account => Ok(()),
        SpaceRef::Project { project } => {
            let status = oa_auth::repos::status(&state.dir, account).map_err(|_| failed())?;
            if status
                .projects
                .iter()
                .any(|p| p.id == *project && p.sharing.as_ref().is_none_or(|s| s.role.edits()))
            {
                Ok(())
            } else {
                Err(accounts::refused(
                    StatusCode::FORBIDDEN,
                    "project_role",
                    "This worker can no longer change this project's Labs.",
                ))
            }
        }
        _ => Err(denied()),
    }
}
fn issuer(state: &ServeState, account: &str, id: &str) -> Result<u64, Response> {
    let store = accounts::sessions_store(state)?
        .store()
        .map_err(|_| failed())?;
    let session = store
        .book
        .session(&SessionId::from(id))
        .filter(|s| {
            s.user.as_str() == account && s.standing(accounts::unix_now()) == SessionState::Active
        })
        .ok_or_else(denied)?;
    Ok(session.expires_at.saturating_mul(1000))
}
async fn enroll(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(request): Json<Enroll>,
) -> Result<Response, Response> {
    let principal = accounts::principal(&state, &headers)?;
    let account = accounts::member_account(&principal)?;
    let session = principal.session().ok_or_else(issuer_required)?;
    let space = if request.space == "me" {
        SpaceRef::Personal {
            account: account.into(),
        }
    } else if let Some(project) = request
        .space
        .strip_prefix("project:")
        .filter(|p| !p.is_empty() && p.len() <= 256)
    {
        SpaceRef::Project {
            project: project.into(),
        }
    } else {
        return Err(accounts::refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Choose your space or a project.",
        ));
    };
    allowed(&state, account, &space)?;
    let now = accounts::unix_now().saturating_mul(1000);
    let remaining = issuer(&state, account, session)?.saturating_sub(now);
    if request.lifetime_ms < 60_000 || request.lifetime_ms > 86_400_000 || remaining < 60_000 {
        return Err(accounts::refused(
            StatusCode::BAD_REQUEST,
            "invalid_expiry",
            "Choose a worker lifetime from one minute to one day, within your current sign-in.",
        ));
    }
    let issued = authority(&state)?
        .enroll_for_session(
            account,
            space,
            request.capabilities,
            now,
            request.lifetime_ms.min(remaining),
            session,
        )
        .await
        .map_err(|e| match e {
            lab_store::Error::Invalid(m) => {
                accounts::refused(StatusCode::BAD_REQUEST, "invalid_request", m)
            }
            lab_store::Error::Refused(r) => {
                accounts::refused(StatusCode::BAD_REQUEST, "invalid_request", r.message)
            }
            lab_store::Error::RateLimited => accounts::refused(
                StatusCode::TOO_MANY_REQUESTS,
                "worker_limit",
                "Remove an unused worker before adding another.",
            ),
            _ => failed(),
        })?;
    Ok(accounts::answered(
        StatusCode::CREATED,
        serde_json::to_value(issued).map_err(|_| failed())?,
    ))
}
async fn revoke(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let principal = accounts::principal(&state, &headers)?;
    let account = accounts::member_account(&principal)?;
    authority(&state)?
        .revoke(account, &id)
        .await
        .map_err(|e| match e {
            lab_store::Error::NotFound => accounts::refused(
                StatusCode::NOT_FOUND,
                "not_found",
                "That worker could not be found.",
            ),
            _ => failed(),
        })?;
    Ok(accounts::answered(StatusCode::OK, json!({"revoked":true})))
}
async fn access(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    if headers.get_all("Authorization").iter().count() != 1 {
        return Err(denied());
    }
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(denied)?;
    let worker: Worker = authority(&state)?
        .authenticate(token, accounts::unix_now().saturating_mul(1000))
        .await
        .map_err(|e| match e {
            lab_store::Error::LeaseLost => denied(),
            _ => failed(),
        })?;
    issuer(
        &state,
        &worker.account,
        worker.issuer_session.as_deref().ok_or_else(denied)?,
    )?;
    allowed(&state, &worker.account, &worker.space)?;
    let experimental = accounts::account_is_site_admin(&state, &worker.account);
    Ok(accounts::answered(
        StatusCode::OK,
        serde_json::to_value(Access {
            worker,
            experimental,
            cad: false,
        })
        .map_err(|_| failed())?,
    ))
}
