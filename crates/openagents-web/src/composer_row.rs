//! The selector row above the chat composer: **Project**, **Branch**, and
//! **Where it runs** (`docs/web/sidebar.md`, "Composer selector row").
//!
//! Each selector shows only when it can work for this person on this
//! server, so a signed-out visitor sees the plain composer:
//!
//! - **Project**: signed in. The person's projects (connected GitHub
//!   repositories, [`crate::projects`]) and No project; with none, the
//!   panel's one item is Connect a GitHub repository (`/projects`).
//! - **Branch**: a project is picked. That repository's branches, read from
//!   GitHub as the person ([`crate::composer::branch_names`], kept); the
//!   default branch is picked first.
//! - **Where it runs**: more than Chat is possible. **Chat** answers here.
//!   **Claude Code in {repository name} vN** when the picked project's repository
//!   has a saved environment of the person's, Claude Code can run on this
//!   server (a key), and the request may do agent work, like
//!   `/environments` ([`crate::agent_work`], [`crate::pages::chat_work`]). **Coder on {computer}** for a new chat
//!   when Coder with sync on checked in from that computer lately
//!   ([`crate::coder_sync`]).
//!
//! Claude Code in an environment is offered even before the person has
//! connected Claude (#11234), when this server keeps their own key
//! ([`crate::cloud::byo`]): picking it then shows one card right in the
//! row, **Use a key**, which opens Settings, Claude, instead of an error
//! after the message is sent. A message sent anyway is refused with the
//! same words and stays in the box ([`CONNECT`]).
//!
//! The row carries its picks as three fields of `#chat-form` (`project`,
//! `branch`, `target`), so a plain form post sends them too. The dropdowns
//! load their choices into `#composer-panel` (`GET /composer/row/{kind}`);
//! a choice reloads the row (`GET /composer/row`) with focus back on its
//! dropdown. Nothing is trusted from the fields: sending checks them again
//! ([`checked`]) and refuses a pick that no longer works instead of
//! answering some other way.

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, html};
use oa_auth::repos::{Access, Project};
use openagents_ui::icons::Icon;
use openagents_ui::shell::{ComposerDropdown, HxGet};
use serde::Deserialize;

use crate::App;

/// Where the row reloads from.
pub(crate) const ROW: &str = "/composer/row";
/// What a panel request carries: the row's fields and the open chat.
const INCLUDE: &str = "#composer-row input,#chat-selected";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(ROW, get(reload))
        .route("/composer/row/{kind}", get(choose))
}

/// The row's fields as a form or a query sends them.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Wanted {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub target: String,
    /// The chat the row belongs to; none on the home page.
    #[serde(default)]
    pub chat: Option<String>,
    /// The dropdown to focus after a choice.
    #[serde(default)]
    pub focus: Option<String>,
}

/// Where a message runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// Answered here.
    Chat,
    /// Claude Code in this saved environment (its id).
    Claude(String),
    /// Coder on this computer (its name).
    Coder(String),
}

impl Target {
    pub(crate) fn parse(value: &str) -> Self {
        let value = value.trim();
        if let Some(id) = value.strip_prefix("claude:").filter(|id| !id.is_empty()) {
            Self::Claude(id.to_owned())
        } else if let Some(name) = value.strip_prefix("coder:").filter(|name| !name.is_empty()) {
            Self::Coder(name.to_owned())
        } else {
            Self::Chat
        }
    }

    pub(crate) fn value(&self) -> String {
        match self {
            Self::Chat => String::new(),
            Self::Claude(id) => format!("claude:{id}"),
            Self::Coder(name) => format!("coder:{name}"),
        }
    }
}

/// A saved environment Claude Code can run in now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Environment {
    pub id: String,
    pub repository: String,
    /// The branch the environment was set up on.
    pub branch: String,
    pub version: u64,
}

impl Environment {
    pub(crate) fn label(&self) -> String {
        // The repository's name only: the Project selector names it, and
        // the row stays compact.
        let name = self
            .repository
            .rsplit('/')
            .next()
            .unwrap_or(&self.repository);
        format!("Claude Code in {name} v{}", self.version)
    }
}

/// What this person can pick on this server, now.
#[derive(Clone, Debug, Default)]
pub(crate) struct Choices {
    pub projects: Vec<Project>,
    /// GitHub is connected (repositories can be added on `/projects`).
    pub connected: bool,
    /// Saved environments Claude Code can run in, newest first.
    pub environments: Vec<Environment>,
    /// Computers where Coder with sync on checked in lately, newest first
    /// (offered for a new chat only).
    pub computers: Vec<String>,
    /// Claude Code is offered in [`Self::environments`] but can't run until
    /// the person adds their Claude key (#11234): picking it shows the
    /// connect card ([`connect_card`]).
    pub claude_connect: bool,
    /// The person may connect their Claude subscription here (the
    /// subscription-token allowlist): the connect card leads with it and
    /// opens its dialog in place ([`crate::settings::subscription`]).
    pub claude_subscription: bool,
}

/// Why a message for Claude Code wasn't sent while Claude isn't connected
/// yet; the message stays in the box (#11234).
pub(crate) const CONNECT: Refused = "Claude Code needs your Claude key before it can run here. Add it in Settings, Claude, then send again: your message stays in the box.";

/// Where the connect card's button goes: Settings, Claude, at the key.
pub(crate) const CONNECT_KEY: &str = "/settings/claude#key";

/// What the row shows and a message sent now uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Picked {
    pub project: Option<Project>,
    pub branch: Option<String>,
    pub target: Target,
}

impl Choices {
    fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    /// The environment Claude Code runs in for `project`: the newest saved
    /// one for its repository.
    pub(crate) fn environment(&self, project: &Project) -> Option<&Environment> {
        self.environments
            .iter()
            .find(|env| env.repository.eq_ignore_ascii_case(&project.repository))
    }

    /// Where a message can run with `project` picked, Chat first.
    pub(crate) fn targets(&self, project: Option<&Project>) -> Vec<(Target, String)> {
        let mut targets = vec![(Target::Chat, "Chat".to_owned())];
        if let Some(env) = project.and_then(|project| self.environment(project)) {
            targets.push((Target::Claude(env.id.clone()), env.label()));
        }
        for computer in &self.computers {
            targets.push((
                Target::Coder(computer.clone()),
                format!("Coder on {computer}"),
            ));
        }
        targets
    }

    /// The picks that work, from what was asked: an unknown project is no
    /// project, a branch only with a project (its default when none was
    /// asked), and a place to run only when it is offered.
    pub(crate) fn resolve(&self, wanted: &Wanted) -> Picked {
        let project = self.project(wanted.project.trim()).cloned();
        let branch = project.as_ref().map(|project| {
            let asked = wanted.branch.trim();
            if !asked.is_empty() && coder_access::cloud::branch(asked).is_ok() {
                asked.to_owned()
            } else {
                project.default_branch.clone()
            }
        });
        let asked = Target::parse(&wanted.target);
        let target = if self
            .targets(project.as_ref())
            .iter()
            .any(|(target, _)| *target == asked)
        {
            asked
        } else {
            Target::Chat
        };
        Picked {
            project,
            branch,
            target,
        }
    }
}

/// What this request can pick, or `None` when nobody is signed in (or the
/// account service can't be reached): then there is no row. Coder
/// computers count only for a new chat (`new_chat`).
pub(crate) async fn choices(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    new_chat: bool,
) -> Option<Choices> {
    let sidebar = crate::projects::sidebar(app).await?;
    let connected = !matches!(sidebar.status.access, Access::None);
    let mut environments = Vec::new();
    let mut claude_connect = false;
    if let Some(studio) = app.config.environments.studio()
        && let Some(scope) = crate::agent_work::scope(app, headers).await
        // Not connected yet, but the person can add their own key here:
        // offered, with the connect card (#11234).
        && let Some(ready) = Some(crate::environments::claude_ready(app, studio, headers).await)
            .filter(|ready| *ready || app.config.cloud_byo.is_some())
    {
        claude_connect = !ready;
        environments = scope
            .rows(studio)
            .into_iter()
            .filter_map(|row| {
                Some(Environment {
                    version: row.saved?,
                    id: row.id,
                    repository: row.repository,
                    branch: row.branch,
                })
            })
            .collect();
    }
    let claude_subscription =
        claude_connect && crate::settings::subscription::offered(app, headers).await;
    let mut computers = Vec::new();
    if new_chat
        && crate::chat_store::is_account_owner(owner)
        && let Ok(seen) = app.config.chat_store.computers(owner).await
    {
        let mut online: Vec<(String, u64)> = seen
            .seen
            .iter()
            .filter(|(name, _)| crate::coder_sync::online(&seen, name))
            .map(|(name, at)| (name.clone(), *at))
            .collect();
        online.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        computers = online.into_iter().map(|(name, _)| name).collect();
    }
    Some(Choices {
        projects: sidebar.status.projects.clone(),
        connected,
        environments,
        computers,
        claude_connect,
        claude_subscription,
    })
}

/// Why a message's picks can't be used now.
pub(crate) type Refused = &'static str;

/// The picks a message is sent with, checked again now: a project that
/// isn't the person's, a branch its repository doesn't have, or a place to
/// run that isn't offered anymore is refused, never swapped for another.
pub(crate) async fn checked(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    wanted: &Wanted,
    new_chat: bool,
) -> Result<Picked, Refused> {
    let asked = Target::parse(&wanted.target);
    let Some(choices) = choices(app, headers, owner, new_chat).await else {
        if asked != Target::Chat {
            return Err("Sign in to run this somewhere else. Reload the page.");
        }
        return Ok(Picked {
            project: None,
            branch: None,
            target: Target::Chat,
        });
    };
    let picked = choices.resolve(wanted);
    if matches!(picked.target, Target::Claude(_)) && choices.claude_connect {
        return Err(CONNECT);
    }
    if !wanted.project.trim().is_empty() && picked.project.is_none() {
        return Err("That project isn't available anymore. Pick another one.");
    }
    if picked.target != asked {
        return Err(match asked {
            Target::Coder(_) => {
                "Coder on that computer isn't online now. Pick where it runs again."
            }
            _ => "Claude Code can't run there now. Pick where it runs again.",
        });
    }
    if let (Some(project), Some(branch)) = (&picked.project, &picked.branch)
        && *branch != project.default_branch
    {
        let (names, _) = crate::composer::branch_names(app, headers, &project.repository).await?;
        if !names.iter().any(|name| name == branch) {
            return Err("That branch isn't in the repository anymore. Pick another one.");
        }
    }
    Ok(picked)
}

fn query(picked: &Picked, chat: Option<&str>) -> Vec<(&'static str, String)> {
    let mut pairs = vec![
        (
            "project",
            picked
                .project
                .as_ref()
                .map(|project| project.id.clone())
                .unwrap_or_default(),
        ),
        ("branch", picked.branch.clone().unwrap_or_default()),
        ("target", picked.target.value()),
    ];
    if let Some(chat) = chat {
        pairs.push(("chat", chat.to_owned()));
    }
    pairs
}

fn href(path: &str, pairs: &[(&str, String)]) -> String {
    let encoded = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish();
    format!("{path}?{encoded}")
}

/// The row with `picked`: its three fields and the selectors this person
/// can use (none at all without `choices`). `focus` names the dropdown
/// that takes focus; `oob` replaces the page's row.
pub(crate) fn row(
    choices: Option<&Choices>,
    picked: &Picked,
    chat: Option<&str>,
    focus: Option<&str>,
    oob: bool,
) -> Markup {
    let load = |kind: &str| {
        HxGet::new(format!("{ROW}/{kind}"))
            .include(INCLUDE)
            .target("#composer-panel")
            .swap("innerHTML")
            .sync("#composer-panel:replace")
    };
    let pairs = query(picked, chat);
    html! {
        div #composer-row.oa-composer-selector-group hx-swap-oob=[oob.then_some("outerHTML")] {
            @if let Some(choices) = choices {
                @for (name, value) in &pairs[..3] {
                    input type="hidden" name=(name) value=(value) form="chat-form";
                }
                (ComposerDropdown::new(
                    "Project",
                    picked.project.as_ref().map_or("No project", |project| project.name.as_str()),
                )
                .icon(Icon::Folder)
                .hx(load("project"))
                .autofocus(focus == Some("project")))
                @if let Some(branch) = &picked.branch {
                    (ComposerDropdown::new("Branch", branch.as_str())
                        .icon(Icon::Branch)
                        .hx(load("branch"))
                        .autofocus(focus == Some("branch")))
                }
                @let targets = choices.targets(picked.project.as_ref());
                @if targets.len() > 1 {
                    // Short on the row (the Project selector names the
                    // repository); the panel says it in full.
                    @let label = match &picked.target {
                        Target::Claude(id) => choices
                            .environments
                            .iter()
                            .find(|env| env.id == *id)
                            .map_or_else(|| "Claude Code".to_owned(), |env| format!("Claude Code v{}", env.version)),
                        _ => targets
                            .iter()
                            .find(|(target, _)| *target == picked.target)
                            .map_or_else(|| "Chat".to_owned(), |(_, label)| label.clone()),
                    };
                    (ComposerDropdown::new("Where it runs", label)
                        .icon(match picked.target {
                            Target::Chat => Icon::Chat,
                            Target::Claude(_) => Icon::Code,
                            Target::Coder(_) => Icon::Terminal,
                        })
                        .hx(load("target"))
                        .autofocus(focus == Some("target")))
                }
                @if matches!(picked.target, Target::Claude(_)) && choices.claude_connect {
                    (connect_card(choices.claude_subscription, chat))
                }
            }
        }
    }
}

/// The card shown when Claude Code is picked before Claude is connected
/// (#11234): what it needs, in plain words, and the controls that work
/// from here. With `subscription` (the allowlist) it leads with Use your
/// Claude subscription, which loads the Settings, Claude dialog over the
/// page (`hx-get`, appended to the body, outside this form) and, without
/// script, opens the same steps as a page; it returns to `chat`.
pub(crate) fn connect_card(subscription: bool, chat: Option<&str>) -> Markup {
    use crate::settings::subscription::PATH;
    let back = chat.map_or_else(|| "/".to_owned(), |chat| format!("/chat/{chat}"));
    let page = href(PATH, &[("back", back.clone())]);
    let dialog = href(PATH, &[("part", "dialog".to_owned()), ("back", back)]);
    html! {
        div.oa-composer-connect #composer-connect role="status" {
            strong { "Connect Claude to run Claude Code" }
            @if subscription {
                span {
                    "Each run starts on a fresh computer, so it needs your Claude subscription, Anthropic API key, or cloud credential. Your message stays in the box."
                }
                a.oa-composer-connect-action href=(page) hx-get=(dialog) hx-target="body"
                    hx-swap="beforeend" aria-haspopup="dialog" {
                    "Use your Claude subscription"
                }
            } @else {
                span {
                    "Each run starts on a fresh computer, so it needs your Anthropic API key or cloud credential. Your message stays in the box."
                }
            }
            a.oa-composer-connect-action href=(CONNECT_KEY) { "Use a key" }
        }
    }
}

/// The row for the home page (`project` preselected from `/?project=`).
pub(crate) async fn home(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    project: Option<&str>,
) -> Option<Markup> {
    let choices = choices(app, headers, owner, true).await?;
    let picked = choices.resolve(&Wanted {
        project: project.unwrap_or_default().to_owned(),
        ..Wanted::default()
    });
    Some(row(Some(&choices), &picked, None, None, false))
}

/// What a chat's row starts with: its project and branch, and Claude Code
/// in its environment when its newest message started a run there.
pub(crate) fn wanted_for(chat: &crate::chat_store::Conversation) -> Wanted {
    use crate::chat_store::{Role, TaskKind};
    let last_user = chat.messages.iter().rposition(|m| m.role == Role::User);
    let target = match (chat.tasks.last(), &chat.environment, last_user) {
        (Some(task), Some(env), Some(at))
            if task.kind == TaskKind::Claude
                && !env.removed
                && task.environment == env.id
                && task.after_message == at + 1 =>
        {
            Target::Claude(env.id.clone()).value()
        }
        _ => String::new(),
    };
    Wanted {
        project: chat.project.clone().unwrap_or_default(),
        branch: chat.branch.clone().unwrap_or_default(),
        target,
        chat: Some(chat.id.clone()),
        focus: None,
    }
}

/// A chat's row (`oob` when another chat opens in place). Always the
/// element, empty for a person who can pick nothing, so the next chat
/// opened in place can fill it.
pub(crate) async fn for_chat(
    app: &App,
    headers: &HeaderMap,
    chat: &crate::chat_store::Conversation,
    oob: bool,
) -> Markup {
    let choices = choices(app, headers, &chat.owner, false).await;
    let wanted = wanted_for(chat);
    let picked = choices.as_ref().map_or(
        Picked {
            project: None,
            branch: None,
            target: Target::Chat,
        },
        |choices| choices.resolve(&wanted),
    );
    row(choices.as_ref(), &picked, Some(&chat.id), None, oob)
}

fn respond(markup: Markup) -> Response {
    crate::chat_html::protect(Html(markup.into_string()).into_response())
}

fn plain(status: StatusCode, message: &'static str) -> Response {
    crate::chat_html::protect(
        (
            status,
            Html(html! { p role="alert" { (message) } }.into_string()),
        )
            .into_response(),
    )
}

/// A choice reloads the row with the new picks and closes the panel.
async fn reload(
    State(app): State<App>,
    headers: HeaderMap,
    Query(wanted): Query<Wanted>,
) -> Response {
    let Some(owner) = crate::pages::chat::reader(&app, &headers).await else {
        return plain(StatusCode::FORBIDDEN, "Sign in to pick a project.");
    };
    let chat = wanted.chat.as_deref().filter(|id| !id.is_empty());
    let Some(choices) = choices(&app, &headers, &owner, chat.is_none()).await else {
        return plain(StatusCode::FORBIDDEN, "Sign in to pick a project.");
    };
    let picked = choices.resolve(&wanted);
    respond(html! {
        (row(Some(&choices), &picked, chat, wanted.focus.as_deref(), false))
        div #composer-panel hx-swap-oob="innerHTML" {}
    })
}

/// One choice in a panel: picking it reloads the row.
fn choice(url: String, title: &str, detail: Option<&str>, current: bool) -> Markup {
    html! {
        button type="button" class="oa-composer-choice" hx-get=(url)
            hx-target="#composer-row" hx-swap="outerHTML" hx-sync="#composer-panel:replace"
            aria-current=[current.then_some("true")] autofocus[current] {
            strong { (title) }
            @if let Some(detail) = detail { small { (detail) } }
        }
    }
}

/// The panel for one selector.
async fn choose(
    State(app): State<App>,
    headers: HeaderMap,
    Path(kind): Path<String>,
    Query(wanted): Query<Wanted>,
) -> Response {
    if !matches!(kind.as_str(), "project" | "branch" | "target") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(owner) = crate::pages::chat::reader(&app, &headers).await else {
        return plain(StatusCode::FORBIDDEN, "Sign in to pick a project.");
    };
    let chat = wanted.chat.as_deref().filter(|id| !id.is_empty());
    let Some(choices) = choices(&app, &headers, &owner, chat.is_none()).await else {
        return plain(StatusCode::FORBIDDEN, "Sign in to pick a project.");
    };
    let picked = choices.resolve(&wanted);
    let with = |change: &dyn Fn(&mut Picked), focus: &str| {
        let mut next = picked.clone();
        change(&mut next);
        let mut pairs = query(&next, chat);
        pairs.push(("focus", focus.to_owned()));
        href(ROW, &pairs)
    };
    let body = match kind.as_str() {
        "project" => project_panel(&choices, &picked, &with),
        "branch" => branch_panel(&app, &headers, &picked, &with).await,
        _ => target_panel(&choices, &picked, &with),
    };
    respond(body)
}

type With<'a> = dyn Fn(&dyn Fn(&mut Picked), &str) -> String + Sync + 'a;

fn project_panel(choices: &Choices, picked: &Picked, with: &With<'_>) -> Markup {
    let add = if choices.connected {
        "Add a repository"
    } else {
        "Connect a GitHub repository"
    };
    crate::composer::panel(
        "Project",
        html! {
            @if choices.projects.is_empty() {
                p { "Work in one of your GitHub repositories." }
                a.oa-composer-choice href=(crate::projects::PAGE) autofocus {
                    strong { "Connect a GitHub repository" }
                    small { "Pick the repositories OpenAgents can use." }
                }
            } @else {
                (choice(
                    with(&|next: &mut Picked| { next.project = None; next.branch = None; }, "project"),
                    "No project",
                    Some("Just chat."),
                    picked.project.is_none(),
                ))
                @for project in &choices.projects {
                    (choice(
                        with(&|next: &mut Picked| {
                            next.project = Some(project.clone());
                            next.branch = None;
                        }, "project"),
                        &project.name,
                        Some(&project.repository),
                        picked.project.as_ref().is_some_and(|p| p.id == project.id),
                    ))
                }
                p { a href=(crate::projects::PAGE) { (add) } }
            }
        },
    )
}

async fn branch_panel(app: &App, headers: &HeaderMap, picked: &Picked, with: &With<'_>) -> Markup {
    let Some(project) = &picked.project else {
        return crate::composer::panel("Branch", html! { p { "Pick a project first." } });
    };
    let listed = crate::composer::branch_names(app, headers, &project.repository).await;
    let (mut names, more) = match listed {
        Ok(found) => found,
        Err(message) => {
            return crate::composer::panel(
                "Branch",
                html! { p role="alert" { (message) } p { "Nothing changed." } },
            );
        }
    };
    // The default branch first.
    names.retain(|name| *name != project.default_branch);
    names.insert(0, project.default_branch.clone());
    crate::composer::panel(
        "Branch",
        html! {
            p { (project.repository) }
            @for name in &names {
                (choice(
                    with(&|next: &mut Picked| next.branch = Some(name.clone()), "branch"),
                    name,
                    (*name == project.default_branch).then_some("Default branch"),
                    picked.branch.as_deref() == Some(name.as_str()),
                ))
            }
            @if more { p class="oa-composer-note" { "Showing the first 100 branches." } }
        },
    )
}

fn target_panel(choices: &Choices, picked: &Picked, with: &With<'_>) -> Markup {
    let targets = choices.targets(picked.project.as_ref());
    crate::composer::panel(
        "Where it runs",
        html! {
            @for (target, label) in &targets {
                (choice(
                    with(&|next: &mut Picked| next.target = target.clone(), "target"),
                    label,
                    Some(match target {
                        Target::Chat => "Answers here. Nothing runs.",
                        Target::Claude(_) => "Runs on a fresh computer with this repository. Its answer shows in this chat.",
                        Target::Coder(_) => "Coder answers on your computer, with its files and tools.",
                    }),
                    *target == picked.target,
                ))
            }
        },
    )
}

#[cfg(test)]
mod tests;
