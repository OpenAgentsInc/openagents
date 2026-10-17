//! Signed-out chats for the apps (#11978): someone who just installed the
//! phone app chats with OpenAgents before making an account.
//!
//! The chats are the website's signed-out chats ([`crate::chat_owner`]),
//! answered by the same chat worker (Gemini Flash through our gateway). A
//! browser names its chats with the `oa_visitor` cookie; an app names them
//! with a visitor token instead, which the server signs and checks with its
//! own salt, so nothing is stored to issue one.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /v1/visitors` | `201 {token, messages}`: a new visitor and how many messages it may send |
//! | `POST /v1/visitors/claim` `{token}` | Signed in (`Bearer sess_…`): move the visitor's chats to the account, `{moved}` |
//!
//! With `Authorization: Bearer ovis_…`, these routes answer the visitor's
//! own chats: `POST /v1/threads`, `GET /v1/threads/{id}`,
//! `POST /v1/threads/{id}/messages`, `GET /v1/threads/{id}/events`, and
//! `POST /v1/threads/{id}/stop`. Words only: no files, projects, or
//! computers. After [`MESSAGES`] messages across its chats (deleting a chat
//! doesn't give them back), sending is refused with `sign_in_required`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::post;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;

use crate::App;
use crate::coder_sync::{answer, refused};
use crate::pages::chat::{AppSent, follow_from_app, start_from_app};

/// What a visitor token starts with.
pub(crate) const PREFIX: &str = "ovis_";
/// The messages one visitor may send, across all its chats.
pub(crate) const MESSAGES: usize = 10;
/// The chats one visitor may start.
pub(crate) const CHATS: usize = 5;
/// New visitors one address may make in an hour.
const MINTS_PER_HOUR: usize = 20;
/// The longest message, in characters (the web composer's limit).
const MAX_CHARS: usize = 4_000;
/// What a visitor reads when it has used its messages.
pub(crate) const SIGN_IN: &str = "Sign in to keep chatting. It's free.";
const TAG: &[u8] = b"openagents.app.visitor.v1:";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/visitors", post(mint))
        .route("/v1/visitors/claim", post(claim))
}

fn mac(salt: &[u8; 32], visitor: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(salt).expect("HMAC accepts 32 bytes");
    mac.update(TAG);
    mac.update(visitor.as_bytes());
    mac
}

/// The token that names `visitor` (32 hex characters).
pub(crate) fn token(salt: &[u8; 32], visitor: &str) -> String {
    let tag: String = mac(salt, visitor)
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{PREFIX}{visitor}{tag}")
}

/// The visitor a token names, when the server signed it.
pub(crate) fn visitor_of(salt: &[u8; 32], token: &str) -> Option<String> {
    let rest = token.strip_prefix(PREFIX)?;
    if rest.len() != 32 + 64 || !rest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let (visitor, tag) = rest.split_at(32);
    let tag: Vec<u8> = (0..tag.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&tag[i..i + 2], 16))
        .collect::<Result<_, _>>()
        .ok()?;
    mac(salt, visitor).verify_slice(&tag).ok()?;
    Some(visitor.to_ascii_lowercase())
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// With a visitor token: its visitor, or the refusal for a token this
/// server didn't sign. `None` when the request carries no visitor token.
pub(crate) fn caller(app: &App, headers: &HeaderMap) -> Option<Result<String, Response>> {
    let token = bearer(headers).filter(|t| t.starts_with(PREFIX))?;
    Some(visitor_of(&app.config.ask_salt, token).ok_or_else(|| {
        refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Start a new chat to keep going.",
        )
    }))
}

/// Whose chats a chat route uses: the visitor's (`true`) or the account's.
pub(crate) async fn owner(app: &App, headers: &HeaderMap) -> Result<(String, bool), Response> {
    match caller(app, headers) {
        Some(visitor) => visitor.map(|v| (v, true)),
        None => crate::coder_sync::owner(app, headers)
            .await
            .map(|owner| (owner, false)),
    }
}

/// The messages `visitor` has sent and the chats it has, deleted ones too.
async fn used(app: &App, visitor: &str) -> Result<(usize, usize), Response> {
    let chats = app
        .config
        .chat_store
        .list_with_deleted(visitor)
        .await
        .map_err(|e| crate::coder_sync::stored(&e))?;
    Ok((chats.iter().map(|c| c.requests.len()).sum(), chats.len()))
}

fn limited() -> Response {
    refused(StatusCode::CONFLICT, "sign_in_required", SIGN_IN)
}

/// Whether one more message may be sent; the messages left after it.
async fn admit(app: &App, visitor: &str, new_chat: bool) -> Result<usize, Response> {
    let (sent, chats) = used(app, visitor).await?;
    if sent >= MESSAGES || (new_chat && chats >= CHATS) {
        app.config.analytics.event("app_visitor_limit", "");
        return Err(limited());
    }
    Ok(MESSAGES - sent - 1)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sent {
    request_id: String,
    text: String,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    project: Option<String>,
}

fn parse(body: &[u8]) -> Result<(String, String), Response> {
    let invalid = || {
        refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {request_id (a UUID), text}.",
        )
    };
    let sent: Sent = serde_json::from_slice(body).map_err(|_| invalid())?;
    if !crate::chat_store::valid_id(&sent.request_id) {
        return Err(invalid());
    }
    if !sent.files.is_empty() || sent.project.is_some() {
        return Err(refused(StatusCode::CONFLICT, "sign_in_required", SIGN_IN));
    }
    let text = sent.text.trim();
    if text.is_empty() || text.chars().count() > MAX_CHARS {
        return Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Enter a message of at most 4,000 characters.",
        ));
    }
    if secret_screen::credential_in(text).is_some() {
        return Err(refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "This looks like it holds a password or key, so it wasn't sent.",
        ));
    }
    Ok((sent.request_id, text.to_owned()))
}

fn sent(sent: AppSent, status: StatusCode, body: serde_json::Value) -> Response {
    match sent {
        AppSent::Answering => answer(status, body),
        AppSent::Busy(message) => refused(StatusCode::CONFLICT, "busy", message),
        AppSent::Invalid => refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Enter a message of at most 4,000 characters.",
        ),
        AppSent::Missing => refused(StatusCode::NOT_FOUND, "not_found", "No such chat."),
        AppSent::Unavailable => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Try again later.",
        ),
    }
}

/// `POST /v1/threads` as a visitor: start a chat answered by OpenAgents.
pub(crate) async fn start(app: &App, headers: &HeaderMap, visitor: &str, body: &Bytes) -> Response {
    let (id, text) = match parse(body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let store = &app.config.chat_store;
    let existed = matches!(store.load(visitor, &id).await, Ok(Some(_)));
    let left = if existed {
        used(app, visitor)
            .await
            .map(|(sent, _)| MESSAGES.saturating_sub(sent))
    } else {
        admit(app, visitor, true).await
    };
    let left = match left {
        Ok(left) => left,
        Err(r) => return r,
    };
    if !existed {
        app.config.analytics.event("app_visitor_chat", "");
    }
    let outcome = start_from_app(
        app,
        visitor,
        &id,
        text,
        Vec::new(),
        None,
        crate::cad_gate::Plan::SignedOut,
        headers,
    )
    .await;
    let status = if existed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    sent(
        outcome,
        status,
        json!({"id": id, "answering": true, "left": left}),
    )
}

/// `POST /v1/threads/{id}/messages` as a visitor.
pub(crate) async fn reply(
    app: &App,
    headers: &HeaderMap,
    visitor: &str,
    id: &str,
    body: &Bytes,
) -> Response {
    let (request_id, text) = match parse(body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let store = &app.config.chat_store;
    let resent = match store.load(visitor, id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => loaded
            .conversation
            .requests
            .iter()
            .any(|r| r.id == request_id),
        Ok(_) => return refused(StatusCode::NOT_FOUND, "not_found", "No such chat."),
        Err(e) => return crate::coder_sync::stored(&e),
    };
    let left = if resent {
        used(app, visitor)
            .await
            .map(|(sent, _)| MESSAGES.saturating_sub(sent))
    } else {
        admit(app, visitor, false).await
    };
    let left = match left {
        Ok(left) => left,
        Err(r) => return r,
    };
    let outcome = follow_from_app(
        app,
        visitor,
        None,
        id,
        &request_id,
        &text,
        Vec::new(),
        crate::cad_gate::Plan::SignedOut,
        headers,
    )
    .await;
    sent(
        outcome,
        StatusCode::ACCEPTED,
        json!({"answering": true, "left": left}),
    )
}

/// The sender's address for the hourly limit: the first forwarded hop.
fn sender(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default()
}

fn allowed(who: &str) -> bool {
    static SEEN: OnceLock<Mutex<HashMap<String, Vec<Instant>>>> = OnceLock::new();
    let Ok(mut seen) = SEEN.get_or_init(Default::default).lock() else {
        return true;
    };
    let now = Instant::now();
    if seen.len() > 50_000 {
        seen.clear();
    }
    let times = seen.entry(who.to_owned()).or_default();
    times.retain(|at| now.duration_since(*at) < Duration::from_secs(3600));
    if times.len() >= MINTS_PER_HOUR {
        return false;
    }
    times.push(now);
    true
}

/// `POST /v1/visitors`: a new visitor.
async fn mint(State(app): State<App>, headers: HeaderMap) -> Response {
    if !allowed(&sender(&headers)) {
        return refused(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many new chats from here. Sign in, or try again later.",
        );
    }
    app.config.analytics.event("app_visitor_started", "");
    let visitor = crate::ask::new_visitor();
    answer(
        StatusCode::CREATED,
        json!({"token": token(&app.config.ask_salt, &visitor), "messages": MESSAGES}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Claim {
    token: String,
}

/// `POST /v1/visitors/claim {token}`: signed in, move the visitor's chats to
/// the account, as the website does at sign-in. A chat still being
/// answered stays with the visitor.
async fn claim(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(r) => return r,
    };
    let Some(visitor) = serde_json::from_slice::<Claim>(&body)
        .ok()
        .and_then(|c| visitor_of(&app.config.ask_salt, &c.token))
    else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {token}.");
    };
    // Whether the visitor had reached the limit (as [`admit`] refuses):
    // read before its chats move.
    let limited = used(&app, &visitor)
        .await
        .is_ok_and(|(sent, chats)| sent >= MESSAGES || chats >= CHATS);
    let moved = crate::chat_owner::adopt_all(&app.config.chat_store, &visitor, &owner).await;
    if moved > 0 {
        app.config.analytics.event("app_visitor_claimed", "");
        if limited {
            app.config
                .analytics
                .event("app_visitor_signed_in_after_limit", "");
        }
    }
    answer(StatusCode::OK, json!({"moved": moved}))
}

#[cfg(test)]
#[path = "visitor_api_tests.rs"]
mod flow_tests;

#[cfg(test)]
mod tests {
    use super::*;

    const SALT: [u8; 32] = [7; 32];
    const VISITOR: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn tokens_name_their_visitor_and_nothing_else() {
        let token = token(&SALT, VISITOR);
        assert!(token.starts_with(PREFIX));
        assert_eq!(visitor_of(&SALT, &token).as_deref(), Some(VISITOR));
        assert_eq!(visitor_of(&[8; 32], &token), None);
        let mut changed = token.clone().into_bytes();
        let at = PREFIX.len() + 3;
        changed[at] = if changed[at] == b'0' { b'1' } else { b'0' };
        assert_eq!(
            visitor_of(&SALT, &String::from_utf8(changed).unwrap()),
            None
        );
        assert_eq!(visitor_of(&SALT, &token[..token.len() - 2]), None);
        assert_eq!(visitor_of(&SALT, "ovis_"), None);
        assert_eq!(visitor_of(&SALT, VISITOR), None);
    }

    #[test]
    fn minting_is_limited_per_address() {
        let who = "198.51.100.200";
        for _ in 0..MINTS_PER_HOUR {
            assert!(allowed(who));
        }
        assert!(!allowed(who));
        assert!(allowed("198.51.100.201"));
    }
}
