//! Signed source and runtime selections for the shared chat composer.
//!
//! Public GitHub metadata supplies source identities. A native runtime remains
//! an independently admitted catalog entry; choosing it dispatches no work.

use std::{
    sync::OnceLock,
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
            "The selection exceeds its limit. Reload this page.",
        ));
    }
    let Some((payload, tag)) = token.split_once('.') else {
        return Err(refused(
            StatusCode::BAD_REQUEST,
            "The selection ticket is invalid. Reload this page.",
        ));
    };
    let payload = URL_SAFE_NO_PAD.decode(payload).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "The selection ticket is invalid. Reload this page.",
        )
    })?;
    let tag = URL_SAFE_NO_PAD.decode(tag).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "The selection ticket is invalid. Reload this page.",
        )
    })?;
    if payload.len() > MAX_PAYLOAD
        || signature(app, owner, domain, &payload)
            .verify_slice(&tag)
            .is_err()
    {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "The selection ticket changed or belongs to another browser. Reload this page.",
        ));
    }
    serde_json::from_slice(&payload).map_err(|_| {
        refused(
            StatusCode::BAD_REQUEST,
            "The selection ticket is invalid. Reload this page.",
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
            "The selected source or runtime is invalid. Reload this page.",
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

/// Whether the composer shows the Repository, Branch and Environment
/// selectors. They act only when the visitor can choose an admitted Cloud
/// runtime (a signed-in workspace with a connected computer), or when the
/// chat already pins a selection, which they then show. A visitor without
/// one gets no selectors: their panels offered only "Web answers" and a
/// public repository name the answer service cannot read.
pub(crate) async fn selectors_shown(app: &App, headers: &HeaderMap, selection: &Selection) -> bool {
    if *selection != Selection::default() {
        return true;
    }
    matches!(choices(app, headers).await, Ok(choices) if !choices.is_empty())
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

fn panel(title: &str, content: Markup) -> Markup {
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

async fn choices(app: &App, headers: &HeaderMap) -> Result<Vec<RuntimeChoice>, Response> {
    match crate::cloud::composer::choices(app, headers).await {
        Ok(choices) => {
            if choices.iter().any(|choice| {
                choice.runtime.validate().is_err()
                    || choice.repository.as_ref().is_some_and(|repository| {
                        coder_access::cloud::repository(repository).is_err()
                    })
                    || choice
                        .branch
                        .as_ref()
                        .is_some_and(|branch| coder_access::cloud::branch(branch).is_err())
                    || choice.template.as_ref().is_some_and(|template| {
                        template.len() > 256 || template.chars().any(char::is_control)
                    })
                    || choice.size.len() > 128
                    || choice.size.chars().any(char::is_control)
            }) {
                return Err(refused(
                    StatusCode::BAD_GATEWAY,
                    "The native runtime catalog is invalid. Reopen its workspace.",
                ));
            }
            Ok(choices)
        }
        Err(response)
            if matches!(
                response.status(),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
            ) =>
        {
            Ok(Vec::new())
        }
        Err(response) => Err(response),
    }
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
    let Some(owner) = crate::ask::visitor(&headers) else {
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
        && let Err(response) = crate::cloud::composer::authorize(&app, &headers, runtime).await
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
                html! { p role="alert" { "This conversation changed. Reopen it before selecting another source." } },
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
        "branch" => branch_panel(&app, &owner, &selection, input.chat.as_deref(), &choices).await,
        "context" => context_panel(&selection),
        "model" => model_panel(&selection),
        _ => panel(
            "Voice input",
            html! { p { "Voice input is not available in this browser yet. Type your message in the composer." } },
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
                    dt { "Branch" } dd { (if source.branch == source.revision { "Pinned revision" } else { &source.branch }) }
                    dt { "Commit" } dd { code { (source.revision) } }
                }
            } @else {
                p { "No repository is selected. Your message supplies the conversation's context." }
            }
            (picker_link("repository", "Choose repository"))
            p class="oa-composer-note" { "File uploads are not available yet. Repository commands require an admitted execution runtime and a separate review." }
        },
    )
}

fn model_panel(selection: &Selection) -> Markup {
    panel(
        "Model",
        html! {
            @if let Some(runtime) = &selection.runtime {
                p { "The selected runtime uses " strong { (runtime.model.as_deref().unwrap_or("Native policy")) } "." }
                p { "The connected computer's profile selects its model." }
            } @else {
                p { strong { "Auto" } " uses the managed Web answer service." }
                p { "Select an admitted execution runtime to use its configured model." }
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
            p { "Select a public GitHub source or a repository admitted by your connected computer. An admitted repository also selects its configured runtime." }
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
                h3 { "Admitted repositories" }
                @for choice in choices.iter().filter(|choice| choice.repository.is_some()) {
                    form action="/composer/repository" method="post" hx-post="/composer/repository" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                        (fields(app, owner, selection, chat))
                        button type="submit" name="value" value=(native_token(app, owner, choice)) class="oa-composer-choice" disabled[!choice.available] {
                            strong { (choice.repository.as_deref().unwrap_or_default()) }
                            small { (choice.branch.as_deref().unwrap_or("Pinned revision")) " · " (revision_prefix(&choice.runtime.source_revision)) }
                            small { "Runtime: " (choice.runtime.profile) }
                            @if !choice.available { small { "Runtime unavailable" } }
                        }
                    }
                }
            }
            @if choices.is_empty() {
                p class="oa-composer-note" { "To select an admitted repository, open Cloud and choose a workspace with a connected computer." }
                a href="/cloud/app" { "Choose Cloud workspace" }
            }
            form action="/composer/repository" method="post" hx-post="/composer/repository" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                (fields(app, owner, selection, chat)) button type="submit" name="value" value="none" class="oa-composer-choice" { "No repository" }
            }
            p class="oa-composer-note" { "Public metadata does not connect a computer or authorize code execution." }
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
                p { "These source revisions are admitted by your connected computer." }
                @for choice in native {
                    form action="/composer/branch" method="post" hx-post="/composer/branch" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                        (fields(app, owner, selection, chat))
                        button type="submit" name="value" value=(native_token(app, owner, choice)) class="oa-composer-choice" disabled[!choice.available] {
                            strong { (choice.branch.as_deref().unwrap_or("Pinned revision")) }
                            small { (revision_prefix(&choice.runtime.source_revision)) " · " (choice.runtime.profile) }
                            small { "Selects this source and its configured runtime." }
                            @if !choice.available { small { "Runtime unavailable" } }
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
                p { "The selected native source is not in the current catalog. Choose a current admitted repository and runtime." }
                (picker_link("repository", "Choose repository"))
                a href="/cloud/app" { "Choose Cloud workspace" }
            },
        );
    }
    let (branches, more) = match public_branches(&source.repository).await {
        Ok(value) => value,
        Err(message) => {
            return panel(
                "Branch",
                html! { p role="alert" { (message) } p { "Your selection has not changed." } },
            );
        }
    };
    panel(
        "Branch",
        html! {
            p { (source.repository) " · selection pins the branch's current commit." }
            @for branch in branches {
                form action="/composer/branch" method="post" hx-post="/composer/branch" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                    (fields(app, owner, selection, chat))
                    button type="submit" name="value" value=(branch.name) class="oa-composer-choice" {
                        strong { (branch.name) } small { (revision_prefix(&branch.commit.sha)) }
                    }
                }
            }
            @if more { p class="oa-composer-note" { "This is a bounded page of up to 100 branches. Enter another branch below." } }
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
                    strong { "Web answers" } small { "Questions and conversation. Choosing Web answers clears an admitted repository's source pins." }
                }
            }
            @for choice in choices {
                @let compatible = selection.repository.as_ref().is_none_or(|source| matches_source(choice, source));
                form action="/composer/environment" method="post" hx-post="/composer/environment" hx-target="#composer-panel" hx-swap="innerHTML" hx-sync="#composer-panel:replace" {
                    (fields(app, owner, selection, chat))
                    button type="submit" name="value" value=(native_token(app, owner, choice)) class="oa-composer-choice" disabled[!choice.available || !compatible] {
                        strong { (choice.runtime.profile) }
                        small { (choice.runtime.placement) " · " (choice.size) " · " (choice.runtime.executor) }
                        small { "Source " (revision_prefix(&choice.runtime.source_revision)) " · model " (choice.runtime.model.as_deref().unwrap_or("Native policy")) }
                        @if let Some(template) = &choice.template { small { "Runtime image: " (template) } }
                        @if !choice.available { small { "Runtime unavailable" } }
                        @if !compatible { small { "Does not match the selected source" } }
                    }
                }
            }
            @if choices.is_empty() { p { "No execution runtime is admitted for this browser. Open Cloud and choose a workspace with a connected computer." } }
            p class="oa-composer-note" { "A runtime selection stages a separate native review when you send a message. Saved project-image preparation remains a separate operation." }
            a href="/cloud/app" { "Open connected computers and native profiles" }
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
        .ok_or("Choose a current admitted runtime.")?;
    let original: RuntimeChoice = decode(app, owner, b"openagents.web.composer.runtime.v1:", token)
        .map_err(|_| "The runtime ticket changed. Reopen its selector.")?;
    let choice = choices
        .iter()
        .find(|choice| **choice == original)
        .cloned()
        .ok_or("The runtime changed. Reopen its selector.")?;
    crate::cloud::composer::validate(app, headers, &choice.runtime)
        .await
        .map_err(|_| "The native runtime admission changed. Reopen the workspace.")?;
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
    let owner = match crate::pages::chat::validate_form(&app, &headers, &form.csrf) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let previous = match state(&app, &owner, &form.selection) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Some(runtime) = &previous.runtime
        && let Err(response) = crate::cloud::composer::authorize(&app, &headers, runtime).await
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
            html! { p role="alert" { "This conversation changed. Reopen it before choosing another source." } },
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
                let source = source_from(&choice)
                    .ok_or("This profile does not declare a repository source.")?;
                if kind == "branch"
                    && previous
                        .repository
                        .as_ref()
                        .is_none_or(|old| !old.repository.eq_ignore_ascii_case(&source.repository))
                {
                    return Err("Choose a branch from the currently selected repository.");
                }
                next.repository = Some(source);
                next.runtime = Some(choice.runtime);
            }
            "repository" => {
                next.repository = Some(public_source(value, None).await?);
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
                    return Err(
                        "Choose one of the source revisions admitted by the connected computer.",
                    );
                }
                next.repository = Some(public_source(&repository.repository, Some(value)).await?);
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
                    return Err("This native runtime is unavailable.");
                }
                if next
                    .repository
                    .as_ref()
                    .is_some_and(|source| !matches_source(&choice, source))
                {
                    return Err("The runtime does not match the selected repository revision.");
                }
                if next.repository.is_none() {
                    next.repository = source_from(&choice);
                }
                next.runtime = Some(choice.runtime);
            }
            _ => return Err("This selector is unavailable."),
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
                "This selection cannot advance. Start another conversation.",
            );
        }
    };
    if validate_selection(&next).is_err() {
        return refused(
            StatusCode::BAD_REQUEST,
            "The selected source or runtime is invalid.",
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
                    "This conversation cannot advance. Start another conversation.",
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
                    html! { p role="alert" { "This conversation changed or could not be saved. Reopen it before choosing another source." } },
                ));
            }
        };
        // The selectors are on the page: this request came from one.
        ticket = crate::pages::chat::ticket(&app, &saved.conversation, true, true);
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
#[derive(Deserialize)]
struct Commit {
    sha: String,
}
#[derive(Deserialize)]
struct Branch {
    name: String,
    commit: Commit,
}

fn github(path: &[&str]) -> Result<reqwest::Url, &'static str> {
    let mut url = reqwest::Url::parse("https://api.github.com")
        .map_err(|_| "GitHub metadata is unavailable.")?;
    url.path_segments_mut()
        .map_err(|_| "GitHub metadata is unavailable.")?
        .extend(path.iter().copied());
    Ok(url)
}

async fn metadata<T: DeserializeOwned>(url: reqwest::Url) -> Result<(T, bool), &'static str> {
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
        .map_err(|_| "GitHub metadata is unavailable.")?;
    let mut response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .send()
        .await
        .map_err(|_| "GitHub metadata is temporarily unavailable. Try again.")?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            404 => {
                "Public repository or branch not found. Private sources require an admitted computer profile."
            }
            403 | 429 => "GitHub metadata is temporarily limited. Try again later.",
            301 | 302 | 307 | 308 => "The repository moved. Enter its current owner and name.",
            _ => "GitHub metadata is temporarily unavailable. Try again.",
        });
    }
    let more = response
        .headers()
        .get("link")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("rel=\"next\""));
    if response
        .content_length()
        .is_some_and(|length| length > MAX_METADATA as u64)
    {
        return Err("GitHub metadata exceeds the selector's bound.");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "GitHub metadata was interrupted. Try again.")?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_METADATA {
            return Err("GitHub metadata exceeds the selector's bound.");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        serde_json::from_slice(&bytes).map_err(|_| "GitHub returned invalid source metadata.")?,
        more,
    ))
}

async fn public_source(
    repository: &str,
    branch: Option<&str>,
) -> Result<RepositorySource, &'static str> {
    coder_access::cloud::repository(repository)
        .map_err(|_| "Enter a GitHub repository as owner/repository.")?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or("Enter a GitHub repository as owner/repository.")?;
    let (info, _): (Repository, _) = metadata(github(&["repos", owner, name])?).await?;
    if info.private || !info.full_name.eq_ignore_ascii_case(repository) {
        return Err("Only public metadata is available for this repository.");
    }
    coder_access::cloud::repository(&info.full_name)
        .map_err(|_| "GitHub returned an unsupported repository name.")?;
    let branch = branch.unwrap_or(&info.default_branch);
    coder_access::cloud::branch(branch).map_err(|_| "This branch name is unsupported.")?;
    let (info_branch, _): (Branch, _) =
        metadata(github(&["repos", owner, name, "branches", branch])?).await?;
    if info_branch.name != branch || !sha(&info_branch.commit.sha) {
        return Err("GitHub returned an invalid branch revision.");
    }
    Ok(RepositorySource {
        repository: info.full_name,
        branch: info_branch.name,
        revision: info_branch.commit.sha.to_ascii_lowercase(),
    })
}

async fn public_branches(repository: &str) -> Result<(Vec<Branch>, bool), &'static str> {
    coder_access::cloud::repository(repository).map_err(|_| "Choose a valid repository first.")?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or("Choose a valid repository first.")?;
    let mut url = github(&["repos", owner, name, "branches"])?;
    url.query_pairs_mut().append_pair("per_page", "100");
    let (mut branches, mut more): (Vec<Branch>, bool) = metadata(url).await?;
    if branches.len() > 100 {
        return Err("GitHub returned too many branches.");
    }
    let original = branches.len();
    branches.retain(|branch| {
        coder_access::cloud::branch(&branch.name).is_ok() && sha(&branch.commit.sha)
    });
    more |= branches.len() != original;
    Ok((branches, more))
}
