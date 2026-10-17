//! `/layertree`: the LayerTree demo, a Verse scene of a shared Bitcoin
//! coin's tree with thousands of members (#11987,
//! `crates/layertree-web/README.md`).
//!
//! The page is a canvas filling the window and the build's own start
//! script. The `layertree-web` build (`scripts/build-layertree-web.sh`) is
//! read from the directory the server was started with (`--layertree DIR`,
//! or `/srv/layertree` when the image carries it):
//!
//! ```text
//! DIR/layertree_web.js          wasm-bindgen's `--target web` glue
//! DIR/layertree_web_bg.wasm     the module
//! DIR/start.js                  imports the glue and starts the scene
//! DIR/*.gz                      gzip copies, sent when the request accepts them
//! ```
//!
//! `/layertree/{file}` serves any `.js` or `.wasm` file directly in `DIR`.
//! Without the directory, or without the three files in it, the page says
//! the demo is unavailable and runs no script.

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

/// The demo's page.
pub(crate) const LAYERTREE_PATH: &str = "/layertree";

/// The build's files, as `scripts/build-layertree-web.sh` names them.
pub(crate) const LAYERTREE_GLUE: &str = "layertree_web.js";
pub(crate) const LAYERTREE_WASM: &str = "layertree_web_bg.wasm";
pub(crate) const LAYERTREE_START: &str = "start.js";

/// The canvas the scene draws in; `layertree-web` looks it up by this id.
pub(crate) const LAYERTREE_CANVAS: &str = "layertree-canvas";

/// The page's policy: the site's, plus scripts and the module from this
/// site only, compiling WebAssembly, and same-origin reads of the module.
/// The scene draws every word on its canvas, so no inline style is
/// admitted.
pub(crate) const LAYERTREE_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

const TITLE: &str = "LayerTree";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(LAYERTREE_PATH, get(page))
        .route("/layertree/{file}", get(build_file))
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.layertree.as_deref()?;
    [LAYERTREE_GLUE, LAYERTREE_WASM, LAYERTREE_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// The canvas, filling the window, with the loading line over its foot.
/// The heading and the description are for screen readers.
fn body() -> String {
    format!(
        "<h1 class=\"unseen\">{TITLE}</h1>\
<p class=\"unseen\">Thousands of people share one Bitcoin coin. Watch them join, pay each other \
and leave, then see the signer switched off and everyone paid back on Bitcoin.</p>\
<div class=\"glade\" id=\"layertree\">\
<canvas id=\"{LAYERTREE_CANVAS}\" tabindex=\"0\" aria-label=\"The shared coin's tree of members, \
in 3D\"></canvas>\
<p class=\"glade-status\" id=\"layertree-status\" aria-live=\"polite\">Loading\u{2026}</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to see the demo.</p></noscript></div>\
<script type=\"module\" src=\"{LAYERTREE_PATH}/{LAYERTREE_START}\"></script>"
    )
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="layertree-title" {
            (MarkdownRoot::new(html! {
                h1 id="layertree-title" { (TITLE) }
                p.oa-page-lead { "The LayerTree demo is unavailable right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(LAYERTREE_PATH)
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
        HeaderValue::from_static(LAYERTREE_POLICY),
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
        assert!(LAYERTREE_POLICY.starts_with("default-src 'none'"));
        assert!(LAYERTREE_POLICY.contains("script-src 'self' 'wasm-unsafe-eval';"));
        assert!(LAYERTREE_POLICY.contains("connect-src 'self';"));
        assert!(!LAYERTREE_POLICY.contains("unsafe-inline"));
        assert!(!LAYERTREE_POLICY.contains("'unsafe-eval'"));
        assert!(!LAYERTREE_POLICY.contains("http"));
    }

    #[test]
    fn the_page_has_one_script_from_the_build_and_no_inline_style() {
        let html = body();
        let lower = html.to_ascii_lowercase();
        assert_eq!(lower.matches("<script").count(), 1);
        assert!(html.contains("src=\"/layertree/start.js\""));
        assert!(!lower.contains(" style="));
        assert!(!lower.contains("<style"));
        assert!(html.contains(&format!("<canvas id=\"{LAYERTREE_CANVAS}\"")));
    }
}
