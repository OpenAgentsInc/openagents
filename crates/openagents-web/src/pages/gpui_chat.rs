//! `/gpui` and `/gpui/chat/{id}`: the account's chats drawn by the
//! OpenAgents desktop app's GPUI interface, compiled to WebAssembly.
//!
//! This is a second renderer beside the HTML chat at `/` and `/chat/{id}`,
//! which it leaves alone; `docs/audits/2026-10-19-gpui-web-chat.md` says how
//! to make it the default. The app is a client of the chat API the phone
//! uses (`/v1/threads`, `/v1/threads/{id}/events`, `/v1/projects`), signed
//! in by the site's cookie through [`crate::cookie_api`], with the CSRF value
//! this page puts in its `openagents-csrf` meta tag.
//!
//! The page paints a plain HTML outline of the chat list first, then its
//! start script checks for WebGPU or WebGL2, fetches the module and the
//! fonts together, and swaps the outline for the canvas. Without either
//! graphics API it says the chat needs a newer browser and links to the
//! regular chat. `apps/openagents/scripts/build-web-chat.sh DIR` writes the
//! build this server reads (`--gpui-chat DIR`, default `/srv/gpui-chat`):
//!
//! ```text
//! DIR/openagents_web_chat.js        wasm-bindgen's `--target web` glue
//! DIR/openagents_web_chat_bg.wasm   the module
//! DIR/start.js                      checks the browser, loads, and launches
//! DIR/fonts/*.ttf                   the faces the chat draws
//! DIR/**/*.gz, *.br                 compressed copies, sent when accepted
//! ```

use std::path::Path;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::chat_owner::Who;
use crate::ui_page::{UiPage, action_link};

pub(crate) const GPUI_CHAT_PATH: &str = "/gpui";
const BUILD_PATH: &str = "/gpui/build";
const SHELL_CSS_PATH: &str = "/gpui/shell.css";

/// The build's files, as `build-web-chat.sh` names them.
pub(crate) const GPUI_CHAT_GLUE: &str = "openagents_web_chat.js";
pub(crate) const GPUI_CHAT_WASM: &str = "openagents_web_chat_bg.wasm";
pub(crate) const GPUI_CHAT_START: &str = "start.js";

/// The page's policy: scripts, the module, styles, and data from this site
/// only, and compiling WebAssembly. GPUI styles its canvas and its text
/// field through the CSSOM, which `style-src` doesn't govern.
pub(crate) const GPUI_CHAT_POLICY: &str = "default-src 'none'; style-src 'self'; \
img-src 'self'; font-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The fonts may be cached for a day: their names change with the faces.
const FONT_CACHE: &str = "public, max-age=86400";
/// The most chats the outline lists.
const OUTLINE_CHATS: usize = 40;

const SHELL_CSS: &str = include_str!("gpui_chat.css");

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(GPUI_CHAT_PATH, get(new_chat))
        .route("/gpui/", get(new_chat))
        .route("/gpui/chat/{id}", get(chat))
        .route(SHELL_CSS_PATH, get(shell_css))
        .route("/gpui/build/{file}", get(build_file))
        .route("/gpui/build/fonts/{file}", get(font_file))
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.gpui_chat.as_deref()?;
    [GPUI_CHAT_GLUE, GPUI_CHAT_WASM, GPUI_CHAT_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

async fn new_chat(State(app): State<App>, headers: HeaderMap) -> Response {
    page(&app, &headers, None).await
}

async fn chat(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    if !crate::chat_store::valid_id(&id) {
        return crate::not_found().await;
    }
    page(&app, &headers, Some(id)).await
}

/// One chat in the outline.
struct Row {
    id: String,
    title: String,
}

/// The canvas page for `chat`, or a new chat. The regular chat routes can
/// answer with this to swap the canvas in (the plan's "How to swap it in").
pub(crate) async fn page(app: &App, headers: &HeaderMap, chat: Option<String>) -> Response {
    let path = match &chat {
        Some(id) => format!("{GPUI_CHAT_PATH}/chat/{id}"),
        None => GPUI_CHAT_PATH.to_owned(),
    };
    let regular = match &chat {
        Some(id) => format!("/chat/{id}"),
        None => "/".to_owned(),
    };
    if build(app).is_none() {
        return unavailable(headers, &path, &regular);
    }
    let owner = match crate::chat_owner::who(app, headers).await {
        Who::Account(owner) => owner,
        Who::Unchecked(_) => {
            return crate::ui_page::problem(
                headers,
                StatusCode::SERVICE_UNAVAILABLE,
                "Chat",
                "We couldn't check your login. Try again in a minute.",
                ("Open the regular chat", "/"),
            );
        }
        Who::Browser(_) | Who::Nobody => {
            return Redirect::to(&crate::auth::login_href(&path, false)).into_response();
        }
    };
    let rows: Vec<Row> = crate::phone_api::listed(&app.config.chat_store, &owner)
        .await
        .map(|(rows, _)| {
            rows.into_iter()
                .take(OUTLINE_CHATS)
                .map(|row| Row {
                    id: row["id"].as_str().unwrap_or_default().to_owned(),
                    title: row["title"].as_str().unwrap_or_default().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    let title = chat
        .as_deref()
        .and_then(|id| rows.iter().find(|row| row.id == id))
        .map(|row| row.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| if chat.is_some() { "Chat" } else { "New chat" }.to_owned());
    let csrf = crate::pages::chat::csrf(app, &owner);
    let document = shell(&title, &csrf, &rows, chat.as_deref(), &regular);
    let mut response = Html(document.into_string()).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(GPUI_CHAT_POLICY),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    response
}

/// The page: the outline the canvas replaces, the message for browsers
/// that can't draw it, and the start script.
fn shell(title: &str, csrf: &str, rows: &[Row], current: Option<&str>, regular: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover";
                meta name="color-scheme" content="dark";
                meta name="robots" content="noindex";
                meta name="openagents-csrf" content=(csrf);
                meta name="openagents-chat-base" content=(GPUI_CHAT_PATH);
                title { (title) " \u{b7} OpenAgents" }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                link rel="stylesheet" href=(SHELL_CSS_PATH);
                link rel="modulepreload" href={ (BUILD_PATH) "/" (GPUI_CHAT_GLUE) };
            }
            body {
                div #gpui-shell .g-shell {
                    aside .g-side {
                        div .g-brand { "OpenAgents" }
                        a .g-new href=(GPUI_CHAT_PATH) { "New chat" }
                        nav .g-list aria-label="Chats" {
                            @for row in rows {
                                a .g-row .g-current[current == Some(row.id.as_str())]
                                    href={ (GPUI_CHAT_PATH) "/chat/" (row.id) } {
                                    @if row.title.is_empty() { "New chat" } @else { (row.title) }
                                }
                            }
                        }
                    }
                    main .g-main {
                        header .g-head { h1 .g-title { (title) } }
                        p #gpui-status .g-status aria-live="polite" { "Loading\u{2026}" }
                        div #gpui-unsupported .g-unsupported hidden {
                            h2 { "This chat needs a newer browser" }
                            p { "Use the latest Chrome, Edge, Safari, or Firefox, or open this chat in the regular view." }
                            a .g-button href=(regular) { "Open the regular chat" }
                        }
                        noscript {
                            div .g-unsupported {
                                h2 { "Turn on JavaScript to use this chat" }
                                a .g-button href=(regular) { "Open the regular chat" }
                            }
                        }
                    }
                }
                script type="module" src={ (BUILD_PATH) "/" (GPUI_CHAT_START) } {}
            }
        }
    }
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(headers: &HeaderMap, path: &str, regular: &str) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="gpui-chat-title" {
            (MarkdownRoot::new(html! {
                h1 id="gpui-chat-title" { "Chat" }
                p.oa-page-lead { "This view of your chats isn't available here right now." }
            }))
            div.oa-page-actions {
                (action_link("Open the regular chat", regular))
            }
        }
    });
    UiPage::new("Chat")
        .path(path)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn shell_css() -> Response {
    let mut response = (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        SHELL_CSS,
    )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    response
}

async fn build_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) = (build(&app), super::everglade::build_type(&file))
    else {
        return crate::not_found().await;
    };
    super::everglade::serve_build(directory, &file, content_type, &request).await
}

async fn font_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) = (build(&app), font_type(&file)) else {
        return crate::not_found().await;
    };
    super::everglade::serve_build_cached(
        &directory.join("fonts"),
        &file,
        content_type,
        &request,
        FONT_CACHE,
    )
    .await
}

/// A font file's type, for a plain name ending in `.ttf` or `.otf`.
fn font_type(name: &str) -> Option<&'static str> {
    let plain = !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !plain {
        return None;
    }
    if name.ends_with(".ttf") {
        Some("font/ttf")
    } else if name.ends_with(".otf") {
        Some("font/otf")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        vec![
            Row {
                id: "c1".into(),
                title: "Fix the build <now>".into(),
            },
            Row {
                id: "c2".into(),
                title: String::new(),
            },
        ]
    }

    #[test]
    fn the_policy_allows_only_same_origin_scripts_wasm_and_data() {
        assert!(GPUI_CHAT_POLICY.starts_with("default-src 'none'"));
        assert!(GPUI_CHAT_POLICY.contains("script-src 'self' 'wasm-unsafe-eval';"));
        assert!(GPUI_CHAT_POLICY.contains("connect-src 'self';"));
        assert!(!GPUI_CHAT_POLICY.contains("unsafe-inline"));
    }

    #[test]
    fn the_shell_paints_the_chat_list_and_loads_the_start_script() {
        let html = shell("Chat", "abc123", &rows(), Some("c1"), "/chat/c1").into_string();
        assert!(html.contains("src=\"/gpui/build/start.js\""));
        assert!(html.contains("rel=\"modulepreload\" href=\"/gpui/build/openagents_web_chat.js\""));
        assert!(html.contains("name=\"openagents-csrf\" content=\"abc123\""));
        assert!(html.contains("name=\"openagents-chat-base\" content=\"/gpui\""));
        assert!(html.contains("href=\"/gpui/chat/c1\""));
        assert!(html.contains("Fix the build &lt;now&gt;"));
        assert!(html.contains("class=\"g-row g-current\""));
        assert!(html.contains("id=\"gpui-unsupported\""));
        assert!(html.contains("href=\"/chat/c1\""));
    }

    #[test]
    fn the_shell_copy_is_plain_and_names_no_upstream() {
        let html = shell("New chat", "x", &rows(), None, "/").into_string();
        let text = oa_copy::visible_text(&html);
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        assert!(!html.to_lowercase().contains("zeron"));
        let css = SHELL_CSS.to_lowercase();
        assert!(!css.contains("zeron"));
    }

    #[test]
    fn only_font_files_are_served_from_the_font_folder() {
        assert_eq!(font_type("Geist.ttf"), Some("font/ttf"));
        assert_eq!(font_type("../Geist.ttf"), None);
        assert_eq!(font_type("start.js"), None);
    }
}
