//! Sign-in keys for agents (#11305): a key the person makes so an agent or
//! script acts as their account without the device code (`coder login`).
//!
//! A key (`oa_sk_…`) is a session on the account in the account service
//! (`oa_auth::sign_in_keys`, `tenancy::sessions::keys`): every route that
//! takes an app's `Bearer sess_…` takes it through the same check
//! ([`crate::cloud::session::session_token`]). It is stored as a hash,
//! shown once, and revocable here at any time.
//!
//! - `/settings/sign-in-keys` — Settings: name a key, pick an expiry
//!   (never, 30 or 90 days), see it once with a copy button and the line
//!   to use it; the list shows name, made, last used, expiry, and Revoke.
//! - `GET /v1/keys`, `POST /v1/keys` `{name, expires_in_days?}`, and
//!   `DELETE /v1/keys/{id}` — the same, for `openagents keys` and any
//!   agent, under any signed-in credential (`Authorization: Bearer
//!   sess_…` or `oa_sk_…`). `?kind=api` (or `"kind": "api"`) does the same
//!   for the account's API keys (`oak_…`, Settings > API keys). A listing
//!   never carries a secret; a new key's secret is in its create answer
//!   only.
//!
//! Settings and the API call the same account-service functions on
//! [`CloudSession`]; the only difference is where the credential comes
//! from (the browser's cookie with a form ticket, or the bearer).

use axum::Router;
use axum::extract::rejection::{FormRejection, JsonRejection};
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color, CopyButton};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::{Field, Input, InputType, Select};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::device::{MadeKey, SignInKey};
use crate::cloud::session::{CloudSession, SessionError, Viewer, session_token};
use crate::coder_sync::refused;
use crate::settings::{PAGE as SETTINGS, page, viewer};
use crate::ui_page::action_link;

pub(crate) const PAGE: &str = "/settings/sign-in-keys";
pub(crate) const REVOKE: &str = "/settings/sign-in-keys/revoke";
pub(crate) const API: &str = "/v1/keys";
const API_ONE: &str = "/v1/keys/{id}";
const MAKE_SCOPE: &str = "sign-in-key-make";
const REVOKE_SCOPE: &str = "sign-in-key-revoke";

/// The expiries the form offers: the field value, the words, and days.
const EXPIRIES: [(&str, &str, Option<u64>); 3] = [
    ("never", "Never", None),
    ("30", "In 30 days", Some(30)),
    ("90", "In 90 days", Some(90)),
];

/// What an API key made from the command line may do: what Settings
/// ticks by default (call models, list them, read usage).
const API_KEY_SCOPES: [&str; 3] = ["responses", "models:read", "usage:read"];

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(settings_page).post(settings_make))
        .route(REVOKE, post(settings_revoke))
        .route(API, get(api_list).post(api_create))
        .route(API_ONE, delete(api_revoke))
}

/// Whether `path` is one of these routes, so the site answers it itself.
pub(crate) fn owns(path: &str) -> bool {
    path == API
        || path
            .strip_prefix(API)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The environment variable an agent sets to the key
/// (`openagents_login::TOKEN_ENV`).
pub(crate) const TOKEN_ENV: &str = "OPENAGENTS_TOKEN";

/// The one line that runs Coder as the account with `token`, with the
/// site named when it isn't openagents.com.
pub(crate) fn use_line(origin: &str, token: &str) -> String {
    if origin == "https://openagents.com" {
        format!("{TOKEN_ENV}={token} coder")
    } else {
        format!("OPENAGENTS_ORIGIN={origin} {TOKEN_ENV}={token} coder")
    }
}

// ------------------------------------------------------------- Settings

fn target(viewer: &Viewer, request: &str) -> String {
    format!("{}:{request}", viewer.account_id)
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Login keys",
        text,
        (PAGE, "Login keys"),
    ))
}

pub(crate) const UNREACHABLE: &str =
    "Your login keys can't be reached right now. Try again in a minute.";

fn unreachable_service() -> Response {
    problem(StatusCode::BAD_GATEWAY, UNREACHABLE)
}

async fn settings_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Ok(token) = service.browser_token(&headers) else {
        return crate::cloud::refused(SessionError::Unauthenticated);
    };
    let keys = match service.sign_in_keys(&token).await {
        Ok(keys) => keys,
        Err(_) => return unreachable_service(),
    };
    let mut tickets = Vec::new();
    for scope in [MAKE_SCOPE, REVOKE_SCOPE] {
        let request = fresh_request();
        match service.csrf(&headers, &viewer, scope, &target(&viewer, &request)) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let body = keys_content(
        &keys,
        (&tickets[0].0, &tickets[0].1),
        (&tickets[1].0, &tickets[1].1),
    );
    page(&headers, service, &viewer, "Login keys", PAGE, body)
}

/// A key's dates in words.
fn key_hint(key: &SignInKey) -> String {
    let date = crate::device::date;
    let used = key.last_used_at.map_or_else(
        || "Never used".to_owned(),
        |at| format!("Last used {}", date(at)),
    );
    let expires = key.expires_at.map_or_else(
        || "Never expires".to_owned(),
        |at| format!("Expires {}", date(at)),
    );
    format!("Made {} · {used} · {expires}", date(key.created_at))
}

/// The page: what a key is, the keys, the form that makes one.
pub(crate) fn keys_content(keys: &[SignInKey], make: (&str, &str), revoke: (&str, &str)) -> Markup {
    let name = Field::new("sign-in-key-name", "Name")
        .description("So you can tell your keys apart, such as \"build agent\".");
    let expires = Field::new("sign-in-key-expires", "Expires");
    let mut choices = Select::new("expires").aria(expires.aria());
    for (value, words, _) in EXPIRIES {
        choices = choices.option(value, words);
    }
    html! {
        p { (action_link("Settings", SETTINGS)) }
        (MarkdownRoot::new(html! {
            h1 { "Login keys for agents" }
            p {
                "A login key lets an agent or a script act as you, like "
                code { "coder login" } ", without a code to approve. Anyone with the key can do what you can, so keep it secret and revoke it when you're done."
            }
        }))
        @if keys.is_empty() {
            p { "You have no login keys." }
        } @else {
            section class="oa-settings-group" aria-labelledby="sign-in-keys-list" {
                h2 #sign-in-keys-list { "Your login keys" }
                @for key in keys {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { (key.name) }
                            span class="oa-settings-hint" { (key_hint(key)) }
                        }
                        div class="oa-settings-control" {
                            form method="post" action=(REVOKE) {
                                input type="hidden" name="csrf" value=(revoke.0);
                                input type="hidden" name="request" value=(revoke.1);
                                input type="hidden" name="key" value=(key.id);
                                (Button::new("Revoke")
                                    .kind(ButtonType::Submit)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
            }
        }
        section class="oa-settings-group" aria-labelledby="sign-in-keys-make" {
            h2 #sign-in-keys-make { "Make a login key" }
            form method="post" action=(PAGE) autocomplete="off" {
                input type="hidden" name="csrf" value=(make.0);
                input type="hidden" name="request" value=(make.1);
                (name.clone().control(
                    Input::new("name")
                        .input_type(InputType::Text)
                        .aria(name.aria()),
                ))
                (expires.clone().control(choices))
                p { (Button::new("Make key").kind(ButtonType::Submit)) }
            }
        }
    }
}

/// A line with its copy button.
fn copy_line(text: &str) -> Markup {
    html! {
        div class="oa-terminal-command" {
            pre { code { (text) } }
            (CopyButton::new(text)
                .label("Copy")
                .copied_label("Copied")
                .variant(ButtonVariant::Soft)
                .color(Color::Secondary))
        }
    }
}

/// The new key, shown once, with the line to use it.
pub(crate) fn made_content(origin: &str, made: &MadeKey) -> Markup {
    let save = if origin == "https://openagents.com" {
        "coder login --key".to_owned()
    } else {
        format!("OPENAGENTS_ORIGIN={origin} coder login --key")
    };
    html! {
        p { (action_link("Login keys", PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Your new login key" }
            p { "Copy " strong { (made.key.name) } " now. It won't be shown again." }
        }))
        (copy_line(&made.token))
        (MarkdownRoot::new(html! {
            p { "Give it to an agent with this line:" }
        }))
        (copy_line(&use_line(origin, &made.token)))
        (MarkdownRoot::new(html! {
            p { "Or save it on a computer, pasting the key when asked:" }
        }))
        (copy_line(&save))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MakeForm {
    csrf: String,
    request: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    expires: String,
}

async fn settings_make(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<MakeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        MAKE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let Some((_, _, days)) = EXPIRIES.iter().find(|(value, _, _)| *value == form.expires) else {
        return problem(StatusCode::BAD_REQUEST, "Pick when the key expires.");
    };
    let name = form.name.trim();
    let name = if name.is_empty() { "Agent" } else { name };
    let Ok(token) = service.browser_token(&headers) else {
        return crate::cloud::refused(SessionError::Unauthenticated);
    };
    let made = match service.create_sign_in_key(&token, name, *days).await {
        Ok(Ok(made)) => made,
        Ok(Err(words)) => return problem(StatusCode::BAD_REQUEST, &words),
        Err(_) => return unreachable_service(),
    };
    let body = made_content(service.origin(), &made);
    let mut response = page(&headers, service, &viewer, "Your new login key", PAGE, body);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeForm {
    csrf: String,
    request: String,
    key: String,
}

async fn settings_revoke(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RevokeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        REVOKE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let Ok(token) = service.browser_token(&headers) else {
        return crate::cloud::refused(SessionError::Unauthenticated);
    };
    match service.revoke_sign_in_key(&token, &form.key).await {
        Ok(_) => {
            crate::coder_sync::forget_checked(&form.key);
            protect(Redirect::to(PAGE).into_response())
        }
        Err(_) => unreachable_service(),
    }
}

// ------------------------------------------------------------------ API

/// Which keys a call is about: sign-in keys (the default) or API keys.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[default]
    Signin,
    Api,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Self::Signin => "signin",
            Self::Api => "api",
        }
    }
}

#[derive(Deserialize, Default)]
struct KindQuery {
    #[serde(default)]
    kind: Option<String>,
}

fn kind_of(word: Option<&str>) -> Result<Kind, Response> {
    match word.unwrap_or("signin") {
        "signin" | "sign-in" => Ok(Kind::Signin),
        "api" => Ok(Kind::Api),
        _ => Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid_kind",
            "kind is signin or api.",
        )),
    }
}

fn signed_out() -> Response {
    refused(
        StatusCode::UNAUTHORIZED,
        "signed_out",
        "Log in first: coder login, or a login key in OPENAGENTS_TOKEN.",
    )
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Keys can't be reached right now. Try again in a minute.",
    )
}

/// The service and the request's own credential: a sign-in or a sign-in
/// key in `Authorization: Bearer`.
fn credential<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, String), Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        ));
    };
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| session_token(token))
        .ok_or_else(signed_out)?;
    Ok((service, token.to_owned()))
}

fn session_failure(error: SessionError) -> Response {
    match error {
        SessionError::Unauthenticated | SessionError::InvalidRequest => signed_out(),
        _ => unavailable(),
    }
}

/// The account's own workspace, where its API keys live.
fn api_workspace(viewer: &Viewer) -> Result<String, Response> {
    crate::api_keys::own_workspace(viewer).ok_or_else(|| {
        refused(
            StatusCode::CONFLICT,
            "no_workspace",
            "Your account has no workspace of its own yet, so there's nowhere to keep a key.",
        )
    })
}

/// `GET /v1/keys[?kind=api]` — `{kind, account: {id, label}, keys}`.
async fn api_list(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<KindQuery>,
) -> Response {
    let kind = match kind_of(query.kind.as_deref()) {
        Ok(kind) => kind,
        Err(response) => return response,
    };
    let (service, token) = match credential(&app, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let viewer = match service.app_viewer(&token).await {
        Ok(viewer) => viewer,
        Err(error) => return session_failure(error),
    };
    let keys = match kind {
        Kind::Signin => match service.sign_in_keys(&token).await {
            Ok(keys) => json!(keys),
            Err(error) => return session_failure(error),
        },
        Kind::Api => {
            let workspace = match api_workspace(&viewer) {
                Ok(workspace) => workspace,
                Err(response) => return response,
            };
            match viewer.client().account().keys(&workspace).await {
                Ok(listed) => Value::Array(
                    listed
                        .into_iter()
                        .filter(|key| key.status.as_deref() != Some("revoked"))
                        .map(|key| {
                            json!({
                                "id": key.id,
                                "name": key.name,
                                "created": key.created,
                                "status": key.status.unwrap_or_else(|| "active".into()),
                            })
                        })
                        .collect(),
                ),
                Err(_) => return unavailable(),
            }
        }
    };
    crate::coder_sync::answer(
        StatusCode::OK,
        json!({
            "kind": kind.word(),
            "account": {"id": viewer.account_id, "label": viewer.account_label},
            "keys": keys,
        }),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBody {
    name: String,
    #[serde(default)]
    expires_in_days: Option<u64>,
    #[serde(default)]
    kind: Option<String>,
}

/// `POST /v1/keys` — `{name, expires_in_days?, kind?}`: `{kind, key,
/// token}`. The token is in this answer only.
async fn api_create(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<CreateBody>, JsonRejection>,
) -> Response {
    let (service, token) = match credential(&app, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(axum::Json(body)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"name\": \"...\", \"expires_in_days\": 30} as JSON.",
        );
    };
    let kind = match kind_of(body.kind.as_deref()) {
        Ok(kind) => kind,
        Err(response) => return response,
    };
    let created = match kind {
        Kind::Signin => match service
            .create_sign_in_key(&token, &body.name, body.expires_in_days)
            .await
        {
            Ok(Ok(made)) => json!({
                "kind": "signin",
                "key": made.key,
                "token": made.token,
            }),
            Ok(Err(words)) => return refused(StatusCode::BAD_REQUEST, "invalid_key", &words),
            Err(error) => return session_failure(error),
        },
        Kind::Api => {
            if body.expires_in_days.is_some() {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "invalid_expiry",
                    "API keys don't expire; revoke one when you're done with it.",
                );
            }
            let name: String = body.name.trim().chars().take(80).collect();
            if name.is_empty() {
                return refused(StatusCode::BAD_REQUEST, "invalid_name", "Name the key.");
            }
            let viewer = match service.app_viewer(&token).await {
                Ok(viewer) => viewer,
                Err(error) => return session_failure(error),
            };
            let workspace = match api_workspace(&viewer) {
                Ok(workspace) => workspace,
                Err(response) => return response,
            };
            match viewer
                .client()
                .account()
                .issue_key_scoped(&workspace, &name, &API_KEY_SCOPES)
                .await
            {
                Ok(grant) => json!({
                    "kind": "api",
                    "key": {"id": grant.key.id, "name": name, "status": "active"},
                    "token": grant.token.expose(),
                }),
                Err(_) => return unavailable(),
            }
        }
    };
    crate::coder_sync::answer(StatusCode::OK, created)
}

/// `DELETE /v1/keys/{id}[?kind=api]`.
async fn api_revoke(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<KindQuery>,
) -> Response {
    let kind = match kind_of(query.kind.as_deref()) {
        Ok(kind) => kind,
        Err(response) => return response,
    };
    let (service, token) = match credential(&app, &headers) {
        Ok(found) => found,
        Err(response) => return response,
    };
    let gone = || {
        refused(
            StatusCode::NOT_FOUND,
            "unknown_key",
            "There's no working key with that id.",
        )
    };
    match kind {
        Kind::Signin => match service.revoke_sign_in_key(&token, &id).await {
            Ok(true) => crate::coder_sync::forget_checked(&id),
            Ok(false) => return gone(),
            Err(error) => return session_failure(error),
        },
        Kind::Api => {
            let viewer = match service.app_viewer(&token).await {
                Ok(viewer) => viewer,
                Err(error) => return session_failure(error),
            };
            let workspace = match api_workspace(&viewer) {
                Ok(workspace) => workspace,
                Err(response) => return response,
            };
            let account = viewer.client().account();
            let listed = match account.keys(&workspace).await {
                Ok(listed) => listed,
                Err(_) => return unavailable(),
            };
            if !listed
                .iter()
                .any(|key| key.id == id && key.status.as_deref() != Some("revoked"))
            {
                return gone();
            }
            if account.revoke_key(&workspace, &id).await.is_err() {
                return unavailable();
            }
        }
    }
    crate::coder_sync::answer(
        StatusCode::OK,
        json!({"kind": kind.word(), "key": {"id": id, "state": "revoked"}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str, expires: Option<u64>, used: Option<u64>) -> SignInKey {
        SignInKey {
            id: "a".repeat(64),
            name: name.into(),
            created_at: 1_791_504_000,
            last_used_at: used,
            expires_at: expires,
            current: false,
        }
    }

    #[test]
    fn the_page_lists_keys_and_never_a_secret() {
        let keys = [
            key("build agent", None, None),
            key("ci", Some(1_794_096_000), Some(1_791_590_400)),
        ];
        let html = keys_content(&keys, ("t", "r"), ("t2", "r2")).into_string();
        crate::copy_guard::assert_plain(PAGE, &html);
        assert!(html.contains("Login keys for agents"));
        assert!(html.contains("build agent"));
        assert!(html.contains("Never used · Never expires"));
        assert!(html.contains("Last used 2026-10-10 · Expires 2026-11-08"));
        assert!(html.contains(">Revoke<"));
        assert!(html.contains("In 30 days") && html.contains("In 90 days"));
        assert!(!html.contains("oa_sk_"));
        let empty = keys_content(&[], ("t", "r"), ("t2", "r2")).into_string();
        assert!(empty.contains("You have no login keys."));
    }

    #[test]
    fn the_new_key_shows_once_with_the_line_to_use_it() {
        let made = MadeKey {
            key: key("agent", None, None),
            token: format!("oa_sk_{}", "b".repeat(64)),
        };
        let html = made_content("https://openagents.com", &made).into_string();
        crate::copy_guard::assert_plain(PAGE, &html);
        assert!(html.contains("won't be shown again"));
        assert!(html.contains(&format!("OPENAGENTS_TOKEN={} coder", made.token)));
        assert!(html.contains("coder login --key"));
        assert!(html.contains("data-oa-copy"), "a copy button");
        let staging = made_content("https://staging.example", &made).into_string();
        assert!(staging.contains("OPENAGENTS_ORIGIN=https://staging.example OPENAGENTS_TOKEN="));
        assert!(!format!("{made:?}").contains("oa_sk_"));
    }

    #[test]
    fn the_api_paths_are_the_sites() {
        assert!(owns("/v1/keys"));
        assert!(owns("/v1/keys/abc"));
        assert!(!owns("/v1/keysmith"));
    }
}
