//! The site's own pages calling the account API with the sign-in cookie:
//! the GPUI web chat (`crate::pages::gpui_chat`) reads and writes the
//! account's chats through `/v1/threads` and `/v1/projects` (#12217), and
//! the rest of the desktop app's account features through the routes in
//! [`ACCOUNT`] (#12262), as the apps do, without a token of its own.
//!
//! A request to one of those routes with no `Authorization` header and an
//! `oa_cloud_session` cookie is taken only when the browser marks it
//! same-origin (`Sec-Fetch-Site: same-origin`). A write (any method but
//! `GET` and `HEAD`) must also carry the page's CSRF value in
//! [`CSRF_HEADER`], checked as the chat's forms check theirs
//! ([`crate::pages::chat::validate_form`]). The cookie's session token then
//! goes on as `Authorization: Bearer`, so every route checks it exactly as
//! it checks an app's. A request that already has an `Authorization` header
//! is left as it is.
//!
//! Routes that used to take the cookie in their own handlers, without the
//! same-origin check (traces, Mac jobs, work runs, and Labs), are in
//! [`ACCOUNT`] too, so a cookie reaches them only through this layer.
//! `GET /v1/account` is the account service's own route: this site answers
//! it ([`gateway`]) so the cookie can become a Bearer first, and forwards it
//! with no cookies.
//!
//! Signed out, a browser's `oa_visitor` cookie ([`crate::ask::visitor`]) is
//! taken on the chat routes only ([`visitor_covered`]), with the same two
//! checks, and goes on as that
//! visitor's own token ([`crate::visitor_api::token`]): the page then
//! chats as the app's signed-out visitors do, with their limits, and the
//! chats are the ones the home page shows for that browser (`/lander8`).

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::App;
use crate::coder_sync::refused;

/// The header a cookie-signed write carries the page's CSRF value in, as
/// the chat's file uploads do (`crate::chat_files`).
pub(crate) const CSRF_HEADER: &str = crate::chat_files::TOKEN_HEADER;

/// The account routes a signed-in page reaches with the cookie, each with
/// everything under it (#12262). `/v1/projects` covers a project's own
/// routes (`/v1/projects/{id}*`) too.
pub(crate) const ACCOUNT: &[&str] = &[
    "/v1/settings",
    "/v1/account/export",
    "/v1/threads",
    "/v1/projects",
    "/v1/history",
    "/v1/admin/analytics",
    "/v1/admin/attribution",
    "/v1/admin/access/drive-connections",
    "/v1/computers",
    "/v1/computer-usage",
    "/v1/cloud-host",
    "/v1/agent-definitions",
    "/v1/agent-assignments",
    "/v1/published-agents",
    "/v1/schedules",
    "/v1/labs",
    "/v1/cad",
    "/v1/voice-agent",
    "/v1/keys",
    "/v1/device/keys",
    // The account's name@openagents.com name, which Settings > Your ID
    // shows (#12290).
    "/v1/device/name",
    "/v1/device/lightning-address",
    "/v1/device/earnings",
    "/v1/attention",
    "/v1/notify",
    "/v1/land",
    "/v1/files",
    // The browser's phone-approved wallet requests (#12258).
    "/v1/wallet/browser",
    // The browser's Verse key and its owner tag (#12313).
    "/v1/verse/key",
    // Room tokens for the registry, device, and chat rooms (#12263).
    crate::room_tokens::PATH,
    // A chat room's checkpoint for the browser's chat room (#12224).
    crate::room_tokens::CHECKPOINTS,
    // Their handlers read the cookie themselves; this layer checks it first.
    crate::traces::API,
    crate::mac_jobs::PREFIX,
    crate::work_runs::PREFIX,
];

/// Account routes that take the cookie at that exact path only.
/// `/v1/agents` and its actions are the running work Coder reports and
/// Approve, Deny, and Stop on it, for the GPUI chat (#12316); the gateway's
/// agent sign-up routes under `/v1/agents/` stay out.
const EXACT: &[&str] = &["/v1/account", "/v1/agents", "/v1/agents/actions"];

/// `path` is `prefix` or a path under it.
fn under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Whether `path` takes the sign-in cookie.
pub(crate) fn covered(path: &str) -> bool {
    EXACT.contains(&path) || ACCOUNT.iter().any(|prefix| under(path, prefix))
}

/// Whether `path` takes a signed-out browser's visitor cookie: the chat
/// routes a visitor's token reaches (`/lander8`).
fn visitor_covered(path: &str) -> bool {
    path == "/v1/projects" || under(path, "/v1/threads")
}

/// Whether this site answers `method path` only to forward it to the
/// account service ([`gateway`]): a covered route the service owns.
pub(crate) fn owns(method: &Method, path: &str) -> bool {
    matches!(*method, Method::GET | Method::HEAD) && path == "/v1/account"
}

/// The routes this site forwards to the account service once the layer has
/// turned the cookie into a Bearer.
pub(crate) fn routes() -> axum::Router<App> {
    axum::Router::new().route("/v1/account", axum::routing::get(gateway))
}

/// Forward a covered account-service route to the gateway, as the
/// `/api/v1` alias does: the site's cookies stay behind.
async fn gateway(State(app): State<App>, mut request: Request) -> Response {
    use axum::response::IntoResponse;
    let Some(upstream) = &app.config.inference else {
        return StatusCode::NOT_FOUND.into_response();
    };
    request.headers_mut().remove(header::COOKIE);
    upstream.forward(request).await
}

/// Whether the browser says the request came from this site's own page.
fn same_origin(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        == Some("same-origin")
}

/// The browser's visitor, when its cookie names one a visitor token can
/// carry (lowercase, as [`crate::ask::new_visitor`] makes them).
fn visitor(headers: &HeaderMap) -> Option<String> {
    crate::ask::visitor(headers).filter(|v| !v.bytes().any(|b| b.is_ascii_uppercase()))
}

pub(crate) async fn layer(State(app): State<App>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if !covered(path) || request.headers().contains_key(header::AUTHORIZATION) {
        return next.run(request).await;
    }
    let token = match crate::chat_owner::session(request.headers()) {
        Some(token) => token,
        None => match visitor(request.headers()).filter(|_| visitor_covered(path)) {
            Some(visitor) => crate::visitor_api::token(&app.config.ask_salt, &visitor),
            None => return next.run(request).await,
        },
    };
    if !same_origin(request.headers()) {
        return refused(
            StatusCode::FORBIDDEN,
            "cross_site",
            "Open your chats on openagents.com.",
        );
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        let supplied = request
            .headers()
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        if let Err(response) =
            crate::pages::chat::validate_form(&app, request.headers(), &supplied).await
        {
            return response;
        }
    }
    let Ok(bearer) = HeaderValue::from_str(&format!("Bearer {token}")) else {
        return refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in again to keep going.",
        );
    };
    request.headers_mut().insert(header::AUTHORIZATION, bearer);
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agents_routes_take_the_cookie_but_not_the_gateway_sign_up() {
        assert!(covered("/v1/agents"));
        assert!(covered("/v1/agents/actions"));
        assert!(!covered("/v1/agents/signup"));
        assert!(!covered("/v1/agentsx"));
    }

    #[test]
    fn every_account_route_takes_the_cookie() {
        for prefix in ACCOUNT {
            assert!(covered(prefix), "{prefix}");
            assert!(covered(&format!("{prefix}/abc/events")), "{prefix}");
            assert!(!covered(&format!("{prefix}x")), "{prefix}");
        }
        for path in [
            "/v1/threads",
            "/v1/threads/abc/events",
            "/v1/projects",
            "/v1/projects/p1/files",
            "/v1/history/pins",
            "/v1/computers/Studio/sync",
            "/v1/computer-usage",
            "/v1/cloud-host/stop",
            "/v1/agent-definitions/a1/grants",
            "/v1/agent-assignments/a1/commands",
            "/v1/published-agents/p1/fork",
            "/v1/schedules/s1",
            "/v1/labs/agent/kinds",
            "/v1/labs/documents/d1",
            "/v1/voice-agent",
            "/v1/keys/k1",
            "/v1/device/keys/challenge",
            "/v1/device/name",
            "/v1/account",
            "/v1/attention",
            "/v1/notify/n1/read",
            "/v1/land/l1",
            "/v1/files/f1/content",
            "/v1/traces/t1",
            "/v1/mac-jobs/events",
            "/v1/work/w1",
        ] {
            assert!(covered(path), "{path}");
        }
        for path in [
            "/v1/threadsx",
            "/v1/account/github/token",
            "/v1/accounts",
            "/v1/device/nostr/challenge",
            "/v1/work-hosts",
            "/v1/responses",
            "/v1/workspaces/w1/keys",
            "/api/v1/account",
        ] {
            assert!(!covered(path), "{path}");
        }
    }

    #[test]
    fn a_visitor_cookie_reaches_only_the_chat_routes() {
        assert!(visitor_covered("/v1/threads"));
        assert!(visitor_covered("/v1/threads/abc/messages"));
        assert!(visitor_covered("/v1/projects"));
        assert!(!visitor_covered("/v1/projects/p1"));
        assert!(!visitor_covered("/v1/computers"));
        assert!(!visitor_covered("/v1/keys"));
    }

    #[test]
    fn only_reading_the_account_is_forwarded() {
        assert!(owns(&Method::GET, "/v1/account"));
        assert!(owns(&Method::HEAD, "/v1/account"));
        assert!(!owns(&Method::DELETE, "/v1/account"));
        assert!(!owns(&Method::GET, "/v1/account/access"));
    }

    #[test]
    fn a_cookie_request_must_be_same_origin() {
        let mut headers = HeaderMap::new();
        assert!(!same_origin(&headers));
        headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!same_origin(&headers));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(same_origin(&headers));
    }

    #[test]
    fn a_visitor_cookie_names_its_visitor() {
        let mut headers = HeaderMap::new();
        assert_eq!(visitor(&headers), None);
        let id = "0123456789abcdef0123456789abcdef";
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{}={id}", crate::ask::COOKIE)).unwrap(),
        );
        assert_eq!(visitor(&headers).as_deref(), Some(id));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{}={}", crate::ask::COOKIE, id.to_uppercase()))
                .unwrap(),
        );
        assert_eq!(visitor(&headers), None);
    }
}
