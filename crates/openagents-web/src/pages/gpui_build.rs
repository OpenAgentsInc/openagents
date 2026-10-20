//! Content-addressed GPUI builds and their unversioned entry points.

use std::collections::HashMap;
use std::path::Path;

use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const REVALIDATE: &str = "no-cache";

pub(super) fn content_type(file: &str) -> Option<&'static str> {
    super::everglade::build_type(file).or_else(|| {
        if file.starts_with('.')
            || file.contains("..")
            || !file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            return None;
        }
        if file.ends_with(".ttf") {
            Some("font/ttf")
        } else if file.ends_with(".otf") {
            Some("font/otf")
        } else {
            None
        }
    })
}

fn assets(directory: &Path) -> Option<HashMap<String, String>> {
    let bytes = std::fs::read(directory.join("assets.json")).ok()?;
    let names: HashMap<String, String> = serde_json::from_slice(&bytes).ok()?;
    names
        .iter()
        .all(|(plain, hashed)| {
            content_type(plain).is_some()
                && content_type(hashed).is_some()
                && hashed_name(plain, hashed)
        })
        .then_some(names)
}

pub(crate) fn url(directory: Option<&Path>, prefix: &str, plain: &str) -> String {
    let names = directory.and_then(assets);
    let name = names
        .as_ref()
        .and_then(|names| names.get(plain))
        .map(String::as_str)
        .unwrap_or(plain);
    format!("{prefix}/{name}")
}

fn hashed_name(plain: &str, hashed: &str) -> bool {
    let Some((stem, extension)) = plain.rsplit_once('.') else {
        return false;
    };
    hashed
        .strip_prefix(&format!("{stem}."))
        .and_then(|rest| rest.strip_suffix(&format!(".{extension}")))
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// Redirect stable entry points without caching the redirect. Dependencies use
/// their content hashes directly; only those files get immutable caching.
pub(super) async fn serve(
    directory: &Path,
    prefix: &str,
    file: &str,
    content_type: &'static str,
    request: &HeaderMap,
) -> Response {
    let names = assets(directory);
    if let Some(hashed) = names.as_ref().and_then(|names| names.get(file)) {
        let mut response = Redirect::temporary(&format!("{prefix}/{hashed}")).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE));
        return response;
    }
    let immutable = names
        .as_ref()
        .is_some_and(|names| names.values().any(|name| name == file));
    super::everglade::serve_build_cached(
        directory,
        file,
        content_type,
        request,
        if immutable { IMMUTABLE } else { REVALIDATE },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn entry_points_revalidate_and_only_hashed_files_are_immutable() {
        let dir = tempfile::tempdir().unwrap();
        let hashed = format!("app.{}.wasm", "a".repeat(64));
        std::fs::write(dir.path().join(&hashed), b"module").unwrap();
        std::fs::write(dir.path().join(format!("{hashed}.br")), b"compressed").unwrap();
        std::fs::write(dir.path().join("legacy.js"), b"legacy").unwrap();
        std::fs::write(
            dir.path().join("assets.json"),
            serde_json::to_vec(&HashMap::from([("app.wasm", &hashed)])).unwrap(),
        )
        .unwrap();
        let headers =
            HeaderMap::from_iter([(header::ACCEPT_ENCODING, HeaderValue::from_static("br"))]);
        let response = serve(
            dir.path(),
            "/build",
            "app.wasm",
            "application/wasm",
            &headers,
        )
        .await;
        assert_eq!(
            response.status(),
            axum::http::StatusCode::TEMPORARY_REDIRECT
        );
        assert_eq!(
            response.headers()[header::LOCATION],
            format!("/build/{hashed}")
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], REVALIDATE);
        let response = serve(dir.path(), "/build", &hashed, "application/wasm", &headers).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], IMMUTABLE);
        assert_eq!(response.headers()[header::CONTENT_ENCODING], "br");
        assert_eq!(response.headers()[header::VARY], "accept-encoding");
        let response = serve(
            dir.path(),
            "/build",
            "legacy.js",
            "text/javascript",
            &headers,
        )
        .await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], REVALIDATE);
    }

    #[test]
    fn a_manifest_cannot_redirect_outside_the_build_or_label_mutable_files_immutable() {
        assert!(!hashed_name("app.js", "../app.js"));
        assert!(!hashed_name("app.js", "app.js"));
        assert!(!hashed_name(
            "app.js",
            &format!("app.{}.js", "A".repeat(64))
        ));
    }
}
