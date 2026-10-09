//! `/demo`: public, synthetic example chats in the shared `openagents-ui`
//! shell. The left panel lists the scripted chats; the main area shows the
//! selected thread with the composer docked under it.
//!
//! Switching chats is a plain link that HTMX upgrades to swap only the
//! thread (and push its URL). A message sent from the composer gets an
//! honest scripted reply; nothing is stored, and no agent, provider, or
//! account is involved.

mod chats;
mod view;

use axum::{
    Form, Router,
    extract::{DefaultBodyLimit, Path},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use maud::html;
use serde::Deserialize;

use crate::App;
use crate::ui_page::UiPage;
use chats::{CHATS, DemoChat};
use openagents_ui::shell::Message;

/// Scripts come from this origin only (`ui.js`, Alpine, HTMX); no inline
/// script or style, and no request leaves the site.
const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// HTMX never evaluates expressions or fragment scripts, and never caches
/// pages in history (a back navigation reloads).
const HTMX_CONFIG: &str = r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"historyRestoreAsHxRequest":false,"refreshOnHistoryMiss":true,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/demo", get(index))
        .route("/demo/{chat}", get(show))
        .route("/demo/{chat}/thread", get(thread))
        .route("/demo/{chat}/message", post(message))
        .layer(DefaultBodyLimit::max(32 * 1024))
}

fn protect(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::VARY, HeaderValue::from_static("Cookie, HX-Request"));
    response
}

fn missing(headers: &HeaderMap) -> Response {
    protect(crate::ui_page::problem(
        headers,
        StatusCode::NOT_FOUND,
        "Demo chat not found",
        "There's no demo chat at this address.",
        ("/demo", "Back to the demo"),
    ))
}

/// The full page for `chat`, with `extra` turns after its script.
fn page(headers: &HeaderMap, chat: &DemoChat, extra: &[Message]) -> Response {
    let path = format!("/demo/{}", chat.slug);
    let page = UiPage::new(chat.title)
        .path(path)
        .app()
        .head(html! {
            meta name="htmx-config" content=(HTMX_CONFIG);
            script src="/static/htmx.min.js" defer {}
        })
        .breadcrumb(view::breadcrumb(chat, false))
        .sidebar_section(view::chat_list(chat, false))
        .content(html! {
            div id=(view::CONTENT_ID) class="oa-thread-view" { (view::thread(chat, extra)) }
        })
        .composer(view::dock(chat, false));
    protect(page.respond(headers))
}

async fn index(headers: HeaderMap) -> Response {
    page(&headers, &CHATS[0], &[])
}

async fn show(headers: HeaderMap, Path(slug): Path<String>) -> Response {
    // The earlier demo addressed chats by number (`/demo/5`).
    if slug.bytes().all(|b| b.is_ascii_digit()) {
        return protect(Redirect::to("/demo").into_response());
    }
    match chats::find(&slug) {
        Some(chat) => page(&headers, chat, &[]),
        None => missing(&headers),
    }
}

/// The thread alone, for HTMX: the selected thread, plus the chat list and
/// composer out of band so the highlight and the post address follow.
async fn thread(headers: HeaderMap, Path(slug): Path<String>) -> Response {
    let Some(chat) = chats::find(&slug) else {
        return missing(&headers);
    };
    let body = html! {
        title { (chat.title) " \u{b7} OpenAgents" }
        (view::thread(chat, &[]))
        (view::breadcrumb(chat, true))
        (view::chat_list(chat, true))
        (view::dock(chat, true))
    };
    let mut response = protect(body.into_response());
    if let Ok(url) = HeaderValue::from_str(&format!("/demo/{}", chat.slug)) {
        response.headers_mut().insert("HX-Push-Url", url);
    }
    response
}

#[derive(Deserialize)]
struct Prompt {
    q: String,
}

/// A message from the composer. With HTMX, the exchange is appended to the
/// thread and the composer is replaced with an empty one; without it, the
/// page is rendered again with the exchange at the end.
async fn message(
    headers: HeaderMap,
    Path(slug): Path<String>,
    Form(prompt): Form<Prompt>,
) -> Response {
    let Some(chat) = chats::find(&slug) else {
        return missing(&headers);
    };
    let text = prompt.q.trim();
    if text.is_empty() || text.chars().count() > view::MAX_PROMPT_CHARS {
        return protect(StatusCode::BAD_REQUEST.into_response());
    }
    let exchange = view::exchange(text);
    if headers.get("hx-request").is_some_and(|h| h == "true") {
        let body = html! {
            div hx-swap-oob="beforeend:#demo-transcript" {
                @for turn in &exchange { (turn) }
            }
            (view::dock(chat, true))
        };
        protect(body.into_response())
    } else {
        page(&headers, chat, &exchange)
    }
}

#[cfg(test)]
mod tests;
