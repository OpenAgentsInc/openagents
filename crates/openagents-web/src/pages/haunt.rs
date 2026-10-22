//! `/games/the-haunted-house`: The Haunted House in the browser
//! (`docs/verse/games/the-haunted-house.md`), served like The Ice Cream
//! Man (`super::icecream`).
//!
//! The `haunt-web` build (`scripts/build-haunt-web.sh`) is read from the
//! directory the server was started with (`--haunt DIR`, or `/srv/haunt`
//! when the image has it): `haunt_web.js`, `haunt_web_bg.wasm`,
//! `start.js`, and `version.txt`. The page loads them from
//! `/games/the-haunted-house/v/{version}/`, cached for a year, and is
//! itself `no-cache`.
//!
//! The canvas names `/labs/game/haunt/house`: a signed-in visitor sees
//! their own Game Lab copy of the house and each change they make to it,
//! within a second, without a reload. `?v=N` shows version N and stays on
//! it. A visitor who isn't signed in walks version 1, built into the page.

use std::path::Path;

use axum::Router;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use maud::html;
use openagents_ui::content::{MarkdownRoot, PageColumn};
use serde::Deserialize;

use crate::App;
use crate::ui_page::{UiPage, action_link};

/// The house's page.
pub(crate) const HAUNT_PATH: &str = "/games/the-haunted-house";
/// The build's files, as `scripts/build-haunt-web.sh` names them.
const GLUE: &str = "haunt_web.js";
const WASM: &str = "haunt_web_bg.wasm";
const START: &str = "start.js";
const VERSION_FILE: &str = "version.txt";
/// The canvas the house draws in; `haunt-web` looks it up by this id.
const CANVAS: &str = "haunt-canvas";
/// Where a signed-in visitor's house comes from.
const HOUSE_URL: &str = "/labs/game/haunt/house";
const TITLE: &str = "The Haunted House";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(HAUNT_PATH, get(page))
        .route("/games/the-haunted-house/{file}", get(file))
        .route(
            "/games/the-haunted-house/v/{version}/{file}",
            get(versioned_file),
        )
}

fn build(app: &App) -> Option<&Path> {
    let directory = app.config.haunt.as_deref()?;
    [GLUE, WASM, START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

fn plain_version(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 40
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'.' || b == b'-')
}

fn version(directory: &Path) -> String {
    std::fs::read_to_string(directory.join(VERSION_FILE))
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| plain_version(t))
        .unwrap_or_else(|| "build".into())
}

fn document(version: &str, house: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\" class=\"stage\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, \
user-scalable=no, viewport-fit=cover\">\
<meta name=\"color-scheme\" content=\"dark\"><title>{TITLE} \u{b7} OpenAgents</title>\
<meta name=\"theme-color\" content=\"#2B3346\">\
<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\
<link rel=\"stylesheet\" href=\"/static/legacy-demo.css\"></head><body>\
<main id=\"content\"><h1 class=\"unseen\">{TITLE}</h1><div class=\"glade\">\
<canvas id=\"{CANVAS}\" tabindex=\"0\" data-version=\"{version}\" data-house-url=\"{house}\" \
aria-label=\"The Haunted House\"></canvas>\
<noscript><p class=\"glade-status\">Turn on JavaScript to walk in.</p></noscript></div>\
<script type=\"module\" src=\"{HAUNT_PATH}/v/{version}/{START}\"></script></main></body></html>"
    )
}

fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="haunt-title" {
            (MarkdownRoot::new(html! {
                h1 id="haunt-title" { (TITLE) }
                p.oa-page-lead { "The house can't be walked here right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(HAUNT_PATH)
        .scriptless()
        .content(content)
        .respond(headers)
}

#[derive(Deserialize)]
struct PageQuery {
    v: Option<u64>,
}

async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let Some(directory) = build(&app) else {
        return unavailable(&headers);
    };
    let house = match query.v {
        Some(v) => format!("{HOUSE_URL}?v={v}"),
        None => HOUSE_URL.to_owned(),
    };
    let mut response = Html(document(&version(directory), &house)).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(super::icecream::ICECREAM_POLICY),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(NO_CACHE));
    response
}

async fn file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) = (build(&app), super::everglade::build_type(&file))
    else {
        return crate::not_found().await;
    };
    super::everglade::serve_build_cached(directory, &file, content_type, &request, NO_CACHE).await
}

async fn versioned_file(
    State(app): State<App>,
    UrlPath((wanted, file)): UrlPath<(String, String)>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) = (build(&app), super::everglade::build_type(&file))
    else {
        return crate::not_found().await;
    };
    if !plain_version(&wanted) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let cache = if wanted == version(directory) {
        IMMUTABLE
    } else {
        NO_CACHE
    };
    super::everglade::serve_build_cached(directory, &file, content_type, &request, cache).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_runs_one_versioned_script_and_names_the_house_it_follows() {
        let html = document("3f9a2c1b0d4e", HOUSE_URL);
        let lower = html.to_ascii_lowercase();
        assert_eq!(lower.matches("<script").count(), 1);
        assert!(html.contains("src=\"/games/the-haunted-house/v/3f9a2c1b0d4e/start.js\""));
        assert!(html.contains("data-house-url=\"/labs/game/haunt/house\""));
        assert!(html.contains(&format!("<canvas id=\"{CANVAS}\"")));
        assert!(!lower.contains(" style=") && !lower.contains("<style"));
        let text = oa_copy::visible_text(&html);
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
    }
}
