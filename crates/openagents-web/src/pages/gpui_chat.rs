//! The GPUI chat renderer at `/` and `/chat/{id}`, with an HTML fallback.
//!
//! `/gpui` and `/gpui/chat/{id}` permanently redirect to those addresses.
//! Builds, text, and side-panel fragments keep their `/gpui/` paths.
//! `?ui=gpui` selects GPUI and remembers the choice; `?ui=html` opens the
//! HTML chat once, and `?ui=off` clears the choice. The `gpui_default`
//! feature selects GPUI when the browser supports it. The app calls the
//! same chat API as the phone, using the site's cookie and CSRF value.
//!
//! The page paints a plain HTML outline of the chat list first, then its
//! start script selects WebGPU or WebGL2, fetches that module and the
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
use axum::extract::{OriginalUri, Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::chat_owner::Who;
use crate::ui_page::{UiPage, action_link};

pub(crate) const GPUI_CHAT_PATH: &str = "/gpui";
/// The cookie that keeps the GPUI chat on at `/` and `/chat/{id}`.
pub(crate) const UI_COOKIE: &str = "oa_chat_ui";
/// The query switch: `?ui=gpui` turns the GPUI chat on, `?ui=html` shows
/// the HTML chat once, and `?ui=off` turns the GPUI chat off.
const UI_QUERY: &str = "ui";
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
img-src 'self'; font-src 'self'; media-src 'self'; frame-src 'self'; \
script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The fonts may be cached for a day: their names change with the faces.
const FONT_CACHE: &str = "public, max-age=86400";
/// The most chats the outline lists.
const OUTLINE_CHATS: usize = 40;

const SHELL_CSS: &str = include_str!("gpui_chat.css");

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(GPUI_CHAT_PATH, get(moved))
        .route("/gpui/", get(moved))
        .route("/gpui/chat/{id}", get(moved))
        .route(SHELL_CSS_PATH, get(shell_css))
        .route("/gpui/build/{file}", get(build_file))
        .route("/gpui/build/visuals/{file}", get(visual_file))
        .route("/gpui/build/fonts/{file}", get(font_file))
        .route(SIDE_PATH, get(side))
        .route(FILES_PATH, get(files))
        .route(TEXT_PATH, get(text))
}

/// The side panel's live part: what the open chat waits on (#12223).
const SIDE_PATH: &str = "/gpui/side";
/// The side panel's "Send with files" composer.
const FILES_PATH: &str = "/gpui/side/files";
/// The text view: the open chat as the HTML chat draws it (#12220).
const TEXT_PATH: &str = "/gpui/text";
/// The event the start script sends when the canvas opens another chat.
const CHAT_EVENT: &str = "oa-chat";

/// The chat a fragment request is for: `?chat=`, else the page's own
/// address, which HTMX sends as `HX-Current-URL` (`/chat/{id}` or
/// `/gpui/chat/{id}`). `None` for a new chat.
fn open_chat(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    let named = query.and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "chat").then(|| value.to_owned())
        })
    });
    let id = named.or_else(|| {
        let url = headers.get("hx-current-url")?.to_str().ok()?;
        let path = url
            .split_once("://")
            .map_or(url, |(_, rest)| rest.find('/').map_or("", |at| &rest[at..]));
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let id = path
            .strip_prefix(GPUI_CHAT_PATH)
            .unwrap_or(path)
            .strip_prefix("/chat/")?;
        Some(id.trim_end_matches('/').to_owned())
    })?;
    crate::chat_store::valid_id(&id).then_some(id)
}

/// A fragment for the open chat: `draw`'s markup, or nothing for a new
/// chat or one this person can't read.
async fn fragment<F, Fut>(app: &App, headers: &HeaderMap, query: Option<&str>, draw: F) -> Response
where
    F: FnOnce(crate::chat_store::Conversation) -> Fut,
    Fut: std::future::Future<Output = Markup>,
{
    let markup = match open_chat(headers, query) {
        Some(id) => match super::chat::load(app, headers, &id).await {
            Ok(loaded) => draw(loaded.conversation).await,
            Err(_) => html! {},
        },
        None => html! {},
    };
    crate::chat_html::protect(markup.into_response())
}

async fn side(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Response {
    fragment(&app, &headers, query.as_deref(), |chat| {
        let (app, headers) = (app.clone(), headers.clone());
        async move { super::chat::gpui_side(&app, &headers, &chat).await }
    })
    .await
}

async fn files(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Response {
    fragment(&app, &headers, query.as_deref(), |chat| {
        let (app, headers) = (app.clone(), headers.clone());
        async move { super::chat::gpui_files(&app, &headers, &chat).await }
    })
    .await
}

async fn text(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Response {
    fragment(&app, &headers, query.as_deref(), |chat| {
        let (app, headers) = (app.clone(), headers.clone());
        async move { super::chat::gpui_text(&app, &headers, &chat).await }
    })
    .await
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

async fn moved(OriginalUri(uri): OriginalUri) -> Redirect {
    let path = uri.path().strip_prefix(GPUI_CHAT_PATH).unwrap_or("");
    let path = if path.is_empty() || path == "/" {
        "/"
    } else {
        path
    };
    let to = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_owned(),
    };
    Redirect::permanent(&to)
}

#[derive(serde::Deserialize, Default)]
struct PageQuery {
    page: Option<String>,
}

/// The new-chat shell, including the selected page's HTML fallback.
pub(crate) async fn new_page(app: &App, headers: &HeaderMap, query: Option<&str>) -> Response {
    let page = query
        .and_then(|query| format!("/?{query}").parse::<axum::http::Uri>().ok())
        .and_then(|uri| axum::extract::Query::<PageQuery>::try_from_uri(&uri).ok())
        .and_then(|query| query.page.clone());
    let admin_fallback = match page.as_deref() {
        Some("admin-analytics") => Some("/admin/analytics?ui=html"),
        Some("admin-attribution") => Some("/admin/attribution?ui=html"),
        Some("admin-drive-access") => Some("/admin/access/drive-connections?ui=html"),
        _ => None,
    };
    if admin_fallback.is_some() && !crate::analytics::dashboard::allowed(app, headers).await {
        return crate::analytics::dashboard::guarded(crate::not_found_page());
    }
    let fallback = admin_fallback
        .map(str::to_owned)
        .or_else(|| page.as_deref().and_then(cad_fallback));
    page_with_fallback(app, headers, None, "", fallback).await
}

/// Keep a CAD page's document, version, and project in the HTML fallback.
fn cad_fallback(path: &str) -> Option<String> {
    if path == "cad" || path == "cad/" {
        return Some("/cad?ui=html".into());
    }
    let rest = path.strip_prefix("cad/")?;
    let mut parts = rest.split('/');
    let doc = parts.next()?;
    if doc == "_world" {
        return Some("/cad-world?ui=html".into());
    }
    if !crate::cad_web::valid_doc(doc) {
        return None;
    }
    let version = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let project = parts.next().unwrap_or("");
    if !project
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return None;
    }
    let config = parts.next() == Some("configurator");
    let query = format!("ui=html&v={version}&project={project}");
    Some(format!(
        "/cad/{doc}{}?{query}",
        if config { "/configurator" } else { "" }
    ))
}

/// What the person chose for the chat's view at `/` and `/chat/{id}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    /// Draw the GPUI chat.
    pub(crate) gpui: bool,
    /// What to do with [`UI_COOKIE`]: set it (`Some(true)`), clear it
    /// (`Some(false)`), or leave it.
    remember: Option<bool>,
}

/// For an account page the GPUI app also draws (#12329, #12330, #12331):
/// when this browser shows the GPUI chat ([`choice`]), the GPUI page at
/// `?page=PATH` over it; `None` keeps the HTML page, which stays for every
/// other browser and for `?ui=html`.
pub(crate) fn page_for(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    path: &str,
) -> Option<Response> {
    open_with(app, headers, query, "page", path)
}

/// For a work address (#12332): the GPUI chat with its work panel open at
/// `target` (`work:ID`, `env:ID`, `env:ID/run:RUN`, `mac:ID`, or `env`),
/// as [`page_for`] decides.
pub(crate) fn work_for(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    target: &str,
) -> Option<Response> {
    open_with(app, headers, query, "work", target)
}

fn open_with(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    key: &str,
    path: &str,
) -> Option<Response> {
    let choice = choice(app.config.features, headers, query);
    if !choice.serves(app, headers) {
        return None;
    }
    let encoded: String = path
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' | b':' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    let mut response =
        axum::response::Redirect::to(&format!("{GPUI_CHAT_PATH}?{key}={encoded}")).into_response();
    choice.remember(app, &mut response);
    Some(response)
}

/// The view `query` (the request's query string) and the cookie ask for:
/// the query first, then the cookie (`gpui` or `html`); for a browser that
/// chose neither, the GPUI chat when `features` has
/// [`crate::features::Feature::GpuiDefault`] on and it isn't a phone
/// (#12226), else the HTML chat.
pub(crate) fn choice(
    features: crate::features::Features,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Choice {
    let asked = query.and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (key == UI_QUERY).then_some(value)
        })
    });
    match asked {
        Some("gpui") => Choice {
            gpui: true,
            remember: Some(true),
        },
        Some("html") => Choice {
            gpui: false,
            remember: None,
        },
        Some("off") => Choice {
            gpui: false,
            remember: Some(false),
        },
        _ => {
            let kept = headers
                .get_all(header::COOKIE)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .flat_map(|value| value.split(';'))
                .filter_map(|pair| pair.trim().split_once('='))
                .find(|(name, _)| *name == UI_COOKIE)
                .map(|(_, value)| value.to_owned());
            let gpui = match kept.as_deref() {
                Some("gpui") => true,
                Some("html") => false,
                _ => features.on(crate::features::Feature::GpuiDefault) && !phone(headers),
            };
            Choice {
                gpui,
                remember: None,
            }
        }
    }
}

/// Whether the browser says it's a phone: phones keep the HTML chat until
/// they pass their own check (the plan's "Phones").
fn phone(headers: &HeaderMap) -> bool {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|agent| {
            ["Mobi", "Android", "iPhone", "iPad"]
                .iter()
                .any(|word| agent.contains(word))
        })
}

impl Choice {
    /// Whether the GPUI chat answers this request at a regular address. An
    /// in-page HTMX swap keeps the HTML chat it came from, and a server
    /// without the build has nothing to show.
    pub(crate) fn serves(&self, app: &App, headers: &HeaderMap) -> bool {
        self.gpui && !headers.contains_key("hx-request") && build(app).is_some()
    }

    /// Keeps or clears [`UI_COOKIE`] on `response`, as the query asked.
    pub(crate) fn remember(&self, app: &App, response: &mut Response) {
        let Some(on) = self.remember else {
            return;
        };
        let secure = if app.config.secure_cookies {
            "; Secure"
        } else {
            ""
        };
        // `?ui=off` remembers the HTML chat, so a default of GPUI (#12226)
        // doesn't bring the canvas back.
        let view = if on { "gpui" } else { "html" };
        let cookie = format!("{UI_COOKIE}={view}; Path=/; Max-Age=31536000; SameSite=Lax{secure}");
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
}

/// One chat in the outline.
struct Row {
    id: String,
    title: String,
}

/// The canvas page for `chat`, or a new chat, with its addresses under
/// `base`: [`GPUI_CHAT_PATH`], or `""` when the regular chat routes answer
/// with it (the plan's "How to swap it in"). Signed out, the page is the
/// browser's visitor's, who gets a visitor cookie here when it has none.
pub(crate) async fn page(
    app: &App,
    headers: &HeaderMap,
    chat: Option<String>,
    base: &str,
) -> Response {
    page_with_fallback(app, headers, chat, base, None).await
}

pub(crate) async fn settings_page(app: &App, headers: &HeaderMap, fallback: &str) -> Response {
    page_with_fallback(app, headers, None, "", Some(fallback.to_owned())).await
}
async fn page_with_fallback(
    app: &App,
    headers: &HeaderMap,
    chat: Option<String>,
    base: &str,
    fallback: Option<String>,
) -> Response {
    let path = match &chat {
        Some(id) => format!("{base}/chat/{id}"),
        None if base.is_empty() => "/".to_owned(),
        None => base.to_owned(),
    };
    // The HTML page, for a browser that can't draw the canvas.
    let regular = fallback.unwrap_or_else(|| match &chat {
        Some(id) => format!("/chat/{id}?{UI_QUERY}=html"),
        None => format!("/new?{UI_QUERY}=html"),
    });
    if build(app).is_none() {
        return unavailable(headers, &path, &regular);
    }
    let (owner, signed_in, fresh) = match crate::chat_owner::who(app, headers).await {
        Who::Account(owner) => (owner, true, false),
        Who::Browser(owner) => (owner, false, false),
        Who::Nobody => (crate::ask::new_visitor(), false, true),
        Who::Unchecked(_) => {
            return crate::ui_page::problem(
                headers,
                StatusCode::SERVICE_UNAVAILABLE,
                "Chat",
                "We couldn't check your login. Try again in a minute.",
                ("Open the regular chat", "/"),
            );
        }
    };
    let rows: Vec<Row> = if fresh {
        Vec::new()
    } else {
        crate::phone_api::listed(&app.config.chat_store, &owner)
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
            .unwrap_or_default()
    };
    let title = chat
        .as_deref()
        .and_then(|id| rows.iter().find(|row| row.id == id))
        .map(|row| row.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| if chat.is_some() { "Chat" } else { "New chat" }.to_owned());
    let csrf = crate::pages::chat::csrf(app, &owner);
    // The shared font files (`super::gpui_fonts`) when the site has them;
    // otherwise the start script loads the build's own copies.
    let fonts = super::gpui_fonts::fonts(app)
        .and_then(|fonts| fonts.preloads(&super::gpui_fonts::CHAT_FACES));
    // The frame this browser kept from its last visit to this page, which
    // the painter shows before the module loads (#12244). Kept per person
    // (a digest of the page's CSRF value) and dropped when the outline
    // of chats changes.
    let painter = build(app).and_then(|dir| {
        super::first_frame::Painter::cached(&dir.join(super::first_frame::DIRECTORY))
    });
    let first = painter.as_ref().map(|painter| {
        let mut outline = format!("{}\n", chat.as_deref().unwrap_or(""));
        for row in &rows {
            outline.push_str(&format!("{}\t{}\n", row.id, row.title));
        }
        painter.kept(
            &format!(
                "/__first-frame/chat/{}{path}",
                super::first_frame::short_digest(&csrf)
            ),
            &super::first_frame::short_digest(&outline),
        )
    });
    let document = shell(&Shell {
        title: &title,
        csrf: &csrf,
        rows: &rows,
        current: chat.as_deref(),
        regular: &regular,
        base,
        signed_in,
        fonts,
        build: build(app),
        first,
        features: app.config.features,
    });
    let mut response = Html(document.into_string()).into_response();
    let policy = match &painter {
        Some(painter) => painter.policy(GPUI_CHAT_POLICY),
        None => GPUI_CHAT_POLICY.to_owned(),
    };
    // The account's rooms on `coder-sync` (#12263).
    let policy = match crate::room_tokens::connect_source(app) {
        Some(socket) => policy.replacen(
            "connect-src 'self'",
            &format!("connect-src 'self' {socket}"),
            1,
        ),
        None => policy,
    };
    let headers = response.headers_mut();
    if let Ok(policy) = HeaderValue::from_str(&policy) {
        headers.insert(header::CONTENT_SECURITY_POLICY, policy);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex"));
    crate::pages::chat::cookie(app, &owner, fresh, &mut response);
    response
}

/// What the page is drawn from.
struct Shell<'a> {
    title: &'a str,
    csrf: &'a str,
    rows: &'a [Row],
    current: Option<&'a str>,
    /// The HTML chat's address for this chat.
    regular: &'a str,
    /// The app's addresses start with this ([`GPUI_CHAT_PATH`] or `""`).
    base: &'a str,
    signed_in: bool,
    /// The shared font files' preloads, when the site has them.
    fonts: Option<Markup>,
    /// The build's directory, for its content-hashed names.
    build: Option<&'a Path>,
    /// The painter and the kept frame's address, when the build has the
    /// painter.
    first: Option<Markup>,
    /// The features that are on, for the canvas to hide the rest
    /// ([`crate::features::META`]).
    features: crate::features::Features,
}

/// The page: the outline the canvas replaces, the message for browsers
/// that can't draw it, and the start script.
fn shell(page: &Shell<'_>) -> Markup {
    let Shell {
        title,
        csrf,
        rows,
        current,
        regular,
        base,
        signed_in,
        ref fonts,
        build,
        ref first,
        features,
    } = *page;
    let settings_page = match regular.split('?').next().unwrap_or_default() {
        "/settings/connections" => Some("connections"),
        "/settings/terminal" => Some("terminal"),
        "/settings/vault" => Some("vault"),
        path if path.starts_with("/projects/") && path.ends_with("/vault") => Some("vault"),
        _ => None,
    };
    let new_chat = if base.is_empty() { "/" } else { base };
    html! {
        (DOCTYPE)
        // The canvas is dark, so the HTML chat's parts beside it are too.
        html lang="en" data-theme="dark" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover";
                meta name="color-scheme" content="dark";
                meta name="robots" content="noindex";
                meta name="openagents-csrf" content=(csrf);
                meta name="openagents-chat-base" content=(base);
                meta name="openagents-signed-in" content=(if signed_in { "1" } else { "0" });
                @if let Some(page) = settings_page {
                    meta name="openagents-settings" content=(page);
                }
                meta name=(crate::features::META) content=(features.meta());
                title { (title) " \u{b7} OpenAgents" }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                (PreEscaped(crate::theme::style_tag()))
                link rel="stylesheet" href=(SHELL_CSS_PATH);
                (PreEscaped(crate::theme::script_tags()))
                (crate::chat_html::fragment_head())
                @if let Some(fonts) = fonts { (fonts) }
            }
            body {
                @if let Some(first) = first { (first) }
                div #gpui-shell .g-shell {
                    aside .g-side {
                        div .g-brand { "OpenAgents" }
                        a .g-new href=(new_chat) { "New chat" }
                        nav .g-list aria-label="Chats" {
                            @for row in rows {
                                a .g-row .g-current[current == Some(row.id.as_str())]
                                    href={ (base) "/chat/" (row.id) } {
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
                // What the canvas doesn't draw yet, as the HTML chat draws
                // it (#12223): read again when the canvas opens another
                // chat, and every few seconds.
                aside #gpui-side .g-side-layer aria-label="Chat actions" {
                    div #gpui-side-live hx-get=(SIDE_PATH)
                        hx-trigger={ "load, " (CHAT_EVENT) " from:window, every 6s" } {}
                    div #gpui-side-files hx-get=(FILES_PATH)
                        hx-trigger={ "load, " (CHAT_EVENT) " from:window" } {}
                }
                // The text view (#12220): the open chat's text, live, for
                // screen readers, links, and the browser's find. Cmd+F or
                // Ctrl+F shows it; it stays readable to assistive
                // technology while hidden.
                section #gpui-text .g-text aria-label="Chat text" {
                    div .g-text-bar {
                        button #gpui-text-toggle .g-text-toggle type="button" aria-expanded="false" { "Show the chat as text" }
                    }
                    div #gpui-text-body hx-get=(TEXT_PATH)
                        hx-trigger={ "load, " (CHAT_EVENT) " from:window" } {}
                }
                script type="module" src=(super::gpui_build::url(build, BUILD_PATH, GPUI_CHAT_START)) {}
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

async fn visual_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) =
        (build(&app), super::gpui_build::content_type(&file))
    else {
        return crate::not_found().await;
    };
    super::gpui_build::serve(
        &directory.join("visuals"),
        &format!("{}/visuals", BUILD_PATH),
        &file,
        content_type,
        &request,
    )
    .await
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
    super::gpui_build::serve(directory, BUILD_PATH, &file, content_type, &request).await
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
    fn cad_fallback_keeps_the_document_version_and_project() {
        assert_eq!(cad_fallback("cad"), Some("/cad?ui=html".into()));
        assert_eq!(
            cad_fallback("cad/bracket/2/prj_1/configurator"),
            Some("/cad/bracket/configurator?ui=html&v=2&project=prj_1".into())
        );
        assert!(cad_fallback("cad/../2/").is_none());
        assert!(cad_fallback("cad/bracket/2/evil?x=1").is_none());
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
        let rows = rows();
        let html = shell(&Shell {
            title: "Chat",
            csrf: "abc123",
            rows: &rows,
            current: Some("c1"),
            regular: "/chat/c1?ui=html",
            base: GPUI_CHAT_PATH,
            signed_in: true,
            fonts: None,
            build: None,
            first: None,
            features: crate::features::Features::all(),
        })
        .into_string();
        assert!(html.contains("src=\"/gpui/build/start.js\""));
        assert!(!html.contains("rel=\"modulepreload\""));
        assert!(!html.contains("as=\"fetch\""));
        assert!(html.contains("name=\"openagents-csrf\" content=\"abc123\""));
        assert!(html.contains("name=\"openagents-features\" content=\"wallet,earn,"));
        assert!(html.contains("name=\"openagents-chat-base\" content=\"/gpui\""));
        assert!(html.contains("href=\"/gpui/chat/c1\""));
        assert!(html.contains("Fix the build &lt;now&gt;"));
        assert!(html.contains("class=\"g-row g-current\""));
        assert!(html.contains("id=\"gpui-unsupported\""));
        assert!(html.contains("href=\"/chat/c1?ui=html\""));
        assert!(html.contains("name=\"openagents-signed-in\" content=\"1\""));
    }

    #[test]
    fn at_the_regular_addresses_the_shell_links_there() {
        let rows = rows();
        let html = shell(&Shell {
            title: "New chat",
            csrf: "x",
            rows: &rows,
            current: None,
            regular: "/new?ui=html",
            base: "",
            signed_in: false,
            fonts: None,
            build: None,
            first: None,
            features: crate::features::Features::all(),
        })
        .into_string();
        assert!(html.contains("name=\"openagents-chat-base\" content=\"\""));
        assert!(html.contains("name=\"openagents-signed-in\" content=\"0\""));
        assert!(html.contains("class=\"g-new\" href=\"/\""));
        assert!(html.contains("href=\"/chat/c1\""));
    }

    #[test]
    fn fragments_name_the_open_chat_from_the_page_address() {
        let id = "5e1f0c3a-7d2b-4c4e-9a1b-000000000001";
        let mut headers = HeaderMap::new();
        assert_eq!(open_chat(&headers, None), None);
        headers.insert(
            "hx-current-url",
            HeaderValue::from_str(&format!("https://openagents.com/chat/{id}?ui=gpui")).unwrap(),
        );
        assert_eq!(open_chat(&headers, None).as_deref(), Some(id));
        headers.insert(
            "hx-current-url",
            HeaderValue::from_str(&format!("http://127.0.0.1:4300/gpui/chat/{id}/")).unwrap(),
        );
        assert_eq!(open_chat(&headers, None).as_deref(), Some(id));
        headers.insert(
            "hx-current-url",
            HeaderValue::from_static("https://openagents.com/"),
        );
        assert_eq!(open_chat(&headers, None), None);
        headers.insert(
            "hx-current-url",
            HeaderValue::from_static("https://openagents.com/chat/../settings"),
        );
        assert_eq!(open_chat(&headers, None), None);
        assert_eq!(
            open_chat(&headers, Some(&format!("chat={id}"))).as_deref(),
            Some(id)
        );
    }

    #[test]
    fn the_shell_carries_the_side_panel_and_the_text_view() {
        let rows = rows();
        let html = shell(&Shell {
            title: "Chat",
            csrf: "x",
            rows: &rows,
            current: None,
            regular: "/new?ui=html",
            base: "",
            signed_in: true,
            fonts: None,
            build: None,
            first: None,
            features: crate::features::Features::all(),
        })
        .into_string();
        assert!(html.contains("id=\"gpui-side-live\" hx-get=\"/gpui/side\""));
        assert!(html.contains("id=\"gpui-side-files\" hx-get=\"/gpui/side/files\""));
        assert!(html.contains("id=\"gpui-text-body\" hx-get=\"/gpui/text\""));
        assert!(html.contains("oa-chat from:window"));
        assert!(html.contains("src=\"/static/htmx.min.js\""));
        assert!(!html.contains("chat-start.js"));
    }

    fn cookie(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn the_query_then_the_cookie_choose_the_view() {
        let none = HeaderMap::new();
        assert!(!choice(crate::features::Features::none(), &none, None).gpui);
        assert!(!choice(crate::features::Features::none(), &none, Some("project=p1")).gpui);
        let on = choice(
            crate::features::Features::none(),
            &none,
            Some("project=p1&ui=gpui"),
        );
        assert!(on.gpui);
        assert_eq!(on.remember, Some(true));
        let kept = cookie("oa_visitor=abc; oa_chat_ui=gpui");
        assert!(choice(crate::features::Features::none(), &kept, None).gpui);
        assert_eq!(
            choice(crate::features::Features::none(), &kept, None).remember,
            None
        );
        let once = choice(crate::features::Features::none(), &kept, Some("ui=html"));
        assert!(!once.gpui);
        assert_eq!(once.remember, None);
        let off = choice(crate::features::Features::none(), &kept, Some("ui=off"));
        assert!(!off.gpui);
        assert_eq!(off.remember, Some(false));
        assert!(
            !choice(
                crate::features::Features::none(),
                &cookie("oa_chat_ui=html"),
                None
            )
            .gpui
        );
    }

    #[test]
    fn the_default_is_gpui_only_when_turned_on_and_not_on_a_phone() {
        use crate::features::{Feature, Features};
        let (on, _) = Features::parse(Feature::GpuiDefault.name());
        let none = HeaderMap::new();
        assert!(!choice(Features::none(), &none, None).gpui);
        assert!(choice(on, &none, None).gpui);
        // A choice the browser made wins over the default.
        assert!(!choice(on, &cookie("oa_chat_ui=html"), None).gpui);
        assert!(!choice(on, &none, Some("ui=html")).gpui);
        let off = choice(on, &none, Some("ui=off"));
        assert!(!off.gpui);
        assert_eq!(off.remember, Some(false));
        // Phones keep the HTML chat unless they ask.
        let mut phone = HeaderMap::new();
        phone.insert(
            header::USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Mobile/15E148",
            ),
        );
        assert!(!choice(on, &phone, None).gpui);
        assert!(choice(on, &phone, Some("ui=gpui")).gpui);
    }

    #[test]
    fn ui_off_remembers_the_html_chat() {
        let root = tempfile::tempdir().unwrap();
        let config = crate::Config::development(root.path().join("store"));
        let app = crate::App(std::sync::Arc::new(crate::Inner { config }));
        let mut response = axum::response::Response::new(axum::body::Body::empty());
        choice(
            crate::features::Features::none(),
            &HeaderMap::new(),
            Some("ui=off"),
        )
        .remember(&app, &mut response);
        let set = response
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(set.starts_with("oa_chat_ui=html;"), "{set}");
    }

    #[test]
    fn an_htmx_swap_keeps_the_html_chat() {
        let mut headers = cookie("oa_chat_ui=gpui");
        let root = tempfile::tempdir().unwrap();
        let config = crate::Config::development(root.path().join("store"));
        let app = crate::App(std::sync::Arc::new(crate::Inner { config }));
        // No build here, so nothing is served either way.
        assert!(!choice(crate::features::Features::none(), &headers, None).serves(&app, &headers));
        headers.insert("hx-request", HeaderValue::from_static("true"));
        assert!(!choice(crate::features::Features::none(), &headers, None).serves(&app, &headers));
    }

    #[test]
    fn the_shell_loads_the_hashed_start_script_without_preloading_a_backend() {
        let dir = tempfile::tempdir().unwrap();
        let hashed = format!("start.{}.js", "a".repeat(64));
        std::fs::write(
            dir.path().join("assets.json"),
            serde_json::to_vec(&std::collections::HashMap::from([(
                GPUI_CHAT_START,
                &hashed,
            )]))
            .unwrap(),
        )
        .unwrap();
        let html = shell(&Shell {
            title: "Chat",
            csrf: "csrf",
            rows: &[],
            current: None,
            regular: "/",
            base: GPUI_CHAT_PATH,
            signed_in: true,
            fonts: None,
            build: Some(dir.path()),
            first: None,
            features: crate::features::Features::all(),
        })
        .into_string();
        assert!(html.contains(&format!("src=\"{BUILD_PATH}/{hashed}\"")));
        assert!(!html.contains("rel=\"modulepreload\""));
        assert!(!html.contains("as=\"fetch\""));
    }

    #[test]
    fn the_shell_copy_is_plain_and_names_no_upstream() {
        let rows = rows();
        let html = shell(&Shell {
            title: "New chat",
            csrf: "x",
            rows: &rows,
            current: None,
            regular: "/new?ui=html",
            base: GPUI_CHAT_PATH,
            signed_in: false,
            fonts: None,
            build: None,
            first: None,
            features: crate::features::Features::all(),
        })
        .into_string();
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
