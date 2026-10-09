//! Signed source and runtime selections for the shared chat composer.
//!
//! Public GitHub metadata supplies source identities. A native runtime remains
//! an independently admitted catalog entry; choosing it dispatches no work.

use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Form, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use maud::{Markup, Render, html};
use openagents_ui::shell::{ComposerDropdown, ComposerPanel, HxGet};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;

use crate::{
    App,
    chat_store::{RepositorySource, RuntimeSelection, Selection},
};

const MAX_SAFE_REVISION: u64 = 9_007_199_254_740_991;
const MAX_TOKEN: usize = 6 * 1024;
const MAX_PAYLOAD: usize = 4 * 1024;
const MAX_METADATA: usize = 512 * 1024;
const INCLUDE: &str = "#composer-state,#chat-selected,[name=csrf][form=chat-form]";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeChoice {
    pub runtime: RuntimeSelection,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub template: Option<String>,
    pub size: String,
    pub available: bool,
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/composer/{kind}", get(show).post(select))
        .layer(DefaultBodyLimit::max(32 * 1024))
}

fn response(markup: Markup) -> Response {
    crate::chat_html::protect(Html(markup.into_string()).into_response())
}

fn refused(status: StatusCode, message: &'static str) -> Response {
    crate::chat_html::protect(
        (
            status,
            Html(
                html! {
                    p role="alert" { (message) }
                }
                .into_string(),
            ),
        )
            .into_response(),
    )
}

fn signature(app: &App, owner: &str, domain: &[u8], payload: &[u8]) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&app.config.ask_salt).expect("HMAC accepts 32 bytes");
    mac.update(domain);
    mac.update(owner.as_bytes());
    mac.update(b":");
    mac.update(payload);
    mac
}

fn encode(app: &App, owner: &str, domain: &[u8], value: &impl Serialize) -> String {
    let payload = serde_json::to_vec(value).expect("composer values serialize");
    assert!(
        payload.len() <= MAX_PAYLOAD,
        "composer state exceeds its bound"
    );
    let tag = signature(app, owner, domain, &payload)
        .finalize()
        .into_bytes();
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(tag)
    )
}

fn decode<T: DeserializeOwned>(
    app: &App,
    owner: &str,
    domain: &[u8],
    token: &str,
) -> Result<T, Response> {
    if token.len() > MAX_TOKEN {
        return Err(refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        ));
    }
    let Some((payload, tag)) = token.split_once('.') else {
        return Err(refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        ));
    };
    let payload = URL_SAFE_NO_PAD.decode(payload).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        )
    })?;
    let tag = URL_SAFE_NO_PAD.decode(tag).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        )
    })?;
    if payload.len() > MAX_PAYLOAD
        || signature(app, owner, domain, &payload)
            .verify_slice(&tag)
            .is_err()
    {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "This page is out of date. Reload it.",
        ));
    }
    serde_json::from_slice(&payload).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        )
    })
}

pub(crate) fn seal(app: &App, owner: &str, selection: &Selection) -> String {
    encode(app, owner, b"openagents.web.composer.state.v1:", selection)
}

pub(crate) fn state(app: &App, owner: &str, token: &str) -> Result<Selection, Response> {
    if token.is_empty() {
        return Ok(Selection::default());
    }
    let selection: Selection = decode(app, owner, b"openagents.web.composer.state.v1:", token)?;
    validate_selection(&selection).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        )
    })?;
    Ok(selection)
}

fn validate_selection(selection: &Selection) -> Result<(), ()> {
    if selection.revision > MAX_SAFE_REVISION {
        return Err(());
    }
    selection.validate().map_err(|_| ())
}

fn sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn revision_prefix(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

pub(crate) fn state_field(app: &App, owner: &str, selection: &Selection, oob: bool) -> Markup {
    html! { input id="composer-state" name="selection" type="hidden" form="chat-form"
    value=(seal(app, owner, selection)) hx-swap-oob=[oob.then_some("outerHTML")]; }
}

pub(crate) fn controls(selection: &Selection, oob: bool) -> Markup {
    let repository = selection
        .repository
        .as_ref()
        .map(|source| source.repository.as_str())
        .unwrap_or("Repository");
    let branch = selection
        .repository
        .as_ref()
        .map(|source| {
            if source.branch == source.revision {
                "Pinned revision"
            } else {
                source.branch.as_str()
            }
        })
        .unwrap_or("Branch");
    let environment = selection
        .runtime
        .as_ref()
        .map(|runtime| runtime.profile.as_str())
        .unwrap_or("Web answers");
    html! {
        div id="composer-controls" class="oa-composer-selector-group" hx-swap-oob=[oob.then_some("outerHTML")] {
            @for (kind, label, selected) in [("repository", "Repository", repository), ("branch", "Branch", branch), ("environment", "Environment", environment)] {
                (ComposerDropdown::new(label, selected).hx(load(kind)))
            }
        }
    }
}

/// A request that loads the `kind` panel into `#composer-panel`.
pub(crate) fn load(kind: &str) -> HxGet {
    HxGet::new(format!("/composer/{kind}"))
        .include(INCLUDE)
        .target("#composer-panel")
        .swap("innerHTML")
        .sync("#composer-panel:replace")
}

#[derive(Default, Deserialize)]
struct Input {
    #[serde(default)]
    selection: String,
    #[serde(alias = "chat_id")]
    chat: Option<String>,
    csrf: Option<String>,
}

#[derive(Deserialize)]
struct Submit {
    selection: String,
    csrf: String,
    #[serde(alias = "chat_id")]
    chat: Option<String>,
    value: String,
}

pub(crate) fn panel(title: &str, content: Markup) -> Markup {
    ComposerPanel::new(title)
        .close(
            HxGet::new("/composer/close")
                .target("#composer-panel")
                .swap("innerHTML")
                .sync("#composer-panel:replace"),
        )
        .close_label("Close selection")
        .body(content)
        .render()
}

fn fields(app: &App, owner: &str, selection: &Selection, chat: Option<&str>) -> Markup {
    html! {
        input type="hidden" name="selection" value=(seal(app, owner, selection));
        input type="hidden" name="csrf" value=(crate::pages::chat::csrf(app, owner));
        @if let Some(chat) = chat { input type="hidden" name="chat" value=(chat); }
    }
}

fn native_token(app: &App, owner: &str, choice: &RuntimeChoice) -> String {
    format!(
        "n.{}",
        encode(app, owner, b"openagents.web.composer.runtime.v1:", choice)
    )
}

/// What a chat says when it reaches for a connected computer.
pub(crate) const RUNTIME_GONE: &str =
    "Running code on a connected computer isn't available from chat anymore.";

/// Connected-computer runtimes left with the Cloud pages
/// (docs/web/cloud-reset.md); Environments replaces them, so none are offered.
#[allow(clippy::unused_async)]
async fn choices(_app: &App, _headers: &HeaderMap) -> Result<Vec<RuntimeChoice>, Response> {
    Ok(Vec::new())
}

/// A sealed selection that still names a runtime is refused.
fn runtime_gone(_runtime: &RuntimeSelection) -> Result<(), Response> {
    Err(refused(StatusCode::GONE, RUNTIME_GONE))
}

async fn show(
    State(app): State<App>,
    headers: HeaderMap,
    Path(kind): Path<String>,
    Query(input): Query<Input>,
) -> Response {
    if kind == "close" {
        return response(html! {});
    }
    if !matches!(
        kind.as_str(),
        "repository" | "branch" | "environment" | "context" | "model" | "voice"
    ) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(owner) = crate::pages::chat::reader(&app, &headers).await else {
        return refused(
            StatusCode::FORBIDDEN,
            "Open the homepage before choosing a source.",
        );
    };
    let selection = match state(&app, &owner, &input.selection) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Some(runtime) = &selection.runtime
        && let Err(response) = runtime_gone(runtime)
    {
        return response;
    }
    if let Some(chat) = &input.chat {
        let loaded = match crate::pages::chat::load(&app, &headers, chat).await {
            Ok(value) => value,
            Err(response) => return response,
        };
        if loaded.conversation.selection.clone().unwrap_or_default() != selection {
            return response(panel(
                "Selection changed",
                html! { p role="alert" { "This chat changed. Open it again to pick a repository." } },
            ));
        }
    }
    let choices = match choices(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let content = match kind.as_str() {
        "repository" => repository_panel(&app, &owner, &selection, input.chat.as_deref(), &choices),
        "environment" => {
            environment_panel(&app, &owner, &selection, input.chat.as_deref(), &choices)
        }
        "branch" => {
            branch_panel(
                &app,
                &headers,
                &owner,
                &selection,
                input.chat.as_deref(),
                &choices,
            )
            .await
        }
        "context" => context_panel(&selection),
        "model" => model_panel(&selection),
        _ => panel(
            "Voice input",
            html! { p { "Voice input doesn't work in this browser. Type your message instead." } },
        ),
    };
    let _ = input.csrf;
    response(content)
}

fn picker_link(kind: &str, label: &str) -> Markup {
    html! {
        button type="button" class="oa-composer-choice" hx-get=(format!("/composer/{kind}"))
            hx-include=(INCLUDE) hx-target="#composer-panel" hx-swap="innerHTML"
            hx-sync="#composer-panel:replace" { (label) }
    }
}

fn context_panel(selection: &Selection) -> Markup {
    panel(
        "Context",
        html! {
            @if let Some(source) = &selection.repository {
                dl class="oa-composer-details" {
                    dt { "Repository" } dd { (source.repository) }
                    dt { "Branch" } dd { (if source.branch == source.revision { "Fixed commit" } else { &source.branch }) }
                    dt { "Commit" } dd { code { (source.revision) } }
                }
            } @else {
                p { "No repository selected." }
            }
            (picker_link("repository", "Choose repository"))
        },
    )
}

fn model_panel(selection: &Selection) -> Markup {
    panel(
        "Model",
        html! {
            @if let Some(runtime) = &selection.runtime {
                p { "This environment uses " strong { (runtime.model.as_deref().unwrap_or("its default model")) } "." }
            } @else {
                p { strong { "Auto" } " picks a model for you." }
            }
            (picker_link("environment", "Choose environment"))
        },
    )
}

fn repository_panel(
    app: &App,
    owner: &str,
    selection: &Selection,
    chat: Option<&str>,
    choices: &[RuntimeChoice],
) -> Markup {
    panel(
        "Repository",
        html! {
            p { "Pick a public GitHub repository." }
            form action="/composer/repository" method="post" hx-post="/composer/repository" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                (fields(app, owner, selection, chat))
                label for="composer-repository" { "Public GitHub repository" }
                div class="oa-composer-entry" {
                    input id="composer-repository" name="value" placeholder="owner/repository" maxlength="140" required
                        value=[selection.repository.as_ref().map(|source| source.repository.as_str())];
                    button type="submit" { "Select" }
                }
            }
            @if !choices.is_empty() {
                h3 { "On your computer" }
                @for choice in choices.iter().filter(|choice| choice.repository.is_some()) {
                    form action="/composer/repository" method="post" hx-post="/composer/repository" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                        (fields(app, owner, selection, chat))
                        button type="submit" name="value" value=(native_token(app, owner, choice)) class="oa-composer-choice" disabled[!choice.available] {
                            strong { (choice.repository.as_deref().unwrap_or_default()) }
                            small { (choice.branch.as_deref().unwrap_or("Fixed commit")) " · " (revision_prefix(&choice.runtime.source_revision)) }
                            small { "Environment: " (choice.runtime.profile) }
                            @if !choice.available { small { "Unavailable" } }
                        }
                    }
                }
            }
            form action="/composer/repository" method="post" hx-post="/composer/repository" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                (fields(app, owner, selection, chat)) button type="submit" name="value" value="none" class="oa-composer-choice" { "No repository" }
            }
        },
    )
}

fn source_from(choice: &RuntimeChoice) -> Option<RepositorySource> {
    let repository = choice.repository.clone()?;
    coder_access::cloud::repository(&repository).ok()?;
    let branch = choice
        .branch
        .clone()
        .unwrap_or_else(|| choice.runtime.source_revision.clone());
    coder_access::cloud::branch(&branch).ok()?;
    sha(&choice.runtime.source_revision).then(|| RepositorySource {
        repository,
        branch,
        revision: choice.runtime.source_revision.clone(),
    })
}

fn matches_source(choice: &RuntimeChoice, source: &RepositorySource) -> bool {
    choice
        .repository
        .as_ref()
        .is_some_and(|repository| repository.eq_ignore_ascii_case(&source.repository))
        && choice.runtime.source_revision == source.revision
        && choice
            .branch
            .as_ref()
            .is_none_or(|branch| branch == &source.branch)
}

async fn branch_panel(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    selection: &Selection,
    chat: Option<&str>,
    choices: &[RuntimeChoice],
) -> Markup {
    let Some(source) = &selection.repository else {
        return panel(
            "Branch",
            html! { p { "Choose a repository first." } button type="button" class="oa-composer-choice" hx-get="/composer/repository" hx-include=(INCLUDE) hx-target="#composer-panel" hx-swap="innerHTML" { "Choose repository" } },
        );
    };
    let native: Vec<_> = choices
        .iter()
        .filter(|choice| {
            choice
                .repository
                .as_ref()
                .is_some_and(|repository| repository.eq_ignore_ascii_case(&source.repository))
        })
        .collect();
    if !native.is_empty() {
        return panel(
            "Branch",
            html! {
                p { "Branches on your connected computer." }
                @for choice in native {
                    form action="/composer/branch" method="post" hx-post="/composer/branch" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                        (fields(app, owner, selection, chat))
                        button type="submit" name="value" value=(native_token(app, owner, choice)) class="oa-composer-choice" disabled[!choice.available] {
                            strong { (choice.branch.as_deref().unwrap_or("Fixed commit")) }
                            small { (revision_prefix(&choice.runtime.source_revision)) " · " (choice.runtime.profile) }
                            @if !choice.available { small { "Unavailable" } }
                        }
                    }
                }
            },
        );
    }
    if selection.runtime.is_some() {
        return panel(
            "Branch",
            html! {
                p { "That repository isn't available anymore. Pick another one." }
                (picker_link("repository", "Choose repository"))
            },
        );
    }
    let (branches, more) =
        match public_branches(&Reader::new(app, headers).await, &source.repository).await {
            Ok(value) => value,
            Err(message) => {
                return panel(
                    "Branch",
                    html! { p role="alert" { (message) } p { "Nothing changed." } },
                );
            }
        };
    panel(
        "Branch",
        html! {
            p { (source.repository) }
            @for branch in branches {
                form action="/composer/branch" method="post" hx-post="/composer/branch" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                    (fields(app, owner, selection, chat))
                    button type="submit" name="value" value=(branch.name) class="oa-composer-choice" {
                        strong { (branch.name) } small { (revision_prefix(&branch.commit.sha)) }
                    }
                }
            }
            @if more { p class="oa-composer-note" { "Showing the first 100 branches. Type a branch name to pick another." } }
            form action="/composer/branch" method="post" hx-post="/composer/branch" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                (fields(app, owner, selection, chat))
                label for="composer-branch" { "Branch name" }
                div class="oa-composer-entry" { input id="composer-branch" name="value" maxlength="256" required; button type="submit" { "Select" } }
            }
        },
    )
}

fn environment_panel(
    app: &App,
    owner: &str,
    selection: &Selection,
    chat: Option<&str>,
    choices: &[RuntimeChoice],
) -> Markup {
    panel(
        "Environment",
        html! {
            form action="/composer/environment" method="post" hx-post="/composer/environment" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                (fields(app, owner, selection, chat))
                button type="submit" name="value" value="none" class="oa-composer-choice" {
                    strong { "Web answers" } small { "Questions and conversation, no repository." }
                }
            }
            @for choice in choices {
                @let compatible = selection.repository.as_ref().is_none_or(|source| matches_source(choice, source));
                form action="/composer/environment" method="post" hx-post="/composer/environment" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                    (fields(app, owner, selection, chat))
                    button type="submit" name="value" value=(native_token(app, owner, choice)) class="oa-composer-choice" disabled[!choice.available || !compatible] {
                        strong { (choice.runtime.profile) }
                        small { (choice.runtime.placement) " · " (choice.size) " · " (choice.runtime.executor) }
                        small { "Source " (revision_prefix(&choice.runtime.source_revision)) " · model " (choice.runtime.model.as_deref().unwrap_or("default")) }
                        @if let Some(template) = &choice.template { small { "Image: " (template) } }
                        @if !choice.available { small { "Unavailable" } }
                        @if !compatible { small { "Doesn't match the selected repository" } }
                    }
                }
            }
            @if choices.is_empty() { p { "No environments yet." } }
        },
    )
}

async fn selected_choice(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    value: &str,
    choices: &[RuntimeChoice],
) -> Result<RuntimeChoice, &'static str> {
    let token = value
        .strip_prefix("n.")
        .ok_or("That environment isn't available anymore. Pick another one.")?;
    let original: RuntimeChoice = decode(app, owner, b"openagents.web.composer.runtime.v1:", token)
        .map_err(|_| "This list is out of date. Open it again.")?;
    let choice = choices
        .iter()
        .find(|choice| **choice == original)
        .cloned()
        .ok_or("This list is out of date. Open it again.")?;
    runtime_gone(&choice.runtime).map_err(|_| RUNTIME_GONE)?;
    Ok(choice)
}

async fn select(
    State(app): State<App>,
    headers: HeaderMap,
    Path(kind): Path<String>,
    Form(form): Form<Submit>,
) -> Response {
    if !matches!(kind.as_str(), "repository" | "branch" | "environment") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let owner = match crate::pages::chat::validate_form(&app, &headers, &form.csrf).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let previous = match state(&app, &owner, &form.selection) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Some(runtime) = &previous.runtime
        && let Err(response) = runtime_gone(runtime)
    {
        return response;
    }
    let mut loaded = if let Some(chat) = &form.chat {
        match crate::pages::chat::load(&app, &headers, chat).await {
            Ok(value) => Some(value),
            Err(response) => return response,
        }
    } else {
        None
    };
    if loaded
        .as_ref()
        .is_some_and(|loaded| loaded.conversation.selection.clone().unwrap_or_default() != previous)
    {
        return response(panel(
            "Selection changed",
            html! { p role="alert" { "This chat changed. Open it again to pick a repository." } },
        ));
    }
    let choices = match choices(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut next = previous.clone();
    let value = form.value.trim();
    let change: Result<(), &'static str> = async {
        match kind.as_str() {
            "repository" if value == "none" => {
                next.repository = None;
                next.runtime = None;
            }
            "repository" | "branch" if value.starts_with("n.") => {
                let choice = selected_choice(&app, &headers, &owner, value, &choices).await?;
                let source = source_from(&choice).ok_or("This environment has no repository.")?;
                if kind == "branch"
                    && previous
                        .repository
                        .as_ref()
                        .is_none_or(|old| !old.repository.eq_ignore_ascii_case(&source.repository))
                {
                    return Err("Pick a branch from the selected repository.");
                }
                next.repository = Some(source);
                next.runtime = Some(choice.runtime);
            }
            "repository" => {
                next.repository =
                    Some(public_source(&Reader::new(&app, &headers).await, value, None).await?);
            }
            "branch" => {
                let repository = previous
                    .repository
                    .as_ref()
                    .ok_or("Choose a repository first.")?;
                if choices.iter().any(|choice| {
                    choice
                        .repository
                        .as_ref()
                        .is_some_and(|value| value.eq_ignore_ascii_case(&repository.repository))
                }) {
                    return Err("Pick one of the branches on your connected computer.");
                }
                next.repository = Some(
                    public_source(
                        &Reader::new(&app, &headers).await,
                        &repository.repository,
                        Some(value),
                    )
                    .await?,
                );
            }
            "environment" if value == "none" => {
                if previous.runtime.is_some() {
                    next.repository = None;
                }
                next.runtime = None;
            }
            "environment" => {
                let choice = selected_choice(&app, &headers, &owner, value, &choices).await?;
                if !choice.available {
                    return Err("This environment is unavailable.");
                }
                if next
                    .repository
                    .as_ref()
                    .is_some_and(|source| !matches_source(&choice, source))
                {
                    return Err("This environment doesn't match the selected repository.");
                }
                if next.repository.is_none() {
                    next.repository = source_from(&choice);
                }
                next.runtime = Some(choice.runtime);
            }
            _ => return Err("This option is unavailable."),
        }
        if kind != "environment"
            && next.runtime.as_ref().is_some_and(|runtime| {
                !choices.iter().any(|choice| {
                    &choice.runtime == runtime
                        && next
                            .repository
                            .as_ref()
                            .is_some_and(|source| matches_source(choice, source))
                })
            })
        {
            next.runtime = None;
        }
        Ok(())
    }
    .await;
    if let Err(message) = change {
        return response(panel(
            "Selection unchanged",
            html! { p role="alert" { (message) } },
        ));
    }
    next.revision = match previous
        .revision
        .checked_add(1)
        .filter(|revision| *revision <= MAX_SAFE_REVISION)
    {
        Some(value) => value,
        None => {
            return refused(
                StatusCode::CONFLICT,
                "Something went wrong. Start a new chat.",
            );
        }
    };
    if validate_selection(&next).is_err() {
        return refused(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        );
    }
    let mut ticket = html! {};
    if let Some(current) = loaded.take() {
        let mut conversation = current.conversation.clone();
        conversation.selection = Some(next.clone());
        conversation.revision = match conversation.revision.checked_add(1) {
            Some(revision) => revision,
            None => {
                return refused(
                    StatusCode::CONFLICT,
                    "Something went wrong. Start a new chat.",
                );
            }
        };
        conversation.updated_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let saved = match app
            .config
            .chat_store
            .compare_and_swap(&current, &conversation)
            .await
        {
            Ok(value) => value,
            Err(_) => {
                return response(panel(
                    "Selection unchanged",
                    html! { p role="alert" { "This chat changed or couldn't be saved. Open it again to pick a repository." } },
                ));
            }
        };
        // The selectors are on the page: this request came from one.
        ticket = crate::pages::chat::ticket(&app, &saved.conversation, true);
    }
    response(html! {
        @if form.chat.is_some() { (ticket) } @else {
            (controls(&next, true))
            (state_field(&app, &owner, &next, true))
        }
        p class="oa-composer-result" role="status" { "Selection updated." }
    })
}

#[derive(Deserialize)]
struct Repository {
    full_name: String,
    default_branch: String,
    private: bool,
}
#[derive(Clone, Deserialize)]
struct Commit {
    sha: String,
}
#[derive(Clone, Deserialize)]
struct Branch {
    name: String,
    commit: Commit,
}

/// How this request reads GitHub: as the signed-in person when they
/// connected GitHub (their token, fetched from the account service for
/// this request and never kept here), else without a token. Reads without
/// a token share GitHub's limit of 60 an hour for this whole server.
struct Reader {
    base: String,
    token: Option<String>,
}

impl Reader {
    async fn new(app: &App, headers: &HeaderMap) -> Self {
        let base = app.config.github.as_ref().map_or_else(
            || "https://api.github.com".to_string(),
            |github| github.endpoints.api_url.trim_end_matches('/').to_string(),
        );
        let token = match app.config.cloud.as_deref() {
            Some(service) => service.github_token(headers).await.ok(),
            None => None,
        };
        Self { base, token }
    }

    fn url(&self, path: &[&str]) -> Result<reqwest::Url, &'static str> {
        let mut url = reqwest::Url::parse(&self.base).map_err(|_| UNREACHABLE)?;
        url.path_segments_mut()
            .map_err(|_| UNREACHABLE)?
            .pop_if_empty()
            .extend(path.iter().copied());
        Ok(url)
    }

    /// Whose branch lists these are: a digest of the token (a new
    /// connection starts afresh), or everyone reading without one; per
    /// GitHub API origin.
    fn group(&self) -> String {
        use sha2::Digest;
        let who = self.token.as_ref().map_or_else(
            || "anonymous".to_string(),
            |token| {
                Sha256::digest(token.as_bytes())[..12]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect()
            },
        );
        format!("{} {who}", self.base)
    }
}

const UNREACHABLE: &str = "Couldn't reach GitHub. Try again.";
/// GitHub's shared limit for reads without a sign-in is spent.
pub(crate) const LIMITED_ANONYMOUS: &str =
    "GitHub is limiting requests without a sign-in. Connect GitHub, or try again later.";
/// The person's own GitHub limit is spent.
pub(crate) const LIMITED: &str =
    "GitHub is limiting requests right now. Try again in a few minutes.";

/// Below this many anonymous reads left in the hour, kept branch lists
/// are served without reading GitHub again.
const LOW_ANONYMOUS: u64 = 10;
/// The last anonymous `x-ratelimit-remaining` and `x-ratelimit-reset`, per
/// GitHub API origin.
static ANONYMOUS_BUDGET: Mutex<BTreeMap<String, (u64, u64)>> = Mutex::new(BTreeMap::new());

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn anonymous_budget_low(base: &str) -> bool {
    ANONYMOUS_BUDGET
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(base)
        .is_some_and(|(remaining, reset)| *remaining < LOW_ANONYMOUS && *reset > unix_now())
}

async fn metadata<T: DeserializeOwned>(
    reader: &Reader,
    url: reqwest::Url,
) -> Result<(T, bool), &'static str> {
    static CLIENT: OnceLock<Result<reqwest::Client, reqwest::Error>> = OnceLock::new();
    let client = CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("OpenAgents-source-selector")
                .build()
        })
        .as_ref()
        .map_err(|_| UNREACHABLE)?;
    let mut request = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", oa_auth::github::API_VERSION);
    if let Some(token) = &reader.token {
        request = request.bearer_auth(token);
    }
    let mut response = request.send().await.map_err(|_| UNREACHABLE)?;
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .map(str::to_owned)
    };
    let number = |name: &str| header(name).and_then(|value| value.parse::<u64>().ok());
    let remaining = number("x-ratelimit-remaining");
    let retry_after = header("retry-after").is_some();
    let more = header("link").is_some_and(|value| value.contains("rel=\"next\""));
    if reader.token.is_none()
        && let (Some(remaining), Some(reset)) = (remaining, number("x-ratelimit-reset"))
    {
        let mut budgets = ANONYMOUS_BUDGET
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if budgets.len() >= 64 {
            budgets.clear();
        }
        budgets.insert(reader.base.clone(), (remaining, reset));
    }
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let limited = status == 429
            || (status == 403
                && (remaining == Some(0) || retry_after || {
                    let body = response.text().await.unwrap_or_default();
                    body.to_ascii_lowercase().contains("rate limit")
                }));
        if limited {
            return Err(if reader.token.is_some() {
                LIMITED
            } else {
                LIMITED_ANONYMOUS
            });
        }
        return Err(match status {
            404 => "That repository or branch wasn't found.",
            401 => "GitHub access ended. Connect GitHub again.",
            403 => "GitHub didn't allow access to that repository.",
            301 | 302 | 307 | 308 => "The repository moved. Enter its current owner and name.",
            500..=599 => "GitHub had a problem answering. Try again in a minute.",
            _ => UNREACHABLE,
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_METADATA as u64)
    {
        return Err("That repository is too big to list here.");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| UNREACHABLE)? {
        if bytes.len().saturating_add(chunk.len()) > MAX_METADATA {
            return Err("That repository is too big to list here.");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        serde_json::from_slice(&bytes)
            .map_err(|_| "GitHub sent something we couldn't read. Try again.")?,
        more,
    ))
}

async fn public_source(
    reader: &Reader,
    repository: &str,
    branch: Option<&str>,
) -> Result<RepositorySource, &'static str> {
    coder_access::cloud::repository(repository)
        .map_err(|_| "Enter a GitHub repository as owner/repository.")?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or("Enter a GitHub repository as owner/repository.")?;
    let (info, _): (Repository, _) = metadata(reader, reader.url(&["repos", owner, name])?).await?;
    if info.private || !info.full_name.eq_ignore_ascii_case(repository) {
        return Err("Only public repositories work here.");
    }
    coder_access::cloud::repository(&info.full_name)
        .map_err(|_| "That repository name isn't supported.")?;
    let branch = branch.unwrap_or(&info.default_branch);
    coder_access::cloud::branch(branch).map_err(|_| "That branch name isn't supported.")?;
    let (info_branch, _): (Branch, _) = metadata(
        reader,
        reader.url(&["repos", owner, name, "branches", branch])?,
    )
    .await?;
    if info_branch.name != branch || !sha(&info_branch.commit.sha) {
        return Err("GitHub sent something we couldn't read. Try again.");
    }
    Ok(RepositorySource {
        repository: info.full_name,
        branch: info_branch.name,
        revision: info_branch.commit.sha.to_ascii_lowercase(),
    })
}

/// Branch lists, per reader (see [`Reader::group`]) and repository, kept
/// the way repository pages are ([`oa_auth::cache`]: fresh for 5 minutes,
/// kept for an hour and served while one read refreshes them).
type BranchCache = oa_auth::cache::Lists<String, String, (Vec<Branch>, bool)>;

static BRANCHES: std::sync::LazyLock<BranchCache> =
    std::sync::LazyLock::new(oa_auth::cache::Lists::new);

async fn public_branches(
    reader: &Reader,
    repository: &str,
) -> Result<(Vec<Branch>, bool), &'static str> {
    coder_access::cloud::repository(repository).map_err(|_| "Choose a valid repository first.")?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or("Choose a valid repository first.")?;
    let mut url = reader.url(&["repos", owner, name, "branches"])?;
    url.query_pairs_mut().append_pair("per_page", "100");
    let may_refresh = reader.token.is_some() || !anonymous_budget_low(&reader.base);
    let read = Reader {
        base: reader.base.clone(),
        token: reader.token.clone(),
    };
    BRANCHES
        .get(
            reader.group(),
            repository.to_ascii_lowercase(),
            may_refresh,
            move || async move { read_branches(&read, url).await },
            UNREACHABLE,
        )
        .await
}

async fn read_branches(
    reader: &Reader,
    url: reqwest::Url,
) -> Result<(Vec<Branch>, bool), &'static str> {
    let (mut branches, mut more): (Vec<Branch>, bool) = metadata(reader, url).await?;
    if branches.len() > 100 {
        return Err("That repository has too many branches to list.");
    }
    let original = branches.len();
    branches.retain(|branch| {
        coder_access::cloud::branch(&branch.name).is_ok() && sha(&branch.commit.sha)
    });
    more |= branches.len() != original;
    Ok((branches, more))
}

/// The branch names of `repository` (at most 100, and whether GitHub has
/// more), read as the signed-in person when they connected GitHub and kept
/// like every branch list here ([`BRANCHES`]).
pub(crate) async fn branch_names(
    app: &App,
    headers: &HeaderMap,
    repository: &str,
) -> Result<(Vec<String>, bool), &'static str> {
    let (branches, more) = public_branches(&Reader::new(app, headers).await, repository).await?;
    Ok((
        branches.into_iter().map(|branch| branch.name).collect(),
        more,
    ))
}

#[cfg(test)]
pub(crate) fn age_branch_lists(by: Duration) {
    BRANCHES.age_where(by, |_| true);
}
