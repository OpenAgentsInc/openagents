//! `/games/snap-station`: Snap Station in the browser
//! (`docs/verse/games/snap-station.md`), served like Grow Little Bunny
//! (`super::bunny`) and The Ice Cream Man (`super::icecream`).
//!
//! The page is a canvas filling the window and the build's own start
//! script. The `snap-web` build (`scripts/build-snap-web.sh`) is read from
//! the directory the server was started with (`--snap DIR`, or `/srv/snap`
//! when the image has it):
//!
//! ```text
//! DIR/snap_web.js          wasm-bindgen's `--target web` glue
//! DIR/snap_web_bg.wasm     the module
//! DIR/start.js             imports the glue and starts the game
//! DIR/*.gz                 gzip copies, sent when the request accepts them
//! ```
//!
//! `/games/snap-station/{file}` serves any `.js` or `.wasm` file directly
//! in `DIR`. Without the directory, or without the three files in it, the
//! page says the game can't be played here and runs no script.

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
pub(crate) const SNAP_PATH: &str = "/games/snap-station";

/// The build's files, as `scripts/build-snap-web.sh` names them.
pub(crate) const SNAP_GLUE: &str = "snap_web.js";
pub(crate) const SNAP_WASM: &str = "snap_web_bg.wasm";
pub(crate) const SNAP_START: &str = "start.js";

/// The canvas the game draws in; `snap-web` looks it up by this id.
pub(crate) const SNAP_CANVAS: &str = "snap-canvas";

/// The page's policy: Grow Little Bunny's (scripts and the module from
/// this site only, compiling WebAssembly, same-origin reads).
const SNAP_POLICY: &str = super::bunny::BUNNY_POLICY;

const TITLE: &str = "Snap Station";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(super::snap_shares::routes())
        .route(SNAP_PATH, get(page))
        .route("/games/snap-station/{file}", get(build_file))
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.snap.as_deref()?;
    [SNAP_GLUE, SNAP_WASM, SNAP_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// The canvas, filling the window, with the loading line over its foot.
/// The heading is for screen readers only.
fn body() -> String {
    format!(
        "<link rel=\"stylesheet\" href=\"{SNAP_PATH}/style.css\"><h1 class=\"unseen\">{TITLE}</h1>\
<div class=\"glade\" id=\"snap\">\
<canvas id=\"{SNAP_CANVAS}\" tabindex=\"0\" aria-label=\"Snap Station: build in space with magnetic pieces\"></canvas>\
<p class=\"glade-status\" id=\"snap-status\" aria-live=\"polite\">Loading\u{2026}</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to play.</p></noscript></div>\
<script type=\"module\" src=\"{SNAP_PATH}/{SNAP_START}\"></script>"
    )
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="snap-title" {
            (MarkdownRoot::new(html! {
                h1 id="snap-title" { (TITLE) }
                p.oa-page-lead { "The game can't be played here right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(SNAP_PATH)
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
        HeaderValue::from_static(SNAP_POLICY),
    );
    response
}

async fn build_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) = (
        build(&app),
        if file == "style.css" {
            Some("text/css; charset=utf-8")
        } else {
            super::everglade::build_type(&file)
        },
    ) else {
        return crate::not_found().await;
    };
    super::everglade::serve_build(directory, &file, content_type, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_has_one_script_from_the_build_and_no_inline_style() {
        let html = body();
        let lower = html.to_ascii_lowercase();
        assert_eq!(lower.matches("<script").count(), 1);
        assert!(html.contains("src=\"/games/snap-station/start.js\""));
        assert!(!lower.contains(" style="));
        assert!(!lower.contains("<style"));
        assert!(html.contains(&format!("<canvas id=\"{SNAP_CANVAS}\"")));
        assert!(html.contains("id=\"snap-status\""));
    }
}
