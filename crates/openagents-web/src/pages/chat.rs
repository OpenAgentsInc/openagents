//! In-memory chats started from the homepage composer.
//!
//! `POST /chat` allocates a UUID, stores the first message, and redirects
//! to `/chat/{uuid}`. Later posts append to that chat. This process keeps
//! the messages; a restart forgets them. Nothing here starts Coder or
//! reaches a computer.

use std::collections::HashMap;
use std::sync::Mutex;

use axum::Router;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::Deserialize;

use crate::App;
use crate::layout::{escape, page, problem};

/// Pages that load `static/chat.js`.
pub(crate) const COMPOSER_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
script-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

const MAX_CHARS: usize = 4_000;
const MAX_CHATS: usize = 256;
const MAX_MESSAGES: usize = 64;

pub(crate) struct Store {
    chats: Mutex<HashMap<String, Vec<String>>>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            chats: Mutex::new(HashMap::new()),
        }
    }
}

impl Store {
    fn create(&self, message: String) -> Option<String> {
        let mut chats = self.chats.lock().ok()?;
        if chats.len() >= MAX_CHATS {
            if let Some(oldest) = chats.keys().next().cloned() {
                chats.remove(&oldest);
            }
        }
        let id = new_id();
        chats.insert(id.clone(), vec![message]);
        Some(id)
    }

    fn get(&self, id: &str) -> Option<Vec<String>> {
        self.chats.lock().ok()?.get(id).cloned()
    }

    fn append(&self, id: &str, message: String) -> bool {
        let Ok(mut chats) = self.chats.lock() else {
            return false;
        };
        let Some(messages) = chats.get_mut(id) else {
            return false;
        };
        if messages.len() >= MAX_MESSAGES {
            return false;
        }
        messages.push(message);
        true
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat", post(start))
        .route("/chat/{id}", get(show).post(follow))
}

#[derive(Deserialize)]
struct Prompt {
    q: String,
}

async fn start(State(app): State<App>, form: Form<Prompt>) -> Response {
    let Some(message) = normalize(&form.q) else {
        return Redirect::to("/").into_response();
    };
    let Some(id) = app.chats.create(message) else {
        return problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Chat unavailable",
            "The chat store is busy. Try sending the message again.",
            ("/", "Home"),
        );
    };
    Redirect::to(&format!("/chat/{id}")).into_response()
}

async fn show(State(app): State<App>, Path(id): Path<String>) -> Response {
    if !valid_id(&id) {
        return missing();
    }
    let Some(messages) = app.chats.get(&id) else {
        return missing();
    };
    let mut response = page("Chat", None, &thread(&id, &messages));
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(COMPOSER_POLICY),
    );
    response
}

async fn follow(State(app): State<App>, Path(id): Path<String>, form: Form<Prompt>) -> Response {
    if !valid_id(&id) || app.chats.get(&id).is_none() {
        return missing();
    }
    let Some(message) = normalize(&form.q) else {
        return Redirect::to(&format!("/chat/{id}")).into_response();
    };
    if !app.chats.append(&id, message) {
        let back = format!("/chat/{id}");
        return problem(
            StatusCode::BAD_REQUEST,
            "Message not added",
            "This chat cannot take another message.",
            (&back, "Back to the chat"),
        );
    }
    Redirect::to(&format!("/chat/{id}")).into_response()
}

fn thread(id: &str, messages: &[String]) -> String {
    let mut body = String::from("<section class=\"thread\" aria-label=\"Chat\">");
    for message in messages {
        body.push_str(&format!(
            "<p class=\"thread-said\"><span class=\"term-mark\">You</span> {}</p>",
            escape(message)
        ));
    }
    body.push_str("</section>");
    body.push_str(&composer(&format!("/chat/{id}"), "Continue this chat"));
    body
}

/// A 24-pixel lucide icon path set, drawn at `size` with a 2-pixel stroke.
fn icon(size: u8, paths: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" \
viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" \
stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\" focusable=\"false\">\
{paths}</svg>"
    )
}

const CHEVRON_DOWN: &str = "<path d=\"m6 9 6 6 6-6\"/>";
const CLOUD: &str = "<path d=\"M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z\"/>";
const PLUS: &str = "<path d=\"M5 12h14\"/><path d=\"M12 5v14\"/>";
const MIC: &str = "<path d=\"M12 19v3\"/><path d=\"M19 10v2a7 7 0 0 1-14 0v-2\"/>\
<rect x=\"9\" y=\"2\" width=\"6\" height=\"13\" rx=\"3\"/>";
const ARROW_UP: &str = "<path d=\"m5 12 7-7 7 7\"/><path d=\"M12 19V5\"/>";

/// A text-style picker above the card. Not wired yet, so it says so.
fn picker(label: &str, content: &str) -> String {
    format!(
        "<button type=\"button\" aria-disabled=\"true\" title=\"{label} (coming soon)\" \
class=\"tw:inline-flex tw:items-center tw:gap-1 tw:h-6 tw:px-1.5 tw:rounded-md tw:bg-transparent \
tw:text-xs tw:text-noir-content-secondary tw:hover:bg-noir-surface-raised tw:hover:text-noir-content \
tw:active:bg-noir-stroke-subtle\">{content}{}</button>",
        icon(12, CHEVRON_DOWN)
    )
}

/// The homepage and chat composer: the repository, branch, and environment
/// pickers, then a 640 by 195 pixel card holding the text box and its
/// toolbar. Only the text box and the send button do anything yet.
pub(crate) fn composer(action: &str, label: &str) -> String {
    let pickers = format!(
        "{}{}{}",
        picker(
            "Repository",
            "<span class=\"tw:truncate\">openagents</span>"
        ),
        picker("Branch", "<span class=\"tw:truncate\">main</span>"),
        picker("Environment", &icon(14, CLOUD)),
    );
    let round = "tw:inline-flex tw:items-center tw:justify-center tw:size-6 tw:shrink-0 \
tw:rounded-full tw:p-0";
    let quiet = "tw:bg-noir-surface-raised tw:text-noir-content-secondary \
tw:hover:bg-noir-stroke-subtle tw:hover:text-noir-content tw:active:bg-noir-stroke";
    format!(
        "<section class=\"composer tw:w-full tw:max-w-[640px]\" aria-label=\"{label}\">\
<form id=\"chat-form\" action=\"{action}\" method=\"post\">\
<div class=\"tw:flex tw:items-center tw:gap-2 tw:min-h-8 tw:px-1.5 tw:pb-1.5\">{pickers}</div>\
<div id=\"chat-card\" class=\"chat-composer-card tw:relative tw:flex tw:flex-col tw:overflow-hidden \
tw:w-full tw:h-[195px] tw:rounded-xl tw:cursor-text tw:border tw:border-noir-stroke-subtle \
tw:bg-noir-surface-subtle tw:focus-within:border-noir-stroke\">\
<label class=\"unseen\" for=\"chat-input\">Message</label>\
<textarea id=\"chat-input\" name=\"q\" rows=\"4\" maxlength=\"{MAX_CHARS}\" required autofocus \
placeholder=\"Ask OpenAgents to build, fix bugs, explore\" \
class=\"tw:block tw:flex-1 tw:w-full tw:min-h-[72px] tw:max-h-[400px] tw:m-0 tw:px-3 tw:py-3 \
tw:border-0 tw:bg-transparent tw:resize-none tw:font-mono tw:text-sm \
tw:text-noir-content tw:placeholder:text-noir-content-secondary tw:outline-none \
tw:focus-visible:outline-none\"></textarea>\
<div class=\"tw:flex tw:items-center tw:gap-3 tw:px-3 tw:py-3\">\
<button type=\"button\" aria-disabled=\"true\" aria-label=\"Add context and tools\" \
title=\"Add context and tools (coming soon)\" class=\"{round} {quiet}\">{plus}</button>\
<button type=\"button\" aria-disabled=\"true\" title=\"Model (coming soon)\" \
class=\"tw:inline-flex tw:items-center tw:gap-1 tw:h-6 tw:pl-2 tw:pr-1.5 tw:rounded-full \
tw:bg-transparent tw:text-xs tw:text-noir-content-secondary tw:hover:bg-noir-surface-raised \
tw:hover:text-noir-content tw:active:bg-noir-stroke-subtle\">Auto{chevron}</button>\
<div class=\"tw:flex-1\"></div>\
<button type=\"button\" aria-disabled=\"true\" aria-label=\"Voice input\" \
title=\"Voice input (coming soon)\" class=\"{round} {quiet}\">{mic}</button>\
<button type=\"submit\" aria-label=\"Send\" title=\"Send\" class=\"{round} \
tw:bg-noir-accent-solid tw:text-noir-on-accent-solid tw:hover:bg-noir-content-secondary \
tw:active:bg-noir-content-tertiary\">{arrow}</button>\
</div></div></form></section>\
<script src=\"/static/chat.js\" defer></script>",
        plus = icon(14, PLUS),
        chevron = icon(12, CHEVRON_DOWN),
        mic = icon(14, MIC),
        arrow = icon(16, ARROW_UP),
    )
}

fn normalize(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_CHARS {
        return None;
    }
    Some(trimmed.to_owned())
}

fn valid_id(id: &str) -> bool {
    let mut parts = id.split('-');
    matches!(
        (
            parts.next().map(|p| p.len() == 8 && hex(p)),
            parts.next().map(|p| p.len() == 4 && hex(p)),
            parts
                .next()
                .map(|p| p.len() == 4 && p.starts_with('4') && hex(p)),
            parts.next().map(|p| p.len() == 4
                && p.as_bytes()
                    .first()
                    .is_some_and(|b| matches!(b, b'8' | b'9' | b'a' | b'b'))
                && hex(p)),
            parts.next().map(|p| p.len() == 12 && hex(p)),
            parts.next(),
        ),
        (
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            Some(true),
            None
        )
    )
}

fn hex(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn new_id() -> String {
    let mut bytes = secp256k1::rand::random::<[u8; 16]>();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn missing() -> Response {
    problem(
        StatusCode::NOT_FOUND,
        "Not found",
        "Nothing on this site has that address.",
        ("/", "Home"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_version_four_uuids() {
        let id = new_id();
        assert!(valid_id(&id), "{id}");
        assert!(!valid_id("not-a-uuid"));
        assert!(!valid_id("00000000-0000-0000-0000-000000000000"));
    }
}
