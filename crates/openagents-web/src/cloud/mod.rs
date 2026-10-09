//! Account services behind sign-in and Settings: the browser session
//! ([`session`]), private files ([`private`]), the credential vault
//! ([`custody`]), the customer's own Claude credential ([`byo`]), and host
//! bindings with their request journal ([`hosts`], `effects`).
//!
//! The old Cloud pages under `/cloud/app` are gone (docs/web/cloud-reset.md).
//! This module answers `/sign-in` and `/sign-out` and sends old Cloud
//! addresses to their new homes.

pub mod byo;
pub mod custody;
// Host bindings and their request journal wait for `/environments`, which
// runs Claude Code on them; nothing calls them until it lands.
#[allow(dead_code)]
mod effects;
#[allow(dead_code)]
pub mod hosts;
mod private;
pub mod session;
#[cfg(test)]
mod tests;

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, OriginalUri, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::html;
use openagents_ui::actions::{Button, ButtonType};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::{Field, Input, InputType};
use serde::Deserialize;

use crate::App;
use crate::account::Account;
use crate::layout::problem;
use crate::ui_page::UiPage;
use session::{CloudSession, SessionError};

/// The account pages' policy: this site's scripts and styles only.
const POLICY: &str = "default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// Where sign-in and sign-out live.
pub(crate) const SIGN_IN: &str = "/sign-in";
pub(crate) const SIGN_OUT: &str = "/sign-out";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(SIGN_IN, get(sign_in).post(sign_in_submit))
        .route(SIGN_OUT, post(sign_out))
        .route("/cloud/sign-in", get(|| async { see_other(SIGN_IN) }))
        .route("/cloud/app", get(moved))
        .route("/cloud/app/{*rest}", get(moved))
        .layer(DefaultBodyLimit::max(8192))
}

/// Whether this server has an account service to sign in with.
pub(crate) fn ready(app: &App) -> bool {
    app.config.cloud.is_some()
}

pub(crate) fn service(app: &App) -> Result<&CloudSession, Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Sign-in unavailable",
            "This server doesn't offer accounts.",
        ));
    };
    service.health().map_err(|_| {
        failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Sign-in unavailable",
            "Sign-in isn't working right now. Try again later.",
        )
    })?;
    Ok(service)
}

/// The private headers every account page and answer carries.
pub(crate) fn protect(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    // Forms keep an exact Origin; outside links receive no referrer.
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    headers.insert(header::VARY, HeaderValue::from_static("Cookie"));
    response
}

pub(crate) fn failure(status: StatusCode, title: &str, message: &str) -> Response {
    protect(problem(status, title, message, ("/", "Back to chat")))
}

pub(crate) fn refused(error: SessionError) -> Response {
    let status = match error {
        SessionError::Unauthenticated => StatusCode::UNAUTHORIZED,
        SessionError::Forbidden | SessionError::Csrf => StatusCode::FORBIDDEN,
        SessionError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        SessionError::InvalidRequest => StatusCode::BAD_REQUEST,
        SessionError::Conflict => StatusCode::CONFLICT,
    };
    failure(status, "That didn't work", &error.to_string())
}

fn see_other(to: &str) -> Response {
    protect(Redirect::to(to).into_response())
}

/// Old Cloud page addresses: Settings pages keep their place, everything
/// else goes home.
async fn moved(OriginalUri(uri): OriginalUri) -> Response {
    see_other(moved_to(uri.path()))
}

fn moved_to(path: &str) -> &'static str {
    let rest = path.strip_prefix("/cloud/app").unwrap_or_default();
    if rest == "/settings/claude" || rest.starts_with("/settings/claude/") {
        crate::settings::CLAUDE
    } else if rest == "/settings" || rest.starts_with("/settings/") {
        crate::settings::PAGE
    } else {
        "/"
    }
}

async fn sign_in(State(app): State<App>, headers: HeaderMap) -> Response {
    // With GitHub sign-in, people log in at /login; the key form below stays
    // only for servers without it (and for the account-key fixtures).
    if app.config.github.is_some() && app.config.cloud.is_some() {
        return protect(Redirect::to("/login").into_response());
    }
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Ok(viewer) = service.authenticate(&headers).await {
        let cookies = match service.refresh_cookies(&headers, &viewer) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        let mut response = see_other("/");
        for cookie in cookies {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
        return response;
    }
    let csrf = match service.login_csrf(&headers, "sign-in", "") {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let key = Field::new("sign-in-key", "Account key").required(true);
    let form = html! {
        form method="post" action=(SIGN_IN) autocomplete="off" {
            input type="hidden" name="csrf" value=(csrf.token);
            (key.clone().control(
                Input::new("credential")
                    .input_type(InputType::Password)
                    .aria(key.aria())
                    .required(true)
                    .autocomplete("off")
                    .spellcheck(false),
            ))
            p { (Button::new("Sign in").kind(ButtonType::Submit)) }
        }
    };
    let content = PageColumn::new(html! {
        (MarkdownRoot::new(html! {
            h1 { "Sign in" }
            p { "Enter your OpenAgents account key." }
        }))
        (form)
    });
    let mut response = protect(
        UiPage::new("Sign in")
            .path(SIGN_IN)
            .account(Account::Unknown)
            .content(content)
            .respond(&headers),
    );
    for cookie in csrf.legacy_cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    if let Some(cookie) = csrf.cookie {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// The workspace a session opens in when none is selected yet. There is no
/// workspace picker: the account's own workspace, else its first.
pub(crate) fn default_workspace(viewer: &session::Viewer) -> Option<&session::Workspace> {
    viewer
        .workspaces
        .iter()
        .find(|workspace| workspace.role == "owner")
        .or_else(|| viewer.workspaces.first())
}

/// The cookies that select [`default_workspace`] (none when the account has
/// no workspace). Every request checks the selection again, so a stale
/// cookie never grants anything.
pub(crate) fn default_workspace_cookies(
    service: &CloudSession,
    viewer: &session::Viewer,
) -> Result<Vec<HeaderValue>, SessionError> {
    match default_workspace(viewer) {
        Some(workspace) => service.workspace_cookies(&workspace.id),
        None => Ok(Vec::new()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignIn {
    credential: String,
    csrf: String,
}

async fn sign_in_submit(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SignIn>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(&headers, None, "sign-in", "", &form.csrf) {
        return refused(error);
    }
    // An OpenAgents account credential only: a claude.ai login or
    // `claude setup-token` value is refused before it goes anywhere.
    if coder_cloud::claude::admit_value(&form.credential).is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let grant = match service.sign_in(&form.credential).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let mut cookies = match grant.cookies() {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    match default_workspace_cookies(service, &grant.viewer) {
        Ok(selected) => cookies.extend(selected),
        Err(error) => return refused(error),
    }
    let mut response = see_other("/");
    for cookie in cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignOut {
    csrf: String,
}

async fn sign_out(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SignOut>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let Some(service) = app.config.cloud.as_deref() else {
        return refused(SessionError::Unavailable);
    };
    // Clearing this browser works even when the account service is down.
    if let Err(error) = service.verify_logout_csrf(&headers, &form.csrf) {
        return refused(error);
    }
    let mut response = match service.sign_out_current(&headers).await {
        Ok(()) => see_other("/"),
        Err(_) => failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Signed out here",
            "This browser is signed out, but we couldn't reach your account to end the session everywhere. Try signing out again later.",
        ),
    };
    for cookie in service.clear_cookies() {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[cfg(test)]
mod moved_tests {
    #[test]
    fn old_cloud_addresses_go_to_settings_or_home() {
        assert_eq!(
            super::moved_to("/cloud/app/settings/claude"),
            "/settings/claude"
        );
        assert_eq!(
            super::moved_to("/cloud/app/settings/claude/credential"),
            "/settings/claude"
        );
        assert_eq!(super::moved_to("/cloud/app/settings"), "/settings");
        assert_eq!(super::moved_to("/cloud/app"), "/");
        assert_eq!(super::moved_to("/cloud/app/hosts/a/tasks"), "/");
        assert_eq!(super::moved_to("/cloud/app/settingsx"), "/");
    }
}
