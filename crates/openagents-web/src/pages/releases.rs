//! The release proxy: `https://openagents.com/releases/<name>` is the one
//! durable name in front of the public bucket that holds Coder Terminal's
//! installers, binaries, checksum files, and channel pointers.
//!
//! The proxy adds no authority: every object is already world-readable at
//! the bucket's URL. It adds a stable address, the right content type and
//! cache lifetime, and an allowlist on the one path segment a caller
//! controls, so a request cannot address anything outside the bucket.
//!
//! A `Range` request is answered with `206` and the range, never `200` and
//! the whole body: an installer that fetches ranges in parallel and joins
//! them would otherwise build a corrupt binary from several whole copies.

use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderName, Method, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use futures_util::TryStreamExt;

use crate::App;

/// The longest object name the proxy admits.
const NAME_MAX_BYTES: usize = 128;

/// A version-pinned object never changes.
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

/// A channel pointer or an installer moves; keep it short.
const POINTER_CACHE: &str = "public, max-age=60";

/// How long a channel pointer read for the install page is reused, the
/// same as the pointer's own cache lifetime.
const POINTER_REUSE: Duration = Duration::from_secs(60);

/// What a channel row says when its pointer cannot be read.
const UNKNOWN_VERSION: &str = "unknown";

/// The bucket's headers a download keeps.
const PASSED_HEADERS: [HeaderName; 5] = [
    header::CONTENT_LENGTH,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
    header::ETAG,
    header::LAST_MODIFIED,
];

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/releases/{name}", get(release))
        .route(
            "/install-terminal.sh",
            get(|| async { Redirect::permanent("/releases/install-terminal.sh") }),
        )
        .route(
            "/install-terminal.ps1",
            get(|| async { Redirect::permanent("/releases/install-terminal.ps1") }),
        )
}

/// Whether `name` may be put into the bucket URL: one conservative segment
/// of letters, digits, dot, underscore, and hyphen, starting with a letter
/// or digit, with no `..`.
pub(crate) fn admits_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= NAME_MAX_BYTES
        && first.is_ascii_alphanumeric()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !name.contains("..")
}

/// Whether a `Range` header may pass to the bucket: byte ranges only.
pub(crate) fn admits_range(range: &str) -> bool {
    range.strip_prefix("bytes=").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, ',' | '-' | ' '))
    })
}

/// A version-pinned binary, as opposed to a sums file, a pointer, or an
/// installer. `coder-terminal.stable` shares a prefix, so the hyphen counts.
fn is_artifact(name: &str) -> bool {
    name.starts_with("coder-terminal-") || name.starts_with("openagents-")
}

pub(crate) fn content_type(name: &str) -> &'static str {
    if is_artifact(name) {
        "application/octet-stream"
    } else {
        "text/plain; charset=utf-8"
    }
}

pub(crate) fn cache_control(name: &str) -> &'static str {
    if is_artifact(name) || name.starts_with("SHA256SUMS-") {
        IMMUTABLE_CACHE
    } else {
        POINTER_CACHE
    }
}

/// `GET` and `HEAD /releases/{name}`.
async fn release(
    State(app): State<App>,
    method: Method,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !admits_name(&name) {
        return (StatusCode::NOT_FOUND, "No release has that name.").into_response();
    }
    let Some(client) = app.http.as_ref() else {
        return unavailable();
    };
    let url = format!("{}/{name}", app.config.releases_url.trim_end_matches('/'));
    let head = method == Method::HEAD;
    let mut request = if head {
        client.head(&url)
    } else {
        client.get(&url)
    };
    if let Some(range) = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| admits_range(value))
    {
        request = request.header(header::RANGE, range);
    }
    let Ok(upstream) = request.send().await else {
        return unavailable();
    };
    let status = upstream.status();
    let status = match status.as_u16() {
        200 => StatusCode::OK,
        206 => StatusCode::PARTIAL_CONTENT,
        404 | 403 => {
            return (StatusCode::NOT_FOUND, "No release has that name.").into_response();
        }
        416 => StatusCode::RANGE_NOT_SATISFIABLE,
        _ => return unavailable(),
    };
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type(&name))
        .header(header::CACHE_CONTROL, cache_control(&name));
    for name in PASSED_HEADERS {
        if let Some(value) = upstream.headers().get(&name) {
            builder = builder.header(name, value);
        }
    }
    let body = if head {
        Body::empty()
    } else {
        Body::from_stream(upstream.bytes_stream().map_err(std::io::Error::other))
    };
    builder.body(body).unwrap_or_else(|_| unavailable())
}

fn unavailable() -> Response {
    (
        StatusCode::BAD_GATEWAY,
        [(header::CACHE_CONTROL, "no-store")],
        "The release store could not be read. Try again in a minute.",
    )
        .into_response()
}

/// Whether `text` is a version the installer admits: three numbers and an
/// optional prerelease. A pointer arrives over the network, so it is
/// checked, not trusted.
pub(crate) fn is_version(text: &str) -> bool {
    let (numbers, prerelease) = match text.split_once('-') {
        Some((numbers, prerelease)) => (numbers, Some(prerelease)),
        None => (text, None),
    };
    let mut parts = numbers.split('.');
    let numbered = (0..3).all(|_| {
        parts
            .next()
            .is_some_and(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    });
    if !numbered || parts.next().is_some() {
        return false;
    }
    prerelease.is_none_or(|tail| {
        !tail.is_empty()
            && tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_'))
    })
}

async fn read_pointer(app: &App, channel: &str) -> Option<String> {
    let client = app.http.as_ref()?;
    let url = format!(
        "{}/coder-terminal.{channel}",
        app.config.releases_url.trim_end_matches('/')
    );
    let response = client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let version = response.text().await.ok()?.trim().to_owned();
    is_version(&version).then_some(version)
}

/// The versions `stable` and `rc` name, reused for a minute. A channel
/// that cannot be read is `unknown`.
pub(crate) async fn channel_versions(app: &App) -> (String, String) {
    let mut held = app.pointers.lock().await;
    if let Some((at, stable, rc)) = held.as_ref()
        && at.elapsed() < POINTER_REUSE
    {
        return (stable.clone(), rc.clone());
    }
    let stable = read_pointer(app, "stable")
        .await
        .unwrap_or_else(|| UNKNOWN_VERSION.to_owned());
    let rc = read_pointer(app, "rc")
        .await
        .unwrap_or_else(|| UNKNOWN_VERSION.to_owned());
    *held = Some((Instant::now(), stable.clone(), rc.clone()));
    (stable, rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_allowlist_admits_release_objects_and_nothing_else() {
        for name in [
            "install-terminal.sh",
            "install-terminal.ps1",
            "coder-terminal.stable",
            "coder-terminal-0.4.0-macos-aarch64",
            "SHA256SUMS-coder-terminal-0.4.0",
        ] {
            assert!(admits_name(name), "{name}");
        }
        for name in [
            "",
            ".hidden",
            "a/b",
            "..",
            "a..b",
            "%2e%2e",
            "x?y",
            &"a".repeat(129),
        ] {
            assert!(!admits_name(name), "{name}");
        }
    }

    #[test]
    fn binaries_are_immutable_and_pointers_are_short_lived_text() {
        assert_eq!(
            content_type("coder-terminal-0.4.0-linux-x86_64"),
            "application/octet-stream"
        );
        assert_eq!(
            cache_control("coder-terminal-0.4.0-linux-x86_64"),
            IMMUTABLE_CACHE
        );
        assert_eq!(
            content_type("coder-terminal.stable"),
            "text/plain; charset=utf-8"
        );
        assert_eq!(cache_control("coder-terminal.stable"), POINTER_CACHE);
        assert_eq!(
            cache_control("SHA256SUMS-coder-terminal-0.4.0"),
            IMMUTABLE_CACHE
        );
    }

    #[test]
    fn only_byte_ranges_pass() {
        assert!(admits_range("bytes=0-1023"));
        assert!(admits_range("bytes=0-1,5-9"));
        assert!(!admits_range("bytes="));
        assert!(!admits_range("items=0-1"));
        assert!(!admits_range("bytes=0-1\r\nX: y"));
    }

    #[test]
    fn only_a_version_passes_the_pointer_check() {
        assert!(is_version("0.4.0"));
        assert!(is_version("0.5.0-rc.1"));
        assert!(!is_version("0.4"));
        assert!(!is_version("0.4.0.1"));
        assert!(!is_version("<b>0.4.0</b>"));
        assert!(!is_version("0.4.0-"));
    }
}
