//! Verse's opt-in hand landmarker, served only from the configured build.

use crate::App;
use axum::{
    Router,
    extract::{Path as UrlPath, State},
    http::HeaderMap,
    response::Response,
    routing::get,
};

/// The landmarker's files the page may load, by where they are served.
pub(crate) const HAND_FILES: [(&str, &str); 2] = [
    ("vision_bundle.mjs", "text/javascript; charset=utf-8"),
    ("hand_landmarker.task", "application/octet-stream"),
];
pub(crate) const HAND_WASM: [(&str, &str); 2] = [
    ("vision_wasm_internal.js", "text/javascript; charset=utf-8"),
    ("vision_wasm_internal.wasm", "application/wasm"),
];

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/verse/hands/{file}", get(hand_file))
        .route("/verse/hands/wasm/{file}", get(hand_wasm))
}

async fn hand_file(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    serve_hand(&app, &HAND_FILES, "", &file, &request).await
}

async fn hand_wasm(
    State(app): State<App>,
    UrlPath(file): UrlPath<String>,
    request: HeaderMap,
) -> Response {
    serve_hand(&app, &HAND_WASM, "wasm", &file, &request).await
}

/// One of the landmarker's files, by name from the list; nothing else.
async fn serve_hand(
    app: &App,
    files: &[(&str, &'static str)],
    sub: &str,
    file: &str,
    request: &HeaderMap,
) -> Response {
    let Some((_, content_type)) = files.iter().find(|(name, _)| *name == file) else {
        return crate::not_found().await;
    };
    let Some(root) = app.config.lander9.as_deref() else {
        return crate::not_found().await;
    };
    let directory = root.join("hands").join(sub);
    super::everglade::serve_build(&directory, file, content_type, request).await
}
