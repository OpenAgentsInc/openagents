//! `/settings/agents` (#11228): every agent the account's computers and
//! cloud environments report, with Stop and Message, the same list the
//! phone shows (`GET /v1/agents`, [`crate::phone_api`]).
//!
//! An issue run on a cloud environment such as `oa-dev-env-1`, or on a GCE
//! pool host, is reported by `openagents chat work` on that machine; Coder
//! reports the background agents of a terminal. Stop and Message queue a
//! command for that machine ([`phone_api::queue_action`]), which it takes
//! at its next report, within seconds while something works.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /settings/agents` | The agents, by computer, newest report first |
//! | `GET /settings/agents/message?computer=&item=` | The message form for one agent |
//! | `POST /settings/agents/stop` `{csrf, computer, item}` | Stop it |
//! | `POST /settings/agents/message` `{csrf, computer, item, text}` | Send it a message |
//!
//! The list reads itself again every few seconds while an agent works.

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::PageColumn;
use openagents_ui::forms::Textarea;
use serde::Deserialize;

use crate::App;
use crate::account::Account;
use crate::chat_store::{MAX_AGENT_MESSAGE, Store, account_owner, now_unix};
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, Viewer};
use crate::coder_sync::{line, online};
use crate::phone_api::{self, Acted, Command, Item};
use crate::settings::viewer;
use crate::ui_page::UiPage;

pub(crate) const PAGE: &str = "/settings/agents";
const MESSAGE_PAGE: &str = "/settings/agents/message";
const STOP: &str = "/settings/agents/stop";
/// How often the list reads itself again while an agent works, in seconds.
const REFRESH_SECONDS: u32 = 5;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(list_page))
        .route(MESSAGE_PAGE, get(message_page).post(message_route))
        .route(STOP, post(stop_route))
}

/// One computer's agents as the list shows them.
pub(crate) struct Machine {
    pub name: String,
    pub online: bool,
    pub updated_unix: u64,
    pub agents: Vec<Item>,
}

/// The account's computers that report agents, newest report first.
async fn machines(store: &Store, owner: &str) -> Vec<Machine> {
    let (Ok(agents), Ok(computers)) = (
        phone_api::read_agents(store, owner).await,
        store.computers(owner).await,
    ) else {
        return Vec::new();
    };
    let mut found: Vec<Machine> = agents
        .boards
        .into_iter()
        .map(|(name, board)| Machine {
            online: online(&computers, &name),
            agents: board
                .items
                .into_iter()
                .filter(|item| item.kind == "agent")
                .collect(),
            updated_unix: board.updated_unix,
            name,
        })
        .filter(|machine| !machine.agents.is_empty())
        .collect();
    found.sort_by_key(|machine| std::cmp::Reverse(machine.updated_unix));
    found
}

fn live(item: &Item) -> bool {
    matches!(item.status.as_str(), "working" | "asking")
}

fn status_words(status: &str) -> &'static str {
    match status {
        "working" => "Working",
        "asking" => "Waiting for you",
        "done" => "Done",
        "failed" => "Failed",
        _ => "Stopped",
    }
}

fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

/// What the list says after an action, by the code the action left.
fn notice_words(code: &str) -> Option<&'static str> {
    Some(match code {
        "stopping" => "Asked it to stop. It stops at its next step.",
        "sent" => "Sent. It reads your message at its next step.",
        "gone" => "That agent isn't running there anymore.",
        "offline" => "That computer isn't online now.",
        "empty" => "Write a message first.",
        "long" => "Keep a message to 4,000 characters.",
        "secret" => "This looks like it holds a password or key, so it wasn't sent.",
        "failed" => "That didn't go through. Try again.",
        _ => return None,
    })
}

fn target_query(computer: &str, item: &str) -> String {
    let encode = |value: &str| -> String {
        value
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect()
    };
    format!("computer={}&item={}", encode(computer), encode(item))
}

fn agent_row(machine: &Machine, item: &Item, csrf: &str, now: u64) -> Markup {
    let end = item
        .finished_unix
        .unwrap_or(if live(item) { now } else { item.started_unix });
    let elapsed = end.saturating_sub(item.started_unix);
    let controls = live(item) && machine.online;
    html! {
        div class="oa-settings-row" data-status=(item.status) {
            div class="oa-settings-text" {
                span class="oa-settings-label" { (item.title) }
                span class="oa-settings-hint" {
                    (status_words(&item.status))
                    @if let Some(engine) = &item.engine { " · " (engine) }
                    " · " (agent_fleet::elapsed_words(elapsed))
                    @if let Some(cost) = item.cost_usd { " · " (agent_fleet::dollars(cost)) }
                }
                @if let Some(line) = &item.line { span class="oa-settings-hint" { (line) } }
            }
            @if controls {
                div class="oa-settings-control" {
                    form method="post" action=(STOP) {
                        input type="hidden" name="csrf" value=(csrf);
                        input type="hidden" name="computer" value=(machine.name);
                        input type="hidden" name="item" value=(item.id);
                        (Button::new("Stop")
                            .kind(ButtonType::Submit)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary))
                    }
                    " "
                    a href=(format!("{MESSAGE_PAGE}?{}", target_query(&machine.name, &item.id))) { "Message" }
                }
            }
        }
    }
}

/// The list: each computer's agents, with Stop and Message on the ones
/// that work while their computer is online.
pub(crate) fn list_markup(
    machines: &[Machine],
    csrf: &str,
    now: u64,
    notice: Option<&str>,
) -> Markup {
    html! {
        div class="oa-settings" {
            p {
                "Agents running on your computers and cloud environments, including issue runs. "
                "Stop or message one here or from your phone."
            }
            @if let Some(notice) = notice { p role="status" { (notice) } }
            @if machines.is_empty() {
                section class="oa-settings-group" aria-labelledby="agents-none" {
                    h2 #agents-none { "No agents yet" }
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-hint" {
                                "Sign in on the computer, then start issue runs with: openagents chat work --issues NUMBERS"
                            }
                        }
                    }
                }
            }
            @for (index, machine) in machines.iter().enumerate() {
                @let heading = format!("agents-computer-{index}");
                section class="oa-settings-group" aria-labelledby=(heading) {
                    h2 id=(heading) {
                        (machine.name) " · "
                        @if machine.online { "Online" } @else { "Last seen " (ago(machine.updated_unix, now)) }
                    }
                    @for item in &machine.agents { (agent_row(machine, item, csrf, now)) }
                }
            }
        }
    }
}

/// The page with the account menu, reading itself again while `refresh`.
fn page(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    title: &str,
    path: &str,
    refresh: bool,
    body: Markup,
) -> Response {
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    let mut page = UiPage::new(title)
        .path(path)
        .section(crate::settings::PAGE)
        .account(account)
        .content(PageColumn::new(body));
    if refresh {
        page = page.head(html! {
            meta http-equiv="refresh" content=(REFRESH_SECONDS.to_string());
        });
    }
    protect(page.respond(headers))
}

#[derive(Deserialize, Default)]
struct ListQuery {
    #[serde(default)]
    said: String,
}

async fn list_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let found = machines(&app.config.chat_store, &owner).await;
    let going = found.iter().any(|machine| machine.agents.iter().any(live));
    let csrf = crate::pages::chat::csrf(&app, &owner);
    let body = list_markup(&found, &csrf, now_unix(), notice_words(&query.said));
    page(&headers, service, &viewer, "Agents", PAGE, going, body)
}

#[derive(Deserialize, Default)]
struct Target {
    #[serde(default)]
    computer: String,
    #[serde(default)]
    item: String,
}

/// The message form for one agent.
pub(crate) fn message_markup(item: &Item, computer: &str, csrf: &str) -> Markup {
    html! {
        div class="oa-settings" {
            p { a href=(PAGE) { "All agents" } }
            section class="oa-settings-group" aria-labelledby="agent-message" {
                h2 #agent-message { "Message " (item.title) }
                form method="post" action=(MESSAGE_PAGE) {
                    input type="hidden" name="csrf" value=(csrf);
                    input type="hidden" name="computer" value=(computer);
                    input type="hidden" name="item" value=(item.id);
                    (Textarea::new("text")
                        .rows(4)
                        .maxlength(MAX_AGENT_MESSAGE as u32)
                        .aria_label(format!("Message {}", item.title)))
                    (Button::new("Send").kind(ButtonType::Submit))
                }
                p { "It reads your message at its next step, on " (computer) "." }
            }
        }
    }
}

async fn message_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(target): Query<Target>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let found = machines(&app.config.chat_store, &owner).await;
    let Some(item) = found
        .iter()
        .filter(|machine| machine.name == target.computer)
        .flat_map(|machine| machine.agents.iter())
        .find(|item| item.id == target.item && live(item))
    else {
        return protect(Redirect::to(&format!("{PAGE}?said=gone")).into_response());
    };
    let csrf = crate::pages::chat::csrf(&app, &owner);
    let body = message_markup(item, &target.computer, &csrf);
    page(
        &headers,
        service,
        &viewer,
        "Message an agent",
        PAGE,
        false,
        body,
    )
}

/// Whether `supplied` is this owner's form token, compared in constant time.
fn token_fits(app: &App, owner: &str, supplied: &str) -> bool {
    let expected = crate::pages::chat::csrf(app, owner);
    expected.len() == supplied.len()
        && expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[derive(Deserialize)]
struct ActForm {
    csrf: String,
    computer: String,
    item: String,
    #[serde(default)]
    text: String,
}

/// Queue `action` for an agent, the way the phone's Stop and Message do;
/// the code the list then explains.
async fn act(app: &App, owner: &str, form: &ActForm, action: &str) -> &'static str {
    let computer = line(&form.computer, 64);
    if computer.is_empty() || form.item.is_empty() || form.item.len() > 128 {
        return "gone";
    }
    let text = form.text.trim();
    if action == "message" {
        if text.is_empty() {
            return "empty";
        }
        if text.chars().count() > MAX_AGENT_MESSAGE {
            return "long";
        }
        if secret_screen::credential_in(text).is_some() {
            return "secret";
        }
    }
    let command = Command {
        id: crate::cloud::byo::fresh_request(),
        item: form.item.clone(),
        action: action.to_owned(),
        question: None,
        text: (action == "message").then(|| text.to_owned()),
    };
    match phone_api::queue_action(&app.config.chat_store, owner, &computer, command).await {
        Ok(Acted::Queued) if action == "stop" => "stopping",
        Ok(Acted::Queued) => "sent",
        Ok(Acted::Unknown) => "gone",
        Ok(Acted::Offline) => "offline",
        Err(_) => "failed",
    }
}

async fn acted(
    app: &App,
    headers: &HeaderMap,
    form: Result<Form<ActForm>, FormRejection>,
    action: &str,
) -> Response {
    let (_, viewer) = match viewer(app, headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let code = match form {
        Ok(Form(form)) if token_fits(app, &owner, &form.csrf) => {
            act(app, &owner, &form, action).await
        }
        _ => "failed",
    };
    protect(Redirect::to(&format!("{PAGE}?said={code}")).into_response())
}

async fn stop_route(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ActForm>, FormRejection>,
) -> Response {
    acted(&app, &headers, form, "stop").await
}

async fn message_route(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ActForm>, FormRejection>,
) -> Response {
    acted(&app, &headers, form, "message").await
}

/// The Settings row that leads here, once a computer reports an agent.
pub(crate) async fn settings_row(store: &Store, owner: &str) -> Markup {
    let found = machines(store, owner).await;
    if found.is_empty() {
        return html! {};
    }
    let working = found
        .iter()
        .flat_map(|machine| machine.agents.iter())
        .filter(|item| live(item))
        .count();
    html! {
        div class="oa-settings" {
            section class="oa-settings-group" aria-labelledby="settings-agents" {
                h2 #settings-agents { "Agents" }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { a href=(PAGE) { "Agents on your computers and cloud environments" } }
                        span class="oa-settings-hint" { (working) " working" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "agents_page_tests.rs"]
mod tests;
