//! WorkOS sign-in on the website (#12323,
//! docs/auth/2026-10-20-workos-source-of-truth.md): our own pages over the
//! WorkOS User Management API, never the hosted AuthKit page.
//!
//! The pages offer only the methods this environment has turned on in
//! WorkOS (`workos.methods`): GitHub and Google through WorkOS, single
//! sign-on for an organization's email domain, email and password with
//! sign-up and email verification, and an emailed one-time code. Signing
//! in with a Nostr key (the phone app's code or a browser extension) stays
//! beside them, unchanged.
//!
//! When WorkOS signs a person in, the access token goes to the account
//! service, which checks it against WorkOS's keys, finds or makes the
//! account, and in `dual` mode issues a legacy session. In `workos` mode
//! this server uses only the sealed WorkOS session cookie
//! ([`WORKOS_COOKIE`]). [`layer`] refreshes the sealed session when its
//! access token expires; in `workos` mode a refresh WorkOS refuses (the
//! session was revoked) signs the browser out. Signing out revokes the
//! WorkOS session.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use maud::{PreEscaped, html};
use oa_auth::workos::client::{Authenticated, NewUser, Refusal};
use oa_auth::workos::{CALLBACK_PATH, Grant, Method, Mode};
use openagents_ui::actions::{
    Alert, Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize, Variant,
};
use openagents_ui::forms::{Field, Input, InputType};
use openagents_ui::icons::Icon;
use serde::{Deserialize, Serialize};

use crate::App;
use crate::account::Account;
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, SessionError, WORKOS_COOKIE, WebWorkos};
use crate::ui_page::{EmptyMedia, UiPage, auth_card, empty_state};

/// The short-lived cookie a provider sign-in's `state` lives in.
const FLOW_COOKIE: &str = "oa_workos_flow";
/// The cookie an unfinished email sign-in (a code to type) lives in.
const PENDING_COOKIE: &str = "oa_workos_pending";
/// How long either lasts.
const FLOW_SECONDS: u64 = 600;
/// The CSRF scope every form here shares.
const SCOPE: &str = "workos";

/// GitHub's and Google's marks for their buttons.
const GITHUB_MARK: &str = r#"<svg class="oa-icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="currentColor"><path d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/></svg>"#;
const GOOGLE_MARK: &str = r#"<svg class="oa-icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" fill="currentColor"><path d="M15.68 8.18c0-.57-.05-1.12-.15-1.64H8v3.1h4.3a3.68 3.68 0 0 1-1.6 2.41v2h2.58c1.51-1.39 2.4-3.44 2.4-5.87ZM8 16c2.16 0 3.97-.72 5.29-1.94l-2.58-2c-.72.48-1.63.76-2.71.76-2.08 0-3.85-1.4-4.48-3.3H.86v2.07A8 8 0 0 0 8 16Zm-4.48-6.48a4.8 4.8 0 0 1 0-3.04V4.41H.86a8 8 0 0 0 0 7.18l2.66-2.07ZM8 3.18c1.17 0 2.23.4 3.06 1.2l2.29-2.3A7.7 7.7 0 0 0 8 0 8 8 0 0 0 .86 4.41l2.66 2.07C4.15 4.58 5.92 3.18 8 3.18Z"/></svg>"#;

/// Whether this server signs people in with WorkOS.
pub(crate) fn on(app: &App) -> Option<(&CloudSession, &WebWorkos)> {
    let service = app.config.cloud.as_deref()?;
    Some((service, service.workos()?))
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The provider sign-in in flight: its `state` and where to go after.
#[derive(Serialize, Deserialize)]
struct Flow {
    state: String,
    return_to: String,
    exp: u64,
}

/// An email sign-in waiting for a code: which kind, for which email, and
/// for an email verification the token that finishes it.
#[derive(Serialize, Deserialize)]
struct Pending {
    /// `verify` (a sign-up or log-in that needs the email confirmed) or
    /// `code` (an emailed one-time code).
    kind: String,
    email: String,
    #[serde(default)]
    token: Option<String>,
    return_to: String,
    exp: u64,
}

fn seal<T: Serialize>(workos: &WebWorkos, value: &T) -> Option<String> {
    oa_auth::workos::sdk::seal(value, workos.cookie_password()).ok()
}

fn open<T: for<'de> Deserialize<'de>>(
    workos: &WebWorkos,
    headers: &HeaderMap,
    name: &str,
) -> Option<T> {
    let sealed = CloudSession::sealed_value(headers, name)?;
    oa_auth::workos::sdk::unseal(&sealed, workos.cookie_password()).ok()
}

fn pending(workos: &WebWorkos, headers: &HeaderMap) -> Option<Pending> {
    open::<Pending>(workos, headers, PENDING_COOKIE).filter(|pending| pending.exp > now())
}

fn random_state() -> String {
    let bytes: [u8; 24] = secp256k1::rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The words for a page notice, from a refusal or a `?error=` code.
pub(crate) fn notice_for(code: &str) -> &'static str {
    match code {
        "invalid_credentials" => Refusal::InvalidCredentials.message(),
        "weak_password" => Refusal::WeakPassword.message(),
        "email_taken" => Refusal::EmailTaken.message(),
        "invalid_email" => Refusal::InvalidEmail.message(),
        "code_expired" => Refusal::Expired.message(),
        "rate_limited" => Refusal::RateLimited(60).message(),
        "sign_in_unsupported" => "This account needs a sign-in step we don't offer yet.",
        "no_sso" => "Single sign-on isn't set up for that email's domain.",
        "canceled" => "Login was canceled. Nothing changed.",
        "provider_failed" => "Sign-in didn't finish. Try again.",
        "expired" => "That login expired. Start again from this browser.",
        "invite_only" => "Login is invite-only for now.",
        "taken" => "That login is already used by another OpenAgents account.",
        "agent" => "An agent account uses its API key; it can't log in here.",
        _ => "Login isn't available right now. Try again in a minute.",
    }
}

/// The `/login` or `/signup` page with WorkOS on.
pub(crate) fn page(
    app: &App,
    headers: &HeaderMap,
    signup: bool,
    return_to: &str,
    notice: Option<&str>,
    email: Option<&str>,
    status: StatusCode,
) -> Response {
    let Some((service, workos)) = on(app) else {
        return crate::auth::unavailable_page(headers, crate::local_request(headers));
    };
    let csrf = match service.login_csrf(headers, SCOPE, "") {
        Ok(csrf) => csrf,
        Err(_) => {
            return crate::auth::try_again(
                headers,
                StatusCode::SERVICE_UNAVAILABLE,
                "Login isn't available right now",
                "Try again in a minute.",
            );
        }
    };
    let mut response = render_login(
        headers,
        signup,
        return_to,
        notice,
        email,
        &workos.client.settings().methods,
        &csrf.token,
        status,
    );
    for cookie in csrf.legacy_cookies.into_iter().chain(csrf.cookie) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// The login or sign-up page offering `methods`, with the form ticket
/// `csrf` (the screens gallery draws it with a sample one).
#[allow(clippy::too_many_arguments)]
pub fn render_login(
    headers: &HeaderMap,
    signup: bool,
    return_to: &str,
    notice: Option<&str>,
    email: Option<&str>,
    methods: &[Method],
    csrf: &str,
    status: StatusCode,
) -> Response {
    struct Offers<'a>(&'a [Method]);
    impl Offers<'_> {
        fn offers(&self, method: Method) -> bool {
            self.0.contains(&method)
        }
    }
    let settings = Offers(methods);
    let back = encode(return_to);
    let title = if signup { "Sign up" } else { "Log in" };
    let path = if signup { "/signup" } else { "/login" };
    let email_value = email.unwrap_or_default().to_string();
    let hidden = html! {
        input type="hidden" name="csrf" value=(csrf);
        input type="hidden" name="return_to" value=(return_to);
    };
    let email_input = |id: &str| {
        let field = Field::new(id, "Email").required(true);
        field.clone().control(
            Input::new("email")
                .id(id)
                .input_type(InputType::Email)
                .aria(field.aria())
                .required(true)
                .autocomplete("email")
                .value(email_value.clone())
                .spellcheck(false),
        )
    };
    let password_form = settings.offers(Method::Password).then(|| {
        let password = Field::new(if signup { "signup-password" } else { "login-password" }, "Password").required(true);
        let password = if signup {
            password.description("At least 10 characters.")
        } else {
            password
        };
        html! {
            form.oa-auth-login-form data-slot="login-form" method="post" action=(path) {
                (hidden)
                @if signup {
                    @let name = Field::new("signup-name", "Name").optional(true);
                    (name.clone().control(Input::new("name").id("signup-name").aria(name.aria()).autocomplete("name").maxlength(120)))
                }
                (email_input(if signup { "signup-email" } else { "login-email" }))
                (password.clone().control(
                    Input::new("password")
                        .id(password.id().to_string())
                        .input_type(InputType::Password)
                        .aria(password.aria())
                        .required(true)
                        .minlength(if signup { 10 } else { 1 })
                        .autocomplete(if signup { "new-password" } else { "current-password" }),
                ))
                (Button::new(if signup { "Create account" } else { "Log in" }).kind(ButtonType::Submit).size(ControlSize::Lg).block(true))
                @if !signup {
                    p.oa-auth-switch { a href=(format!("/login/forgot?return_to={back}")) { "Forgot your password?" } }
                }
            }
        }
    });
    let code_form = settings.offers(Method::MagicCode).then(|| html! {
        form.oa-auth-login-form data-slot="login-form" method="post" action="/login/code" {
            (hidden)
            (email_input(if signup { "signup-code-email" } else { "login-code-email" }))
            (Button::new("Email me a code").kind(ButtonType::Submit).variant(ButtonVariant::Outline).size(ControlSize::Lg).block(true))
        }
    });
    let sso_form = settings.offers(Method::Sso).then(|| {
        let field = Field::new("sso-email", "Work email").required(true);
        html! {
            form.oa-auth-login-form data-slot="login-form" method="post" action="/login/sso" {
                (hidden)
                (field.clone().control(
                    Input::new("email").id("sso-email").input_type(InputType::Email).aria(field.aria())
                        .required(true).autocomplete("email").spellcheck(false),
                ))
                (Button::new("Continue with single sign-on").kind(ButtonType::Submit).variant(ButtonVariant::Outline).size(ControlSize::Lg).block(true))
            }
        }
    });
    let providers = html! {
        @if settings.offers(Method::Github) {
            (ButtonLink::new("Continue with GitHub", &format!("/auth/workos/github?return_to={back}"))
                .size(ControlSize::Lg).block(true).icon_start(PreEscaped(GITHUB_MARK)))
        }
        @if settings.offers(Method::Google) {
            (ButtonLink::new("Continue with Google", &format!("/auth/workos/google?return_to={back}"))
                .size(ControlSize::Lg).block(true).icon_start(PreEscaped(GOOGLE_MARK)))
        }
    };
    let heading = if signup {
        "Create your account"
    } else {
        "Welcome back"
    };
    let card = auth_card(
        heading,
        Some(html! { "The OpenAgents app needs no sign-in. Open it and start." }),
        html! {
            div.oa-auth-login-form data-slot="login-form" {
                @if let Some(notice) = notice {
                    (Alert::new().color(Color::Danger).variant(Variant::Soft).title(notice))
                }
                (providers)
                @if let Some(form) = password_form { (form) }
                @if let Some(form) = code_form {
                    p.oa-auth-switch { "Or get a one-time code by email." }
                    (form)
                }
                @if let Some(form) = sso_form { (form) }
                p.oa-auth-switch { "Or log in with your Nostr key." }
                (crate::web_sign_in::sign_in_block(return_to))
                @if signup {
                    p.oa-auth-switch { "Have an account? " a href=(format!("/login?return_to={back}")) { "Log in" } }
                } @else {
                    p.oa-auth-switch { "New here? " a href=(format!("/signup?return_to={back}")) { "Create an account" } }
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
        .status(status)
        .account(Account::Unknown)
        .bare()
        .content(content);
    if let Some(preview) = crate::link_preview::signed_out(return_to) {
        page = page.link_preview(preview);
    }
    protect(page.respond(headers))
}

#[derive(Deserialize)]
pub(crate) struct Back {
    return_to: Option<String>,
}

/// `GET /auth/workos/{github|google}`: send the browser to WorkOS for that
/// provider, with a fresh `state` in a flow cookie.
pub(crate) async fn start(
    State(app): State<App>,
    headers: HeaderMap,
    Path(method): Path<String>,
    Query(back): Query<Back>,
) -> Response {
    let return_to = oa_auth::return_to(back.return_to.as_deref());
    let Some((service, workos)) = on(&app) else {
        return crate::auth::notice(
            &headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        );
    };
    let Some(method) = Method::from_slug(&method)
        .filter(|method| method.provider().is_some() && workos.client.settings().offers(*method))
    else {
        return crate::auth::notice(
            &headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        );
    };
    redirect_to_provider(service, workos, method.provider(), None, &return_to, None)
}

fn redirect_to_provider(
    service: &CloudSession,
    workos: &WebWorkos,
    provider: Option<&str>,
    organization: Option<&str>,
    return_to: &str,
    login_hint: Option<&str>,
) -> Response {
    let state = random_state();
    let redirect = format!("{}{CALLBACK_PATH}", service.origin());
    let Ok(url) =
        workos
            .client
            .authorization_url(provider, organization, &redirect, &state, login_hint)
    else {
        return problem_redirect(return_to, "unavailable");
    };
    let flow = Flow {
        state,
        return_to: return_to.to_string(),
        exp: now() + FLOW_SECONDS,
    };
    let mut response = protect(Redirect::to(&url).into_response());
    // The callback is a cross-site navigation, which a SameSite=Strict
    // cookie isn't sent on; the flow cookie is Lax for that one trip.
    if let Some(sealed) = seal(workos, &flow)
        && let Ok(cookie) = service.sealed_cookie(FLOW_COOKIE, &sealed, FLOW_SECONDS)
        && let Ok(lax) = HeaderValue::from_str(
            &cookie
                .to_str()
                .unwrap_or_default()
                .replace("SameSite=Strict", "SameSite=Lax"),
        )
    {
        response.headers_mut().append(header::SET_COOKIE, lax);
    }
    response
}

fn problem_redirect(return_to: &str, code: &str) -> Response {
    protect(
        Redirect::to(&format!(
            "/login?error={code}&return_to={}",
            encode(return_to)
        ))
        .into_response(),
    )
}

#[derive(Deserialize)]
pub(crate) struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// `GET /auth/callback`: WorkOS sends the browser back here with a code.
pub(crate) async fn callback(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<Callback>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Some((service, workos)) = on(&app) else {
        return crate::auth::notice(
            &headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        );
    };
    let flow = open::<Flow>(workos, &headers, FLOW_COOKIE).filter(|flow| flow.exp > now());
    let clear = service.clear_cookie(FLOW_COOKIE);
    let mut response = match (query.ok().map(|q| q.0), flow) {
        (_, None) => problem_redirect("/", "expired"),
        (
            Some(Callback {
                error: Some(error),
                error_description,
                ..
            }),
            Some(flow),
        ) => {
            let code = if matches!(error.as_str(), "access_denied" | "canceled") {
                "canceled"
            } else {
                eprintln!(
                    "openagents-web: WorkOS callback failed: error={error:?} description={error_description:?}"
                );
                "provider_failed"
            };
            problem_redirect(&flow.return_to, code)
        }
        (
            Some(Callback {
                code: Some(code),
                state: Some(state),
                ..
            }),
            Some(flow),
        ) if state == flow.state => match workos.client.authenticate(Grant::Code { code }).await {
            Ok(signed) => {
                finish(
                    &app,
                    service,
                    workos,
                    &headers,
                    signed,
                    &flow.return_to,
                    true,
                )
                .await
            }
            Err(refusal) => problem_redirect(&flow.return_to, refusal.code()),
        },
        (_, Some(flow)) => problem_redirect(&flow.return_to, "expired"),
    };
    response.headers_mut().append(header::SET_COOKIE, clear);
    response
}

/// Finish a WorkOS sign-in: our session from the account service, the
/// sealed WorkOS session beside it, then back where the person was going.
/// `cross_site` is a provider trip, which continues with a same-site step
/// so the next request carries the Strict session cookie.
async fn finish(
    app: &App,
    service: &CloudSession,
    workos: &WebWorkos,
    headers: &HeaderMap,
    signed: Authenticated,
    return_to: &str,
    cross_site: bool,
) -> Response {
    let grant = match service.sign_in_workos(signed.access_token.expose()).await {
        Ok(grant) => grant,
        Err(error) => {
            // No account session: end the WorkOS one too.
            if let Ok(claims) = workos.client.verify(signed.access_token.expose()).await {
                workos.client.revoke_session(&claims.sid).await.ok();
            }
            let code = match error {
                SessionError::InviteOnly => "invite_only",
                SessionError::Conflict => "taken",
                SessionError::SignupClosed => "invite_only",
                SessionError::Unauthenticated | SessionError::Forbidden => "agent",
                _ => "unavailable",
            };
            return problem_redirect(return_to, code);
        }
    };
    let Ok(mut cookies) = grant.cookies() else {
        return problem_redirect(return_to, "unavailable");
    };
    let Ok(selected) = crate::cloud::default_workspace_cookies(service, &grant.viewer) else {
        return problem_redirect(return_to, "unavailable");
    };
    cookies.extend(selected);
    let remaining = if workos.mode == Mode::Workos {
        8 * 3600
    } else {
        grant.viewer.expires_at.saturating_sub(now()).max(60)
    };
    let data = oa_auth::workos::sdk::SessionData {
        access_token: signed.access_token.clone(),
        refresh_token: signed.refresh_token.clone(),
        user: None,
        impersonator: None,
    };
    if let Ok(sealed) = oa_auth::workos::sdk::seal_session(&data, workos.cookie_password())
        && let Ok(cookie) = service.sealed_cookie(WORKOS_COOKIE, &sealed, remaining)
    {
        cookies.push(cookie);
    }
    cookies.push(service.clear_cookie(PENDING_COOKIE));
    // Chats this browser made signed out now belong to the account (#11039).
    crate::chat_owner::claim(app, headers, &grant.viewer.account_id).await;
    let ticket = crate::attribution::ticket(app, headers);
    let mut response = if cross_site {
        let content = empty_state(
            EmptyMedia::Icon(Icon::CheckCircle),
            "You're logged in",
            "Taking you back now.",
            ButtonLink::new("Continue", return_to),
        );
        protect(
            UiPage::new("Logged in")
                .account(Account::Unknown)
                .scriptless()
                .bare()
                .head(html! { meta http-equiv="refresh" content=(format!("0;url={return_to}")); })
                .content(content)
                .respond(headers),
        )
    } else {
        protect(Redirect::to(return_to).into_response())
    };
    for cookie in cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    if let Some(ticket) = ticket.as_ref().filter(|_| grant.created) {
        app.config.attribution.signed_up(ticket);
    }
    if crate::attribution::has_cookie(headers) {
        response.headers_mut().append(
            header::SET_COOKIE,
            crate::attribution::clear(service.secure()),
        );
    }
    response
}

#[derive(Deserialize)]
pub(crate) struct PasswordForm {
    csrf: String,
    #[serde(default)]
    return_to: Option<String>,
    email: String,
    password: String,
    #[serde(default)]
    name: Option<String>,
}

fn checked<'a>(
    app: &'a App,
    headers: &HeaderMap,
    csrf: &str,
) -> Result<(&'a CloudSession, &'a WebWorkos), Response> {
    let Some((service, workos)) = on(app) else {
        return Err(crate::auth::notice(
            headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        ));
    };
    if service.verify_csrf(headers, None, SCOPE, "", csrf).is_err() {
        return Err(problem_redirect("/", "expired"));
    }
    Ok((service, workos))
}

fn email_ok(email: &str) -> bool {
    let email = email.trim();
    email.len() <= 254
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        })
        && !email.chars().any(char::is_whitespace)
        && !oa_auth::workos::is_synthetic(email)
}

/// After a refusal that needs the email confirmed: keep the pending
/// sign-in in a cookie and ask for the emailed code.
fn ask_for_code(service: &CloudSession, workos: &WebWorkos, pending: Pending) -> Response {
    let mut response = protect(Redirect::to("/login/verify").into_response());
    if let Some(sealed) = seal(workos, &pending)
        && let Ok(cookie) = service.sealed_cookie(PENDING_COOKIE, &sealed, FLOW_SECONDS)
    {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// `POST /login`: log in with email and password.
pub(crate) async fn password_login(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<PasswordForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "invalid_credentials");
    };
    let return_to = oa_auth::return_to(form.return_to.as_deref());
    let (service, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !workos.client.settings().offers(Method::Password) || !email_ok(&form.email) {
        return page(
            &app,
            &headers,
            false,
            &return_to,
            Some(Refusal::InvalidCredentials.message()),
            Some(&form.email),
            StatusCode::BAD_REQUEST,
        );
    }
    let email = form.email.trim().to_string();
    match workos
        .client
        .authenticate(Grant::Password {
            email: email.clone(),
            password: form.password,
        })
        .await
    {
        Ok(signed) => finish(&app, service, workos, &headers, signed, &return_to, false).await,
        Err(Refusal::EmailVerificationRequired { pending, .. }) => ask_for_code(
            service,
            workos,
            Pending {
                kind: "verify".into(),
                email,
                token: Some(pending),
                return_to,
                exp: now() + FLOW_SECONDS,
            },
        ),
        Err(refusal) => page(
            &app,
            &headers,
            false,
            &return_to,
            Some(refusal.message()),
            Some(&email),
            StatusCode::from_u16(refusal.status()).unwrap_or(StatusCode::BAD_REQUEST),
        ),
    }
}

/// `POST /signup`: make a WorkOS User with a password, then confirm the
/// email with the code WorkOS sends.
pub(crate) async fn password_signup(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<PasswordForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "invalid_email");
    };
    let return_to = oa_auth::return_to(form.return_to.as_deref());
    let (service, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !workos.client.settings().offers(Method::Password) {
        return page(
            &app,
            &headers,
            true,
            &return_to,
            Some(notice_for("unavailable")),
            None,
            StatusCode::NOT_FOUND,
        );
    }
    let email = form.email.trim().to_string();
    if !email_ok(&email) {
        return page(
            &app,
            &headers,
            true,
            &return_to,
            Some(Refusal::InvalidEmail.message()),
            Some(&email),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }
    if form.password.chars().count() < 10 || form.password.len() > 256 {
        return page(
            &app,
            &headers,
            true,
            &return_to,
            Some(Refusal::WeakPassword.message()),
            Some(&email),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }
    let name = form
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let (first, last) = match name.and_then(|name| name.split_once(' ')) {
        Some((first, last)) => (Some(first.to_string()), Some(last.trim().to_string())),
        None => (name.map(str::to_string), None),
    };
    let made = workos
        .client
        .create_user(NewUser {
            email: email.clone(),
            password: Some(form.password.clone()),
            first_name: first,
            last_name: last,
            ..NewUser::default()
        })
        .await;
    if let Err(refusal) = made {
        return page(
            &app,
            &headers,
            true,
            &return_to,
            Some(refusal.message()),
            Some(&email),
            StatusCode::from_u16(refusal.status()).unwrap_or(StatusCode::BAD_REQUEST),
        );
    }
    match workos
        .client
        .authenticate(Grant::Password {
            email: email.clone(),
            password: form.password,
        })
        .await
    {
        Ok(signed) => finish(&app, service, workos, &headers, signed, &return_to, false).await,
        Err(Refusal::EmailVerificationRequired { pending, .. }) => ask_for_code(
            service,
            workos,
            Pending {
                kind: "verify".into(),
                email,
                token: Some(pending),
                return_to,
                exp: now() + FLOW_SECONDS,
            },
        ),
        Err(refusal) => page(
            &app,
            &headers,
            true,
            &return_to,
            Some(refusal.message()),
            Some(&email),
            StatusCode::BAD_REQUEST,
        ),
    }
}

#[derive(Deserialize)]
pub(crate) struct EmailForm {
    csrf: String,
    #[serde(default)]
    return_to: Option<String>,
    email: String,
}

/// `POST /login/code`: email a one-time code.
pub(crate) async fn send_code(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<EmailForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "invalid_email");
    };
    let return_to = oa_auth::return_to(form.return_to.as_deref());
    let (service, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let email = form.email.trim().to_string();
    if !workos.client.settings().offers(Method::MagicCode) || !email_ok(&email) {
        return page(
            &app,
            &headers,
            false,
            &return_to,
            Some(Refusal::InvalidEmail.message()),
            Some(&email),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }
    if let Err(refusal) = workos.client.send_code(&email).await {
        return page(
            &app,
            &headers,
            false,
            &return_to,
            Some(refusal.message()),
            Some(&email),
            StatusCode::from_u16(refusal.status()).unwrap_or(StatusCode::BAD_REQUEST),
        );
    }
    ask_for_code(
        service,
        workos,
        Pending {
            kind: "code".into(),
            email,
            token: None,
            return_to,
            exp: now() + FLOW_SECONDS,
        },
    )
}

/// `GET /login/verify`: type the emailed code.
pub(crate) async fn code_page(State(app): State<App>, headers: HeaderMap) -> Response {
    code_form(&app, &headers, None, StatusCode::OK)
}

fn code_form(app: &App, headers: &HeaderMap, notice: Option<&str>, status: StatusCode) -> Response {
    let Some((service, workos)) = on(app) else {
        return crate::auth::notice(
            headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        );
    };
    let Some(pending) = pending(workos, headers) else {
        return problem_redirect("/", "expired");
    };
    let Ok(csrf) = service.login_csrf(headers, SCOPE, "") else {
        return problem_redirect("/", "unavailable");
    };
    let mut response = render_code(
        headers,
        &pending.email,
        pending.kind == "verify",
        &pending.return_to,
        notice,
        &csrf.token,
        status,
    );
    for cookie in csrf.legacy_cookies.into_iter().chain(csrf.cookie) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// The page that asks for the code WorkOS emailed to `email`: to confirm
/// the address (`verify`) or to log in.
pub fn render_code(
    headers: &HeaderMap,
    email: &str,
    verify: bool,
    return_to: &str,
    notice: Option<&str>,
    csrf: &str,
    status: StatusCode,
) -> Response {
    let field = Field::new("email-code", "Code").required(true);
    let lead = if verify {
        format!("We sent a code to {email} to confirm it's yours. Enter it to finish.")
    } else {
        format!("We sent a code to {email}. Enter it to log in.")
    };
    let card = auth_card(
        "Check your email",
        Some(html! { (lead) }),
        html! {
            form.oa-auth-login-form data-slot="login-form" method="post" action="/login/verify" autocomplete="off" {
                @if let Some(notice) = notice {
                    (Alert::new().color(Color::Danger).variant(Variant::Soft).title(notice))
                }
                input type="hidden" name="csrf" value=(csrf);
                (field.clone().control(
                    Input::new("code").id("email-code").aria(field.aria()).required(true)
                        .inputmode("numeric").autocomplete("one-time-code").maxlength(12).spellcheck(false).autofocus(true),
                ))
                (Button::new("Continue").kind(ButtonType::Submit).size(ControlSize::Lg).block(true))
                p.oa-auth-switch { a href=(format!("/login?return_to={}", encode(return_to))) { "Use another way to log in" } }
            }
        },
    );
    protect(
        UiPage::new("Check your email")
            .path("/login/verify")
            .status(status)
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .content(card)
            .respond(headers),
    )
}

#[derive(Deserialize)]
pub(crate) struct CodeForm {
    csrf: String,
    code: String,
}

/// `POST /login/verify`: finish with the emailed code.
pub(crate) async fn verify_code(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<CodeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "invalid_credentials");
    };
    let (service, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(pending) = pending(workos, &headers) else {
        return problem_redirect("/", "expired");
    };
    let code: String = form.code.chars().filter(|c| !c.is_whitespace()).collect();
    if code.is_empty() || code.len() > 12 || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return code_form(
            &app,
            &headers,
            Some(Refusal::InvalidCredentials.message()),
            StatusCode::BAD_REQUEST,
        );
    }
    let grant = match (pending.kind.as_str(), pending.token.clone()) {
        ("verify", Some(token)) => Grant::EmailVerification {
            code,
            pending: token,
        },
        ("code", _) => Grant::MagicCode {
            email: pending.email.clone(),
            code,
        },
        _ => return problem_redirect(&pending.return_to, "expired"),
    };
    match workos.client.authenticate(grant).await {
        Ok(signed) => {
            finish(
                &app,
                service,
                workos,
                &headers,
                signed,
                &pending.return_to,
                false,
            )
            .await
        }
        Err(Refusal::InvalidCredentials) => code_form(
            &app,
            &headers,
            Some("That code isn't right. Check it and try again."),
            StatusCode::BAD_REQUEST,
        ),
        Err(refusal) => code_form(
            &app,
            &headers,
            Some(refusal.message()),
            StatusCode::from_u16(refusal.status()).unwrap_or(StatusCode::BAD_REQUEST),
        ),
    }
}

/// `POST /login/sso`: single sign-on for the organization that holds the
/// email's domain.
pub(crate) async fn sso(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<EmailForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "no_sso");
    };
    let return_to = oa_auth::return_to(form.return_to.as_deref());
    let (service, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let email = form.email.trim().to_string();
    let domain = email
        .rsplit_once('@')
        .map(|(_, domain)| domain.to_ascii_lowercase());
    if !workos.client.settings().offers(Method::Sso) || !email_ok(&email) {
        return page(
            &app,
            &headers,
            false,
            &return_to,
            Some(notice_for("no_sso")),
            Some(&email),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }
    match workos
        .client
        .organization_for_domain(domain.as_deref().unwrap_or_default())
        .await
    {
        Ok(Some(org)) => redirect_to_provider(
            service,
            workos,
            None,
            Some(&org.id),
            &return_to,
            Some(&email),
        ),
        Ok(None) => page(
            &app,
            &headers,
            false,
            &return_to,
            Some(notice_for("no_sso")),
            Some(&email),
            StatusCode::NOT_FOUND,
        ),
        Err(refusal) => page(
            &app,
            &headers,
            false,
            &return_to,
            Some(refusal.message()),
            Some(&email),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    }
}

/// `GET /login/forgot`: ask for a reset link.
pub(crate) async fn forgot_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(back): Query<Back>,
) -> Response {
    let return_to = oa_auth::return_to(back.return_to.as_deref());
    forgot_form(&app, &headers, &return_to, None)
}

fn forgot_form(app: &App, headers: &HeaderMap, return_to: &str, sent: Option<&str>) -> Response {
    let Some((service, workos)) =
        on(app).filter(|(_, w)| w.client.settings().offers(Method::Password))
    else {
        return crate::auth::notice(
            headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        );
    };
    let _ = workos;
    let Ok(csrf) = service.login_csrf(headers, SCOPE, "") else {
        return problem_redirect("/", "unavailable");
    };
    let mut response = render_forgot(headers, return_to, sent, &csrf.token);
    for cookie in csrf.legacy_cookies.into_iter().chain(csrf.cookie) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// The page that asks where to send a password-reset link, or (with
/// `sent`) says it was sent.
pub fn render_forgot(
    headers: &HeaderMap,
    return_to: &str,
    sent: Option<&str>,
    csrf: &str,
) -> Response {
    let field = Field::new("forgot-email", "Email").required(true);
    let body = match sent {
        Some(email) => html! {
            div.oa-auth-login-form data-slot="login-form" {
                (Alert::new().color(Color::Info).variant(Variant::Soft).title("Check your email")
                    .description(format!("If an account uses {email}, we sent it a link to choose a new password.")))
                (ButtonLink::new("Back to login", &format!("/login?return_to={}", encode(return_to))).variant(ButtonVariant::Outline).size(ControlSize::Lg).block(true))
            }
        },
        None => html! {
            form.oa-auth-login-form data-slot="login-form" method="post" action="/login/forgot" {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="return_to" value=(return_to);
                (field.clone().control(
                    Input::new("email").id("forgot-email").input_type(InputType::Email).aria(field.aria())
                        .required(true).autocomplete("email").spellcheck(false),
                ))
                (Button::new("Email me a link").kind(ButtonType::Submit).size(ControlSize::Lg).block(true))
                p.oa-auth-switch { a href=(format!("/login?return_to={}", encode(return_to))) { "Back to login" } }
            }
        },
    };
    let card = auth_card(
        "Reset your password",
        Some(html! { "We'll email you a link to choose a new one." }),
        body,
    );
    protect(
        UiPage::new("Reset your password")
            .path("/login/forgot")
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .content(card)
            .respond(headers),
    )
}

/// `POST /login/forgot`: WorkOS emails the reset link. The answer is the
/// same whether or not an account uses the email.
pub(crate) async fn forgot(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<EmailForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "invalid_email");
    };
    let return_to = oa_auth::return_to(form.return_to.as_deref());
    let (_, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let email = form.email.trim().to_string();
    if email_ok(&email) {
        workos.client.send_password_reset(&email).await.ok();
    }
    forgot_form(&app, &headers, &return_to, Some(&email))
}

#[derive(Deserialize)]
pub(crate) struct ResetQuery {
    token: Option<String>,
}

/// `GET /login/reset?token=`: choose a new password (the link WorkOS
/// emails points here).
pub(crate) async fn reset_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ResetQuery>,
) -> Response {
    reset_form(
        &app,
        &headers,
        query.token.as_deref().unwrap_or_default(),
        None,
        StatusCode::OK,
    )
}

fn reset_form(
    app: &App,
    headers: &HeaderMap,
    token: &str,
    notice: Option<&str>,
    status: StatusCode,
) -> Response {
    let Some((service, _)) = on(app).filter(|(_, w)| w.client.settings().offers(Method::Password))
    else {
        return crate::auth::notice(
            headers,
            StatusCode::NOT_FOUND,
            "Login isn't available",
            "This server doesn't offer that login.",
        );
    };
    if token.is_empty()
        || token.len() > 512
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return problem_redirect("/", "code_expired");
    }
    let Ok(csrf) = service.login_csrf(headers, SCOPE, "") else {
        return problem_redirect("/", "unavailable");
    };
    let mut response = render_reset(headers, token, notice, &csrf.token, status);
    for cookie in csrf.legacy_cookies.into_iter().chain(csrf.cookie) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// The page that sets a new password with the emailed `token`.
pub fn render_reset(
    headers: &HeaderMap,
    token: &str,
    notice: Option<&str>,
    csrf: &str,
    status: StatusCode,
) -> Response {
    let field = Field::new("reset-password", "New password")
        .required(true)
        .description("At least 10 characters.");
    let card = auth_card(
        "Choose a new password",
        None,
        html! {
            form.oa-auth-login-form data-slot="login-form" method="post" action="/login/reset" {
                @if let Some(notice) = notice {
                    (Alert::new().color(Color::Danger).variant(Variant::Soft).title(notice))
                }
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="token" value=(token);
                (field.clone().control(
                    Input::new("password").id("reset-password").input_type(InputType::Password).aria(field.aria())
                        .required(true).minlength(10).autocomplete("new-password"),
                ))
                (Button::new("Save and log in").kind(ButtonType::Submit).size(ControlSize::Lg).block(true))
            }
        },
    );
    protect(
        UiPage::new("Choose a new password")
            .path("/login/reset")
            .status(status)
            .account(Account::Unknown)
            .scriptless()
            .bare()
            .content(card)
            .respond(headers),
    )
}

#[derive(Deserialize)]
pub(crate) struct ResetForm {
    csrf: String,
    token: String,
    password: String,
}

/// `POST /login/reset`: set the new password, then log in with it.
pub(crate) async fn reset(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ResetForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem_redirect("/", "code_expired");
    };
    let (service, workos) = match checked(&app, &headers, &form.csrf) {
        Ok(found) => found,
        Err(response) => return response,
    };
    if form.password.chars().count() < 10 || form.password.len() > 256 {
        return reset_form(
            &app,
            &headers,
            &form.token,
            Some(Refusal::WeakPassword.message()),
            StatusCode::UNPROCESSABLE_ENTITY,
        );
    }
    let user = match workos
        .client
        .reset_password(&form.token, &form.password)
        .await
    {
        Ok(user) => user,
        Err(Refusal::WeakPassword) => {
            return reset_form(
                &app,
                &headers,
                &form.token,
                Some(Refusal::WeakPassword.message()),
                StatusCode::UNPROCESSABLE_ENTITY,
            );
        }
        Err(refusal) => return problem_redirect("/", refusal.code()),
    };
    match workos
        .client
        .authenticate(Grant::Password {
            email: user.email.clone(),
            password: form.password,
        })
        .await
    {
        Ok(signed) => finish(&app, service, workos, &headers, signed, "/", false).await,
        Err(Refusal::EmailVerificationRequired { pending, .. }) => ask_for_code(
            service,
            workos,
            Pending {
                kind: "verify".into(),
                email: user.email,
                token: Some(pending),
                return_to: "/".into(),
                exp: now() + FLOW_SECONDS,
            },
        ),
        Err(_) => problem_redirect("/", "invalid_credentials"),
    }
}

/// End the WorkOS session this browser holds, when it holds one. Called
/// on sign-out; best effort.
pub(crate) async fn revoke(app: &App, headers: &HeaderMap) {
    let Some((_, workos)) = on(app) else {
        return;
    };
    let Some(sealed) = CloudSession::sealed_value(headers, WORKOS_COOKIE) else {
        return;
    };
    let state = workos
        .client
        .sdk()
        .session(sealed, workos.cookie_password())
        .authenticate()
        .await;
    if !state.session_id.is_empty() {
        workos.client.revoke_session(&state.session_id).await.ok();
    }
}

/// What a refresh of one sealed session came to, kept a minute so
/// requests that arrive together use one refresh (refresh tokens are
/// single use).
type Refreshed = Option<Result<String, ()>>;

fn refreshes() -> &'static tokio::sync::Mutex<HashMap<[u8; 32], (Instant, Refreshed)>> {
    static MAP: std::sync::OnceLock<tokio::sync::Mutex<HashMap<[u8; 32], (Instant, Refreshed)>>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

/// Refresh `sealed`: `Some(Ok(new))`, `Some(Err(()))` when WorkOS refused
/// (the session ended), `None` when WorkOS couldn't be reached.
async fn refresh(workos: &WebWorkos, sealed: &str) -> Refreshed {
    use sha2::Digest;
    let key: [u8; 32] = sha2::Sha256::digest(sealed.as_bytes()).into();
    let mut map = refreshes().lock().await;
    map.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(60));
    if let Some((_, done)) = map.get(&key) {
        return done.clone();
    }
    let answer = match workos
        .client
        .sdk()
        .session(sealed, workos.cookie_password())
        .refresh(oa_auth::workos::sdk::SessionRefreshOptions::default())
        .await
    {
        Ok(result) if result.authenticated && !result.sealed_session.is_empty() => {
            Some(Ok(result.sealed_session))
        }
        Ok(_) => Some(Err(())),
        Err(_) => None,
    };
    map.insert(key, (Instant::now(), answer.clone()));
    answer
}

/// Replace cookie `name`'s value in a request's `Cookie` header (`None`
/// drops it), so the handlers see the refreshed (or ended) session.
fn rewrite_cookie(headers: &mut HeaderMap, name: &str, value: Option<&str>) {
    let Some(cookies) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) else {
        return;
    };
    let mut kept: Vec<String> = cookies
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter_map(|part| match part.split_once('=') {
            Some((key, _)) if key == name => value.map(|value| format!("{name}={value}")),
            _ => Some(part.to_string()),
        })
        .collect();
    if let Some(value) = value
        && !kept
            .iter()
            .any(|pair| pair.starts_with(&format!("{name}=")))
    {
        kept.push(format!("{name}={value}"));
    }
    if let Ok(joined) = HeaderValue::from_str(&kept.join("; ")) {
        headers.insert(header::COOKIE, joined);
    }
}

/// Keep a browser's sealed WorkOS session current: refresh it when its
/// access token has expired. In `workos` mode a session WorkOS ended ends
/// ours too; in `dual` mode only the sealed cookie is dropped.
pub(crate) async fn layer(State(app): State<App>, mut request: Request, next: Next) -> Response {
    let Some((service, workos)) = on(&app) else {
        return next.run(request).await;
    };
    if workos.mode == Mode::Workos {
        rewrite_cookie(request.headers_mut(), "oa_cloud_session", None);
    }
    let Some(sealed) = CloudSession::sealed_value(request.headers(), WORKOS_COOKIE) else {
        return next.run(request).await;
    };
    let state = workos
        .client
        .sdk()
        .session(sealed.clone(), workos.cookie_password())
        .authenticate()
        .await;
    if state.authenticated {
        if workos.mode == Mode::Workos
            && let Ok(token) = service.credential(request.headers())
        {
            rewrite_cookie(request.headers_mut(), "oa_cloud_session", Some(&token));
        }
        return next.run(request).await;
    }
    let mut set = Vec::new();
    let mut renewed_session = false;
    if state.needs_refresh {
        match refresh(workos, &sealed).await {
            Some(Ok(renewed)) => {
                if let Ok(cookie) = service.sealed_cookie(WORKOS_COOKIE, &renewed, 8 * 3600) {
                    let value = cookie
                        .to_str()
                        .ok()
                        .and_then(|text| text.split(';').next())
                        .and_then(|pair| pair.split_once('='))
                        .map(|(_, value)| value.to_string());
                    if let Some(value) = value {
                        rewrite_cookie(request.headers_mut(), WORKOS_COOKIE, Some(&value));
                    }
                    set.push(cookie);
                    renewed_session = true;
                }
            }
            Some(Err(())) => {
                set.push(service.clear_cookie(WORKOS_COOKIE));
                rewrite_cookie(request.headers_mut(), WORKOS_COOKIE, None);
                if workos.mode == Mode::Workos {
                    if let Some(token) = CloudSession::session_cookie(request.headers()) {
                        service.end_session(&token).await.ok();
                    }
                    rewrite_cookie(request.headers_mut(), "oa_cloud_session", None);
                    set.extend(service.clear_cookies());
                }
            }
            // WorkOS unreachable: keep going on our session.
            None => {}
        }
    } else {
        // Not a session we sealed: forget it.
        set.push(service.clear_cookie(WORKOS_COOKIE));
    }
    if workos.mode == Mode::Workos && renewed_session {
        if let Ok(token) = service.credential(request.headers()) {
            rewrite_cookie(request.headers_mut(), "oa_cloud_session", Some(&token));
        }
    }
    let mut response = next.run(request).await;
    for cookie in set {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

/// Forward the operator's own bearer; the gateway checks its admin token.
/// Browser cookies are never substituted for this credential.
pub(crate) async fn admin_sessions(
    State(app): State<App>,
    Path(operation): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !matches!(operation.as_str(), "retire-sessions" | "restore-sessions") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some((service, _)) = on(&app) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(bearer) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("Bearer ") && v.len() <= 8192)
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if body.len() > 4096 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    match service
        .nostr_call(
            reqwest::Method::POST,
            &format!("/v1/admin/workos/{operation}"),
            Some(bearer),
            Some(body.to_vec()),
        )
        .await
    {
        Ok(answer) => protect(
            (
                StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY),
                axum::Json(answer.body),
            )
                .into_response(),
        ),
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}
