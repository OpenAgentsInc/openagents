//! Chat builds and session API used by /lander10 and /lander11.

use std::path::PathBuf;

use axum::Router;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use crate::App;
use crate::chat_owner::Who;

/// What the chat reads before it sends anything.
pub(crate) const LANDER8_SESSION: &str = "/lander8/session";

/// The chat's two builds, by the directory each sits in.
pub(crate) const LANDER8_VARIANTS: [&str; 2] = ["webgpu", "webgl"];
/// Each build's files, as `build-web-lander-chat.sh` names them.
pub(crate) const LANDER8_GLUE: &str = "openagents_web_lander_chat.js";
pub(crate) const LANDER8_WASM: &str = "openagents_web_lander_chat_bg.wasm";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(LANDER8_SESSION, get(session))
        .route("/lander8/build/{variant}/{file}", get(build_file))
}

/// `{csrf, signed_in}` for the chat: the value its writes carry in
/// `x-openagents-csrf`, checked as the home page's form checks its own, and
/// whether the chats are a signed-in account's. A browser with no visitor
/// cookie gets one here, as on the home page.
async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    let who = crate::chat_owner::who(&app, &headers).await;
    let (owner, fresh) = match &who {
        Who::Account(owner) | Who::Browser(owner) => (owner.clone(), false),
        Who::Nobody => (crate::ask::new_visitor(), true),
        Who::Unchecked(_) => {
            return crate::coder_sync::refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "We couldn't check your login. Try again in a minute.",
            );
        }
    };
    let body = json!({
        "csrf": super::chat::csrf(&app, &owner),
        "signed_in": who.signed_in(),
    });
    let mut response = (
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    super::chat::cookie(&app, &owner, fresh, &mut response);
    response
}

/// One of the chat's files, brotli or gzip when accepted. Only the two
/// builds' glue and module are served.
async fn build_file(
    State(app): State<App>,
    UrlPath((variant, file)): UrlPath<(String, String)>,
    request: HeaderMap,
) -> Response {
    let content_type = match file.as_str() {
        LANDER8_GLUE => "text/javascript; charset=utf-8",
        LANDER8_WASM => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(root) = app.config.lander8.as_deref() else {
        return crate::not_found().await;
    };
    if !LANDER8_VARIANTS.contains(&variant.as_str()) {
        return crate::not_found().await;
    }
    let directory: PathBuf = root.join(&variant);
    super::everglade::serve_build(&directory, &file, content_type, &request).await
}
