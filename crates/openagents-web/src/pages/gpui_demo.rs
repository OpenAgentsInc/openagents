//! `/demo/gpui`: the OpenAgents desktop chat UI running in the browser.
//!
//! The desktop app's GPUI interface, compiled to WebAssembly from
//! `apps/openagents/web` and drawing the app's offline demo chats into a
//! canvas. `apps/openagents/scripts/build-web-demo.sh OUTDIR` writes the
//! build, which this server reads from the directory it was started with
//! (`--gpui-demo DIR`):
//!
//! ```text
//! DIR/openagents_web_demo.js        wasm-bindgen's `--target web` glue
//! DIR/openagents_web_demo_bg.wasm   the module
//! DIR/start.js                      imports the glue and starts the demo
//! DIR/*.gz, *.br                    gzip and brotli copies, sent when accepted
//! ```
//!
//! `/demo/gpui/{file}` serves any `.js` or `.wasm` file directly in `DIR`.
//! Without the directory, or without the three files in it, the page says
//! the demo isn't available here and runs no script.

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
pub(crate) const GPUI_DEMO_PATH: &str = "/demo/gpui";

/// The build's files, as `build-web-demo.sh` names them.
pub(crate) const GPUI_DEMO_GLUE: &str = "openagents_web_demo.js";
pub(crate) const GPUI_DEMO_WASM: &str = "openagents_web_demo_bg.wasm";
pub(crate) const GPUI_DEMO_START: &str = "start.js";

/// The page's policy: scripts and the module from this site only, compiling
/// WebAssembly, and same-origin reads of the module. GPUI styles its canvas
/// and its text field through the CSSOM, which `style-src` doesn't govern.
pub(crate) const GPUI_DEMO_POLICY: &str = "default-src 'none'; style-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

const TITLE: &str = "Chat demo";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(GPUI_DEMO_PATH, get(page))
        .route("/demo/gpui/{file}", get(build_file))
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.gpui_demo.as_deref()?;
    [GPUI_DEMO_GLUE, GPUI_DEMO_WASM, GPUI_DEMO_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// The loading line; the demo adds its canvas to the page and the start
/// script removes the line once the demo runs. `fonts` preloads the shared
/// font files the start script hands to the module.
fn body(fonts: &str, build: Option<&Path>) -> String {
    let start = super::gpui_build::url(build, GPUI_DEMO_PATH, GPUI_DEMO_START);
    let glue = super::gpui_build::url(build, GPUI_DEMO_PATH, GPUI_DEMO_GLUE);
    let wasm = super::gpui_build::url(build, GPUI_DEMO_PATH, GPUI_DEMO_WASM);
    format!(
        "<link rel=\"modulepreload\" href=\"{glue}\">\
<link rel=\"preload\" as=\"fetch\" crossorigin=\"anonymous\" href=\"{wasm}\">{fonts}<h1 class=\"unseen\">{TITLE}</h1>\
<p class=\"glade-status\" id=\"gpui-status\" aria-live=\"polite\">Loading\u{2026}</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to see the demo.</p></noscript>\
<script type=\"module\" src=\"{start}\"></script>"
    )
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="gpui-demo-title" {
            (MarkdownRoot::new(html! {
                h1 id="gpui-demo-title" { (TITLE) }
                p.oa-page-lead { "The demo isn't available here right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(GPUI_DEMO_PATH)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    let fonts = super::gpui_fonts::fonts(&app)
        .and_then(|fonts| fonts.preloads(&super::gpui_fonts::CHAT_FACES));
    let (Some(_), Some(fonts)) = (build(&app), fonts) else {
        return unavailable(&headers);
    };
    let mut response = fullscreen(TITLE, &body(&fonts.into_string(), build(&app)));
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(GPUI_DEMO_POLICY),
    );
    response
}

async fn build_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) =
        (build(&app), super::gpui_build::content_type(&file))
    else {
        return crate::not_found().await;
    };
    super::gpui_build::serve(directory, GPUI_DEMO_PATH, &file, content_type, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_allows_only_same_origin_scripts_and_wasm() {
        assert!(GPUI_DEMO_POLICY.starts_with("default-src 'none'"));
        assert!(GPUI_DEMO_POLICY.contains("script-src 'self' 'wasm-unsafe-eval';"));
        assert!(GPUI_DEMO_POLICY.contains("connect-src 'self';"));
        assert!(!GPUI_DEMO_POLICY.contains("unsafe-inline"));
    }

    #[test]
    fn the_page_loads_the_start_script_from_the_build() {
        let html = body("<link data-gpui-font=\"Geist.ttf\">", None);
        assert!(html.contains("src=\"/demo/gpui/start.js\""));
        assert!(html.contains(
            "as=\"fetch\" crossorigin=\"anonymous\" href=\"/demo/gpui/openagents_web_demo_bg.wasm\""
        ));
        assert!(html.contains("id=\"gpui-status\""));
    }

    #[test]
    fn the_page_copy_is_plain_and_names_no_upstream() {
        let text = oa_copy::visible_text(&body("", None));
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        assert!(!body("", None).to_lowercase().contains("zeron"));
    }
}
