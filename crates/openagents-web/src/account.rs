//! Who is signed in, for the account button at the bottom of the left panel.
//!
//! A middleware ([`scope`]) reads the Cloud web session (the same
//! [`crate::cloud::session::CloudSession::authenticate`] the Cloud app
//! uses) once for a full-page HTML `GET` and keeps the result for the
//! request's handler; [`crate::ui_page::UiPage`] reads it with
//! [`current`]. Visitors without the session cookie cost nothing, and
//! HTMX fragments, scripts, streams and posts skip it.
//!
//! Sign-in is checked once per request: the whole request runs inside
//! [`crate::cloud::session::shared`], so a Cloud page's own
//! `authenticate` reuses the view this lookup read (docs/auth).

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::App;

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
    /// Signed in as `name`; `sign_out` is the sign-out form's CSRF token.
    SignedIn {
        name: String,
        sign_out: Option<String>,
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
