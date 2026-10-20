//! The site's own pages calling the chat API with the sign-in cookie
//! (#12217): the GPUI web chat (`crate::pages::gpui_chat`) reads and writes
//! the account's chats through `/v1/threads` and `/v1/projects`, as the
//! apps do, without a token of its own.
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
//! Signed out, a browser's `oa_visitor` cookie ([`crate::ask::visitor`]) is
//! taken the same way, with the same two checks, and goes on as that
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

/// Whether `path` takes the sign-in cookie.
pub(crate) fn covered(path: &str) -> bool {
    path == "/v1/projects"
        || path
            .strip_prefix("/v1/threads")
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
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
    if !covered(request.uri().path()) || request.headers().contains_key(header::AUTHORIZATION) {
        return next.run(request).await;
    }
    let token = match crate::chat_owner::session(request.headers()) {
        Some(token) => token,
        None => match visitor(request.headers()) {
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
    fn only_the_chat_routes_take_the_cookie() {
        assert!(covered("/v1/threads"));
        assert!(covered("/v1/threads/abc/events"));
        assert!(covered("/v1/projects"));
        assert!(!covered("/v1/threadsx"));
        assert!(!covered("/v1/computers"));
        assert!(!covered("/v1/projects/p1/files"));
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
