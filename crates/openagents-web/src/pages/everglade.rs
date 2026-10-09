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
//! DIR/kit/<KIT_SHA256>.vtp      the medieval kit pack, from the private
//!                               bucket at build time, when it's there
//! DIR/kit/bake/<SHA256>.vlay    the reviewed offline light layers
//! ```
//!
//! `/everglade/{file}` serves any `.js` or `.wasm` file directly in `DIR`,
//! `/everglade/pack/{sha}.vtp` any digest-named pack in `DIR/pack`, and
//! `/everglade/kit/{sha}.vtp` any digest-named kit pack in `DIR/kit`, with
//! a year's immutable cache. Every client fetches the kit pack here
//! (`docs/verse/everglade-medieval-refactor.md`); without it, Everglade
//! draws the kit's committed proxies. Without the directory, or without the
//! glue module in it, the page says Everglade is unavailable and runs no
//! script.
//!
//! `/druid` (#10611) is the same page and build for the druid demo
//! (`docs/verse/druid-demo.md`): the module starts in the Grove when the
//! page's path is `/druid`, so the page needs no query and loads the same
//! files from `/everglade/`.
//!
//! `/grid` (#10587) is the same build again: on this path the module opens
//! the shared Grid and joins the other players over a WebSocket to the
//! public relay, so its policy also admits that one connection.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use maud::html;
use openagents_ui::content::{MarkdownRoot, PageColumn};

use crate::App;
use crate::layout::fullscreen;
use crate::ui_page::{UiPage, action_link};

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
pub(crate) const EVERGLADE_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The relay the Grid page's module joins: the public Verse relay
/// (`verse::session::PUBLIC_RELAY`).
pub(crate) const GRID_RELAY: &str = "wss://relay.openagents.com";

/// The Grid page's policy: the build's, plus a WebSocket to [`GRID_RELAY`].
pub(crate) const GRID_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; \
connect-src 'self' wss://relay.openagents.com; \
base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// How long a build file may be cached. Its name carries no digest, so a
/// new build replaces it under the same name.
const BUILD_CACHE: &str = "public, max-age=300";

/// How long a pack may be cached: its name is its SHA-256.
const PACK_CACHE: &str = "public, max-age=31536000, immutable";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/everglade", get(everglade))
        .route("/druid", get(druid))
        .route("/grid", get(grid))
        .route("/everglade/{file}", get(build_file))
        .route("/everglade/pack/{file}", get(pack_file))
        .route("/everglade/kit/{file}", get(kit_file))
        .route("/everglade/kit/bake/{file}", get(bake_file))
}

/// The build directory, when it holds the glue module and the wasm.
fn build(app: &App) -> Option<&Path> {
    let directory = app.config.everglade.as_deref()?;
    (directory.join(GLUE).is_file() && directory.join(WASM).is_file()).then_some(directory)
}

/// What a page of the build shows: its heading, its path, its canvas's
/// label, the name its status line loads, and its content security policy.
struct Stage {
    title: &'static str,
    path: &'static str,
    label: &'static str,
    loading: &'static str,
    policy: &'static str,
}

const EVERGLADE: Stage = Stage {
    title: "Everglade",
    path: "/everglade",
    label: "The Everglade zone",
    loading: "Everglade",
    policy: EVERGLADE_POLICY,
};

const DRUID: Stage = Stage {
    title: "Druid",
    path: "/druid",
    label: "The Grove, a druid training field",
    loading: "the Grove",
    policy: EVERGLADE_POLICY,
};

const GRID: Stage = Stage {
    title: "Grid",
    path: "/grid",
    label: "The Grid, where players meet",
    loading: "the Grid",
    policy: GRID_POLICY,
};

/// The page is the canvas, filling the window, with the status line over its
/// foot. The heading is for screen readers only.
fn body(stage: &Stage, wasm_bytes: u64) -> String {
    let Stage {
        title,
        label,
        loading,
        ..
    } = stage;
    format!(
        "<h1 class=\"unseen\">{title}</h1>\
<div class=\"glade\" id=\"everglade\" data-module=\"/everglade/{GLUE}\" \
data-wasm=\"/everglade/{WASM}\" data-wasm-bytes=\"{wasm_bytes}\" data-pack=\"{PACK_PATH}\">\
<canvas id=\"{CANVAS_ID}\" tabindex=\"0\" aria-label=\"{label}\"></canvas>\
<p class=\"glade-status\" id=\"everglade-status\" aria-live=\"polite\">Loading {loading}…</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to open {loading}.</p></noscript></div>\
<script type=\"module\" src=\"/static/everglade.js\"></script>"
    )
}

/// The page without the build: it runs no script and keeps the site's
/// policy.
fn unavailable(stage: &Stage, headers: &HeaderMap) -> Response {
    let content = PageColumn::new(html! {
        section aria-labelledby="everglade-title" {
            (MarkdownRoot::new(html! {
                h1 id="everglade-title" { "Everglade" }
                p.oa-page-lead {
                    "Everglade is unavailable on this server: it was started without the \
    Everglade web build."
                }
            }))
            div.oa-page-actions {
                (action_link("The Verse", "/docs/verse"))
                span.oa-page-meta { "The guide to the Verse on your phone and your Mac." }
            }
        }
    });
    UiPage::new(stage.title)
        .path(stage.path)
        .scriptless()
        .content(content)
        .respond(headers)
}

async fn everglade(State(app): State<App>, headers: HeaderMap) -> Response {
    stage(&app, &EVERGLADE, &headers).await
}

/// `/druid`: the same build, which starts in the Grove on this path.
async fn druid(State(app): State<App>, headers: HeaderMap) -> Response {
    stage(&app, &DRUID, &headers).await
}

/// `/grid`: the same build, which opens the shared Grid on this path.
async fn grid(State(app): State<App>, headers: HeaderMap) -> Response {
    stage(&app, &GRID, &headers).await
}

async fn stage(app: &App, stage: &Stage, headers: &HeaderMap) -> Response {
    if build(app).is_none() {
        return unavailable(stage, headers);
    }
    // The loader reports the module's download against its uncompressed
    // size, since a compressed response's length is not what it reads.
    let wasm_bytes = match build(app) {
        Some(directory) => tokio::fs::metadata(directory.join(WASM))
            .await
            .map_or(0, |metadata| metadata.len()),
        None => 0,
    };
    let mut response = fullscreen(stage.title, &body(stage, wasm_bytes));
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(stage.policy),
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
    digest_name(name, ".vtp")
}

fn digest_name(name: &str, extension: &str) -> bool {
    name.strip_suffix(extension).is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

async fn build_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    let (Some(directory), Some(content_type)) = (build(&app), build_type(&file)) else {
        return crate::not_found().await;
    };
    // The build stage writes a gzip copy beside each file; the wasm is about
    // a third smaller compressed.
    let gzip = accepts_gzip(&request);
    if gzip {
        let compressed = directory.join(format!("{file}.gz"));
        if tokio::fs::metadata(&compressed)
            .await
            .is_ok_and(|metadata| metadata.is_file())
        {
            let mut response = serve(compressed, content_type, BUILD_CACHE).await;
            if response.status().is_success() {
                let headers = response.headers_mut();
                headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
                headers.insert(header::VARY, HeaderValue::from_static("accept-encoding"));
            }
            return response;
        }
    }
    let mut response = serve(directory.join(&file), content_type, BUILD_CACHE).await;
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("accept-encoding"));
    response
}

/// Whether the request's `Accept-Encoding` admits gzip.
pub(crate) fn accepts_gzip(request: &HeaderMap) -> bool {
    request
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|coding| {
            let mut parts = coding.split(';');
            let name = parts.next().unwrap_or("").trim();
            let refused = parts.any(|param| {
                param
                    .trim()
                    .strip_prefix("q=")
                    .is_some_and(|q| q.trim().parse::<f32>().is_ok_and(|q| q == 0.0))
            });
            (name.eq_ignore_ascii_case("gzip") || name == "*") && !refused
        })
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

/// The medieval kit pack's directory under the build directory.
const KIT_DIRECTORY: &str = "kit";

async fn kit_file(State(app): State<App>, UrlPath(file): UrlPath<String>) -> Response {
    let Some(directory) = app.config.everglade.as_deref() else {
        return crate::not_found().await;
    };
    if !pack_name(&file) {
        return crate::not_found().await;
    }
    serve(
        directory.join(KIT_DIRECTORY).join(&file),
        "application/octet-stream",
        PACK_CACHE,
    )
    .await
}

/// Offline light layers for desktop clients, beside the licensed kit pack.
async fn bake_file(State(app): State<App>, UrlPath(file): UrlPath<String>) -> Response {
    let Some(directory) = app.config.everglade.as_deref() else {
        return crate::not_found().await;
    };
    if !digest_name(&file, ".vlay") {
        return crate::not_found().await;
    }
    serve_stream(
        directory.join(KIT_DIRECTORY).join("bake").join(&file),
        "application/octet-stream",
        PACK_CACHE,
    )
    .await
}

/// Streams large regular files without a buffered response length.
async fn serve_stream(path: PathBuf, content_type: &'static str, cache: &'static str) -> Response {
    use tokio::io::AsyncReadExt;
    if !tokio::fs::metadata(&path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
    {
        return crate::not_found().await;
    }
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return crate::not_found().await;
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    // Cloud Run caps buffered HTTP/1 responses at 32 MiB. Leave the
    // length unknown so Hyper streams these larger files in chunks.
    let stream = futures_util::stream::try_unfold(file, |mut file| async move {
        let mut bytes = vec![0; 64 * 1024];
        let count = file.read(&mut bytes).await?;
        bytes.truncate(count);
        Ok::<_, std::io::Error>((count != 0).then_some((bytes, file)))
    });
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

/// One regular file's bytes, or `404` when it isn't there.
async fn serve(path: PathBuf, content_type: &'static str, cache: &'static str) -> Response {
    let Ok(metadata) = tokio::fs::metadata(&path).await else {
        return crate::not_found().await;
    };
    if !metadata.is_file() {
        return crate::not_found().await;
    }
    if metadata.len() >= 32 * 1024 * 1024 {
        return serve_stream(path, content_type, cache).await;
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
    fn gzip_is_sent_only_when_the_request_accepts_it() {
        let with = |value: &'static str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static(value));
            accepts_gzip(&headers)
        };
        assert!(with("gzip, deflate, br"));
        assert!(with("br;q=1.0, GZIP;q=0.5"));
        assert!(with("*"));
        assert!(!with("br"));
        assert!(!with("gzip;q=0"));
        assert!(!accepts_gzip(&HeaderMap::new()));
    }

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

    #[test]
    fn the_grid_policy_adds_only_the_public_relay() {
        assert_eq!(
            GRID_POLICY.replace(&format!(" {GRID_RELAY}"), ""),
            EVERGLADE_POLICY
        );
        assert!(GRID_POLICY.contains(&format!("connect-src 'self' {GRID_RELAY};")));
    }
}
