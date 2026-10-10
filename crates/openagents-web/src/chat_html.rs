//! Shared HTML transport and assets for chat projections.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, html};

use crate::App;

pub(crate) const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// HTMX never stores private HTML, evaluates expressions, or runs fragment scripts.
/// The chat and composer styles come from `openagents-ui` (`/static/ui.css`).
pub(crate) fn head() -> Markup {
    html! {
        meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"historyRestoreAsHxRequest":false,"refreshOnHistoryMiss":false,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;
        script src="/static/htmx.min.js" defer {}
        script src="/static/htmx-sse.js" defer {}
        script type="module" src="/static/chat-start.js" {}
        // Files in the composer (#11174): paste, drop, or pick.
        link rel="stylesheet" href=(crate::chat_files::STYLE_PATH);
        script src=(crate::chat_files::SCRIPT_PATH) defer {}
        script src=(crate::analytics::SCRIPT) defer {}
    }
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
    headers.insert(header::VARY, HeaderValue::from_static("Cookie, HX-Request"));
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
