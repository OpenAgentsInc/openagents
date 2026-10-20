//! `/gpui` and `/gpui/chat/{id}`: the account's chats drawn by the
//! OpenAgents desktop app's GPUI interface, compiled to WebAssembly.
//!
//! It is a second renderer beside the HTML chat at `/` and `/chat/{id}`.
//! Those addresses draw it too when the person turns it on (#12221,
//! #12222): `?ui=gpui` on either address shows it and keeps a cookie
//! ([`UI_COOKIE`]) so later visits show it again, `?ui=html` shows the HTML
//! chat once (the canvas links there for what it doesn't draw yet), and
//! `?ui=off` goes back to the HTML chat and clears the cookie ([`choice`]).
//! Without any of these, the HTML chat stays the default. The app is a client of the chat API the
//! phone uses (`/v1/threads`, `/v1/threads/{id}/events`, `/v1/projects`),
//! signed in by the site's cookie through [`crate::cookie_api`], with the
//! CSRF value this page puts in its `openagents-csrf` meta tag. Signed out,
//! it chats as the browser's visitor (`oa_visitor`), as `/lander8` does.
//!
//! The page paints a plain HTML outline of the chat list first, then its
//! start script checks for WebGPU, fetches the module and the
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
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, html};
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
    page(&app, &headers, None, GPUI_CHAT_PATH).await
}

async fn chat(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    if !crate::chat_store::valid_id(&id) {
        return crate::not_found().await;
    }
    page(&app, &headers, Some(id), GPUI_CHAT_PATH).await
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

/// The view `query` (the request's query string) and the cookie ask for:
/// the query first, then the cookie; the HTML chat when neither says.
pub(crate) fn choice(headers: &HeaderMap, query: Option<&str>) -> Choice {
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
        _ => Choice {
            gpui: headers
                .get_all(header::COOKIE)
                .iter()
                .filter_map(|value| value.to_str().ok())
                .flat_map(|value| value.split(';'))
                .filter_map(|pair| pair.trim().split_once('='))
                .any(|(name, value)| name == UI_COOKIE && value == "gpui"),
            remember: None,
        },
    }
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
        let cookie = if on {
            format!("{UI_COOKIE}=gpui; Path=/; Max-Age=31536000; SameSite=Lax{secure}")
        } else {
            format!("{UI_COOKIE}=; Path=/; Max-Age=0; SameSite=Lax{secure}")
        };
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
    let path = match &chat {
        Some(id) => format!("{base}/chat/{id}"),
        None if base.is_empty() => "/".to_owned(),
        None => base.to_owned(),
    };
    // The HTML chat, for a browser that can't draw the canvas.
    let regular = match &chat {
        Some(id) => format!("/chat/{id}?{UI_QUERY}=html"),
        None => format!("/new?{UI_QUERY}=html"),
    };
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
    });
    let mut response = Html(document.into_string()).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(GPUI_CHAT_POLICY),
    );
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
    } = *page;
    let new_chat = if base.is_empty() { "/" } else { base };
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover";
                meta name="color-scheme" content="dark";
                meta name="robots" content="noindex";
                meta name="openagents-csrf" content=(csrf);
                meta name="openagents-chat-base" content=(base);
                meta name="openagents-signed-in" content=(if signed_in { "1" } else { "0" });
                title { (title) " \u{b7} OpenAgents" }
                link rel="icon" type="image/svg+xml" href="/favicon.svg";
                link rel="stylesheet" href=(SHELL_CSS_PATH);
                link rel="modulepreload" href=(super::gpui_build::url(build, BUILD_PATH, GPUI_CHAT_GLUE));
                link rel="preload" as="fetch" crossorigin="anonymous" href=(super::gpui_build::url(build, BUILD_PATH, GPUI_CHAT_WASM));
                @if let Some(fonts) = fonts { (fonts) }
            }
            body {
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
        })
        .into_string();
        assert!(html.contains("src=\"/gpui/build/start.js\""));
        assert!(html.contains("rel=\"modulepreload\" href=\"/gpui/build/openagents_web_chat.js\""));
        assert!(html.contains("name=\"openagents-csrf\" content=\"abc123\""));
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
        })
        .into_string();
        assert!(html.contains("name=\"openagents-chat-base\" content=\"\""));
        assert!(html.contains("name=\"openagents-signed-in\" content=\"0\""));
        assert!(html.contains("class=\"g-new\" href=\"/\""));
        assert!(html.contains("href=\"/chat/c1\""));
    }

    fn cookie(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn the_query_then_the_cookie_choose_the_view() {
        let none = HeaderMap::new();
        assert!(!choice(&none, None).gpui);
        assert!(!choice(&none, Some("project=p1")).gpui);
        let on = choice(&none, Some("project=p1&ui=gpui"));
        assert!(on.gpui);
        assert_eq!(on.remember, Some(true));
        let kept = cookie("oa_visitor=abc; oa_chat_ui=gpui");
        assert!(choice(&kept, None).gpui);
        assert_eq!(choice(&kept, None).remember, None);
        let once = choice(&kept, Some("ui=html"));
        assert!(!once.gpui);
        assert_eq!(once.remember, None);
        let off = choice(&kept, Some("ui=off"));
        assert!(!off.gpui);
        assert_eq!(off.remember, Some(false));
        assert!(!choice(&cookie("oa_chat_ui=html"), None).gpui);
    }

    #[test]
    fn an_htmx_swap_keeps_the_html_chat() {
        let mut headers = cookie("oa_chat_ui=gpui");
        let root = tempfile::tempdir().unwrap();
        let config = crate::Config::development(root.path().join("store"));
        let app = crate::App(std::sync::Arc::new(crate::Inner { config }));
        // No build here, so nothing is served either way.
        assert!(!choice(&headers, None).serves(&app, &headers));
        headers.insert("hx-request", HeaderValue::from_static("true"));
        assert!(!choice(&headers, None).serves(&app, &headers));
    }

    #[test]
    fn the_preload_and_loader_use_the_same_hashed_module() {
        let dir = tempfile::tempdir().unwrap();
        let hashed = format!("openagents_web_chat_bg.{}.wasm", "a".repeat(64));
        std::fs::write(
            dir.path().join("assets.json"),
            serde_json::to_vec(&std::collections::HashMap::from([(
                GPUI_CHAT_WASM,
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
        })
        .into_string();
        assert!(html.contains(&format!("href=\"{BUILD_PATH}/{hashed}\"")));
        assert!(!html.contains(&format!("href=\"{BUILD_PATH}/{GPUI_CHAT_WASM}\"")));
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
