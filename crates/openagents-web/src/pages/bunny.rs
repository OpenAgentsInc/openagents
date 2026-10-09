//! `/games/grow-little-bunny`: Grow Little Bunny in the browser
//! (`docs/verse/games/grow-little-bunny.md`).
//!
//! The page is a canvas filling the window and the build's own start
//! script. The `bunny-web` build (`scripts/build-bunny-web.sh`) is read from
//! the directory the server was started with (`--bunny DIR`):
//!
//! ```text
//! DIR/bunny_web.js          wasm-bindgen's `--target web` glue
//! DIR/bunny_web_bg.wasm     the module
//! DIR/start.js              imports the glue and starts the game
//! DIR/*.gz                  gzip copies, sent when the request accepts them
//! ```
//!
//! `/games/grow-little-bunny/{file}` serves any `.js` or `.wasm` file
//! directly in `DIR`. Without the directory, or without the three files in
//! it, the page says the game can't be played here and runs no script.

use std::path::Path;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use maud::html;
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::layout::fullscreen;
use crate::ui_page::{UiPage, action_link};

/// The game's page.
pub(crate) const BUNNY_PATH: &str = "/games/grow-little-bunny";

/// The build's files, as `scripts/build-bunny-web.sh` names them.
pub(crate) const BUNNY_GLUE: &str = "bunny_web.js";
pub(crate) const BUNNY_WASM: &str = "bunny_web_bg.wasm";
pub(crate) const BUNNY_START: &str = "start.js";

/// The canvas the game draws in; `bunny-web` looks it up by this id.
pub(crate) const BUNNY_CANVAS: &str = "bunny-canvas";

/// The page's policy: the site's, plus scripts and the module from this
/// site only, compiling WebAssembly, and same-origin reads of the module.
/// The game styles its own controls through the CSSOM, which `style-src`
/// doesn't govern, so no inline style is admitted.
pub(crate) const BUNNY_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

const TITLE: &str = "Grow Little Bunny";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(BUNNY_PATH, get(page))
        .route("/games/grow-little-bunny/{file}", get(build_file))
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.bunny.as_deref()?;
    [BUNNY_GLUE, BUNNY_WASM, BUNNY_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// The canvas, filling the window, with the loading line over its foot.
/// The heading is for screen readers only.
fn body() -> String {
    format!(
        "<h1 class=\"unseen\">{TITLE}</h1>\
<div class=\"glade\" id=\"bunny\">\
<canvas id=\"{BUNNY_CANVAS}\" tabindex=\"0\" aria-label=\"The garden, where your bunny runs\"></canvas>\
<p class=\"glade-status\" id=\"bunny-status\" aria-live=\"polite\">Loading\u{2026}</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to play.</p></noscript></div>\
<script type=\"module\" src=\"{BUNNY_PATH}/{BUNNY_START}\"></script>"
    )
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="bunny-title" {
            (MarkdownRoot::new(html! {
                h1 id="bunny-title" { (TITLE) }
                p.oa-page-lead { "The game can't be played here right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(BUNNY_PATH)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    if build(&app).is_none() {
        return unavailable(&headers);
    }
    let mut response = fullscreen(TITLE, &body());
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(BUNNY_POLICY),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_allows_only_same_origin_scripts_and_wasm() {
        assert!(BUNNY_POLICY.starts_with("default-src 'none'"));
        assert!(BUNNY_POLICY.contains("script-src 'self' 'wasm-unsafe-eval';"));
        assert!(BUNNY_POLICY.contains("connect-src 'self';"));
        assert!(!BUNNY_POLICY.contains("unsafe-inline"));
        assert!(!BUNNY_POLICY.contains("'unsafe-eval'"));
        assert!(!BUNNY_POLICY.contains("http"));
    }

    #[test]
    fn the_page_has_one_script_from_the_build_and_no_inline_style() {
        let html = body();
        let lower = html.to_ascii_lowercase();
        assert_eq!(lower.matches("<script").count(), 1);
        assert!(html.contains("src=\"/games/grow-little-bunny/start.js\""));
        assert!(!lower.contains(" style="));
        assert!(!lower.contains("<style"));
        assert!(html.contains(&format!("<canvas id=\"{BUNNY_CANVAS}\"")));
    }
}
