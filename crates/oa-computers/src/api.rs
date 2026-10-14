//! The HTTP API. It records intent and returns at once; the reconciler
//! does the work.
//!
//! Callers are our own services (the web server) with the service bearer
//! token; they name the signed-in account in `X-OpenAgents-Account` and
//! see only that account's computers.
//!
//! - `POST /v1/computers` `{chat?, image?, size?, idempotency_key?,
//!   environment?, version?}` → `202` with the new computer's id (`200`
//!   with the existing one for the same live chat or key). A request the
//!   driver cannot serve is refused before anything is written.
//! - `GET /v1/computers`, `GET /v1/computers/{id}`.
//! - `DELETE /v1/computers/{id}` → `202`; `POST /v1/computers/{id}/stop`,
//!   `/wake` → `202`.
//! - `GET /v1/capabilities`, `GET /healthz`.

use crate::reconcile::now_ms;
use crate::store::{Desired, Entry, NewComputer, Store};
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use coder_environment::{ProjectLink, SourcePin, valid_id};
use coder_working_computer::{BaseLink, Bounds, Computer, Principal, Spec};
use computer_driver::Capabilities;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const ACCOUNT_HEADER: &str = "x-openagents-account";

/// Lifecycle bounds for a new computer: stop after 30 idle minutes, never
/// run one boot past 12 hours.
pub const BOUNDS: Bounds = Bounds {
    idle_ms: 30 * 60_000,
    observed_extension_ms: 30 * 60_000,
    absolute_ms: 12 * 3_600_000,
};

#[derive(Clone)]
pub struct Api {
    pub store: Store,
    pub caps: Capabilities,
    /// SHA-256 of the service bearer token.
    pub token: [u8; 32],
    /// Turn credentials waiting in memory (shared with the reconciler).
    pub turns: crate::turns::Secrets,
}

fn sha(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}
impl Api {
    pub fn token_digest(token: &str) -> [u8; 32] {
        sha(token)
    }
}

fn err(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"error": code, "message": message}))).into_response()
}

async fn auth(State(api): State<Arc<Api>>, req: Request, next: Next) -> Response {
    if matches!(req.uri().path(), "/health" | "/healthz") {
        return next.run(req).await;
    }
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| sha(v.trim()));
    let good = given.is_some_and(|g| {
        g.iter()
            .zip(api.token.iter())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            == 0
    });
    if !good {
        return err(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid service token is required.",
        );
    }
    next.run(req).await
}

fn account(headers: &HeaderMap) -> Result<String, Response> {
    headers
        .get(ACCOUNT_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|a| !a.is_empty() && a.len() <= 256 && !a.chars().any(char::is_control))
        .map(str::to_owned)
        .ok_or_else(|| {
            err(
                StatusCode::BAD_REQUEST,
                "no_account",
                "Name the account in X-OpenAgents-Account.",
            )
        })
}

/// An opaque principal for the lifecycle record: no account ID in it.
pub fn principal_of(owner: &str) -> String {
    let d = sha(&format!("oa-computers/owner/{owner}"));
    format!(
        "p-{}",
        d[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

pub fn view(e: &Entry) -> Value {
    json!({
        "id": e.id,
        "chat": e.chat,
        "driver": e.driver,
        "image": e.image,
        "size": e.size,
        "desired": e.desired,
        "state": e.phase,
        "resource": e.record.resource(),
        "turns": e.record.turn.dispatched,
        "attempts": e.attempts,
        "last_error": e.last_error,
        "created_unix": e.created_unix,
        "updated_unix": e.updated_unix,
    })
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    #[serde(default)]
    pub chat: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// The saved environment and version the image was built from.
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

fn new_id() -> String {
    let mut b = [0u8; 10];
    let _ = getrandom::fill(&mut b);
    format!(
        "cmp_{}",
        b.iter().map(|x| format!("{x:02x}")).collect::<String>()
    )
}

/// Validate a create and build its record. Every refusal happens here,
/// before anything is written.
pub fn admit_create(
    caps: &Capabilities,
    owner: &str,
    body: &CreateBody,
) -> Result<Computer, (StatusCode, &'static str, String)> {
    if let Err(no) = coder_working_computer::on_driver::check(caps) {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported",
            no.to_string(),
        ));
    }
    let size = match &body.size {
        Some(s) => s.clone(),
        None if caps.sizes.iter().any(|s| s == "small") => "small".into(),
        None => caps
            .sizes
            .first()
            .cloned()
            .unwrap_or_else(|| "default".into()),
    };
    if !caps.sizes.is_empty() && !caps.sizes.contains(&size) {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid_size",
            format!("Size {size} is not offered."),
        ));
    }
    let bad = |what: &str| {
        (
            StatusCode::BAD_REQUEST,
            "invalid",
            format!("The {what} is invalid."),
        )
    };
    for (what, v) in [
        ("chat", &body.chat),
        ("idempotency key", &body.idempotency_key),
        ("environment", &body.environment),
        ("version", &body.version),
    ] {
        if v.as_deref().is_some_and(|v| !valid_id(v)) {
            return Err(bad(what));
        }
    }
    if body.image.as_deref().is_some_and(|i| {
        i.is_empty() || i.len() > 256 || i.chars().any(|c| c.is_control() || c == ' ')
    }) {
        return Err(bad("image"));
    }
    let base = match (&body.environment, &body.version) {
        (Some(environment), Some(version)) => Some(BaseLink {
            environment: environment.clone(),
            version: version.clone(),
        }),
        (None, None) => None,
        _ => return Err(bad("environment version")),
    };
    let id = new_id();
    let principal = principal_of(owner);
    let spec = Spec {
        chat: body.chat.clone().unwrap_or_else(|| id.clone()),
        id,
        owner: Principal {
            workspace: principal.clone(),
            principal: principal.clone(),
        },
        project: ProjectLink {
            workspace: principal,
            project: "computers".into(),
        },
        source: SourcePin {
            repository: None,
            revision: "0".repeat(40),
            digest: "0".repeat(64),
        },
        base,
        size,
        credential_names: Default::default(),
        services: vec![],
        bounds: BOUNDS,
    };
    Computer::new(spec, now_ms()).map_err(|m| (StatusCode::BAD_REQUEST, "invalid", m.to_owned()))
}

async fn create(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let owner = match account(&headers) {
        Ok(o) => o,
        Err(r) => return r,
    };
    let body: CreateBody = if body.is_empty() {
        CreateBody::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(b) => b,
            Err(e) => return err(StatusCode::BAD_REQUEST, "invalid_body", &e.to_string()),
        }
    };
    let record = match admit_create(&api.caps, &owner, &body) {
        Ok(r) => r,
        Err((status, code, message)) => return err(status, code, &message),
    };
    let made = api
        .store
        .create(&NewComputer {
            owner: &owner,
            chat: body.chat.as_deref(),
            idempotency_key: body.idempotency_key.as_deref(),
            driver: &api.caps.driver,
            image: body.image.as_deref(),
            record: &record,
        })
        .await;
    match made {
        Ok((e, true)) => (StatusCode::ACCEPTED, Json(view(&e))).into_response(),
        Ok((e, false)) => {
            // The chat's computer already exists: make sure it is wanted up.
            if e.desired == Desired::Stopped {
                let _ = api.store.set_desired(&owner, &e.id, Desired::Running).await;
            }
            (StatusCode::OK, Json(view(&e))).into_response()
        }
        Err(e) => {
            eprintln!("oa-computers: create: {e}");
            err(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "The computer could not be recorded. Try again.",
            )
        }
    }
}

async fn list(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let owner = match account(&headers) {
        Ok(o) => o,
        Err(r) => return r,
    };
    match api.store.list(&owner).await {
        Ok(all) => {
            Json(json!({"computers": all.iter().map(view).collect::<Vec<_>>()})).into_response()
        }
        Err(e) => {
            eprintln!("oa-computers: list: {e}");
            err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.")
        }
    }
}

async fn get_one(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match account(&headers) {
        Ok(o) => o,
        Err(r) => return r,
    };
    match api.store.get(&owner, &id).await {
        Ok(Some(e)) => Json(view(&e)).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, "not_found", "No such computer."),
        Err(e) => {
            eprintln!("oa-computers: get: {e}");
            err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.")
        }
    }
}

async fn want(api: &Api, headers: &HeaderMap, id: &str, d: Desired) -> Response {
    let owner = match account(headers) {
        Ok(o) => o,
        Err(r) => return r,
    };
    match api.store.set_desired(&owner, id, d).await {
        Ok(true) => match api.store.get(&owner, id).await {
            Ok(Some(e)) => (StatusCode::ACCEPTED, Json(view(&e))).into_response(),
            _ => StatusCode::ACCEPTED.into_response(),
        },
        Ok(false) => err(
            StatusCode::NOT_FOUND,
            "not_found",
            "No such computer, or it was deleted.",
        ),
        Err(e) => {
            eprintln!("oa-computers: set desired: {e}");
            err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.")
        }
    }
}

async fn delete(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    want(&api, &headers, &id, Desired::Deleted).await
}
async fn stop(State(api): State<Arc<Api>>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    want(&api, &headers, &id, Desired::Stopped).await
}
async fn wake(State(api): State<Arc<Api>>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    want(&api, &headers, &id, Desired::Running).await
}

async fn capabilities(State(api): State<Arc<Api>>) -> Response {
    Json(api.caps.clone()).into_response()
}

async fn healthz() -> Response {
    Json(json!({"ok": true})).into_response()
}

pub fn router(api: Arc<Api>) -> Router {
    Router::new()
        // Google's front end keeps /healthz for itself on run.app hosts.
        .route("/health", get(healthz))
        .route("/healthz", get(healthz))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/computers", get(list).post(create))
        .route("/v1/computers/{id}", get(get_one).delete(delete))
        .route("/v1/computers/{id}/stop", post(stop))
        .route("/v1/computers/{id}/wake", post(wake))
        .merge(crate::turns::routes())
        .layer(middleware::from_fn_with_state(api.clone(), auth))
        .with_state(api)
}
