//! `/settings/api-keys` (#11065): the signed-in person's API keys for the
//! OpenAgents API (`docs/inference/gateway.md`, section 7). They make a
//! key, with an optional monthly spending limit they choose, see the
//! secret once, and revoke keys. The keys are the account service's
//! `oak_` keys on the person's own workspace; nothing about a key's secret
//! is kept or shown here after it is made.

use axum::extract::rejection::FormRejection;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::{Field, Input, InputType};
use serde::Deserialize;

use crate::App;
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::{SessionError, Viewer};
use crate::settings::{PAGE, page, viewer};
use crate::ui_page::action_link;

pub(crate) const KEYS: &str = "/settings/api-keys";
pub(crate) const REVOKE: &str = "/settings/api-keys/revoke";
pub(crate) const OWN: &str = "/settings/api-keys/own";
pub(crate) const OWN_REMOVE: &str = "/settings/api-keys/own/remove";
const OWN_SCOPE: &str = "api-key-own";

/// The providers whose keys the API can use for `pay: "mine"`: the word
/// the account service takes, and the name a person reads.
const PROVIDERS: [(&str, &str); 2] = [
    ("openrouter", "OpenRouter"),
    ("vercel", "Vercel AI Gateway"),
];
const MAKE_SCOPE: &str = "api-key-make";
const REVOKE_SCOPE: &str = "api-key-revoke";

/// Where the API answers.
pub(crate) const BASE_URL: &str = "https://api.openagents.com/v1";

/// The highest monthly limit the form takes, in dollars.
const LIMIT_DOLLARS_MAX: u64 = 100_000;

/// The workspace the person's keys live on: their own.
fn own_workspace(viewer: &Viewer) -> Option<String> {
    viewer
        .workspaces
        .iter()
        .find(|workspace| workspace.role == "owner")
        .map(|workspace| workspace.id.clone())
}

fn target(viewer: &Viewer, workspace: &str, request: &str) -> String {
    format!("{}:{workspace}:{request}", viewer.account_id)
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "API keys",
        text,
        (KEYS, "API keys"),
    ))
}

fn unreachable_service() -> Response {
    problem(
        StatusCode::BAD_GATEWAY,
        "Your keys can't be reached right now. Try again in a minute.",
    )
}

/// One key row: what the person named it, when it was made, and whether
/// it still works.
struct Row {
    id: String,
    name: String,
    created: String,
    active: bool,
}

pub(crate) async fn keys(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, KEYS).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(workspace) = own_workspace(&viewer) else {
        return problem(
            StatusCode::CONFLICT,
            "Your account has no workspace of its own yet, so there's nowhere to keep a key.",
        );
    };
    let listed = match viewer.client().account().keys(&workspace).await {
        Ok(listed) => listed,
        Err(_) => return unreachable_service(),
    };
    let rows: Vec<Row> = listed
        .into_iter()
        .filter(|key| key.status.as_deref() != Some("revoked"))
        .map(|key| Row {
            name: key.name.clone().unwrap_or_else(|| "Unnamed key".to_owned()),
            created: key.created.clone().unwrap_or_default(),
            active: key
                .status
                .as_deref()
                .is_none_or(|status| status == "active"),
            id: key.id,
        })
        .collect();
    // Absent when this server keeps no provider keys: the section hides.
    let own = viewer
        .client()
        .account()
        .provider_keys(&workspace)
        .await
        .ok()
        .map(|saved| {
            PROVIDERS
                .iter()
                .map(|(word, _)| saved.iter().any(|key| key.provider == *word))
                .collect::<Vec<bool>>()
        });
    let mut tickets = Vec::new();
    for scope in [MAKE_SCOPE, REVOKE_SCOPE, OWN_SCOPE] {
        let request = fresh_request();
        match service.csrf(
            &headers,
            &viewer,
            scope,
            &target(&viewer, &workspace, &request),
        ) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let body = html! {
        (keys_content(
            &rows,
            (&tickets[0].0, &tickets[0].1),
            (&tickets[1].0, &tickets[1].1),
        ))
        @if let Some(saved) = &own {
            (own_content(saved, (&tickets[2].0, &tickets[2].1)))
        }
    };
    page(&headers, service, &viewer, "API keys", KEYS, body)
}

/// The page: how to use a key, the keys, the form that makes one.
fn keys_content(rows: &[Row], make: (&str, &str), revoke: (&str, &str)) -> Markup {
    let name = Field::new("api-key-name", "Name")
        .description("So you can tell your keys apart, such as \"laptop\" or \"my app\".");
    let limit = Field::new("api-key-limit", "Monthly spending limit, in dollars").description(
        "Optional. Requests that would go over it are refused. Leave it empty for no limit.",
    );
    html! {
        p { (action_link("Settings", PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "API keys" }
            p {
                "Use a key with the OpenAgents API at " code { (BASE_URL) }
                ". It answers the Open Responses and OpenAI Chat Completions formats, so OpenAI's libraries work when you set their base URL to that address."
            }
            p { "Requests are paid from your credits. A few requests a day on free models cost nothing." }
        }))
        @if rows.is_empty() {
            p { "You have no keys yet." }
        } @else {
            section class="oa-settings-group" aria-labelledby="api-keys-list" {
                h2 #api-keys-list { "Your keys" }
                @for row in rows {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { (row.name) }
                            span class="oa-settings-hint" {
                                @if row.active { "Works" } @else { "Paused" }
                                @if !row.created.is_empty() { " · made " (row.created) }
                            }
                        }
                        div class="oa-settings-control" {
                            form method="post" action=(REVOKE) {
                                input type="hidden" name="csrf" value=(revoke.0);
                                input type="hidden" name="request" value=(revoke.1);
                                input type="hidden" name="key" value=(row.id);
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
        section class="oa-settings-group" aria-labelledby="api-keys-make" {
            h2 #api-keys-make { "Make a key" }
            form method="post" action=(KEYS) autocomplete="off" {
                input type="hidden" name="csrf" value=(make.0);
                input type="hidden" name="request" value=(make.1);
                (name.clone().control(
                    Input::new("name")
                        .input_type(InputType::Text)
                        .aria(name.aria()),
                ))
                (limit.clone().control(
                    Input::new("limit")
                        .input_type(InputType::Text)
                        .inputmode("decimal")
                        .aria(limit.aria()),
                ))
                p { (Button::new("Make key").kind(ButtonType::Submit)) }
            }
        }
    }
}

/// Your own provider keys: requests that ask to pay with them
/// (`"openagents": {"pay": "mine"}`) use only these, and cost nothing here.
fn own_content(saved: &[bool], ticket: (&str, &str)) -> Markup {
    html! {
        section class="oa-settings-group" aria-labelledby="api-keys-own" {
            h2 #api-keys-own { "Your own provider keys" }
            p {
                "Add your own OpenRouter or Vercel AI Gateway key, then send "
                code { "\"openagents\": {\"pay\": \"mine\"}" }
                " in a request to run it on your key only. We don't charge for those requests; your provider bills you."
            }
            @for ((word, label), present) in PROVIDERS.iter().zip(saved) {
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { (label) }
                        span class="oa-settings-hint" { @if *present { "Saved" } @else { "Not added" } }
                    }
                    @if *present {
                        div class="oa-settings-control" {
                            form method="post" action=(OWN_REMOVE) {
                                input type="hidden" name="csrf" value=(ticket.0);
                                input type="hidden" name="request" value=(ticket.1);
                                input type="hidden" name="provider" value=(word);
                                (Button::new("Remove")
                                    .kind(ButtonType::Submit)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
                @let field = Field::new(format!("api-key-own-{word}"), format!("{label} key"));
                form method="post" action=(OWN) autocomplete="off" {
                    input type="hidden" name="csrf" value=(ticket.0);
                    input type="hidden" name="request" value=(ticket.1);
                    input type="hidden" name="provider" value=(word);
                    (field.clone().control(
                        Input::new("key")
                            .input_type(InputType::Password)
                            .aria(field.aria()),
                    ))
                    p { (Button::new(if *present { "Replace" } else { "Save" }).kind(ButtonType::Submit)) }
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnForm {
    csrf: String,
    request: String,
    provider: String,
    #[serde(default)]
    key: String,
}

/// The checked viewer, workspace, and provider word of an own-key form.
async fn own_request<'a>(
    app: &'a App,
    headers: &HeaderMap,
    form: &OwnForm,
) -> Result<(Viewer, String, &'static str), Response> {
    let (service, viewer) = viewer(app, headers, KEYS).await?;
    let Some(workspace) = own_workspace(&viewer) else {
        return Err(problem(
            StatusCode::CONFLICT,
            "Your account has no workspace of its own yet.",
        ));
    };
    service
        .verify_csrf(
            headers,
            Some(&viewer),
            OWN_SCOPE,
            &target(&viewer, &workspace, &form.request),
            &form.csrf,
        )
        .map_err(crate::cloud::refused)?;
    let Some((word, _)) = PROVIDERS.iter().find(|(word, _)| *word == form.provider) else {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "Pick OpenRouter or Vercel AI Gateway.",
        ));
    };
    Ok((viewer, workspace, word))
}

pub(crate) async fn own_save(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<OwnForm>, FormRejection>,
) -> Response {
    let Ok(Form(mut form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (viewer, workspace, word) = match own_request(&app, &headers, &form).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let key = jev::ApiKey::new(std::mem::take(&mut form.key).trim().to_owned());
    if key.expose().is_empty() {
        return problem(StatusCode::BAD_REQUEST, "Paste the key to save it.");
    }
    match viewer
        .client()
        .account()
        .set_provider_key(&workspace, word, &key)
        .await
    {
        Ok(()) => protect(Redirect::to(KEYS).into_response()),
        Err(_) => problem(
            StatusCode::BAD_REQUEST,
            "That key couldn't be saved. Check that it's the whole key, with no spaces.",
        ),
    }
}

pub(crate) async fn own_remove(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<OwnForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (viewer, workspace, word) = match own_request(&app, &headers, &form).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match viewer
        .client()
        .account()
        .remove_provider_key(&workspace, word)
        .await
    {
        Ok(()) => protect(Redirect::to(KEYS).into_response()),
        Err(_) => unreachable_service(),
    }
}

/// The new key, shown once.
fn made_content(token: &str, name: &str) -> Markup {
    html! {
        p { (action_link("API keys", KEYS)) }
        (MarkdownRoot::new(html! {
            h1 { "Your new key" }
            p { "Copy " strong { (name) } " now. It won't be shown again." }
            pre { code { (token) } }
            p { "Try it:" }
            pre { code {
                "curl " (BASE_URL) "/responses \\\n  -H \"Authorization: Bearer $OPENAGENTS_API_KEY\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"model\": \"openagents/chat\", \"input\": \"Say hello\"}'"
            } }
        }))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MakeForm {
    csrf: String,
    request: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    limit: String,
}

/// Dollars typed in the form, as a decimal string the account service
/// takes, or why not.
fn limit_dollars(text: &str) -> Result<Option<String>, &'static str> {
    let text = text.trim().trim_start_matches('$').trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if whole.is_empty() && fraction.is_empty()
        || !digits(whole)
        || !digits(fraction)
        || fraction.len() > 2
    {
        return Err("Write the limit as dollars, such as 20 or 7.50.");
    }
    let whole_value: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| "That limit is too large.")?
    };
    if whole_value > LIMIT_DOLLARS_MAX {
        return Err("That limit is too large.");
    }
    if whole_value == 0 && fraction.chars().all(|c| c == '0') {
        return Err("Set a limit above zero, or leave it empty for no limit.");
    }
    Ok(Some(format!("{whole_value}.{:0<2}", fraction)))
}

pub(crate) async fn make(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<MakeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, KEYS).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(workspace) = own_workspace(&viewer) else {
        return problem(
            StatusCode::CONFLICT,
            "Your account has no workspace of its own yet.",
        );
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        MAKE_SCOPE,
        &target(&viewer, &workspace, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let limit = match limit_dollars(&form.limit) {
        Ok(limit) => limit,
        Err(text) => return problem(StatusCode::BAD_REQUEST, text),
    };
    let name: String = form.name.trim().chars().take(80).collect();
    let name = if name.is_empty() {
        "API key".to_owned()
    } else {
        name
    };
    let account = viewer.client().account();
    let grant = match account.issue_key(&workspace, &name).await {
        Ok(grant) => grant,
        Err(_) => return unreachable_service(),
    };
    if let Some(usd) = limit {
        let limits = serde_json::json!({"spend_cap": {"usd": usd, "period": "month"}});
        if account
            .set_key_limits(&workspace, &grant.key.id, &limits)
            .await
            .is_err()
        {
            // A key without the limit the person asked for must not work.
            let _ = account.revoke_key(&workspace, &grant.key.id).await;
            return problem(
                StatusCode::BAD_GATEWAY,
                "The key's limit couldn't be saved, so the key was removed. Try again in a minute.",
            );
        }
    }
    let body = made_content(grant.token.expose(), &name);
    let mut response = page(&headers, service, &viewer, "Your new key", KEYS, body);
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RevokeForm {
    csrf: String,
    request: String,
    key: String,
}

pub(crate) async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RevokeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, KEYS).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(workspace) = own_workspace(&viewer) else {
        return problem(
            StatusCode::CONFLICT,
            "Your account has no workspace of its own yet.",
        );
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        REVOKE_SCOPE,
        &target(&viewer, &workspace, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    match viewer
        .client()
        .account()
        .revoke_key(&workspace, &form.key)
        .await
    {
        Ok(()) => protect(Redirect::to(KEYS).into_response()),
        Err(_) => unreachable_service(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_read_as_dollars() {
        assert_eq!(limit_dollars(""), Ok(None));
        assert_eq!(limit_dollars(" $20 "), Ok(Some("20.00".into())));
        assert_eq!(limit_dollars("7.5"), Ok(Some("7.50".into())));
        assert_eq!(limit_dollars(".25"), Ok(Some("0.25".into())));
        assert!(limit_dollars("0").is_err());
        assert!(limit_dollars("ten").is_err());
        assert!(limit_dollars("1.234").is_err());
        assert!(limit_dollars("1000000").is_err());
    }

    #[test]
    fn the_page_names_the_address_and_never_a_secret() {
        let rows = [Row {
            id: "k1".into(),
            name: "laptop".into(),
            created: "2026-10-09".into(),
            active: true,
        }];
        let html = keys_content(&rows, ("t", "r"), ("t2", "r2")).into_string();
        assert!(html.contains(BASE_URL));
        assert!(html.contains("laptop"));
        assert!(html.contains(">Revoke<"));
        assert!(
            html.contains(
                "<form method=\"post\" action=\"/settings/api-keys\" autocomplete=\"off\">"
            )
        );
        crate::copy_guard::assert_plain(KEYS, &html);
        let own = own_content(&[true, false], ("t3", "r3")).into_string();
        crate::copy_guard::assert_plain(KEYS, &own);
        assert!(own.contains("pay"));
        assert!(own.contains(">Remove<"), "the saved one can be removed");
        assert!(
            own.contains("type=\"password\""),
            "a key is never shown as typed"
        );
        let made = made_content("oak_abc.def", "laptop").into_string();
        crate::copy_guard::assert_plain(KEYS, &made);
        assert!(made.contains("oak_abc.def"));
        assert!(made.contains("won't be shown again"));
    }
}
