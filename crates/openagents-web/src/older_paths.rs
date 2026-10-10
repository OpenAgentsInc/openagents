//! Our apps' routes under `/v1` (#11158, docs/api/design.md section 6
//! items 4 and 9, section 8 step 5), with the older paths kept as aliases.
//!
//! | Path | Older path (still answered) |
//! | --- | --- |
//! | `POST /v1/device/code`, `/v1/device/token`, `/v1/device/sign-out` | `/device/code`, `/device/token`, `/device/sign-out` |
//! | `GET`/`POST /v1/traces`, `/v1/traces/{id}/...` | `/api/traces`, `/api/traces/{id}/...` |
//! | `GET /v1/threads/synced`, `PUT`/`DELETE /v1/threads/synced/{session}`, `POST .../status`, `POST .../replies` | `/coder/sessions`, `/coder/sessions/{session}`, ... |
//! | `POST /v1/computers/check-in` | `/coder/check-in` |
//! | `GET`/`PUT /v1/computers/{name}/sync` ([`crate::phone_api`]) | `/coder/sync` |
//!
//! Both paths run the same handler. An answer from an older path carries
//! `Deprecation` and `Link: <successor>; rel="successor-version"`; the
//! older paths stay until the two newest releases of every app no longer
//! call them (section 2.4).
//!
//! `/device` (the page a person approves on) is a website page, not API,
//! and keeps its address.

use axum::http::{HeaderValue, Uri, header};
use axum::response::Response;
use axum::routing::MethodRouter;

/// When the older paths were marked deprecated (2026-10-09), as the
/// `Deprecation` header's `@<unix time>` (RFC 9745).
pub(crate) const DEPRECATED_SINCE: &str = "@1791504000";

/// Mark an answer from an older path with its successor.
pub(crate) fn mark(response: &mut Response, successor: &str) {
    let headers = response.headers_mut();
    headers.insert("deprecation", HeaderValue::from_static(DEPRECATED_SINCE));
    if let Ok(link) = HeaderValue::from_str(&format!("<{successor}>; rel=\"successor-version\"")) {
        headers.insert(header::LINK, link);
    }
}

/// The successor of an older `path`: the path with its first `older`
/// replaced by `newer`.
#[must_use]
pub(crate) fn successor(path: &str, older: &str, newer: &str) -> String {
    path.replacen(older, newer, 1)
}

/// `route` answering at an older path (see the module docs).
pub(crate) fn deprecated<S>(
    route: MethodRouter<S>,
    older: &'static str,
    newer: &'static str,
) -> MethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    route.layer(axum::middleware::map_response(
        move |uri: Uri, mut response: Response| async move {
            mark(&mut response, &successor(uri.path(), older, newer));
            response
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_path_names_its_successor() {
        assert_eq!(
            successor(
                "/coder/sessions/s1/status",
                "/coder/sessions",
                "/v1/threads/synced"
            ),
            "/v1/threads/synced/s1/status"
        );
        assert_eq!(
            successor("/api/traces/t1/agents", "/api/traces", "/v1/traces"),
            "/v1/traces/t1/agents"
        );
        let mut response = Response::new(axum::body::Body::empty());
        mark(&mut response, "/v1/device/token");
        assert_eq!(response.headers()["deprecation"], DEPRECATED_SINCE);
        assert_eq!(
            response.headers()[header::LINK],
            "</v1/device/token>; rel=\"successor-version\""
        );
    }
}
