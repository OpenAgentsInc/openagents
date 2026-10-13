//! The screens gallery: every screen a person meets on openagents.com, in
//! every state, with sample data, so the whole site can be reviewed
//! without an account, a computer, or a running service.
//!
//! `/ui` shows the `openagents-ui` builders (the atoms and molecules). This
//! gallery shows what they are built into: the organisms (the app shell,
//! the composer, the chat list) and the pages (sign-in, device approval,
//! chat, settings, ...). Each [`State`] is a real page response built from
//! the same page functions the live routes use, over fixture data only.
//!
//! - `/ui/screens` lists every screen by group with its states.
//! - `/ui/screens/{screen}` shows every state of one screen, in Coder Light
//!   and Coder Noir side by side, each in its own frame.
//! - `/ui/screens/{screen}/{state}?theme=light|dark` is one state as a
//!   whole page, which the frames load.
//!
//! Every state is a sample and nothing on it is live: its page carries a
//! "Sample" strip, its policy allows no form posts and no requests
//! (`form-action 'none'`, `connect-src 'none'`), and the frames are
//! sandboxed without forms or popups. Links still open the real pages.

mod account;
mod auth;
mod chat;
mod site;
mod traces;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::shell::{THEME_COOKIE, Theme};
use serde::Deserialize;

use crate::App;
use crate::ui_page::UiPage;

/// Where the gallery lives.
pub(crate) const PAGE: &str = "/ui/screens";

/// A renderer for one state: the page as the live route would answer it,
/// over fixture data, for a request with `headers`.
pub(crate) type Render = fn(&HeaderMap) -> Response;

/// Which layer of the design a screen belongs to. Atoms and molecules live
/// in `/ui`; this gallery adds the two larger layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    /// A part of a page people use as one thing: the app shell, the
    /// composer, the chat list.
    Organism,
    /// A whole page.
    Page,
}

impl Group {
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Organism => "Organisms",
            Self::Page => "Pages",
        }
    }
}

/// One state of a screen.
pub(crate) struct State {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) render: Render,
}

/// One screen: a page or organism, its live routes, and its states.
pub(crate) struct Screen {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    pub(crate) group: Group,
    /// The live routes this screen stands for.
    pub(crate) routes: &'static [&'static str],
    pub(crate) states: Vec<State>,
}

/// Live GET addresses that are not screens, with why: files, data for
/// programs, redirects, parts a page loads into itself, and the canvas
/// worlds. A name ending in `*` covers every address under it. Every other
/// GET address must belong to a screen (see the route test).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const NOT_SCREENS: &[(&str, &str)] = &[
    ("/static/*", "files"),
    ("/.well-known/*", "files for programs"),
    ("/v1/*", "the API"),
    ("/api/*", "the API"),
    ("/oauth/*", "sign-in for programs"),
    ("/health", "a check for the host"),
    ("/healthz", "a check for the host"),
    ("/favicon.ico", "a file"),
    ("/favicon.svg", "a file"),
    ("/robots.txt", "a file"),
    ("/sitemap.xml", "a file"),
    ("/llms.txt", "a file"),
    ("/llms-full.txt", "a file"),
    ("/openapi.json", "a file"),
    ("/cli/*", "the installers"),
    ("/ui", "the components catalog"),
    ("/ui/*", "this gallery"),
    ("/components*", "the components catalog for apps"),
    ("/account/avatar", "an image"),
    (
        "/share/{token}/card/{file}",
        "a shared chat's preview picture",
    ),
    (
        "/chat/{id}/share",
        "the share step: a dialog over the chat (its no-script page shows the same words)",
    ),
    ("/auth/github", "goes to GitHub"),
    ("/auth/github/install", "goes to GitHub"),
    ("/auth/github/reconnect", "goes to GitHub"),
    ("/auth/github/repos", "goes to GitHub"),
    ("/auth/github/repos/finish", "goes back to Projects"),
    ("/auth/github/setup", "goes back to Projects"),
    ("/cloud", "goes home"),
    ("/install", "goes to Download"),
    ("/desktop", "goes to Download"),
    ("/docs/install", "goes to a guide"),
    ("/docs/help", "goes to a guide"),
    ("/chat", "goes home"),
    ("/chat/list", "part of the chat list"),
    ("/chat/assets/*", "files"),
    ("/chat/{id}/events", "updates for an open chat"),
    ("/chats/events", "updates for the chat list"),
    ("/chat/{id}/transcript", "part of a chat page"),
    ("/chat/{id}/workspace", "part of a chat page"),
    ("/chat/{id}/rename", "part of the chat list"),
    ("/chat/{id}/agents", "part of a chat page"),
    ("/chat/{id}/to-coder", "part of a chat page"),
    ("/chat/{id}/work-offer/{index}", "part of a chat page"),
    ("/chat/{id}/messages/{index}/original", "a message as text"),
    ("/chat/{id}/files/{file}", "a file"),
    ("/chat/{id}/captures/{ask}", "an image"),
    ("/composer/*", "part of the composer"),
    ("/demo/{chat}/thread", "part of a demo page"),
    ("/coder/*", "data for Coder"),
    ("/projects/repositories", "part of Projects"),
    ("/settings/export", "a download"),
    ("/settings/terminal/live", "part of a settings page"),
    ("/settings/mac-jobs/{id}/files/{file}", "a file"),
    ("/vault/*", "data and files for the vault page"),
    ("/everglade*", "a canvas world"),
    ("/druid", "a canvas world"),
    ("/grid", "a canvas world"),
    ("/games/*", "a game"),
    ("/att*", "a canvas app"),
    (
        "/studios/blue-rush*",
        "the Blue Rush studio site, in its own design",
    ),
];

pub(crate) const fn state(id: &'static str, label: &'static str, render: Render) -> State {
    State { id, label, render }
}

/// Every screen, in gallery order. Each area module adds its own list.
pub(crate) fn screens() -> Vec<Screen> {
    let mut all = Vec::new();
    all.extend(auth::screens());
    all.extend(chat::screens());
    all.extend(account::screens());
    all.extend(traces::screens());
    all.extend(site::screens());
    all
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(index))
        .route("/ui/screens/{screen}", get(screen))
        .route("/ui/screens/{screen}/{state}", get(one))
}

/// The gallery's policy: the `/ui` policy plus frames from this site.
const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; \
     script-src 'self'; frame-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

fn respond(headers: &HeaderMap, title: &str, path: &str, content: Markup) -> Response {
    let mut response = UiPage::new(title)
        .path(path)
        .content(content)
        .respond(headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    response
}

fn screen_href(screen: &Screen) -> String {
    format!("{PAGE}/{}", screen.id)
}

fn state_href(screen: &Screen, state: &State, theme: Theme) -> String {
    format!("{PAGE}/{}/{}?theme={}", screen.id, state.id, theme.as_str())
}

/// `GET /ui/screens`: every screen by group.
async fn index(headers: HeaderMap) -> Response {
    let all = screens();
    let content = html! {
        div class="oa-catalog oa-screens" {
            header class="oa-catalog-intro" {
                h1 class="oa-catalog-intro__title" { "Screens" }
                p class="oa-catalog-intro__lede" {
                    "Every page and every part of a page on this site, in each state a person can meet, with sample data."
                }
                (sample_note())
            }
            section class="oa-catalog-group" aria-labelledby="group-atoms" {
                h2 class="oa-catalog-group__title" id="group-atoms" { "Atoms and molecules" }
                p { "The building blocks are in " a href="/ui" { "Components" } ": colors, type, icons, buttons, fields, menus, the composer, and the app shell, each in its variants." }
            }
            @for group in [Group::Organism, Group::Page] {
                section class="oa-catalog-group" aria-labelledby=(format!("group-{}", group.title().to_ascii_lowercase())) {
                    h2 class="oa-catalog-group__title" id=(format!("group-{}", group.title().to_ascii_lowercase())) { (group.title()) }
                    ul class="oa-screens-list" {
                        @for screen in all.iter().filter(|s| s.group == group) {
                            li class="oa-screens-item" {
                                a class="oa-screens-item__title" href=(screen_href(screen)) { (screen.title) }
                                span class="oa-screens-item__routes" {
                                    @for route in screen.routes { code { (route) } " " }
                                }
                                span class="oa-screens-item__states" {
                                    @for state in &screen.states {
                                        a href=(format!("{}#{}", screen_href(screen), state.id)) { (state.label) }
                                        " "
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    respond(&headers, "Screens", PAGE, content)
}

fn find<'a>(all: &'a [Screen], id: &str) -> Option<&'a Screen> {
    all.iter().find(|screen| screen.id == id)
}

fn not_found(what: &str) -> Response {
    crate::layout::problem(
        StatusCode::NOT_FOUND,
        &format!("{what} not found"),
        "Choose one from the screens list.",
        (PAGE, "Screens"),
    )
}

/// `GET /ui/screens/{screen}`: every state, both themes, each in a frame.
async fn screen(headers: HeaderMap, Path(id): Path<String>) -> Response {
    let all = screens();
    let Some(screen) = find(&all, &id) else {
        return not_found("Screen");
    };
    let content = html! {
        div class="oa-catalog oa-screens" {
            header class="oa-catalog-intro" {
                p { a href=(PAGE) { "Screens" } " / " (screen.group.title()) }
                h1 class="oa-catalog-intro__title" { (screen.title) }
                p class="oa-catalog-intro__lede" {
                    @for route in screen.routes { code { (route) } " " }
                }
                (sample_note())
            }
            @for state in &screen.states {
                section class="oa-catalog-section" id=(state.id) {
                    h2 class="oa-catalog-section__title" { (state.label) }
                    div class="oa-catalog-themes" {
                        @for theme in [Theme::Light, Theme::Dark] {
                            div class="oa-catalog-theme" {
                                p class="oa-catalog-theme__name" {
                                    a href=(state_href(screen, state, theme)) {
                                        @match theme { Theme::Light => "Coder Light", Theme::Dark => "Coder Noir" }
                                    }
                                }
                                iframe class="oa-screens-frame"
                                    src=(state_href(screen, state, theme))
                                    title=(format!("{} · {}", state.label, theme.as_str()))
                                    sandbox="allow-same-origin allow-scripts"
                                    loading="lazy" {}
                            }
                        }
                    }
                }
            }
        }
    };
    respond(&headers, screen.title, &screen_href(screen), content)
}

#[derive(Deserialize, Default)]
struct ThemeQuery {
    theme: Option<String>,
}

/// `GET /ui/screens/{screen}/{state}?theme=`: one state as a whole page,
/// in the named theme, allowed inside this site's frames.
async fn one(
    headers: HeaderMap,
    Path((screen, state)): Path<(String, String)>,
    query: Result<Query<ThemeQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let all = screens();
    let Some(screen) = find(&all, &screen) else {
        return not_found("Screen");
    };
    let Some(state) = screen.states.iter().find(|s| s.id == state) else {
        return not_found("State");
    };
    let theme = query
        .ok()
        .and_then(|q| q.0.theme)
        .and_then(|t| Theme::from_cookie(&t));
    render(state, &headers, theme).await
}

/// What every gallery page says about its samples.
fn sample_note() -> Markup {
    html! {
        p class="oa-sample-note" {
            "These are samples. The names, chats, computers, and keys are made up, and the buttons and forms do nothing."
        }
    }
}

/// The strip at the top of every sample page.
const SAMPLE_STRIP: &str = "<p class=\"oa-sample-strip\" role=\"note\">Sample page. Made-up data; buttons and forms do nothing.</p>";

/// `policy` with no form posts and no requests, framable by this site.
pub(crate) fn inert_policy(policy: Option<&str>) -> String {
    let mut directives: Vec<&str> = policy
        .unwrap_or("default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; script-src 'self'; base-uri 'none'")
        .split(';')
        .map(str::trim)
        .filter(|d| {
            !d.is_empty()
                && !d.starts_with("form-action")
                && !d.starts_with("connect-src")
                && !d.starts_with("frame-ancestors")
        })
        .collect();
    directives.extend([
        "connect-src 'none'",
        "form-action 'none'",
        "frame-ancestors 'self'",
    ]);
    directives.join("; ")
}

/// `state` as a page for `headers`, in `theme` when given: a sample that
/// says so, can post nothing, and is framable by this site.
pub(crate) async fn render(state: &State, headers: &HeaderMap, theme: Option<Theme>) -> Response {
    let mut headers = headers.clone();
    // The person's own session never reaches a sample.
    headers.remove(header::COOKIE);
    headers.remove(header::AUTHORIZATION);
    if let Some(theme) = theme {
        let cookie = format!("{THEME_COOKIE}={}", theme.as_str());
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            headers.insert(header::COOKIE, value);
        }
    }
    let response = (state.render)(&headers);
    let (mut parts, body) = response.into_parts();
    let policy = parts
        .headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if let Ok(value) = HeaderValue::from_str(&inert_policy(policy.as_deref())) {
        parts.headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    parts.headers.remove(header::SET_COOKIE);
    parts.headers.remove("HX-Redirect");
    parts.headers.remove("HX-Push-Url");
    parts
        .headers
        .insert("X-Robots-Tag", HeaderValue::from_static("noindex"));
    let bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
    let html = String::from_utf8_lossy(&bytes);
    let marked = match html
        .find("<body")
        .and_then(|at| html[at..].find('>').map(|end| at + end + 1))
    {
        Some(at) => format!("{}{SAMPLE_STRIP}{}", &html[..at], &html[at..]),
        None => format!("{SAMPLE_STRIP}{html}"),
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(marked))
}

#[cfg(test)]
mod tests;
