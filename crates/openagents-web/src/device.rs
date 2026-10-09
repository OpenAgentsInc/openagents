//! Signing an app in on another computer (docs/auth, "Apps and the
//! command line"): RFC 8628's device authorization grant.
//!
//! - `POST /device/code` — the app asks to sign in. Answers `device_code`,
//!   `user_code`, `verification_uri` (this site's `/device`),
//!   `verification_uri_complete`, `expires_in`, and `interval`.
//! - `GET /device` — the signed-in person types the code (or arrives with
//!   it in the link) and sees "Sign in to Coder on <computer>?" with
//!   Approve and Deny. Signing in first comes back here.
//! - `POST /device/token` — the app's poll. `200 {access_token,
//!   token_type, expires_in, account}` once approved; RFC errors
//!   (`authorization_pending`, `slow_down`, `expired_token`,
//!   `access_denied`, `invalid_grant`) until then.
//! - `POST /device/sign-out` — the app signs its own token out.
//! - Settings' Computers section lists signed-in apps with Remove
//!   (`POST /settings/computers/remove`).
//!
//! The app's token is an ordinary session on the account; nothing here
//! stores it.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::{Field, Input};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::account::Account;
use crate::cloud::session::{CloudSession, SessionError, Viewer};
use crate::cloud::{protect, refused, service};
use crate::ui_page::{UiPage, action_link};

pub(crate) const PAGE: &str = "/device";
const CODE: &str = "/device/code";
const TOKEN: &str = "/device/token";
const SIGN_OUT: &str = "/device/sign-out";
const REMOVE: &str = "/settings/computers/remove";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(page).post(decide))
        .route(CODE, post(start))
        .route(TOKEN, post(token))
        .route(SIGN_OUT, post(sign_out))
        .route(REMOVE, post(remove))
}

/// A JSON or form body as a flat map of strings.
fn fields(headers: &HeaderMap, body: &Bytes) -> Option<serde_json::Map<String, Value>> {
    if body.len() > 4096 {
        return None;
    }
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if json {
        return serde_json::from_slice::<Value>(body)
            .ok()?
            .as_object()
            .cloned();
    }
    Some(
        url::form_urlencoded::parse(body)
            .map(|(k, v)| (k.into_owned(), Value::String(v.into_owned())))
            .collect(),
    )
}

fn oauth_error(status: StatusCode, code: &str, description: &str) -> Response {
    protect(
        (
            status,
            axum::Json(json!({"error": code, "error_description": description})),
        )
            .into_response(),
    )
}

fn unavailable() -> Response {
    oauth_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "temporarily_unavailable",
        "Sign-in isn't available right now. Try again in a minute.",
    )
}

/// `POST /device/code` — `{app, computer}` (JSON or form).
async fn start(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let Some(fields) = fields(&headers, &body) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send app and computer.",
        );
    };
    let text = |name: &str| fields.get(name).and_then(Value::as_str).unwrap_or_default();
    let app_name = match text("app") {
        "" => "Coder",
        name => name,
    };
    let pair = Some(text("pair")).filter(|pair| !pair.is_empty());
    match service.device_start(app_name, text("computer"), pair).await {
        Ok(answer) if answer.status == 200 => {
            let mut body = answer.body;
            if let Some(object) = body.as_object_mut() {
                object.remove("v");
            }
            protect(axum::Json(body).into_response())
        }
        Ok(answer) => oauth_error(
            StatusCode::BAD_REQUEST,
            answer.code().unwrap_or("invalid_request"),
            answer.body["error"]["message"].as_str().unwrap_or_default(),
        ),
        Err(_) => unavailable(),
    }
}

/// `POST /device/token` — `{device_code}` (JSON or form). `grant_type`,
/// when sent, must be RFC 8628's.
async fn token(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let Some(fields) = fields(&headers, &body) else {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send device_code.",
        );
    };
    if fields
        .get("grant_type")
        .and_then(Value::as_str)
        .is_some_and(|g| g != "urn:ietf:params:oauth:grant-type:device_code")
    {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "Use the device_code grant.",
        );
    }
    let code = fields
        .get("device_code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match service.device_poll(code).await {
        Ok(answer) if answer.status == 200 => {
            let now = crate::cloud::session::now();
            let expires = answer.body["session"]["expires_at"]
                .as_u64()
                .unwrap_or(now)
                .saturating_sub(now);
            protect(
                axum::Json(json!({
                    "access_token": answer.body["token"],
                    "token_type": "Bearer",
                    "expires_in": expires,
                    "account": answer.body["account"],
                }))
                .into_response(),
            )
        }
        Ok(answer) => {
            let mut response = json!({
                "error": answer.code().unwrap_or("invalid_grant"),
                "error_description": answer.body["error"]["message"],
            });
            if let Some(interval) = answer.body["interval"].as_u64() {
                response["interval"] = json!(interval);
            }
            protect((StatusCode::BAD_REQUEST, axum::Json(response)).into_response())
        }
        Err(SessionError::InvalidRequest) => oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Send the device_code from /device/code.",
        ),
        Err(_) => unavailable(),
    }
}

/// `POST /device/sign-out` with `Authorization: Bearer sess_…`.
async fn sign_out(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    match service.app_sign_out(token).await {
        Ok(()) => protect(axum::Json(json!({"signed_out": true})).into_response()),
        Err(SessionError::InvalidRequest) => oauth_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Send the app's own token.",
        ),
        Err(_) => unavailable(),
    }
}

#[derive(Deserialize)]
struct CodeQuery {
    code: Option<String>,
}

/// The signed-in viewer, or a trip through sign-in that comes back to
/// `back`.
async fn viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
    back: &str,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let service = service(app)?;
    match service.authenticate(headers).await {
        Ok(viewer) => Ok((service, viewer)),
        Err(SessionError::Unauthenticated) => Err(protect(
            Redirect::to(&crate::auth::login_href(back, false)).into_response(),
        )),
        Err(error) => Err(refused(error)),
    }
}

fn respond(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    status: StatusCode,
    title: &str,
    body: Markup,
) -> Response {
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    protect(
        UiPage::new(title)
            .path(PAGE)
            .status(status)
            .account(account)
            .content(PageColumn::new(body))
            .respond(headers),
    )
}

/// Plain words for a code that can't be approved.
fn problem_text(code: &str) -> &'static str {
    match code {
        "expired_token" => "That code expired. Start sign-in again on your computer.",
        "invalid_grant" => "That code isn't right, or it was already used. Check it and try again.",
        "browser_session_required" => "Approve sign-ins from the website while signed in.",
        _ => "That code didn't work. Check it and try again.",
    }
}

/// The form to type a code, with an optional problem above it.
fn code_form(typed: &str, problem: Option<&str>) -> Markup {
    let field = Field::new("device-code", "Code")
        .required(true)
        .error_opt(problem)
        .description("The code your computer shows, like BCDF-GHJK.");
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Sign in on another computer" }
            p { "Enter the code " code { "coder login" } " shows in your terminal." }
        }))
        form method="get" action=(PAGE) autocomplete="off" {
            (field.clone().control(
                Input::new("code")
                    .aria(field.aria())
                    .value(typed)
                    .required(true)
                    .autocomplete("off")
                    .spellcheck(false)
                    .autofocus(true)
                    .invalid(problem.is_some()),
            ))
            p { (Button::new("Continue").kind(ButtonType::Submit)) }
        }
    }
}

/// "Sign in to Coder on <computer>?" with Approve and Deny.
fn approve_form(code: &str, app: &str, computer: &str, csrf: &str) -> Markup {
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Sign in to " (app) " on " (computer) "?" }
            p { "Code " strong { (code) } ". Approve only if you started this sign-in and the code matches the one on your computer." }
            p { (app) " on " (computer) " will be able to use your OpenAgents account. You can remove it any time in Settings." }
        }))
        form method="post" action=(PAGE) {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="code" value=(code);
            div.oa-page-actions {
                (Button::new("Approve").kind(ButtonType::Submit).name("decision").value("approve"))
                (Button::new("Deny")
                    .kind(ButtonType::Submit)
                    .variant(ButtonVariant::Soft)
                    .color(Color::Secondary)
                    .name("decision")
                    .value("deny"))
            }
        }
    }
}

/// `GET /device[?code=]`.
async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<CodeQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let typed = query
        .ok()
        .and_then(|q| q.0.code)
        .unwrap_or_default()
        .chars()
        .take(32)
        .collect::<String>();
    let back = match oa_auth::device::normalize_user_code(&typed) {
        Some(code) => format!("{PAGE}?code={}-{}", &code[..4], &code[4..]),
        None => PAGE.to_string(),
    };
    let (service, viewer) = match viewer(&app, &headers, &back).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if typed.trim().is_empty() {
        let body = code_form("", None);
        return respond(
            &headers,
            service,
            &viewer,
            StatusCode::OK,
            "Sign in on another computer",
            body,
        );
    }
    let Some(code) = oa_auth::device::normalize_user_code(&typed) else {
        let body = code_form(&typed, Some(problem_text("invalid_grant")));
        return respond(
            &headers,
            service,
            &viewer,
            StatusCode::BAD_REQUEST,
            "Sign in on another computer",
            body,
        );
    };
    let shown = format!("{}-{}", &code[..4], &code[4..]);
    match service.device_lookup(&headers, &code).await {
        Ok(Ok(request)) => {
            let csrf = match service.csrf(&headers, &viewer, "device", &code) {
                Ok(csrf) => csrf,
                Err(error) => return refused(error),
            };
            let title = format!("Sign in to {} on {}?", request.app, request.computer);
            let body = approve_form(&shown, &request.app, &request.computer, &csrf);
            respond(&headers, service, &viewer, StatusCode::OK, &title, body)
        }
        Ok(Err(problem)) => {
            let body = code_form(&shown, Some(problem_text(&problem)));
            respond(
                &headers,
                service,
                &viewer,
                StatusCode::BAD_REQUEST,
                "Sign in on another computer",
                body,
            )
        }
        Err(error) => refused(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideForm {
    csrf: String,
    code: String,
    decision: String,
}

/// `POST /device` — Approve or Deny.
async fn decide(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<DecideForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(code) = oa_auth::device::normalize_user_code(&form.code) else {
        return refused(SessionError::InvalidRequest);
    };
    if let Err(error) = service.verify_csrf(&headers, Some(&viewer), "device", &code, &form.csrf) {
        return refused(error);
    }
    let approve = match form.decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return refused(SessionError::InvalidRequest),
    };
    let decided = match service.device_decide(&headers, &code, approve).await {
        Ok(decided) => decided,
        Err(error) => return refused(error),
    };
    let body = match decided {
        Ok(request) if approve => html! {
            (MarkdownRoot::new(html! {
                h1 { (request.app) " on " (request.computer) " is signed in" }
                p {
                    "Go back to your terminal. To show its chats here, type "
                    code { "/sync on" } " in Coder. You can close this tab."
                }
            }))
            p { (action_link("Settings", crate::settings::PAGE)) }
        },
        Ok(request) => html! {
            (MarkdownRoot::new(html! {
                h1 { "Sign-in denied" }
                p { (request.app) " on " (request.computer) " was not signed in." }
            }))
        },
        Err(problem) => code_form(&form.code, Some(problem_text(&problem))),
    };
    let title = if approve {
        "Signed in"
    } else {
        "Sign-in denied"
    };
    respond(&headers, service, &viewer, StatusCode::OK, title, body)
}

/// Settings' Computers section: the signed-in apps, each with Remove and
/// its chats' choice (sync all or keep on the computer, #11089), and the
/// way to connect one. Empty when the account service can't be reached,
/// so Settings still opens.
pub(crate) async fn computers_section(
    app: &App,
    service: &CloudSession,
    headers: &HeaderMap,
    viewer: &Viewer,
) -> Markup {
    let Ok(sessions) = service.app_sessions(headers).await else {
        return html! {};
    };
    let owner = crate::chat_store::account_owner(&viewer.account_id);
    let choices = app
        .config
        .chat_store
        .computers(&owner)
        .await
        .map(|computers| computers.sync)
        .unwrap_or_default();
    let rows: Vec<_> = sessions
        .iter()
        .filter_map(|session| {
            let csrf = service
                .csrf(headers, viewer, "computer-remove", &session.id)
                .ok()?;
            let computer = crate::coder_sync::line(&session.computer, 64);
            let sync = service
                .csrf(headers, viewer, "terminal-sync", &computer)
                .ok()?;
            let choice = choices.get(&computer).copied();
            Some(Row {
                session,
                csrf,
                sync,
                choice,
            })
        })
        .collect();
    computers_markup(&rows)
}

/// One signed-in computer in Settings.
struct Row<'a> {
    session: &'a crate::cloud::session::device::AppSession,
    /// The Remove ticket.
    csrf: String,
    /// The choice forms' ticket.
    sync: String,
    choice: Option<crate::chat_store::SyncChoice>,
}

fn computers_markup(rows: &[Row<'_>]) -> Markup {
    html! {
        div class="oa-settings" {
            section class="oa-settings-group" aria-labelledby="settings-computers" {
                h2 #settings-computers { "Computers" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" {
                            @if rows.is_empty() { "No computers are signed in" } @else { "Connect your terminal" }
                        }
                        span class="oa-settings-hint" { "Install Coder, sign in, and choose where its chats live." }
                    }
                    div class="oa-settings-control" {
                        (action_link("Connect your terminal", crate::terminal_connect::PAGE))
                    }
                }
                @for row in rows {
                    form class="oa-settings-row" method="post" action=(REMOVE) {
                        input type="hidden" name="csrf" value=(row.csrf);
                        input type="hidden" name="session" value=(row.session.id);
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { (row.session.app) " on " (row.session.computer) }
                            span class="oa-settings-hint" {
                                "Signed in " (date(row.session.created_at)) " · "
                                @match row.choice {
                                    Some(crate::chat_store::SyncChoice::All) => "Syncs all its chats",
                                    Some(crate::chat_store::SyncChoice::Local) => "Keeps chats on this computer",
                                    None => "Not asked yet where its chats live",
                                }
                            }
                        }
                        div class="oa-settings-control" {
                            (Button::new("Remove")
                                .kind(ButtonType::Submit)
                                .variant(ButtonVariant::Soft)
                                .color(Color::Secondary))
                        }
                    }
                    (crate::terminal_connect::choice_forms(
                        &crate::coder_sync::line(&row.session.computer, 64),
                        &row.sync,
                        row.choice,
                        true,
                    ))
                }
            }
        }
    }
}

/// A Unix time as a UTC calendar date, `2026-10-09`.
fn date(seconds: u64) -> String {
    // Civil-from-days (Howard Hinnant), for dates after 1970.
    let days = (seconds / 86_400) as i64 + 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveForm {
    csrf: String,
    session: String,
}

/// `POST /settings/computers/remove`.
async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RemoveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, crate::settings::PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        "computer-remove",
        &form.session,
        &form.csrf,
    ) {
        return refused(error);
    }
    match service.revoke_app_session(&headers, &form.session).await {
        Ok(()) => protect(Redirect::to(crate::settings::PAGE).into_response()),
        Err(error) => refused(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_approval_page_names_the_app_the_computer_and_both_choices() {
        let html = approve_form("BCDF-GHJK", "Coder", "chris-mbp", "ticket").into_string();
        assert!(html.contains("Sign in to Coder on chris-mbp?"));
        assert!(html.contains("BCDF-GHJK"));
        assert!(html.contains("name=\"decision\" value=\"approve\""));
        assert!(html.contains("name=\"decision\" value=\"deny\""));
        assert!(html.contains("name=\"csrf\" value=\"ticket\""));
        crate::copy_guard::assert_plain("/device", &html);
        crate::copy_guard::assert_plain(
            "/device",
            &code_form("", Some(problem_text("expired_token"))).into_string(),
        );
    }

    #[test]
    fn computer_names_are_escaped() {
        let html = approve_form("BCDF-GHJK", "Coder", "<script>", "t").into_string();
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn settings_lists_computers_with_remove_or_says_how_to_add_one() {
        let empty = computers_markup(&[]).into_string();
        assert!(empty.contains("/settings/terminal"));
        assert!(empty.contains("No computers are signed in"));
        let session = crate::cloud::session::device::AppSession {
            id: "a".repeat(64),
            app: "Coder".into(),
            computer: "chris-mbp".into(),
            created_at: 1_791_504_000,
            expires_at: 0,
        };
        let html = computers_markup(&[Row {
            session: &session,
            csrf: "t".into(),
            sync: "s".into(),
            choice: Some(crate::chat_store::SyncChoice::All),
        }])
        .into_string();
        assert!(html.contains("Coder on chris-mbp"));
        assert!(html.contains("Signed in 2026-10-09"));
        assert!(html.contains("Syncs all its chats"));
        assert!(html.contains("Sync all my chats (chosen)"));
        assert!(html.contains(r#"name="back" value="settings""#));
        assert!(html.contains(">Remove<"));
        crate::copy_guard::assert_plain("/settings", &html);
        crate::copy_guard::assert_plain("/settings", &empty);
        assert!(html.contains(&format!("value=\"{}\"", "a".repeat(64))));
    }

    #[test]
    fn dates_are_utc_calendar_days() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
    }

    #[test]
    fn bodies_parse_as_json_or_form() {
        let mut headers = HeaderMap::new();
        let form = fields(
            &headers,
            &Bytes::from_static(b"device_code=dvc_1&grant_type=x"),
        )
        .unwrap();
        assert_eq!(form["device_code"], "dvc_1");
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        let json = fields(&headers, &Bytes::from_static(br#"{"computer":"box"}"#)).unwrap();
        assert_eq!(json["computer"], "box");
        assert!(fields(&headers, &Bytes::from(vec![b' '; 5000])).is_none());
    }
}
