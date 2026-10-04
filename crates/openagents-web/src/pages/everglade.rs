//! `/everglade`: the Everglade zone in the browser (#10525).
//!
//! The page hosts a canvas and loads the `everglade-web` wasm build
//! (#10524) through `static/everglade.js`, the site's third script. The
//! build's files and the pinned asset pack are read from the directory the
//! server was started with (`--everglade DIR`), so the binary doesn't embed
//! tens of megabytes:
//!
//! ```text
//! DIR/everglade_web.js          wasm-bindgen's `--target web` glue
//! DIR/everglade_web_bg.wasm     the module
//! DIR/pack/<PACK_SHA256>.vtp    the pinned pack from assets/verse/everglade/
//! ```
//!
//! `/everglade/{file}` serves any `.js` or `.wasm` file directly in `DIR`,
//! and `/everglade/pack/{sha}.vtp` any digest-named pack in `DIR/pack`,
//! with a year's immutable cache. Without the directory, or without the
//! glue module in it, the page says Everglade is unavailable and runs no
//! script.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::App;
use crate::layout::page;

/// The JS glue module's file name in the build directory. It must match
/// the file `scripts/build-everglade-web.sh` writes: wasm-bindgen names it
/// after the crate, so `everglade-web` gives `everglade_web.js`.
pub(crate) const GLUE: &str = "everglade_web.js";

/// The wasm module's file name in the build directory. It must match the
/// build script's output too: wasm-bindgen's `<crate>_bg.wasm`.
pub(crate) const WASM: &str = "everglade_web_bg.wasm";

/// The subdirectory of the build directory that holds the pack, and the
/// URL path it's served under.
const PACK_DIRECTORY: &str = "pack";
pub(crate) const PACK_PATH: &str = "/everglade/pack/";

/// The canvas the build draws in, inside the `#everglade` container.
pub(crate) const CANVAS_ID: &str = "everglade-canvas";

/// The page's policy: the site's, plus its one script and the module it
/// imports from this site, compiling WebAssembly, and same-origin reads of
/// the module and the pack.
pub(crate) const EVERGLADE_POLICY: &str = "default-src 'none'; style-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// How long a build file may be cached. Its name carries no digest, so a
/// new build replaces it under the same name.
const BUILD_CACHE: &str = "public, max-age=300";

/// How long a pack may be cached: its name is its SHA-256.
const PACK_CACHE: &str = "public, max-age=31536000, immutable";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/everglade", get(everglade))
        .route("/everglade/{file}", get(build_file))
        .route("/everglade/pack/{file}", get(pack_file))
}

/// The build directory, when it holds the glue module and the wasm.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.everglade.as_deref()?;
    (directory.join(GLUE).is_file() && directory.join(WASM).is_file()).then_some(directory)
}

fn body() -> String {
    format!(
        "<section class=\"everglade\" aria-labelledby=\"everglade-title\">\
<h1 id=\"everglade-title\">Everglade</h1>\
<p class=\"lede\">A forest glade with a small workshop, the Verse zone where a person works \
with a team of coding agents. It runs here in your browser.</p>\
<div class=\"glade\" id=\"everglade\" data-module=\"/everglade/{GLUE}\" \
data-wasm=\"/everglade/{WASM}\" data-pack=\"{PACK_PATH}\">\
<canvas id=\"{CANVAS_ID}\" tabindex=\"0\" aria-label=\"The Everglade zone\"></canvas></div>\
<p class=\"glade-status\" id=\"everglade-status\" aria-live=\"polite\">Loading Everglade.</p>\
<p class=\"dim\">Move and look as in the Verse on your Mac. The world's download is large \
the first time; your browser keeps it after that.</p>\
<p><a href=\"/docs/verse\">[ The Verse ]</a> <span class=\"dim\">The guide to the Verse \
on your phone and your Mac.</span></p>\
<noscript><p class=\"dim\">Turn on JavaScript to open Everglade.</p></noscript>\
</section>\
<script type=\"module\" src=\"/static/everglade.js\"></script>"
    )
}

const UNAVAILABLE: &str = "<section class=\"everglade\" aria-labelledby=\"everglade-title\">\
<h1 id=\"everglade-title\">Everglade</h1>\
<p class=\"lede\">Everglade is unavailable on this server: it was started without the \
Everglade web build.</p>\
<p><a href=\"/docs/verse\">[ The Verse ]</a> <span class=\"dim\">The guide to the Verse \
on your phone and your Mac.</span></p></section>";

async fn everglade(State(app): State<App>) -> Response {
    if build(&app).is_none() {
        return page("Everglade", None, UNAVAILABLE);
    }
    let mut response = page("Everglade", None, &body());
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(EVERGLADE_POLICY),
    );
    response
}

/// Whether `name` is one plain file name: no separator, no leading dot, and
/// only the characters wasm-bindgen's and the pack's names use. Path
/// traversal (`..`, `/`, `\`, percent-encoded or not) never passes.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// The content type of a build file the site serves, by its extension.
pub(crate) fn build_type(name: &str) -> Option<&'static str> {
    if !plain(name) {
        return None;
    }
    if name.ends_with(".js") {
        Some("text/javascript; charset=utf-8")
    } else if name.ends_with(".wasm") {
        Some("application/wasm")
    } else {
        None
    }
}

/// Whether `name` is a digest-named pack: 64 lowercase hex digits and
/// `.vtp`.
pub(crate) fn pack_name(name: &str) -> bool {
    name.strip_suffix(".vtp").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

async fn build_file(State(app): State<App>, UrlPath(file): UrlPath<String>) -> Response {
    let (Some(directory), Some(content_type)) = (build(&app), build_type(&file)) else {
        return crate::not_found().await;
    };
    serve(directory.join(&file), content_type, BUILD_CACHE).await
}

async fn pack_file(State(app): State<App>, UrlPath(file): UrlPath<String>) -> Response {
    let Some(directory) = app.config.everglade.as_deref() else {
        return crate::not_found().await;
    };
    if !pack_name(&file) {
        return crate::not_found().await;
    }
    serve(
        directory.join(PACK_DIRECTORY).join(&file),
        "application/octet-stream",
        PACK_CACHE,
    )
    .await
}

/// One regular file's bytes, or `404` when it isn't there.
async fn serve(path: PathBuf, content_type: &'static str, cache: &'static str) -> Response {
    let regular = tokio::fs::metadata(&path)
        .await
        .is_ok_and(|metadata| metadata.is_file());
    if !regular {
        return crate::not_found().await;
    }
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, cache),
            ],
            bytes,
        )
            .into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => crate::not_found().await,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_js_and_wasm_names_are_build_files() {
        assert_eq!(build_type(GLUE), Some("text/javascript; charset=utf-8"));
        assert_eq!(build_type(WASM), Some("application/wasm"));
        assert_eq!(
            build_type("snippets.js"),
            Some("text/javascript; charset=utf-8")
        );
        for name in [
            "",
            ".js",
            "..js",
            "../everglade_web.js",
            "a/b.js",
            "a\\b.js",
            "..%2Fsecret.js",
            "everglade_web.d.ts",
            "index.html",
            "pack",
            "everglade_web.js.map",
        ] {
            assert_eq!(build_type(name), None, "{name}");
        }
    }

    #[test]
    fn only_digest_names_are_packs() {
        let digest = "b57e33f733865ff639c87e6c0314f7e8f55c59d6ef313271880459ffb64bc49c";
        assert!(pack_name(&format!("{digest}.vtp")));
        assert!(!pack_name(digest));
        assert!(!pack_name(&format!("{}.vtp", digest.to_uppercase())));
        assert!(!pack_name(&format!("{}.vtp", &digest[1..])));
        assert!(!pack_name(&format!("../{}.vtp", &digest[3..])));
        assert!(!pack_name(&format!("{digest}.zip")));
    }

    #[test]
    fn the_policy_allows_only_same_origin_scripts_requests_and_wasm() {
        assert!(EVERGLADE_POLICY.starts_with("default-src 'none'"));
        assert!(EVERGLADE_POLICY.contains("script-src 'self' 'wasm-unsafe-eval';"));
        assert!(EVERGLADE_POLICY.contains("connect-src 'self';"));
        assert!(!EVERGLADE_POLICY.contains("unsafe-inline"));
        assert!(!EVERGLADE_POLICY.contains("'unsafe-eval'"));
        assert!(!EVERGLADE_POLICY.contains("http"));
    }
}
