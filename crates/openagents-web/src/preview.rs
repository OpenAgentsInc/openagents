//! The private preview. The launch flow (the beta homepage, the playtest,
//! the waitlist) and the Kiki surfaces under `/new-lander` stay deployed but
//! hidden until they've been reviewed: without the preview cookie, [`gate`]
//! answers their addresses with 404.
//!
//! `/preview/{key}` sets [`COOKIE`] and opens the beta when the key's
//! SHA-256 is [`KEY_SHA256`]. The key itself is never in this repository;
//! to change it, put the hash of a new random key here and deploy.

use axum::{
    Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use sha2::{Digest, Sha256};

use crate::App;

/// SHA-256 (hex) of the preview key.
const KEY_SHA256: &str = "b0346a4a8e75dd93412c55985dba42714e876b97c53a89c01d60d6c80783f8d4";
/// Holds the key once the preview link has been opened.
pub(crate) const COOKIE: &str = "oa_preview";
/// The preview lasts 30 days on a device.
const COOKIE_AGE: u64 = 30 * 24 * 3600;
/// Addresses only the preview reaches: each one, and everything under it.
const GATED: [&str; 5] = ["/beta", "/playtest", "/waitlist", "/new-lander", "/landernew"];

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/preview/{key}", get(open))
}

fn valid(key: &str) -> bool {
    format!("{:x}", Sha256::digest(key.as_bytes())) == KEY_SHA256
}

/// Whether `path` is one of the preview's addresses.
pub(crate) fn gated(path: &str) -> bool {
    GATED.iter().any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '-', '?']))
    })
}

fn admitted(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .any(|(name, value)| name == COOKIE && valid(value))
}

async fn open(Path(key): Path<String>) -> Response {
    if !valid(&key) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut response = Redirect::to(crate::pages::new_lander::HOME).into_response();
    let cookie = format!(
        "{COOKIE}={key}; Path=/; Max-Age={COOKIE_AGE}; HttpOnly; Secure; SameSite=Lax"
    );
    if let Ok(value) = cookie.parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    response
}

/// Answers a preview address with 404 unless the request carries the key.
/// A loopback check server started with `OPENAGENTS_WEB_OPEN_PREVIEW=1`
/// (the Aethyr release checks) serves them without it.
pub(crate) async fn gate(State(app): State<App>, request: Request, next: Next) -> Response {
    // The site's own tests exercise the gated pages directly; this module's
    // tests cover the gate.
    if !cfg!(test)
        && !app.config.open_preview
        && gated(request.uri().path())
        && !admitted(request.headers())
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_preview_addresses_are_gated() {
        for path in ["/beta", "/playtest", "/playtest/house", "/waitlist", "/new-lander/house", "/new-lander-house"] {
            assert!(gated(path), "{path}");
        }
        for path in ["/", "/home", "/playtesting", "/betas", "/docs", "/preview/x"] {
            assert!(!gated(path), "{path}");
        }
    }

    #[test]
    fn a_wrong_key_admits_nothing() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "oa_preview=wrong; other=1".parse().unwrap());
        assert!(!admitted(&headers));
        assert!(!valid(""));
    }
}
