//! Log in and sign up: `/login`, `/signup`, and GitHub sign-in
//! (`/auth/github`, `/auth/github/callback`). See docs/auth.
//!
//! The browser half lives here: a fresh `state` and PKCE verifier in a
//! short-lived flow cookie, then the trip to GitHub and back. The callback
//! hands the code and verifier to the account service, which talks to
//! GitHub, finds or creates the account, and issues the session this
//! server sets as the usual Cloud session cookie.
//!
//! Connecting repositories ([`crate::projects`]) uses the same trip with
//! more scopes ([`oa_auth::Purpose::Repos`]) and the same callback, which
//! hands such a trip on to the projects page's finishing step.

use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use maud::{Markup, PreEscaped, html};
use openagents_ui::actions::{Alert, ButtonLink, ButtonVariant, Color, ControlSize, Variant};
use openagents_ui::icons::Icon;
use serde::Deserialize;

use crate::App;
use crate::account::Account;
use crate::cloud::protect;
use crate::cloud::session::SessionError;
use crate::ui_page::{EmptyMedia, UiPage, action_link, auth_card, empty_media, empty_state};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/login", get(login))
        .route("/signup", get(signup))
        .route("/auth/github", get(start))
        .route("/auth/github/callback", get(callback))
}

/// GitHub's mark, for the Continue with GitHub button.
const GITHUB_MARK: &str = r#"<svg class="oa-icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="currentColor"><path d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/></svg>"#;

#[derive(Deserialize)]
struct Back {
    return_to: Option<String>,
}

/// The `/login?return_to=` link for a page at `path`.
pub(crate) fn login_href(path: &str, signup: bool) -> String {
    let base = if signup { "/signup" } else { "/login" };
    let back = oa_auth::return_to(Some(path));
    if back == "/" {
        return base.into();
    }
    format!("{base}?return_to={}", encode(&back))
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Whether this server signs people in with GitHub.
fn github(app: &App) -> Option<(&oa_auth::GithubApp, &crate::cloud::session::CloudSession)> {
    Some((app.config.github.as_deref()?, app.config.cloud.as_deref()?))
}

async fn login(State(app): State<App>, headers: HeaderMap, Query(back): Query<Back>) -> Response {
    page(&app, &headers, back, false).await
}

async fn signup(State(app): State<App>, headers: HeaderMap, Query(back): Query<Back>) -> Response {
    page(&app, &headers, back, true).await
}

async fn page(app: &App, headers: &HeaderMap, back: Back, signup: bool) -> Response {
    let return_to = oa_auth::return_to(back.return_to.as_deref());
    if let Some(service) = app.config.cloud.as_deref()
        && service.authenticate(headers).await.is_ok()
    {
        return protect(Redirect::to(&return_to).into_response());
    }
    if github(app).is_none() {
        // No GitHub sign-in here: the account-key form.
        if crate::cloud::ready(app) {
            return protect(Redirect::to(crate::cloud::SIGN_IN).into_response());
        }
        return unavailable_page(headers, crate::local_request(headers));
    }
    let invite_only = app
        .config
        .cloud
        .as_deref()
        .is_some_and(|service| service.invite_only());
    sign_in_page(headers, signup, invite_only, &return_to)
}

/// The sign-in page (`/login`, or `/signup` when `signup`): shadcn's
/// login block on its own, without the app shell. Our mark and name, a
/// card with one GitHub button, the way to the other page, and the terms
/// line under it.
pub(crate) fn sign_in_page(
    headers: &HeaderMap,
    signup: bool,
    invite_only: bool,
    return_to: &str,
) -> Response {
    let title = if signup { "Sign up" } else { "Log in" };
    let path = if signup { "/signup" } else { "/login" };
    let start = format!("/auth/github?return_to={}", encode(return_to));
    let continue_button = ButtonLink::new("Continue with GitHub", &start)
        .size(ControlSize::Lg)
        .block(true)
        .icon_start(PreEscaped(GITHUB_MARK));
    let (heading, line) = if signup {
        ("Create your account", "Sign up with your GitHub account.")
    } else {
        ("Welcome back", "Log in with your GitHub account.")
    };
    let card = auth_card(
        heading,
        Some(html! { (line) }),
        html! {
            div.oa-auth-login-form data-slot="login-form" {
                @if invite_only {
                    (Alert::new()
                        .color(Color::Info)
                        .variant(Variant::Soft)
                        .title("Login is invite-only for now.")
                        .description("If you were invited, continue with GitHub."))
                }
                (continue_button)
                @if !invite_only {
                    p.oa-auth-switch {
                        @if signup {
                            "Already have an account? " a href=(login_href(return_to, false)) { "Log in" }
                        } @else {
                            "New here? " a href=(login_href(return_to, true)) { "Sign up" }
                        }
                    }
                }
            }
        },
    );
    let content = html! {
        (card)
        p.oa-auth-fine-print {
            "By continuing, you agree to our " a href="/terms" { "Terms" }
            " and " a href="/privacy" { "Privacy Policy" } "."
        }
    };
    let mut page = UiPage::new(title)
        .path(path)
        .account(Account::Unknown)
        .bare()
        .content(content);
    // A link to a page only signed-in people open lands here when a link
    // reader fetches it: its preview names that page's kind, never "Log in"
    // (crate::link_preview).
    if let Some(preview) = crate::link_preview::signed_out(return_to) {
        page = page.link_preview(preview);
    }
    protect(page.respond(headers))
}

async fn start(State(app): State<App>, headers: HeaderMap, Query(back): Query<Back>) -> Response {
    begin(
        &app,
        &headers,
        back.return_to.as_deref(),
        oa_auth::Purpose::SignIn,
    )
}

/// Send the browser to GitHub for `purpose`, with a fresh flow cookie.
pub(crate) fn begin(
    app: &App,
    headers: &HeaderMap,
    return_to: Option<&str>,
    purpose: oa_auth::Purpose,
) -> Response {
    let client = match purpose {
        oa_auth::Purpose::Install => app.config.github_install.as_deref().map(|i| &i.oauth),
        _ => app.config.github.as_deref(),
    };
    let (Some(github), Some(service)) = (client, app.config.cloud.as_deref()) else {
        return notice(
            headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't log people in with GitHub.",
        );
    };
    // The flow cookie must be set on the address GitHub returns to.
    let origin_host = service
        .origin()
        .split_once("://")
        .map_or("", |(_, host)| host);
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if host != origin_host {
        let back = oa_auth::return_to(return_to);
        let path = match purpose {
            oa_auth::Purpose::SignIn => format!("/auth/github?return_to={}", encode(&back)),
            oa_auth::Purpose::Repos { private } => format!(
                "{}?access={}",
                crate::projects::CONNECT,
                if private { "private" } else { "public" }
            ),
            oa_auth::Purpose::Install => crate::projects::INSTALL.to_string(),
            oa_auth::Purpose::Board => {
                format!("{}?return_to={}", crate::github_tools::GRANT, encode(&back))
            }
        };
        return protect(Redirect::to(&format!("{}{path}", service.origin())).into_response());
    }
    let Ok((url, flow)) = oa_auth::Flow::start_for(github, return_to, purpose) else {
        return notice(
            headers,
            StatusCode::SERVICE_UNAVAILABLE,
            "Login isn't available",
            "Try again in a minute.",
        );
    };
    let mut response = protect(Redirect::to(url.as_str()).into_response());
    if let Ok(cookie) = HeaderValue::from_str(&flow.set_cookie(service.secure())) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub(crate) fn flow_cookie(headers: &HeaderMap) -> Option<oa_auth::Flow> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(name, _)| *name == oa_auth::FLOW_COOKIE)
        .find_map(|(_, value)| oa_auth::Flow::from_cookie(value))
}

async fn callback(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<Callback>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Some(service) = app
        .config
        .cloud
        .as_deref()
        .filter(|_| app.config.github.is_some() || app.config.github_install.is_some())
    else {
        return notice(
            &headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't log people in with GitHub.",
        );
    };
    let query = query.ok().map(|q| q.0);
    // A repository trip finishes on the projects page, in a same-site step
    // that carries the session cookie; its flow cookie stays until then.
    if let Some(Callback {
        code: Some(code),
        state: Some(state),
        error: None,
    }) = &query
        && flow_cookie(&headers).is_some_and(|flow| {
            matches!(
                flow.purpose,
                oa_auth::Purpose::Repos { .. }
                    | oa_auth::Purpose::Install
                    | oa_auth::Purpose::Board
            ) && flow.matches(state)
        })
    {
        return crate::projects::continue_page(&headers, code, state);
    }
    let canceled_repos = query.as_ref().is_some_and(|q| q.error.is_some())
        && flow_cookie(&headers).is_some_and(|flow| {
            matches!(
                flow.purpose,
                oa_auth::Purpose::Repos { .. }
                    | oa_auth::Purpose::Install
                    | oa_auth::Purpose::Board
            )
        });
    let clear = HeaderValue::from_str(&oa_auth::flow::clear_cookie(service.secure()))
        .expect("static cookie is valid");
    if canceled_repos {
        let mut response = protect(crate::ui_page::problem(
            &headers,
            StatusCode::OK,
            "GitHub wasn't connected",
            "Nothing changed. You can connect GitHub any time.",
            (crate::projects::PAGE, "Back to projects"),
        ));
        response.headers_mut().append(header::SET_COOKIE, clear);
        return response;
    }
    let mut response = finish(&app, service, &headers, query).await;
    response.headers_mut().append(header::SET_COOKIE, clear);
    response
}

async fn finish(
    app: &App,
    service: &crate::cloud::session::CloudSession,
    headers: &HeaderMap,
    query: Option<Callback>,
) -> Response {
    let again = |status, title: &str, text: &str| try_again(headers, status, title, text);
    let Some(query) = query else {
        return again(
            StatusCode::BAD_REQUEST,
            "Login didn't go through",
            "Start again.",
        );
    };
    // The state must be the one this browser started with; anything else is
    // an old tab, a second attempt, or someone else's link.
    let Some(flow) = flow_cookie(headers).filter(|flow| {
        query
            .state
            .as_deref()
            .is_some_and(|state| flow.matches(state))
    }) else {
        return again(
            StatusCode::BAD_REQUEST,
            "That login expired",
            "Start again from this browser.",
        );
    };
    if query.error.is_some() {
        return again(
            StatusCode::OK,
            "GitHub login was canceled",
            "Nothing changed. You can try again any time.",
        );
    }
    let Some(code) = query.code.as_deref() else {
        return again(
            StatusCode::BAD_REQUEST,
            "Login didn't go through",
            "Start again.",
        );
    };
    let grant = match service.sign_in_github(code, flow.verifier()).await {
        Ok(grant) => grant,
        Err(SessionError::Unavailable) => {
            return again(
                StatusCode::SERVICE_UNAVAILABLE,
                "Login isn't available right now",
                "Try again in a minute.",
            );
        }
        // Not invited: no account was made and no cookie is set.
        Err(SessionError::InviteOnly) => {
            return notice(
                headers,
                StatusCode::FORBIDDEN,
                "Login is invite-only for now",
                "OpenAgents accounts are open to invited people only for now. Nothing was saved.",
            );
        }
        Err(_) => {
            return again(
                StatusCode::BAD_REQUEST,
                "GitHub didn't accept that login",
                "Try again.",
            );
        }
    };
    let Ok(mut cookies) = grant.cookies() else {
        return again(
            StatusCode::BAD_REQUEST,
            "Login didn't go through",
            "Start again.",
        );
    };
    // Open in the account's own workspace (GitHub sign-up creates one), as
    // the key sign-in does: Settings and the Claude credential need it.
    let Ok(selected) = crate::cloud::default_workspace_cookies(service, &grant.viewer) else {
        return again(
            StatusCode::SERVICE_UNAVAILABLE,
            "Login isn't available right now",
            "Try again in a minute.",
        );
    };
    cookies.extend(selected);
    // Chats this browser made signed out now belong to the account (#11039).
    crate::chat_owner::claim(app, headers, &grant.viewer.account_id).await;
    // The session cookie is SameSite=Strict, and this response ends a trip
    // that began on github.com, so continue with a same-site step: the next
    // request then carries the cookie.
    let back = flow.return_to.clone();
    let content = empty_state(
        EmptyMedia::Icon(Icon::CheckCircle),
        "You're logged in",
        "Taking you back now.",
        ButtonLink::new("Continue", &back),
    );
    let mut response = protect(
        UiPage::new("Logged in")
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .head(html! { meta http-equiv="refresh" content=(format!("0;url={back}")); })
            .content(content)
            .respond(headers),
    );
    for cookie in cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// The command that runs sign-in on this computer with the GitHub fixture
/// (docs/auth/github.md, "Local testing").
const LOCAL_SIGN_IN: &str = "cargo run -p openagents-web --example github_auth_fixture -- \\\n  \"$TMPDIR/oa-github\" 127.0.0.1:4301 \\\n  --github-oauth ~/work/.secrets/github-oauth-local.json";

/// `/login` on a server that doesn't sign people in: the login card with
/// the notice. On this computer's own address (`local`, never a public
/// host) it adds how a developer runs sign-in locally.
pub(crate) fn unavailable_page(headers: &HeaderMap, local: bool) -> Response {
    let card = auth_card(
        "Login isn't available",
        Some(html! { "This server doesn't log people in." }),
        html! {
            div.oa-auth-login-form data-slot="login-form" {
                @if local {
                    div.oa-auth-dev {
                        p { "For developers: login runs on this computer with the GitHub fixture. Run:" }
                        pre.oa-auth-command { code { (LOCAL_SIGN_IN) } }
                        p {
                            "Then open " a href="http://127.0.0.1:4301/login" { "127.0.0.1:4301/login" }
                            " (docs/auth/github.md, Local testing)."
                        }
                    }
                }
                (ButtonLink::new("Home", "/").size(ControlSize::Lg).block(true))
            }
        },
    );
    protect(
        UiPage::new("Login isn't available")
            .path("/login")
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .content(card)
            .respond(headers),
    )
}

/// A sign-in that didn't finish: an empty state with Try again (back to
/// `/login`) and Home, on its own page.
pub(crate) fn try_again(
    headers: &HeaderMap,
    status: StatusCode,
    title: &str,
    text: &str,
) -> Response {
    let actions = html! {
        (ButtonLink::new("Try again", "/login").variant(ButtonVariant::Solid))
        (action_link("Home", "/"))
    };
    bare_notice(headers, status, title, text, actions)
}

/// A sign-in notice (not available here, invite-only): an empty state with
/// a way home, on its own page.
pub(crate) fn notice(headers: &HeaderMap, status: StatusCode, title: &str, text: &str) -> Response {
    let actions = html! {
        (ButtonLink::new("Home", "/"))
        (action_link("Docs", crate::layout::DOCS))
    };
    bare_notice(headers, status, title, text, actions)
}

fn bare_notice(
    headers: &HeaderMap,
    status: StatusCode,
    title: &str,
    text: &str,
    actions: Markup,
) -> Response {
    protect(
        UiPage::new(title)
            .status(status)
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .content(empty_state(empty_media(status), title, text, actions))
            .respond(headers),
    )
}

#[cfg(test)]
mod tests;
