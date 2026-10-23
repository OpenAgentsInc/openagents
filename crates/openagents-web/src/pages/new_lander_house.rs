//! The existing browser creator lab, mounted under the isolated Verse preview.

use std::path::{Component, PathBuf};

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use maud::html;

use crate::App;

pub(crate) const HOUSE: &str = "/new-lander/house";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(HOUSE, get(page))
        .route("/new-lander/house/{*file}", get(asset))
}

fn directory() -> Option<PathBuf> {
    let root = std::env::var_os("OPENAGENTS_VERSE_HOUSE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/haunt-preview/dist-verse")
        });
    [
        "verse/index.html",
        "material.wgsl",
        "kiln/pkg/kiln.js",
        "kiln/pkg/kiln_bg.wasm",
        "kiln/tools/manifest.json",
    ]
    .iter()
    .all(|name| root.join(name).is_file())
    .then_some(root)
}

pub(crate) fn available() -> bool {
    directory().is_some()
}

fn creator_label(github: Option<&str>, nostr: Option<&str>, label: &str) -> String {
    if let Some(login) = github {
        return login.chars().take(48).collect();
    }
    if let Some(key) = nostr {
        return format!("Creator {}", key.chars().take(12).collect::<String>());
    }
    if !label.trim().is_empty() && !label.contains('@') && !label.chars().any(char::is_control) {
        return label.chars().take(48).collect();
    }
    "Signed-in creator".into()
}

fn headers(response: &mut Response) {
    let h = response.headers_mut();
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    h.insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; worker-src 'self'; img-src 'self' data: blob:; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
}

async fn page(State(app): State<App>, request: HeaderMap) -> Response {
    let Some(root) = directory() else {
        let mut response = (StatusCode::SERVICE_UNAVAILABLE, Html(html! {
            (maud::DOCTYPE) html lang="en" { head { title { "Haunted House · OpenAgents" } meta name="viewport" content="width=device-width,initial-scale=1"; } body { h1 { "The house isn't open here yet." } p { "You can return to the planets and keep exploring." } a href="/new-lander/verse?world=haunted-house" { "Back to planets" } } }
        }.into_string())).into_response();
        headers(&mut response);
        return response;
    };
    let Ok(source) = tokio::fs::read_to_string(root.join("verse/index.html")).await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let viewer = if let Some(service) = app.config.cloud.as_deref() {
        service.authenticate(&request).await.ok()
    } else {
        None
    };
    let author = viewer.as_ref().map(|v| {
        creator_label(
            v.github.as_ref().map(|(_, login)| login.as_str()),
            v.nostr.as_deref(),
            &v.account_label,
        )
    });
    let access = html! {
        meta name="verse-member" content=(if viewer.is_some() {"true"} else {"false"});
        meta name="verse-login" content=(if app.config.cloud.is_some() {"true"} else {"false"});
        @if let Some(author) = author { meta name="verse-author" content=(author); }
    };
    let mut response =
        Html(source.replace("<!--verse-access-->", &access.into_string())).into_response();
    headers(&mut response);
    response
}

fn content_type(file: &str) -> Option<&'static str> {
    if file.ends_with(".js") {
        Some("text/javascript; charset=utf-8")
    } else if file.ends_with(".css") {
        Some("text/css; charset=utf-8")
    } else if file.ends_with(".wasm") {
        Some("application/wasm")
    } else if file.ends_with(".json") {
        Some("application/json")
    } else if file.ends_with(".wgsl") {
        Some("text/plain; charset=utf-8")
    } else {
        None
    }
}

async fn asset(Path(file): Path<String>) -> Response {
    let path = std::path::Path::new(&file);
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
        || !(file.starts_with("assets/") || file.starts_with("kiln/") || file == "material.wgsl")
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (Some(root), Some(mime)) = (directory(), content_type(&file)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (Ok(root), Ok(target)) = (
        tokio::fs::canonicalize(&root).await,
        tokio::fs::canonicalize(root.join(path)).await,
    ) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !target.starts_with(&root) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(bytes) = tokio::fs::read(target).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut response = ([(header::CONTENT_TYPE, mime)], bytes).into_response();
    headers(&mut response);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    #[test]
    fn new_lander_house_attributes_public_identities_without_exposing_email() {
        assert_eq!(
            creator_label(Some("kiki"), None, "private@example.com"),
            "kiki"
        );
        assert_eq!(
            creator_label(None, Some("abcdef1234567890"), "private@example.com"),
            "Creator abcdef123456"
        );
        assert_eq!(creator_label(None, None, "Kiki"), "Kiki");
        assert_eq!(
            creator_label(None, None, "private@example.com"),
            "Signed-in creator"
        );
    }

    #[tokio::test]
    async fn new_lander_house_never_exposes_source_or_unowned_files() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        for file in [
            "verse/index.html",
            "package.json",
            "src/main.ts",
            "assets/not-there.js",
            "kiln/../../Cargo.toml",
        ] {
            let uri = format!("{HOUSE}/{file}");
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&uri)
                        .header(header::HOST, "127.0.0.1:4300")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{file}");
            assert!(crate::upstream::owned(&uri));
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("verse-member"));
        }
    }
}
