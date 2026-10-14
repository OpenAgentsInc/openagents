//! Shared HTML transport and assets for chat projections.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, html};

use crate::App;

/// `media-src 'self'`: a Video Lab card's video plays through this site (docs/video-lab/mvp.md).
pub(crate) const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; media-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// HTMX never stores private HTML, evaluates expressions, or runs fragment scripts.
/// The chat and composer styles come from `openagents-ui` (`/static/ui.css`).
pub(crate) fn head() -> Markup {
    html! {
        meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"historyRestoreAsHxRequest":false,"refreshOnHistoryMiss":false,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;
        script src="/static/htmx.min.js" defer {}
        script src="/static/htmx-sse.js" defer {}
        script type="module" src="/static/chat-start.js" {}
        // Files in the composer (#11174): paste, drop, or pick. Their
        // styles are in `/static/ui.css` (`openagents-ui`'s chat-files.css).
        script src=(crate::chat_files::SCRIPT_PATH) defer {}
        script src=(crate::analytics::SCRIPT) defer {}
    }
}

/// The request header `/static/chat-start.js` sends when it moves between
/// the new chat and a chat's page in place: it keeps the left panel it
/// already has and swaps only the main surface, so the page leaves the
/// sidebar's chat list out (`main`). `prefetch` marks the new chat it
/// loads ahead of a click, which is not a page view.
pub(crate) const PART_HEADER: &str = "x-oa-part";

/// Whether this request wants only a chat-family page's main surface
/// ([`PART_HEADER`]): the sidebar's chat list is left out.
pub(crate) fn main_only(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(PART_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| matches!(value, "main" | "prefetch"))
}

pub(crate) fn protect(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    headers.insert(
        header::VARY,
        HeaderValue::from_static("Cookie, HX-Request, X-OA-Part"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    response
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/static/htmx.min.js",
            get(|| async { script(include_str!("../static/vendor/htmx-2.0.11.min.js")) }),
        )
        .route(
            "/static/htmx-sse.js",
            get(|| async { script(include_str!("../static/vendor/htmx-sse-2.2.4.js")) }),
        )
        .route(
            "/static/chat-start.js",
            get(|| async { script(include_str!("../static/chat-start.js")) }),
        )
        .route("/chat/assets/{file}", get(asset))
}

fn script(source: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        source,
    )
        .into_response()
}

async fn asset(State(app): State<App>, Path(file): Path<String>) -> Response {
    let content_type = match file.as_str() {
        "coder_chat_web.js" => "text/javascript; charset=utf-8",
        "coder_chat_web_bg.wasm" => "application/wasm",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let Some(dir) = &app.config.chat_build else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match tokio::fs::read(dir.join(file)).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "public, max-age=300"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
