//! Who is signed in, for the account button at the bottom of the left panel.
//!
//! A middleware ([`scope`]) reads the Cloud web session (the same
//! [`crate::cloud::session::CloudSession::authenticate`] sign-in and
//! Settings use) once for a full-page HTML `GET` and keeps the result for the
//! request's handler; [`crate::ui_page::UiPage`] reads it with
//! [`current`]. Visitors without the session cookie cost nothing, and
//! HTMX fragments, scripts, streams and posts skip it.
//!
//! Sign-in is checked once per request: the whole request runs inside
//! [`crate::cloud::session::shared`], so a Cloud page's own
//! `authenticate` reuses the view this lookup read (docs/auth).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::App;

/// Where the signed-in account's picture is served from this site, so the
/// page's image policy (`img-src 'self'`) allows it.
pub const AVATAR: &str = "/account/avatar";

/// The largest picture kept, in bytes.
const AVATAR_MAX_BYTES: usize = 512 * 1024;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(AVATAR, get(avatar))
}

/// Pictures already fetched from GitHub, by picture address.
fn pictures() -> &'static Mutex<HashMap<String, (String, Vec<u8>)>> {
    static PICTURES: OnceLock<Mutex<HashMap<String, (String, Vec<u8>)>>> = OnceLock::new();
    PICTURES.get_or_init(Default::default)
}

/// The signed-in account's GitHub picture, fetched once and kept.
async fn avatar(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(viewer) = service.authenticate(&headers).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(url) = viewer.avatar_url else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let kept = pictures().lock().ok().and_then(|p| p.get(&url).cloned());
    let (kind, bytes) = match kept {
        Some(found) => found,
        None => match fetch_picture(&url).await {
            Some(found) => {
                if let Ok(mut kept) = pictures().lock() {
                    if kept.len() > 1024 {
                        kept.clear();
                    }
                    kept.insert(url, found.clone());
                }
                found
            }
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    if let Ok(kind) = HeaderValue::from_str(&kind) {
        headers.insert(header::CONTENT_TYPE, kind);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=3600"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn fetch_picture(url: &str) -> Option<(String, Vec<u8>)> {
    if !url.starts_with("https://avatars.githubusercontent.com/") {
        return None;
    }
    let sized = if url.contains('?') {
        format!("{url}&s=64")
    } else {
        format!("{url}?s=64")
    };
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?
        .get(sized)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let kind = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| matches!(*v, "image/png" | "image/jpeg" | "image/gif" | "image/webp"))?
        .to_owned();
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= AVATAR_MAX_BYTES).then(|| (kind, bytes.to_vec()))
}

/// The Cloud session cookie ([`crate::cloud::session`]).
const SESSION_COOKIE: &str = "oa_cloud_session";

/// The visitor's account standing for this page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Account {
    /// Not known here (no Cloud sign-in on this server, or not a page
    /// request): no account button and no sign-in link.
    #[default]
    Unknown,
    /// Sign-in is available and the visitor is not signed in.
    SignedOut,
    /// Signed in as `name`; `sign_out` is the sign-out form's CSRF token;
    /// `picture` when the account has a profile picture ([`AVATAR`]);
    /// `admin` when the account is a site admin (the menu then links the
    /// analytics dashboard).
    SignedIn {
        name: String,
        sign_out: Option<String>,
        picture: bool,
        admin: bool,
    },
}

tokio::task_local! {
    static ACCOUNT: Account;
}

/// The account resolved for the current request, [`Account::Unknown`]
/// outside [`scope`].
pub fn current() -> Account {
    ACCOUNT.try_with(Clone::clone).unwrap_or_default()
}

/// Resolves the account for a page request and runs the handler with it.
pub(crate) async fn scope(State(app): State<App>, request: Request, next: Next) -> Response {
    crate::cloud::session::shared(async move {
        let account = resolve(&app, request.method(), request.headers()).await;
        ACCOUNT.scope(account, next.run(request)).await
    })
    .await
}

/// Whether this server can sign people in: a Cloud account connection,
/// and either GitHub sign-in or the Cloud app build.
pub(crate) fn sign_in_available(app: &App) -> bool {
    app.config.cloud.is_some() && (app.config.github.is_some() || crate::cloud::ready(app))
}

async fn resolve(app: &App, method: &Method, headers: &HeaderMap) -> Account {
    if method != Method::GET || headers.contains_key("hx-request") || !wants_html(headers) {
        return Account::Unknown;
    }
    let Some(service) = app.config.cloud.as_deref() else {
        return Account::Unknown;
    };
    if !sign_in_available(app) {
        return Account::Unknown;
    }
    if !has_session(headers) {
        return Account::SignedOut;
    }
    match service.authenticate(headers).await {
        Ok(viewer) => Account::SignedIn {
            sign_out: service.logout_csrf(headers, &viewer).ok(),
            picture: viewer.avatar_url.is_some(),
            admin: viewer.admin,
            name: viewer.account_label,
        },
        Err(_) => Account::SignedOut,
    }
}

fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains("text/html"))
}

fn has_session(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .any(|(name, value)| name == SESSION_COOKIE && !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn only_the_session_cookie_counts_and_outside_a_request_nothing_is_known() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_theme=dark; oa_cloud_session=abc"),
        );
        assert!(has_session(&headers));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_cloud_session="),
        );
        assert!(!has_session(&headers));
        assert_eq!(current(), Account::Unknown);
    }
}
