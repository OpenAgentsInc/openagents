//! Room tokens for the browser (#12263): the GPUI web app opens the
//! account's registry room, a computer's device room, and a chat's room on
//! `coder-sync` the way the phone does, with a credential in the WebSocket
//! URL. Instead of the account's session, that credential is a room token
//! ([`coder_access::room_token`]): signed here, scoped to one room kind and
//! room, and good for [`room_token::LIFETIME`] seconds. A socket it opened
//! closes when it expires, and the app asks for a new one to reconnect.
//!
//! `POST /v1/sync/room-tokens` with `{"room": "registry"}`,
//! `{"room": "device", "id": DEVICE}`, or `{"room": "chat2", "id": CHAT}`
//! answers `{token, room, id, expires_unix, url}`. A page calls it with the
//! sign-in cookie ([`crate::cookie_api`]); an app with its own token. The
//! registry room is the account's own; `coder-sync` still checks that the
//! account owns, or was granted, a device or chat before it lets the socket
//! in. Without a key (`OPENAGENTS_WEB_ROOM_TOKEN_KEY`), it answers 404.

use axum::Router;
use axum::extract::{Json, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use coder_access::room_token::{self, Room};
use serde::Deserialize;
use serde_json::json;

use crate::App;
use crate::coder_sync::refused;

/// The route.
pub(crate) const PATH: &str = "/v1/sync/room-tokens";

pub(crate) fn owns(path: &str) -> bool {
    path == PATH
}

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PATH, post(mint))
}

/// The page's `connect-src` addition: the sync server's socket origin, when
/// this site mints room tokens.
pub(crate) fn connect_source(app: &App) -> Option<String> {
    app.config.room_token_key.as_ref()?;
    let origin = app.config.sync_origin.trim_end_matches('/');
    let socket = origin
        .strip_prefix("https://")
        .map(|rest| format!("wss://{rest}"))
        .or_else(|| {
            origin
                .strip_prefix("http://")
                .map(|rest| format!("ws://{rest}"))
        })?;
    (!socket.contains([' ', ';', '\'', ','])).then_some(socket)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ask {
    room: String,
    #[serde(default)]
    id: Option<String>,
}

async fn mint(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Ask>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(key) = app.config.room_token_key.as_deref() else {
        return refused(
            StatusCode::NOT_FOUND,
            "unavailable",
            "Live updates aren't on here.",
        );
    };
    let account = match crate::coder_sync::account(&app, &headers).await {
        Ok(account) => account,
        Err(response) => return response,
    };
    let Ok(Json(ask)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_room",
            "Choose a chat or computer to open.",
        );
    };
    let Some(room) = Room::parse(&ask.room) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_room",
            "Choose a chat or computer to open.",
        );
    };
    let id = match (room, ask.id) {
        (Room::Registry, None) => account.clone(),
        (Room::Registry, Some(id)) if id == account => id,
        (Room::Registry, Some(_)) => {
            return refused(StatusCode::FORBIDDEN, "forbidden", "That list isn't yours.");
        }
        (_, Some(id)) => id,
        (_, None) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_room",
                "Choose a chat or computer to open.",
            );
        }
    };
    let now = crate::chat_store::now_unix();
    let token = match room_token::mint(key, &account, room, &id, now, room_token::LIFETIME) {
        Ok(token) => token,
        Err(error) if error.code == coder_access::Code::Malformed => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_room",
                "Choose a chat or computer to open.",
            );
        }
        Err(_) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Live updates aren't available right now. Try again later.",
            );
        }
    };
    let socket = connect_source(&app).unwrap_or_default();
    let url = format!(
        "{socket}/{}/{}/ws?token={token}",
        room.as_str(),
        crate::lab_card_view::encoded(&id)
    );
    let mut response = (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "room": room.as_str(),
            "id": id,
            "expires_unix": now + room_token::LIFETIME,
            "url": url,
        })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
#[path = "room_tokens_tests.rs"]
mod tests;
