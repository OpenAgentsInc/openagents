//! WorkOS in the account service (docs/auth/2026-10-20-workos-source-of-truth.md).
//!
//! - `POST /v1/sessions/workos`: the website finished a WorkOS sign-in
//!   (GitHub, Google, SSO, email and password, or an emailed code) and
//!   forwards the access token WorkOS issued. We check it against the
//!   environment's keys ourselves, read the User from WorkOS, find or make
//!   the account ([`oa_auth::workos::link`]), and issue the same session a
//!   GitHub sign-in does. The website never asserts who someone is.
//! - WorkOS access tokens as bearers: [`principal`] resolves one to the
//!   linked account on every account route, beside `sess_`, `oa_sk_`, and
//!   `oak_`, which keep working unchanged.
//! - The dual-write: [`layer`] queues the acting account after every
//!   successful account change, and a worker brings WorkOS in line
//!   ([`oa_auth::workos::sync`]). Our stores stay the source of truth;
//!   a WorkOS outage never fails a request.
//! - `POST /v1/admin/workos/backfill` and `GET /v1/admin/workos/reconcile`
//!   under the inference admin token: the one-time backfill (with a dry
//!   run) and the reconcile report.
//!
//! In `legacy` mode none of this is mounted or consulted.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get, post};
use oa_auth::workos::client::Refusal;
use oa_auth::workos::link::{ResolveError, resolve};
use oa_auth::workos::sync::{self, Options};
use oa_auth::workos::{Mode, Workos};
use serde_json::{Value, json};
use tenancy::sessions::{self};
use tenancy::workspaces::UserId;
use tokio::sync::mpsc;

use crate::accounts::{Principal, answered, refused};
use crate::serve::ServeState;

/// One account to bring in line with WorkOS.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Job {
    pub account: String,
    /// From a run marked `X-OpenAgents-Test: 1`.
    pub test: bool,
}

/// WorkOS for this process: the client and the dual-write queue.
pub struct WorkosState {
    pub(crate) client: Workos,
    pub(crate) mode: Mode,
    queue: mpsc::UnboundedSender<Job>,
    receiver: Mutex<Option<mpsc::UnboundedReceiver<Job>>>,
}

impl std::fmt::Debug for WorkosState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkosState")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl WorkosState {
    /// WorkOS as `settings` configure it, or `None` in `legacy` mode.
    pub fn open(settings: &oa_auth::workos::Settings) -> Result<Option<Self>, String> {
        let mode = settings.mode();
        if !mode.on() {
            return Ok(None);
        }
        Ok(Some(Self::with_client(
            Workos::new(settings.clone())?,
            mode,
        )))
    }

    /// WorkOS over a given client (tests point one at the fake).
    #[must_use]
    pub fn with_client(client: Workos, mode: Mode) -> Self {
        let (queue, receiver) = mpsc::unbounded_channel();
        Self {
            client,
            mode,
            queue,
            receiver: Mutex::new(Some(receiver)),
        }
    }

    /// Queue `account` for the dual-write.
    pub(crate) fn enqueue(&self, account: &str, test: bool) {
        if account.starts_with("acct_") {
            self.queue
                .send(Job {
                    account: account.to_string(),
                    test,
                })
                .ok();
        }
    }
}

/// The routes, mounted only when WorkOS is on.
pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/sessions/workos", post(sign_in)),
        ("/v1/admin/workos/backfill", post(backfill)),
        ("/v1/admin/workos/reconcile", get(reconcile)),
    ]
}

/// Start the dual-write worker and keep the key set warm. Runs inside the
/// runtime, from `serve::router`.
pub(crate) fn resume(state: &Arc<ServeState>) {
    let Some(workos) = state.workos.as_ref() else {
        return;
    };
    let Some(mut receiver) = workos.receiver.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    let keys = workos.client.keys().clone();
    tokio::spawn(async move {
        loop {
            if keys.refresh().await.is_err() {
                eprintln!("workos: the key set couldn't be fetched; retrying in a minute");
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            }
            tokio::time::sleep(Duration::from_secs(1800)).await;
        }
    });
    let state = state.clone();
    tokio::spawn(async move {
        while let Some(first) = receiver.recv().await {
            // Gather what arrived together, so a burst syncs each account once.
            let mut batch = BTreeSet::from([first]);
            tokio::time::sleep(Duration::from_millis(200)).await;
            while let Ok(next) = receiver.try_recv() {
                batch.insert(next);
            }
            for job in batch {
                run(&state, &job).await;
            }
        }
    });
}

async fn run(state: &ServeState, job: &Job) {
    let Some(workos) = state.workos.as_ref() else {
        return;
    };
    let options = Options {
        apply: true,
        test: job.test,
    };
    let mut wait = Duration::from_secs(2);
    for attempt in 1..=4 {
        match sync::account(&state.dir, &workos.client, &job.account, options).await {
            Ok(report) => {
                if report.wrote() || !report.failed.is_empty() {
                    eprintln!(
                        "workos: synced {}: {}",
                        job.account,
                        serde_json::to_string(&report).unwrap_or_default()
                    );
                }
                return;
            }
            Err(Refusal::RateLimited(seconds)) => {
                tokio::time::sleep(Duration::from_secs(seconds.min(120))).await;
            }
            Err(refusal) => {
                if attempt == 4 {
                    eprintln!(
                        "workos: {} not synced after {attempt} tries ({}); the next change or the backfill catches it",
                        job.account,
                        refusal.code()
                    );
                    return;
                }
                tokio::time::sleep(wait).await;
                wait *= 2;
            }
        }
    }
}

/// Resolve a WorkOS access token presented as a bearer. `None` when
/// WorkOS is off or the bearer isn't shaped like one, so the caller goes
/// on to its other credentials.
pub(crate) fn principal(state: &ServeState, token: &str) -> Option<Result<Principal, Response>> {
    let workos = state.workos.as_ref()?;
    if !oa_auth::workos::token::looks_like_access_token(token) {
        return None;
    }
    let claims = match workos.client.keys().verify_cached(token, false) {
        Some(Ok(claims)) => claims,
        Some(Err(_)) => {
            return Some(Err(refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Your sign-in has expired. Sign in again.",
            )));
        }
        None => {
            // The keys aren't cached yet: fetch them for the next try.
            let keys = workos.client.keys().clone();
            tokio::spawn(async move {
                keys.refresh().await.ok();
            });
            let mut response = refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "sign_in_unavailable",
                "Sign-in is starting up. Try again in a second.",
            );
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from_static("1"));
            return Some(Err(response));
        }
    };
    let account = match tenancy::workos_links::Links::open(&state.dir).account_of(&claims.sub) {
        Ok(Some(account)) => account,
        Ok(None) => {
            return Some(Err(refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "This sign-in isn't linked to an account yet. Sign in on the website first.",
            )));
        }
        Err(_) => {
            return Some(Err(refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts_unavailable",
                "The service can't read accounts right now. Try again later.",
            )));
        }
    };
    Some(Ok(Principal::Account {
        account,
        session: None,
    }))
}

fn refusal(refusal: &Refusal) -> Response {
    refused(
        StatusCode::from_u16(refusal.status()).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        refusal.code(),
        refusal.message(),
    )
}

/// `POST /v1/sessions/workos` with `{"access_token": "..."}`.
async fn sign_in(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let Some(workos) = state.workos.as_ref() else {
        return refused(
            StatusCode::NOT_FOUND,
            "workos_disabled",
            "This sign-in isn't set up here.",
        );
    };
    let Some(token) = body["access_token"].as_str() else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send `access_token`.",
        );
    };
    let claims = match workos.client.verify(token).await {
        Ok(claims) => claims,
        Err(Refusal::Unavailable) => return refusal(&Refusal::Unavailable),
        Err(_) => {
            return refused(
                StatusCode::UNAUTHORIZED,
                "sign_in_denied",
                "That sign-in didn't go through. Try again.",
            );
        }
    };
    let user = match workos.client.user(&claims.sub).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return refused(
                StatusCode::UNAUTHORIZED,
                "sign_in_denied",
                "That sign-in didn't go through. Try again.",
            );
        }
        Err(error) => return refusal(&error),
    };
    let config = state.config.accounts.as_ref();
    let invite = config.and_then(|accounts| accounts.invite_only.as_ref());
    // A closed invite list lets in its GitHub people and existing accounts.
    let mut tenant = config.and_then(|accounts| accounts.signup_tenant.clone());
    if invite.is_some_and(|list| list.closed()) {
        let identities = workos.client.identities(&user.id).await.unwrap_or_default();
        let invited = oa_auth::workos::link::github_id(&identities)
            .is_some_and(|id| oa_auth::invite::allowed(invite, id, ""));
        if !invited {
            tenant = None;
        }
    }
    let resolved = match resolve(&state.dir, &workos.client, &user, tenant.as_deref()).await {
        Ok(resolved) => resolved,
        Err(ResolveError::SignupClosed) => {
            return if invite.is_some_and(|list| list.closed()) {
                refused(
                    StatusCode::FORBIDDEN,
                    "invite_only",
                    "Sign-in is invite-only for now.",
                )
            } else {
                refused(
                    StatusCode::FORBIDDEN,
                    "signup_disabled",
                    "This service doesn't make new accounts.",
                )
            };
        }
        Err(ResolveError::Conflict) => {
            return refused(
                StatusCode::CONFLICT,
                "identity_taken",
                "That sign-in is already used by another OpenAgents account.",
            );
        }
        Err(ResolveError::Unavailable) => return refusal(&Refusal::Unavailable),
    };
    if resolved.account.agent.is_some() {
        return refused(
            StatusCode::FORBIDDEN,
            "agent_account",
            "An agent account uses its API key directly; it can't sign in to the website.",
        );
    }
    let sessions = match crate::accounts::sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    let account = resolved.account.id.clone();
    let created = resolved.created;
    let workspace = resolved.workspace.as_ref().map(|w| w.id.clone());
    let issued = match sessions.mutate(|book, access, now| {
        let issued = book.issue(UserId::from(account.as_str()), now)?;
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: account.clone(),
                action: if created { "sign-up" } else { "sign-in" }.to_string(),
                workspace: workspace.clone(),
                session: Some(issued.session.id.as_str().to_string()),
                detail: Some(format!("workos:{}", user.id)),
            },
        );
        Ok(issued)
    }) {
        Ok(issued) => issued,
        Err(refusal) => return crate::accounts::sessions_refusal(refusal),
    };
    workos.enqueue(&account, crate::accounts::test_run(&headers));
    let admin = crate::accounts::account_is_site_admin(&state, &account);
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
            "created": created,
            "admin": admin,
            "github": resolved.github.map(|id| json!({"id": id, "login": ""})),
            "workos": {
                "user_id": user.id,
                "session_id": claims.sid,
                "linked_by": resolved.by.as_str(),
            },
        }),
    )
}

/// Account routes whose success changes something WorkOS mirrors.
fn mirrored(method: &Method, path: &str) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    path.starts_with("/v1/account")
        || path.starts_with("/v1/accounts")
        || path.starts_with("/v1/workspaces")
        || path.starts_with("/v1/invitations")
        || path.starts_with("/v1/recovery")
        || path.starts_with("/v1/sessions")
        || path.starts_with("/v1/device")
}

/// The dual-write: after a successful change, queue the account that made
/// it and any account the answer names as signed in.
pub(crate) async fn layer(
    State(state): State<Arc<ServeState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(workos) = state.workos.as_ref() else {
        return next.run(request).await;
    };
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    if !mirrored(&method, &path) || path.starts_with("/v1/sessions/workos") {
        return next.run(request).await;
    }
    let test = crate::accounts::test_run(request.headers());
    // Who is acting, read before the change (a deleted account has no
    // session afterwards).
    let actor = crate::accounts::principal(&state, request.headers())
        .ok()
        .and_then(|principal| principal.account().map(str::to_string));
    let response = next.run(request).await;
    if !response.status().is_success() {
        return response;
    }
    if let Some(actor) = &actor {
        workos.enqueue(actor, test);
    }
    // A sign-in names its account in the answer.
    let json = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    if actor.is_none() && json {
        let (parts, body) = response.into_parts();
        let Ok(bytes) = axum::body::to_bytes(body, 1024 * 1024).await else {
            return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();
        };
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            for account in [
                &value["session"]["account"],
                &value["account"]["id"],
                &value["account"],
            ] {
                if let Some(account) = account.as_str() {
                    workos.enqueue(account, test);
                }
            }
        }
        return Response::from_parts(parts, axum::body::Body::from(bytes));
    }
    response
}

fn admin(state: &ServeState, headers: &HeaderMap) -> Result<(), Response> {
    if crate::inference_status::admin_authorized(state, headers) {
        Ok(())
    } else {
        Err(refused(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Send the admin token.",
        ))
    }
}

/// `POST /v1/admin/workos/backfill` with `{"apply": bool, "after": "acct_…"}`:
/// every account and organization workspace, under the WorkOS write limit.
/// `apply: false` (the default) is the dry run.
async fn backfill(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    if let Err(response) = admin(&state, &headers) {
        return response;
    }
    let Some(workos) = state.workos.as_ref() else {
        return refused(
            StatusCode::NOT_FOUND,
            "workos_disabled",
            "WorkOS isn't on here.",
        );
    };
    let body = body.map(|Json(body)| body).unwrap_or_default();
    let options = Options {
        apply: body["apply"].as_bool().unwrap_or(false),
        test: false,
    };
    let after = body["after"].as_str().map(str::to_string);
    let mut last = None;
    let report = sync::all(
        &state.dir,
        &workos.client,
        options,
        after.as_deref(),
        // About 4 calls an account: 40 a second stays under 500 in 10.
        || tokio::time::sleep(Duration::from_millis(100)),
        |id, _| last = Some(id.to_string()),
    )
    .await;
    match report {
        Ok(report) => answered(
            StatusCode::OK,
            json!({"applied": options.apply, "last": last, "report": report}),
        ),
        Err(error) => refusal(&error),
    }
}

/// `GET /v1/admin/workos/reconcile`: what is linked, from our side.
async fn reconcile(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if let Err(response) = admin(&state, &headers) {
        return response;
    }
    match sync::reconcile(&state.dir) {
        Ok(report) => answered(StatusCode::OK, json!({"reconcile": report})),
        Err(error) => refusal(&error),
    }
}
