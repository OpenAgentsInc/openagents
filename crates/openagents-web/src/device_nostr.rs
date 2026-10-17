//! Signing an app in with a Nostr key (#11993), forwarded to the account
//! service (`oa_auth::nostr`), beside `/v1/device/apple`:
//!
//! - `GET /v1/device/nostr/challenge` — `{challenge, expires_at,
//!   expires_in}`.
//! - `POST /v1/device/nostr` with `Authorization: Nostr <base64 event>`
//!   (NIP-98 kind 27235 over this URL and `POST`, with a `challenge` tag
//!   and a `payload` tag hashing the exact body) and `{"app", "computer"}`.
//!   Answers what `/v1/device/token` answers once approved, plus
//!   `created` and `pubkey`.
//! - `GET /v1/device/keys/challenge`, `GET` and `POST /v1/device/keys`
//!   `{"proof"}`, and `DELETE /v1/device/keys/{pubkey}`, with the app's
//!   `Authorization: Bearer sess_…`: the account's keys.
//!
//! Refusals pass through in the account service's envelope,
//! `{"error": {"code", "message"}}`.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use serde_json::json;

use crate::App;
use crate::cloud::protect;
use crate::cloud::session::device::Answer;
use crate::cloud::session::session_token;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/device/nostr/challenge", get(challenge))
        .route("/v1/device/nostr", post(sign_in))
        .route("/v1/device/keys/challenge", get(link_challenge))
        .route("/v1/device/keys", get(keys).post(link))
        .route("/v1/device/keys/{pubkey}", delete(unlink))
}

fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    protect(
        (
            status,
            axum::Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response(),
    )
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "sign_in_unavailable",
        "Login isn't available right now. Try again in a minute.",
    )
}

fn passed(answer: Answer) -> Response {
    let status = StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY);
    protect((status, axum::Json(answer.body)).into_response())
}

fn authorization(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
}

/// The app's own session bearer, as `Bearer sess_…`.
fn bearer(headers: &HeaderMap) -> Result<String, Response> {
    authorization(headers)
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|token| session_token(token))
        .map(|token| format!("Bearer {token}"))
        .ok_or_else(|| {
            refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Log in again to continue.",
            )
        })
}

async fn call(
    app: &App,
    method: reqwest::Method,
    path: &str,
    authorization: Option<&str>,
    body: Option<Vec<u8>>,
) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    match service.nostr_call(method, path, authorization, body).await {
        Ok(answer) => passed(answer),
        Err(_) => unavailable(),
    }
}

/// `GET /v1/device/nostr/challenge`.
async fn challenge(State(app): State<App>) -> Response {
    call(
        &app,
        reqwest::Method::GET,
        "/v1/sessions/nostr/challenge",
        None,
        None,
    )
    .await
}

/// `POST /v1/device/nostr`.
async fn sign_in(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(signed) =
        authorization(&headers).filter(|v| v.starts_with("Nostr ") && v.len() <= 8192)
    else {
        return refused(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Sign the request with your key.",
        );
    };
    if body.len() > 4096 {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid_request",
            "That request is too large.",
        );
    }
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let answer = match service
        .nostr_call(
            reqwest::Method::POST,
            "/v1/sessions/nostr",
            Some(signed),
            Some(body.to_vec()),
        )
        .await
    {
        Ok(answer) => answer,
        Err(_) => return unavailable(),
    };
    if answer.status != 200 {
        return passed(answer);
    }
    let body = &answer.body;
    let now = crate::cloud::session::now();
    let expires = body["session"]["expires_at"]
        .as_u64()
        .unwrap_or(now)
        .saturating_sub(now);
    protect(
        axum::Json(json!({
            "access_token": body["token"],
            "token_type": "Bearer",
            "expires_in": expires,
            "account": body["account"],
            "created": body["created"],
            "pubkey": body["pubkey"],
        }))
        .into_response(),
    )
}

/// `GET /v1/device/keys/challenge`.
async fn link_challenge(State(app): State<App>, headers: HeaderMap) -> Response {
    match bearer(&headers) {
        Ok(token) => {
            call(
                &app,
                reqwest::Method::GET,
                "/v1/account/keys/challenge",
                Some(&token),
                None,
            )
            .await
        }
        Err(response) => response,
    }
}

/// `GET /v1/device/keys`.
async fn keys(State(app): State<App>, headers: HeaderMap) -> Response {
    match bearer(&headers) {
        Ok(token) => {
            call(
                &app,
                reqwest::Method::GET,
                "/v1/account/keys",
                Some(&token),
                None,
            )
            .await
        }
        Err(response) => response,
    }
}

/// `POST /v1/device/keys` `{"proof"}`.
async fn link(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let token = match bearer(&headers) {
        Ok(token) => token,
        Err(response) => return response,
    };
    if body.len() > 16 * 1024 {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "invalid_request",
            "That request is too large.",
        );
    }
    call(
        &app,
        reqwest::Method::POST,
        "/v1/account/keys",
        Some(&token),
        Some(body.to_vec()),
    )
    .await
}

/// `DELETE /v1/device/keys/{pubkey}` (hex or `npub`).
async fn unlink(
    State(app): State<App>,
    Path(pubkey): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match bearer(&headers) {
        Ok(token) => token,
        Err(response) => return response,
    };
    if pubkey.is_empty() || pubkey.len() > 128 || !pubkey.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "This account has no such key.",
        );
    }
    call(
        &app,
        reqwest::Method::DELETE,
        &format!("/v1/account/keys/{pubkey}"),
        Some(&token),
        None,
    )
    .await
}
