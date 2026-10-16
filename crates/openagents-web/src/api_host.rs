//! The API's one address, `https://api.openagents.com/v1` (`--api-host`,
//! docs/api/README.md "API address").
//!
//! api.openagents.com's load balancer sends its `/v1/...` paths to this
//! service (deploy/production/api-url-map.yaml), apart from the paths the
//! pay front and the older One service still answer there. On the API
//! host this site answers exactly the `/v1/...` paths it answers on
//! openagents.com, by the same handlers and the same forwarding to the
//! gateway, and nothing else (`404`). Keys (`oak_`, `oa_sk_`, `sess_`) work
//! as they do on openagents.com. Cookie sessions stay on openagents.com:
//! the site's cookies are host-only, a request's `Cookie` is dropped here
//! and a `Set-Cookie` is never sent from here, so a cross-site cookie
//! request on the API host has nothing to ride on.
//!
//! The two older addresses, `openagents.com/v1/...` and
//! `openagents.com/api/v1/...`, keep answering exactly as before (Coder
//! 1.0.0 calls one of them) and say they are deprecated: `Deprecation:
//! true` and `Link: <https://api.openagents.com/v1/...>;
//! rel="successor-version"` for the same path. Never a redirect: a redirect
//! drops a POST's body. No `Sunset` yet; that date is the owner's.

use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::{Hosts, host_guard};

/// The API's origin in production.
pub(crate) const API_ORIGIN: &str = "https://api.openagents.com";

/// The origin the API's routes are published at for a site at `origin`:
/// [`API_ORIGIN`] for openagents.com, else the site itself (staging, a
/// local run), which has no API host of its own.
pub(crate) fn api_origin(origin: &str) -> String {
    if origin.trim_end_matches('/') == "https://openagents.com" {
        API_ORIGIN.to_owned()
    } else {
        origin.to_owned()
    }
}

/// Whether `path` is one the API host answers.
fn on_api(path: &str) -> bool {
    path.starts_with("/v1/")
}

/// Whether `host` is the API host and it answers `path`.
pub(crate) fn serves(api: Option<&str>, host: &str, path: &str) -> bool {
    api == Some(host) && on_api(path)
}

/// The API host's address for an older `/v1/...` or `/api/v1/...` path.
pub(crate) fn successor(api: &str, path: &str) -> Option<String> {
    let rest = match path.strip_prefix("/api") {
        Some(rest) if on_api(rest) => rest,
        _ if on_api(path) => path,
        _ => return None,
    };
    Some(format!("https://{api}{rest}"))
}

/// Mark an answer from an older address with its successor. An older
/// path's own `Deprecation` (crate::older_paths) stays; the Link is added
/// beside any it carries.
fn mark(response: &mut Response, successor: &str) {
    let headers = response.headers_mut();
    headers
        .entry("deprecation")
        .or_insert(HeaderValue::from_static("true"));
    if let Ok(link) = HeaderValue::from_str(&format!("<{successor}>; rel=\"successor-version\"")) {
        headers.append(header::LINK, link);
    }
}

/// Places a request by its host before [`host_guard`] (see the module
/// docs).
pub(super) async fn guard(hosts: Hosts, mut request: Request, next: Next) -> Response {
    let Some(api) = hosts.api_host.clone() else {
        return host_guard(hosts, request, next).await;
    };
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let path = request.uri().path().to_owned();
    if host == api {
        if !on_api(&path) {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": {
                        "code": "not_found",
                        "message": "The API's paths start with /v1/. See https://openagents.com/docs/api.",
                    }
                })),
            )
                .into_response();
        }
        request.headers_mut().remove(header::COOKIE);
        let mut response = host_guard(hosts, request, next).await;
        response.headers_mut().remove(header::SET_COOKIE);
        return response;
    }
    let successor = hosts
        .public
        .contains(&host)
        .then(|| successor(&api, &path))
        .flatten();
    let mut response = host_guard(hosts, request, next).await;
    if let Some(successor) = successor {
        mark(&mut response, &successor);
    }
    response
}

#[cfg(test)]
mod tests;
