//! Settings > Public profile (`/settings/profile`, #12254): claim a
//! `name@openagents.com` name and publish the profile your key signs.
//!
//! The name is the account service's (`PUT /v1/account/name` under the
//! browser's session). The profile is a kind 0 event signed by the
//! person's own key, never by this server:
//!
//! 1. `profile.js` sends the edited fields to `POST /settings/profile/draft`,
//!    which checks them and answers the unsigned event: these fields
//!    written over the key's current profile, so fields set elsewhere stay.
//! 2. The browser's Nostr extension (NIP-07) signs it.
//! 3. `POST /settings/profile/publish` checks the signature, that the key
//!    is one of the account's, and the fields again, then publishes it to
//!    the profile relays.
//!
//! Without an extension the page says to edit the profile in the
//! OpenAgents app, which signs with the key it holds.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use nostr::domain::Event;
use nostr::metadata::Metadata;
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::{Field, Input, InputType};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, SessionError, Viewer};
use crate::settings::{PAGE as SETTINGS, page, viewer};
use crate::ui_page::action_link;

pub(crate) const PAGE: &str = "/settings/profile";
pub(crate) const NAME: &str = "/settings/profile/name";
pub(crate) const DRAFT: &str = "/settings/profile/draft";
pub(crate) const PUBLISH: &str = "/settings/profile/publish";
pub(crate) const SCRIPT: &str = "/settings/profile.js";
/// Follow a key: `/@{handle}/follow`.
pub(crate) const FOLLOW: &str = "/@{handle}/follow";
const NAME_SCOPE: &str = "profile-name";
const PUBLISH_SCOPE: &str = "profile-publish";
/// How far a signed profile's time may be from this server's.
const SKEW: u64 = 600;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(show))
        .route(NAME, post(name))
        .route(DRAFT, post(draft))
        .route(PUBLISH, post(publish))
        .route(SCRIPT, get(script))
        .route(FOLLOW, get(follow_page))
}

pub(super) fn owns(path: &str) -> bool {
    matches!(path, PAGE | NAME | DRAFT | PUBLISH | SCRIPT)
}

fn target(viewer: &Viewer, request: &str) -> String {
    format!("{}:{request}", viewer.account_id)
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Public profile",
        text,
        (PAGE, "Public profile"),
    ))
}

const UNREACHABLE: &str = "Your profile can't be reached right now. Try again in a minute.";

/// The account's keys, as hex.
async fn keys(service: &CloudSession, token: &str) -> Result<Vec<String>, ()> {
    let answer = service
        .nostr_call(
            reqwest::Method::GET,
            "/v1/account/keys",
            Some(&format!("Bearer {token}")),
            None,
        )
        .await
        .map_err(|_| ())?;
    if answer.status != 200 {
        return Err(());
    }
    Ok(answer.body["keys"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k["pubkey"].as_str().map(str::to_string))
        .collect())
}

/// The account's name and the key it points at.
async fn held_name(service: &CloudSession, token: &str) -> Result<Option<(String, String)>, ()> {
    let answer = service
        .nostr_call(
            reqwest::Method::GET,
            "/v1/account/name",
            Some(&format!("Bearer {token}")),
            None,
        )
        .await
        .map_err(|_| ())?;
    if answer.status != 200 {
        return Err(());
    }
    Ok(answer.body["name"].as_str().and_then(|name| {
        answer.body["pubkey"]
            .as_str()
            .map(|key| (name.to_string(), key.to_string()))
    }))
}

/// Everything the page and the posts need about the signed-in person.
struct Mine {
    keys: Vec<String>,
    name: Option<(String, String)>,
}

impl Mine {
    /// The key the profile is for: the one the name points at, else the
    /// first.
    fn key(&self) -> Option<&str> {
        self.name
            .as_ref()
            .map(|(_, key)| key.as_str())
            .or_else(|| self.keys.first().map(String::as_str))
    }
}

async fn mine(service: &CloudSession, headers: &HeaderMap) -> Result<Mine, Response> {
    let Ok(token) = service.browser_token(headers) else {
        return Err(crate::cloud::refused(SessionError::Unauthenticated));
    };
    let (Ok(keys), Ok(name)) = (
        keys(service, &token).await,
        held_name(service, &token).await,
    ) else {
        return Err(problem(StatusCode::BAD_GATEWAY, UNREACHABLE));
    };
    Ok(Mine { keys, name })
}

async fn show(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mine = match mine(service, &headers).await {
        Ok(mine) => mine,
        Err(response) => return response,
    };
    let mut tickets = Vec::new();
    for scope in [NAME_SCOPE, PUBLISH_SCOPE] {
        let request = fresh_request();
        match service.csrf(&headers, &viewer, scope, &target(&viewer, &request)) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let current = match mine.key() {
        Some(key) => Some(super::load(&app.config.profile_relays, key).await),
        None => None,
    };
    let metadata = current
        .as_ref()
        .and_then(|loaded| loaded.metadata.clone())
        .unwrap_or_default();
    let body = content(&mine, &metadata, &tickets[0], &tickets[1]);
    let mut response = page(&headers, service, &viewer, "Public profile", PAGE, body);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

fn text_field(id: &str, label: &str, name: &str, value: Option<&str>, hint: &str) -> Markup {
    let field = Field::new(id, label).description(hint);
    let mut input = Input::new(name)
        .input_type(InputType::Text)
        .aria(field.aria());
    if let Some(value) = value {
        input = input.value(value);
    }
    html! { (field.clone().control(input)) }
}

fn content(
    mine: &Mine,
    metadata: &Metadata,
    name_ticket: &(String, String),
    publish_ticket: &(String, String),
) -> Markup {
    let Some(key) = mine.key() else {
        return html! {
            p { (action_link("Settings", SETTINGS)) }
            (MarkdownRoot::new(html! {
                h1 { "Public profile" }
                p { "Your account has no Nostr key yet. Open the OpenAgents app on your phone or computer to make one, then come back." }
            }))
        };
    };
    let link = super::href(key, mine.name.as_ref().map(|(n, _)| n.as_str()));
    html! {
        p { (action_link("Settings", SETTINGS)) }
        (MarkdownRoot::new(html! {
            h1 { "Public profile" }
            p { "Your profile is public. Anyone can see it at " a href=(link) { (link) } "." }
        }))
        section class="oa-settings-group" aria-labelledby="profile-name-group" {
            h2 #profile-name-group { "Your name" }
            @if let Some((name, _)) = &mine.name {
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { (name) "@" (super::DOMAIN) }
                        span class="oa-settings-hint" { "Nostr apps show this name for your key." }
                    }
                    div class="oa-settings-control" {
                        form method="post" action=(NAME) {
                            input type="hidden" name="csrf" value=(name_ticket.0);
                            input type="hidden" name="request" value=(name_ticket.1);
                            input type="hidden" name="release" value="1";
                            (Button::new("Give up this name")
                                .kind(ButtonType::Submit)
                                .variant(ButtonVariant::Soft)
                                .color(Color::Secondary))
                        }
                    }
                }
            }
            form method="post" action=(NAME) autocomplete="off" {
                input type="hidden" name="csrf" value=(name_ticket.0);
                input type="hidden" name="request" value=(name_ticket.1);
                (text_field(
                    "profile-name",
                    if mine.name.is_some() { "Change your name" } else { "Pick a name" },
                    "name",
                    None,
                    "Letters, numbers, dashes, and underscores. You get name@openagents.com.",
                ))
                p { (Button::new("Save name").kind(ButtonType::Submit)) }
            }
        }
        section class="oa-settings-group" aria-labelledby="profile-fields-group"
            data-oa-profile-edit="" data-pubkey=(key)
            data-csrf=(publish_ticket.0) data-request=(publish_ticket.1) {
            h2 #profile-fields-group { "Your profile" }
            form autocomplete="off" data-oa-profile-form="" {
                (text_field("profile-display-name", "Name", "display_name", metadata.shown_name(), "The name people see."))
                (text_field("profile-picture", "Picture", "picture", metadata.picture.as_deref(), "A link to your picture, starting with https://."))
                (text_field("profile-about", "About", "about", metadata.about.as_deref(), "A line or two about you."))
                (text_field("profile-website", "Website", "website", metadata.website.as_deref(), "Optional."))
                p data-oa-profile-publish="" hidden {
                    (Button::new("Publish").kind(ButtonType::Submit))
                }
            }
            p class="oa-settings-hint" data-oa-profile-note="" {
                "To publish from this page, add a Nostr browser extension. You can also edit your profile in the OpenAgents app."
            }
        }
        script src=(SCRIPT) defer {}
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameForm {
    csrf: String,
    request: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    release: String,
}

async fn name(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<NameForm>, FormRejection>,
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
        NAME_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let Ok(token) = service.browser_token(&headers) else {
        return crate::cloud::refused(SessionError::Unauthenticated);
    };
    let bearer = format!("Bearer {token}");
    let answer = if form.release == "1" {
        service
            .nostr_call(
                reqwest::Method::DELETE,
                "/v1/account/name",
                Some(&bearer),
                None,
            )
            .await
    } else {
        service
            .nostr_call(
                reqwest::Method::PUT,
                "/v1/account/name",
                Some(&bearer),
                Some(json!({"name": form.name}).to_string().into_bytes()),
            )
            .await
    };
    match answer {
        Ok(answer) if answer.status == 200 => {
            if let Some(key) = answer.body["pubkey"].as_str() {
                super::forget(&app.config.profile_relays, key);
            }
            protect(Redirect::to(PAGE).into_response())
        }
        Ok(answer) if (400..500).contains(&answer.status) => problem(
            StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_REQUEST),
            answer.body["error"]["message"]
                .as_str()
                .unwrap_or("That name can't be used. Try another."),
        ),
        _ => problem(StatusCode::BAD_GATEWAY, UNREACHABLE),
    }
}

fn json_refusal(status: StatusCode, message: &str) -> Response {
    protect((status, axum::Json(json!({"error": {"message": message}}))).into_response())
}

/// Check the posting browser: a signed-in viewer and this page's ticket.
async fn checked<'a>(
    app: &'a App,
    headers: &HeaderMap,
    body: &Value,
) -> Result<(&'a CloudSession, Mine), Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(json_refusal(
            StatusCode::NOT_FOUND,
            "Profiles aren't set up here.",
        ));
    };
    let viewer = service
        .authenticate(headers)
        .await
        .map_err(|_| json_refusal(StatusCode::UNAUTHORIZED, "Log in again."))?;
    let (csrf, request) = (
        body["csrf"].as_str().unwrap_or_default(),
        body["request"].as_str().unwrap_or_default(),
    );
    service
        .verify_csrf(
            headers,
            Some(&viewer),
            PUBLISH_SCOPE,
            &target(&viewer, request),
            csrf,
        )
        .map_err(|_| json_refusal(StatusCode::FORBIDDEN, "Reload the page and try again."))?;
    let mine = mine(service, headers)
        .await
        .map_err(|_| json_refusal(StatusCode::BAD_GATEWAY, UNREACHABLE))?;
    Ok((service, mine))
}

fn edited(fields: &Value) -> Metadata {
    let field = |name: &str| {
        Some(
            fields[name]
                .as_str()
                .map(str::trim)
                .unwrap_or_default()
                .to_string(),
        )
    };
    Metadata {
        display_name: field("display_name"),
        picture: field("picture"),
        about: field("about"),
        website: field("website"),
        ..Metadata::default()
    }
}

/// `POST /settings/profile/draft` `{csrf, request, fields}`: the unsigned
/// event to sign.
async fn draft(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Ok(body) = serde_json::from_slice::<Value>(&body) else {
        return json_refusal(StatusCode::BAD_REQUEST, "Reload the page and try again.");
    };
    let (_, mine) = match checked(&app, &headers, &body).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(key) = mine.key().map(str::to_string) else {
        return json_refusal(StatusCode::CONFLICT, "Your account has no Nostr key yet.");
    };
    if let Some(target) = body["fields"]["follow"].as_str() {
        return follow_draft(&app, &key, target).await;
    }
    let mut edit = edited(&body["fields"]);
    let present = |v: &Option<String>| v.clone().filter(|v| !v.is_empty());
    let check = Metadata {
        display_name: present(&edit.display_name),
        picture: present(&edit.picture),
        about: present(&edit.about),
        website: present(&edit.website),
        ..Metadata::default()
    };
    if let Err(words) = check.check() {
        return json_refusal(StatusCode::BAD_REQUEST, words);
    }
    if let Some((name, _)) = &mine.name {
        edit.nip05 = Some(format!("{name}@{}", super::DOMAIN));
        if edit.display_name.as_deref().is_some_and(str::is_empty) {
            edit.name = Some(name.clone());
        }
    }
    let earlier = super::load(&app.config.profile_relays, &key).await;
    let content = edit.merged_content(earlier.content.as_deref());
    protect(
        axum::Json(json!({
            "event": {
                "kind": 0,
                "created_at": crate::cloud::session::now(),
                "tags": [],
                "content": content,
            },
            "pubkey": key,
        }))
        .into_response(),
    )
}

/// `POST /settings/profile/publish` `{csrf, request, event}`.
async fn publish(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Ok(body) = serde_json::from_slice::<Value>(&body) else {
        return json_refusal(StatusCode::BAD_REQUEST, "Reload the page and try again.");
    };
    let (_, mine) = match checked(&app, &headers, &body).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(event) = serde_json::from_value::<Event>(body["event"].clone()) else {
        return json_refusal(
            StatusCode::BAD_REQUEST,
            "Your extension didn't sign the profile.",
        );
    };
    let now = crate::cloud::session::now();
    if !matches!(event.kind, 0 | 3)
        || event.validate_crypto().is_err()
        || event.created_at.abs_diff(now) > SKEW
    {
        return json_refusal(
            StatusCode::BAD_REQUEST,
            "Your extension didn't sign the profile.",
        );
    }
    if !mine.keys.contains(&event.pubkey) {
        return json_refusal(
            StatusCode::FORBIDDEN,
            "Your extension signed with a key that isn't on your account.",
        );
    }
    let read = Metadata::read(&event.content);
    let follows_ok = event.kind == 3
        && event.tags.len() <= nostr::metadata::FOLLOWS_MAX
        && nostr::metadata::follows(&event).len() == event.tags.len();
    if (event.kind == 0 && read.check().is_err())
        || (event.kind == 3 && !follows_ok)
        || event.content.len() > 16 * 1024
    {
        return json_refusal(StatusCode::BAD_REQUEST, "Check your profile and try again.");
    }
    let relays = &app.config.profile_relays;
    let sent = futures_util::future::join_all(
        relays
            .iter()
            .map(|relay| super::relays::publish(relay, &event)),
    )
    .await;
    let published = sent.iter().filter(|r| r.is_ok()).count();
    if published == 0 {
        return json_refusal(
            StatusCode::BAD_GATEWAY,
            "We couldn't publish your profile right now. Try again in a minute.",
        );
    }
    super::forget(relays, &event.pubkey);
    let name = mine
        .name
        .as_ref()
        .filter(|(_, key)| *key == event.pubkey)
        .map(|(name, _)| name.as_str());
    protect(
        axum::Json(json!({
            "published": published,
            "href": super::href(&event.pubkey, name),
        }))
        .into_response(),
    )
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../../static/profile.js"),
    )
        .into_response()
}

/// The unsigned kind 3 that adds `target` to `key`'s follows.
async fn follow_draft(app: &App, key: &str, target: &str) -> Response {
    let Some(target) = oa_target(target) else {
        return json_refusal(StatusCode::BAD_REQUEST, "That isn't a Nostr key.");
    };
    if target == key {
        return json_refusal(StatusCode::BAD_REQUEST, "That's your own key.");
    }
    let current = super::load(&app.config.profile_relays, key).await;
    let mut follows = current.follows.clone();
    if !follows.contains(&target) {
        follows.push(target);
    }
    let tags: Vec<Value> = follows.iter().map(|k| json!(["p", k])).collect();
    protect(
        axum::Json(json!({
            "event": {
                "kind": 3,
                "created_at": crate::cloud::session::now(),
                "tags": tags,
                "content": "",
            },
            "pubkey": key,
        }))
        .into_response(),
    )
}

fn oa_target(text: &str) -> Option<String> {
    match super::handle(text)? {
        super::Handle::Key(key) => Some(key),
        super::Handle::Name(_) => None,
    }
}

/// `/@{handle}/follow`: follow a person or agent with your key.
async fn follow_page(
    State(app): State<App>,
    axum::extract::Path(text): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Response {
    let back = format!("/@{text}/follow");
    let (service, viewer) = match viewer(&app, &headers, &back).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Ok(who) = super::who(&app, &text).await else {
        return problem(StatusCode::NOT_FOUND, "Nobody has that name.");
    };
    let mine = match mine(service, &headers).await {
        Ok(mine) => mine,
        Err(response) => return response,
    };
    let request = fresh_request();
    let csrf = match service.csrf(&headers, &viewer, PUBLISH_SCOPE, &target(&viewer, &request)) {
        Ok(csrf) => csrf,
        Err(error) => return crate::cloud::refused(error),
    };
    let profile = super::load(&app.config.profile_relays, &who.pubkey).await;
    let shown = profile
        .metadata
        .as_ref()
        .and_then(|m| m.shown_name().map(str::to_string))
        .unwrap_or_else(|| super::short_npub(&who.pubkey));
    let link = super::href(&who.pubkey, who.name.as_deref());
    let following = match mine.key() {
        Some(key) => super::load(&app.config.profile_relays, key)
            .await
            .follows
            .contains(&who.pubkey),
        None => false,
    };
    let body = html! {
        (MarkdownRoot::new(html! {
            h1 { "Follow " (shown) }
            p { a href=(link) { (link) } }
        }))
        @if mine.key().is_none() {
            p { "Your account has no Nostr key yet. Open the OpenAgents app on your phone or computer to make one, then come back." }
        } @else if following {
            p { "You follow " (shown) "." }
        } @else {
            section class="oa-settings-group" data-oa-profile-edit="" data-pubkey=(mine.key().unwrap_or_default())
                data-csrf=(csrf) data-request=(request) {
                form autocomplete="off" data-oa-profile-form="" {
                    input type="hidden" name="follow" value=(super::npub(&who.pubkey));
                    p data-oa-profile-publish="" hidden {
                        (Button::new("Follow").kind(ButtonType::Submit))
                    }
                }
                p class="oa-settings-hint" data-oa-profile-note="" {
                    "To follow from this page, add a Nostr browser extension. You can also follow from the OpenAgents app."
                }
            }
            script src=(SCRIPT) defer {}
        }
    };
    page(&headers, service, &viewer, "Follow", &back, body)
}
