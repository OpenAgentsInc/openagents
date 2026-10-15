//! `/games/the-ice-cream-man`: The Ice Cream Man in the browser
//! (`docs/verse/games/the-ice-cream-man.md`), served like Grow Little
//! Bunny (`super::bunny`), and installable as a web app.
//!
//! The page is a canvas filling the window and the build's own start
//! script. The `icecream-web` build (`scripts/build-icecream-web.sh`) is
//! read from the directory the server was started with (`--icecream DIR`,
//! or `/srv/icecream` when the image has it):
//!
//! ```text
//! DIR/icecream_web.js          wasm-bindgen's `--target web` glue
//! DIR/icecream_web_bg.wasm     the module
//! DIR/start.js                 imports the glue and starts the game
//! DIR/version.txt              the build's version (its files' SHA-256)
//! DIR/*.gz                     gzip copies, sent when the request accepts them
//! ```
//!
//! **Fresh builds.** The page loads the build from
//! `/games/the-ice-cream-man/v/{version}/start.js`, and the glue and the
//! module beside it, so a new build is a new address: those files are
//! cached for a year, and the page itself is `no-cache`, so a deploy
//! reaches every player on their next load (a school's cache or proxy
//! can't keep an old build). The version is `version.txt`, or, without
//! it, one made from the files' sizes and times. The canvas carries it as
//! `data-version` for the game's menu. The same files without a version,
//! `/games/the-ice-cream-man/{file}`, are `no-cache`.
//!
//! **Installable.** The page links a web app manifest and icons, and the
//! start script registers a service worker (`static/icecream/sw.js`,
//! scoped to the game) that keeps the current build for offline play, so
//! Add to Home Screen gives a full-screen game with no browser bars.
//!
//! The guide is at `/games/the-ice-cream-man/how-to-play`
//! ([`super::icecream_guide`]); its stylesheet and pictures are served
//! here too. Without the directory, or without the three build files in
//! it, the page says the game can't be played here and runs no script.

use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::LazyLock;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

use maud::html;
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::ui_page::{UiPage, action_link};

/// The game's page.
pub(crate) const ICECREAM_PATH: &str = "/games/the-ice-cream-man";

/// The build's files, as `scripts/build-icecream-web.sh` names them.
pub(crate) const ICECREAM_GLUE: &str = "icecream_web.js";
pub(crate) const ICECREAM_WASM: &str = "icecream_web_bg.wasm";
pub(crate) const ICECREAM_START: &str = "start.js";
/// The build's version, when the build script wrote it.
pub(crate) const ICECREAM_VERSION_FILE: &str = "version.txt";

/// The canvas the game draws in; `icecream-web` looks it up by this id.
pub(crate) const ICECREAM_CANVAS: &str = "icecream-canvas";

/// The page's policy: Grow Little Bunny's (scripts and the module from
/// this site only, compiling WebAssembly, same-origin reads), plus its
/// own manifest and service worker.
pub(crate) const ICECREAM_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; \
connect-src 'self' wss://relay.openagents.com; manifest-src 'self'; worker-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// A file whose address carries its version.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// Always asked again (a `304` when unchanged).
const NO_CACHE: &str = "no-cache";
/// Pictures and icons change rarely.
const PICTURE_CACHE: &str = "public, max-age=86400";

const TITLE: &str = "The Ice Cream Man";

const SERVICE_WORKER: &str = include_str!("../../static/icecream/sw.js");
const MANIFEST: &str = include_str!("../../static/icecream/manifest.webmanifest");

/// The guide's screenshots (960 by 600) and the app icons.
const PICTURES: [(&str, &str, &[u8]); 9] = [
    (
        "icecream-hall.jpg",
        "image/jpeg",
        include_bytes!("../../static/icecream/icecream-hall.jpg"),
    ),
    (
        "icecream-man.jpg",
        "image/jpeg",
        include_bytes!("../../static/icecream/icecream-man.jpg"),
    ),
    (
        "icecream-fudge.jpg",
        "image/jpeg",
        include_bytes!("../../static/icecream/icecream-fudge.jpg"),
    ),
    (
        "icecream-hide.jpg",
        "image/jpeg",
        include_bytes!("../../static/icecream/icecream-hide.jpg"),
    ),
    (
        "icecream-door.jpg",
        "image/jpeg",
        include_bytes!("../../static/icecream/icecream-door.jpg"),
    ),
    (
        "icecream-tour.jpg",
        "image/jpeg",
        include_bytes!("../../static/icecream/icecream-tour.jpg"),
    ),
    (
        "icon-180.png",
        "image/png",
        include_bytes!("../../static/icecream/icon-180.png"),
    ),
    (
        "icon-192.png",
        "image/png",
        include_bytes!("../../static/icecream/icon-192.png"),
    ),
    (
        "icon-512.png",
        "image/png",
        include_bytes!("../../static/icecream/icon-512.png"),
    ),
];

static GUIDE_CSS_VERSION: LazyLock<String> =
    LazyLock::new(|| super::blue_rush::css_version().to_owned());

/// The guide stylesheet's version, for its link.
pub(crate) fn guide_css_version() -> &'static str {
    &GUIDE_CSS_VERSION
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(ICECREAM_PATH, get(page))
        .route(
            super::icecream_guide::ICECREAM_GUIDE_PATH,
            get(super::icecream_guide::page),
        )
        .route("/games/the-ice-cream-man/{file}", get(file))
        .route(
            "/games/the-ice-cream-man/v/{version}/{file}",
            get(versioned_file),
        )
}

/// The build directory, when it holds the glue, the module and the start
/// script.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.icecream.as_deref()?;
    [ICECREAM_GLUE, ICECREAM_WASM, ICECREAM_START]
        .iter()
        .all(|file| directory.join(file).is_file())
        .then_some(directory)
}

/// Whether `text` can be a version in an address: 1 to 40 of `[0-9a-z.-]`.
fn plain_version(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 40
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'.' || b == b'-')
}

/// The build's version: `version.txt`, or 16 hex digits made from the
/// three files' sizes and modification times.
pub(crate) fn version(directory: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(directory.join(ICECREAM_VERSION_FILE)) {
        let text = text.trim();
        if plain_version(text) {
            return text.to_owned();
        }
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for file in [ICECREAM_GLUE, ICECREAM_WASM, ICECREAM_START] {
        if let Ok(metadata) = std::fs::metadata(directory.join(file)) {
            metadata.len().hash(&mut hasher);
            metadata.modified().ok().hash(&mut hasher);
        }
    }
    format!("{:016x}", hasher.finish())
}

/// The canvas, filling the window, with the loading line over its foot.
/// The heading is for screen readers only.
fn body(version: &str) -> String {
    format!(
        "<h1 class=\"unseen\">{TITLE}</h1>\
<div class=\"glade\" id=\"icecream\">\
<canvas id=\"{ICECREAM_CANVAS}\" tabindex=\"0\" data-version=\"{version}\" aria-label=\"The Airbnb, where the Ice Cream Man is\"></canvas>\
<p class=\"glade-status\" id=\"icecream-status\" aria-live=\"polite\">Loading\u{2026}</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to play.</p></noscript></div>\
<script type=\"module\" src=\"{ICECREAM_PATH}/v/{version}/{ICECREAM_START}\"></script>"
    )
}

/// The full-screen page: no zoom, its manifest and icons for Add to Home
/// Screen, and the stage stylesheet every full-window game uses.
fn document(version: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\" class=\"stage\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, maximum-scale=1, \
user-scalable=no, viewport-fit=cover\">\
<meta name=\"color-scheme\" content=\"dark\"><title>{TITLE} \u{b7} OpenAgents</title>\
<meta name=\"theme-color\" content=\"#2b1a12\">\
<meta name=\"mobile-web-app-capable\" content=\"yes\">\
<meta name=\"apple-mobile-web-app-capable\" content=\"yes\">\
<meta name=\"apple-mobile-web-app-status-bar-style\" content=\"black-translucent\">\
<meta name=\"apple-mobile-web-app-title\" content=\"Ice Cream Man\">\
<link rel=\"manifest\" href=\"{ICECREAM_PATH}/manifest.webmanifest\">\
<link rel=\"apple-touch-icon\" href=\"{ICECREAM_PATH}/icon-180.png\">\
<link rel=\"icon\" type=\"image/svg+xml\" href=\"/favicon.svg\">\
<link rel=\"stylesheet\" href=\"/static/legacy-demo.css\"></head><body>\
<main id=\"content\">{}</main></body></html>",
        body(version)
    )
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="icecream-title" {
            (MarkdownRoot::new(html! {
                h1 id="icecream-title" { (TITLE) }
                p.oa-page-lead { "The game can't be played here right now." }
            }))
            div.oa-page-actions {
                (action_link("Back to the home page", "/"))
            }
        }
    });
    UiPage::new(TITLE)
        .path(ICECREAM_PATH)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(directory) = build(&app) else {
        return unavailable(&headers);
    };
    let mut response = Html(document(&version(directory))).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(ICECREAM_POLICY),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(NO_CACHE));
    response
}

fn fixed(content_type: &'static str, cache: &'static str, body: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache),
        ],
        body,
    )
        .into_response()
}

/// A file beside the game: the service worker, the manifest, the guide's
/// stylesheet and pictures, the icons, or an unversioned build file.
async fn file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    match file.as_str() {
        "sw.js" => {
            let mut response = fixed(
                "text/javascript; charset=utf-8",
                NO_CACHE,
                SERVICE_WORKER.as_bytes(),
            );
            // The worker serves the page too, which sits just above its
            // own folder.
            response.headers_mut().insert(
                "service-worker-allowed",
                HeaderValue::from_static(ICECREAM_PATH),
            );
            return response;
        }
        "manifest.webmanifest" => {
            return fixed("application/manifest+json", NO_CACHE, MANIFEST.as_bytes());
        }
        "guide.css" => {
            return fixed(
                "text/css; charset=utf-8",
                IMMUTABLE,
                super::blue_rush::GUIDE_CSS.as_bytes(),
            );
        }
        _ => {}
    }
    if let Some((_, content_type, bytes)) = PICTURES.iter().find(|(name, _, _)| *name == file) {
        return fixed(content_type, PICTURE_CACHE, bytes);
    }
    let (Some(directory), Some(content_type)) = (build(&app), super::everglade::build_type(&file))
    else {
        return crate::not_found().await;
    };
    super::everglade::serve_build_cached(directory, &file, content_type, &request, NO_CACHE).await
}

/// A build file at a versioned address: cached for a year when it's this
/// build's version. An older version's address gets this build's files,
/// `no-cache`, so an old page still starts a whole, matching build.
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
    fn the_page_has_one_versioned_script_from_the_build_and_no_inline_style() {
        let html = document("3f9a2c1b0d4e");
        let lower = html.to_ascii_lowercase();
        assert_eq!(lower.matches("<script").count(), 1);
        assert!(html.contains("src=\"/games/the-ice-cream-man/v/3f9a2c1b0d4e/start.js\""));
        assert!(html.contains("data-version=\"3f9a2c1b0d4e\""));
        assert!(!lower.contains(" style="));
        assert!(!lower.contains("<style"));
        assert!(html.contains(&format!("<canvas id=\"{ICECREAM_CANVAS}\"")));
        assert!(html.contains("id=\"icecream-status\""));
        assert!(html.contains("rel=\"manifest\""));
        assert!(html.contains("rel=\"apple-touch-icon\""));
        assert!(html.contains("user-scalable=no"));
    }

    #[test]
    fn versions_are_plain() {
        assert!(plain_version("3f9a2c1b0d4e"));
        assert!(plain_version("ios-1.0.0"));
        assert!(!plain_version(""));
        assert!(!plain_version("../x"));
        assert!(!plain_version("A"));
        let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["name"], "The Ice Cream Man");
        assert_eq!(manifest["display"], "fullscreen");
        assert_eq!(manifest["start_url"], ICECREAM_PATH);
        assert!(!MANIFEST.contains("Blue Rush"));
    }
}
