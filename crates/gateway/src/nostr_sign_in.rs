//! Sign in with a Nostr key (#11993), over `oa_auth::nostr`:
//!
//! - `GET /v1/sessions/nostr/challenge`: a challenge, standing five
//!   minutes.
//! - `POST /v1/sessions/nostr` with `Authorization: Nostr <base64 event>`
//!   (NIP-98, kind 27235, carrying the challenge and the body's hash) and
//!   `{"app", "computer"}`: the same 30-day app session a device sign-in
//!   issues. A new key makes its account and personal workspace on
//!   `signup_tenant`; there is no invite list or sign-up switch here, the
//!   key is the account. A new key passes `crate::key_accounts` first:
//!   `app_attest` (`{key_id, attestation}`) from the phone, or `pow`, a
//!   proof of work over the challenge, under daily caps (#11994).
//! - `GET /v1/account/keys/challenge`, `GET` and `POST /v1/account/keys`,
//!   and `DELETE /v1/account/keys/{pubkey}`, as a session: the account's
//!   keys, adding one with a proof the new key signed, and removing one
//!   (never the last).
//!
//! Events are accepted for `oa_auth::nostr::DEFAULT_ORIGINS`, the
//! configured `public_origin`, and the request's own `Host`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, delete, get, post};
use oa_auth::nostr::{NostrAuth, Purpose};

use crate::accounts::{member_account, principal, refused, unix_now};
use crate::serve::ServeState;

/// The routes, mounted with the account surface.
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/sessions/nostr/challenge", get(challenge)),
        ("/v1/sessions/nostr", post(sign_in)),
        ("/v1/account/keys/challenge", get(link_challenge)),
        ("/v1/account/keys", get(keys).post(link)),
        ("/v1/account/keys/{pubkey}", delete(unlink)),
    ]
}

fn answer(answer: oa_auth::device::Answer) -> Response {
    (
        StatusCode::from_u16(answer.status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE),
        axum::Json(answer.body),
    )
        .into_response()
}

fn auth(state: &ServeState) -> Result<NostrAuth, Response> {
    let origins: Vec<String> = oa_auth::nostr::DEFAULT_ORIGINS
        .iter()
        .map(|o| (*o).to_string())
        .chain(state.config.public_origin.clone())
        .collect();
    NostrAuth::open(&state.dir, &origins).map_err(|_| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "sign_in_unavailable",
            "Login isn't available right now. Try again in a minute.",
        )
    })
}

fn host(headers: &HeaderMap) -> Option<&str> {
    headers.get("host").and_then(|v| v.to_str().ok())
}

/// The signed-in account behind a session (a sign-in or a sign-in key),
/// not an API key.
fn caller(state: &ServeState, headers: &HeaderMap) -> Result<String, Response> {
    let principal = principal(state, headers)?;
    let account = member_account(&principal)?.to_string();
    if principal.session().is_none() {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "session_required",
            "Keys are managed while signed in, not with an API key.",
        ));
    }
    Ok(account)
}

/// `GET /v1/sessions/nostr/challenge`.
async fn challenge(State(state): State<Arc<ServeState>>) -> Response {
    match auth(&state) {
        Ok(auth) => answer(auth.challenge(Purpose::SignIn, unix_now())),
        Err(response) => response,
    }
}

/// `POST /v1/sessions/nostr`.
async fn sign_in(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(tenant) = state
        .config
        .accounts
        .as_ref()
        .and_then(|config| config.signup_tenant.clone())
    else {
        return refused(
            StatusCode::NOT_FOUND,
            "nostr_disabled",
            "Signing in with a key isn't set up on this service.",
        );
    };
    let auth = match auth(&state) {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    let authorization = headers.get("authorization").and_then(|v| v.to_str().ok());
    // A key with no account passes the key-account limits first (#11994):
    // App Attest from the phone, or a proof of work and the daily caps.
    let mut gate = state.key_accounts.gate(&headers);
    let answered = auth.sign_in_gated(
        host(&headers),
        &tenant,
        authorization,
        &body,
        unix_now(),
        &mut gate,
    );
    let wait = answered.body["error"]["retry_after"].as_u64();
    let mut response = answer(answered);
    if let Some(wait) = wait
        && let Ok(value) = axum::http::HeaderValue::from_str(&wait.to_string())
    {
        response.headers_mut().insert("retry-after", value);
    }
    response
}

/// `GET /v1/account/keys/challenge`: a challenge for adding a key to the
/// caller's account.
async fn link_challenge(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let account = match caller(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    match auth(&state) {
        Ok(auth) => answer(auth.challenge(Purpose::Link(&account), unix_now())),
        Err(response) => response,
    }
}

/// `GET /v1/account/keys`.
async fn keys(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let account = match caller(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    match auth(&state) {
        Ok(auth) => answer(auth.keys(&account)),
        Err(response) => response,
    }
}

/// `POST /v1/account/keys` `{"proof"}`.
async fn link(State(state): State<Arc<ServeState>>, headers: HeaderMap, body: Bytes) -> Response {
    let account = match caller(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    match auth(&state) {
        Ok(auth) => answer(auth.link(host(&headers), &account, &body, unix_now())),
        Err(response) => response,
    }
}

/// `DELETE /v1/account/keys/{pubkey}`.
async fn unlink(
    State(state): State<Arc<ServeState>>,
    Path(pubkey): Path<String>,
    headers: HeaderMap,
) -> Response {
    let account = match caller(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    match auth(&state) {
        Ok(auth) => answer(auth.unlink(&account, &pubkey)),
        Err(response) => response,
    }
}
