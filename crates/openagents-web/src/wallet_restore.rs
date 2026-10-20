//! An isolated, opt-in Rust/Wasm wallet, served only when its build exists.
use crate::App;
use axum::{
    Router,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
const FILES: &[&str] = &[
    "spark_wallet_web.js",
    "spark_wallet_web_bg.wasm",
    "start.js",
    "wallet.css",
];
fn directory() -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(std::env::var_os("OPENAGENTS_WALLET_WEB")?);
    built(&dir).then_some(dir)
}
fn built(dir: &std::path::Path) -> bool {
    use std::io::Read;
    let mut magic = [0; 8];
    if std::fs::File::open(dir.join("spark_wallet_web_bg.wasm"))
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_err()
        || magic != [0, 97, 115, 109, 1, 0, 0, 0]
    {
        return false;
    }
    FILES.iter().all(|file| {
        dir.join(file)
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() > 0)
    })
}
pub(crate) fn available() -> bool {
    directory().is_some()
}
pub(crate) fn owns(path: &str) -> bool {
    path == "/wallet/restore" || path.starts_with("/wallet/restore-assets/")
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/wallet/restore", get(page))
        .route("/wallet/restore-assets/{file}", get(asset))
}
fn protected(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, "no-store, private".parse().unwrap());
    headers.insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::CONTENT_SECURITY_POLICY,"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self' https: wss:; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'".parse().unwrap());
    response
}
async fn page() -> Response {
    if !available() {
        return StatusCode::NOT_FOUND.into_response();
    }
    protected(
        axum::response::Html(include_str!("../../spark-wallet-web/index.html")).into_response(),
    )
}
async fn asset(Path(file): Path<String>) -> Response {
    if !FILES.contains(&file.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(dir) = directory() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = tokio::fs::read(dir.join(&file)).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = match file.as_str() {
        "spark_wallet_web_bg.wasm" => "application/wasm",
        "wallet.css" => "text/css; charset=utf-8",
        _ => "text/javascript; charset=utf-8",
    };
    protected(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_page_has_an_isolated_policy_and_never_submits_a_form() {
        let page = include_str!("../../spark-wallet-web/index.html");
        assert!(!page.contains("<form") && !page.contains("https://"));
        let response = protected(StatusCode::OK.into_response());
        let policy = response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(policy.contains("form-action 'none'") && policy.contains("frame-ancestors 'none'"));
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store, private"
        );
        assert!(
            owns("/wallet/restore") && crate::upstream::owned("/wallet/restore-assets/start.js")
        );
    }
    #[test]
    fn incomplete_builds_do_not_enable_a_restore_control() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!built(dir.path()));
        for file in FILES {
            std::fs::write(dir.path().join(file), b"incomplete").unwrap();
        }
        assert!(!built(dir.path()));
        std::fs::write(
            dir.path().join("spark_wallet_web_bg.wasm"),
            [0, 97, 115, 109, 1, 0, 0, 0],
        )
        .unwrap();
        assert!(built(dir.path()));
    }
}
