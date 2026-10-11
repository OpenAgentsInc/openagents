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

mod account;
mod auth;

use axum::Router;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
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

pub(crate) const fn state(id: &'static str, label: &'static str, render: Render) -> State {
    State { id, label, render }
}

/// Every screen, in gallery order. Each area module adds its own list.
pub(crate) fn screens() -> Vec<Screen> {
    let mut all = Vec::new();
    all.extend(auth::screens());
    all.extend(account::screens());
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
    render(state, &headers, theme)
}

/// `state` as a page for `headers`, in `theme` when given, framable by
/// this site.
pub(crate) fn render(state: &State, headers: &HeaderMap, theme: Option<Theme>) -> Response {
    let mut headers = headers.clone();
    if let Some(theme) = theme {
        headers.remove(header::COOKIE);
        let cookie = format!("{THEME_COOKIE}={}", theme.as_str());
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            headers.insert(header::COOKIE, value);
        }
    }
    let mut response = (state.render)(&headers);
    let policy = response
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .map(|p| p.replace("frame-ancestors 'none'", "frame-ancestors 'self'"));
    if let Some(policy) = policy
        && let Ok(value) = HeaderValue::from_str(&policy)
    {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, value);
    }
    response
}

#[cfg(test)]
mod tests;
