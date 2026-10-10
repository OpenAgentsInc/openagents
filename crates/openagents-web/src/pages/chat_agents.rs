//! A chat's agents (#11164): several runs in parallel, a live list, Stop
//! and Message, and each result back in the chat.
//!
//! The web counterpart of Coder's background agents (#11163,
//! `crates/agent-fleet`), sharing their names, their words and their
//! result text ([`agent_fleet::Notice`]):
//!
//! - **Start several.** The Run Claude Code page (`/chat/{id}/claude`,
//!   [`super::work`]) asks how many agents to run. Each is its own Claude
//!   Code run on a fresh computer from the chat's environment, with a short
//!   name ([`plan`]) and its own branch, `agent/{name}` ([`fleet_prompt`]).
//!   Each run is a chat task ([`ChatTask::agent`]), so the sidebar spinner
//!   shows while any of them works.
//! - **The list.** The chat shows an Agents panel ([`render`],
//!   `GET /chat/{id}/agents`): each agent's status, time, cost, Stop,
//!   Message, and Open transcript. It reloads every few seconds while one
//!   works. A usage-limit pause reads "Paused until 14:05 UTC", never as a
//!   failure. A chat synced from Coder lists the background agents Coder
//!   reported for it from its computer (`crate::phone_api`), with the same
//!   Stop and Message, which reach Coder the way the phone's do.
//! - **Results.** When an agent's run ends, one short result joins the chat
//!   ([`result_text`], written by [`super::work::observe`]).
//! - **Messages.** A message to an agent that is working waits for its run
//!   to end; then its next run starts on the same branch with the message
//!   ([`follow_up_prompt`]), the way a Coder background agent resumes. A
//!   message to an agent that has ended starts its next run at once. The
//!   next run uses the sender's own Claude key, released for that run only
//!   ([`crate::cloud::byo::run_key`]), so it starts while the chat is open
//!   (the panel's reload), never from a stored key.

use std::sync::Mutex;

use coder_environment_operator::studio::claude::MAX_PROMPT;
use openagents_ui::actions::{ButtonLink, ButtonVariant, ControlSize};
use openagents_ui::forms::Textarea;

use crate::chat_store::{
    ChatEnvironment, ChatTask, MAX_AGENT_INBOX, MAX_AGENT_MESSAGE, TaskAgent, TaskKind, TaskState,
};
use crate::phone_api::{self, Acted, Command};

use super::*;

/// The most agents one request starts.
pub(super) const MAX_AGENTS: u32 = 5;
/// How often the panel reloads while an agent works.
const POLL: &str = "every 5s";
/// The longest report a result in the chat carries, in characters; the
/// rest is in the run's transcript.
const RESULT_REPORT_CHARS: usize = 1_500;
/// The longest last report a follow-up run is given, in characters.
const FOLLOW_UP_REPORT_CHARS: usize = 4_000;
/// What separates a run's instructions from the person's request.
const REQUEST_MARK: &str = "\n\nThe request:\n";
/// The engine word in results.
const ENGINE: &str = "Claude Code";

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/agents", get(panel_route))
        .route("/chat/{id}/agents/stop", post(stop_route))
        .route("/chat/{id}/agents/message", post(message_route))
}

/// How many agents a form asked for: 1 when it asked for none.
///
/// # Errors
/// A number outside 1 to [`MAX_AGENTS`].
pub(super) fn count(field: Option<&str>) -> Result<u32, String> {
    let Some(field) = field.map(str::trim).filter(|field| !field.is_empty()) else {
        return Ok(1);
    };
    field
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=MAX_AGENTS).contains(n))
        .ok_or_else(|| format!("Pick from 1 to {MAX_AGENTS} agents."))
}

/// The agents to start for `request`: `count` names unique in the chat,
/// made from the request's first words, each with its own branch.
pub(super) fn plan(chat: &Conversation, request: &str, count: u32) -> Vec<TaskAgent> {
    let base = agent_fleet::name_from(request);
    let mut taken: Vec<String> = chat
        .tasks
        .iter()
        .filter_map(|task| task.agent.as_ref().map(|agent| agent.name.clone()))
        .collect();
    let mut n = 1;
    (0..count)
        .map(|_| {
            let mut name = format!("{base}-{n}");
            while taken.contains(&name) {
                n += 1;
                name = format!("{base}-{n}");
            }
            n += 1;
            taken.push(name.clone());
            TaskAgent {
                branch: format!("agent/{name}"),
                name,
                run: 1,
                inbox: Vec::new(),
            }
        })
        .collect()
}

/// What one agent is told before the request.
fn header(agent: &TaskAgent, count: u32) -> String {
    let together = if count > 1 {
        format!(
            "You are {}, one of {count} agents working on the same request at once, each on its own computer. Take your own approach; the person compares the results.",
            agent.name
        )
    } else {
        format!("You are {}, an agent working on this request.", agent.name)
    };
    format!(
        "{together} Create a branch named {} and do your work there. Commit it, and push the branch when you can. Finish with a short report: what you changed, on which branch, and anything left to do.",
        agent.branch
    )
}

/// The prompt for an agent's first run.
pub(super) fn fleet_prompt(request: &str, agent: &TaskAgent, count: u32) -> String {
    format!("{}{REQUEST_MARK}{}", header(agent, count), request.trim())
}

/// The person's request inside a run's prompt.
fn request_of(prompt: &str) -> &str {
    prompt
        .rsplit_once(REQUEST_MARK)
        .map_or(prompt, |(_, request)| request)
}

fn cut(text: &str, chars: usize) -> (String, bool) {
    let text = text.trim();
    if text.chars().count() <= chars {
        (text.to_owned(), false)
    } else {
        (text.chars().take(chars).collect(), true)
    }
}

/// The prompt for an agent's next run: the same branch, what it reported
/// last, the messages sent to it, and the request, within
/// [`MAX_PROMPT`].
pub(super) fn follow_up_prompt(
    previous_prompt: &str,
    agent: &TaskAgent,
    report: Option<&str>,
    messages: &[String],
) -> String {
    let request = request_of(previous_prompt).trim();
    let mut notes = format!(
        "{}\n\nYou worked on this before, on branch {}: fetch it and check it out, then continue from there.",
        header(agent, 1),
        agent.branch
    );
    if let Some(report) = report.filter(|report| !report.trim().is_empty()) {
        let (report, _) = cut(report, FOLLOW_UP_REPORT_CHARS);
        notes.push_str("\n\nWhat you reported last time:\n");
        notes.push_str(&report);
    }
    notes.push_str("\n\nNew messages from the person, oldest first:");
    for message in messages {
        notes.push_str("\n- ");
        notes.push_str(message.trim());
    }
    let tail = format!("{REQUEST_MARK}{request}");
    let room = MAX_PROMPT.saturating_sub(tail.len());
    if notes.len() > room {
        let mut end = room;
        while !notes.is_char_boundary(end) {
            end -= 1;
        }
        notes.truncate(end);
    }
    notes + &tail
}

/// The short result an agent's ended run puts in the chat: the agent
/// list's own words ([`agent_fleet::Notice::text`]) with its branch and
/// the start of its report.
pub(super) fn result_text(
    task: &ChatTask,
    agent: &TaskAgent,
    state: TaskState,
    report: Option<&str>,
    error: Option<&str>,
    cost_usd: Option<f64>,
    now_unix: u64,
) -> String {
    let status = match state {
        TaskState::Done => agent_fleet::Status::Done,
        TaskState::Failed => agent_fleet::Status::Failed,
        TaskState::Stopped => agent_fleet::Status::Stopped,
        TaskState::Working | TaskState::Paused => agent_fleet::Status::Running,
    };
    let (report, more) = match report.filter(|_| state == TaskState::Done) {
        Some(report) => {
            let (cut_report, more) = cut(report, RESULT_REPORT_CHARS);
            (Some(cut_report), more)
        }
        None => (None, false),
    };
    let notice = agent_fleet::Notice {
        id: task.id.clone(),
        name: agent.name.clone(),
        engine: ENGINE.to_owned(),
        status,
        elapsed_seconds: task
            .finished_unix
            .unwrap_or(now_unix)
            .saturating_sub(task.started_unix),
        tokens: 0,
        cost_usd,
        worktree: None,
        branch: Some(agent.branch.clone()),
        report,
        error: error
            .filter(|_| state == TaskState::Failed)
            .map(str::to_owned),
        parent_session: None,
    };
    let text = notice.text().replacen("Background agent ", "Agent ", 1);
    let (head, rest) = text.split_once('\n').unwrap_or((text.as_str(), ""));
    let mut out = format!("{head}\nIts work is on branch {}.", agent.branch);
    if !rest.is_empty() {
        out.push('\n');
        out.push_str(rest);
    }
    if more {
        out.push_str("\n[The rest is in its transcript.]");
    }
    out
}

/// Where an agent stands, as the panel says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Standing {
    Working,
    WaitingForYou,
    /// A usage limit paused it; it continues at this Unix time, when known.
    Paused(Option<u64>),
    Done,
    Failed,
    Stopped,
}

impl Standing {
    fn key(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::WaitingForYou => "waiting",
            Self::Paused(_) => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Working => "Working".into(),
            Self::WaitingForYou => "Waiting for you".into(),
            Self::Paused(Some(at)) => format!("Paused until {}", clock(*at)),
            Self::Paused(None) => "Paused until the usage limit resets".into(),
            Self::Done => "Done".into(),
            Self::Failed => "Failed".into(),
            Self::Stopped => "Stopped".into(),
        }
    }

    fn live(&self) -> bool {
        matches!(self, Self::Working | Self::WaitingForYou | Self::Paused(_))
    }
}

/// `14:05 UTC` for a Unix time.
pub(super) fn clock(unix: u64) -> String {
    format!("{:02}:{:02} UTC", unix / 3600 % 24, unix / 60 % 60)
}

/// Which agent a form means: a chat task (a Cloud run), or an item Coder
/// reported from a computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Run(String),
    Computer { computer: String, item: String },
}

/// One row of the panel.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub target: Target,
    pub name: String,
    /// Where it runs: the environment, or the computer's name.
    pub place: String,
    pub engine: String,
    pub standing: Standing,
    pub elapsed_seconds: u64,
    pub cost_usd: Option<f64>,
    /// Messages waiting for its next run.
    pub waiting: usize,
    pub can_message: bool,
    /// Its transcript's page, when this person may open it.
    pub transcript: Option<String>,
}

impl Row {
    fn can_stop(&self) -> bool {
        self.standing.live()
    }
}

fn standing(state: TaskState, paused_until: Option<u64>) -> Standing {
    match state {
        TaskState::Working => Standing::Working,
        TaskState::Paused => Standing::Paused(paused_until),
        TaskState::Done => Standing::Done,
        TaskState::Failed => Standing::Failed,
        TaskState::Stopped => Standing::Stopped,
    }
}

/// What the panel reads about one run, when the run can be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Live {
    pub cost_usd: Option<f64>,
    pub paused_until: Option<u64>,
}

/// The panel's rows for the chat's runs: one per agent (its newest run,
/// with time and cost summed over its runs), and one per other run.
/// `read` gives what a run reports now; `links` adds transcript links.
pub(super) fn run_rows(
    chat: &Conversation,
    read: impl Fn(&ChatTask) -> Option<Live>,
    links: bool,
    now_unix: u64,
) -> Vec<Row> {
    let place = chat
        .environment
        .as_ref()
        .map(|env| match env.version {
            Some(n) => format!("{} v{n}", env.repository),
            None => env.repository.clone(),
        })
        .unwrap_or_default();
    let gone = chat.environment.as_ref().is_some_and(|env| env.removed);
    let mut rows: Vec<Row> = Vec::new();
    for (index, task) in chat.tasks.iter().enumerate() {
        let newest_of_agent = task.agent.as_ref().is_none_or(|agent| {
            !chat.tasks[index + 1..].iter().any(|later| {
                later
                    .agent
                    .as_ref()
                    .is_some_and(|other| other.name == agent.name)
            })
        });
        if !newest_of_agent {
            continue;
        }
        let runs: Vec<&ChatTask> = match &task.agent {
            Some(agent) => chat
                .tasks
                .iter()
                .filter(|t| t.agent.as_ref().is_some_and(|a| a.name == agent.name))
                .collect(),
            None => vec![task],
        };
        let live = read(task).unwrap_or_default();
        let elapsed_seconds = runs
            .iter()
            .map(|run| {
                run.finished_unix
                    .unwrap_or(if run.state.finished() {
                        run.started_unix
                    } else {
                        now_unix
                    })
                    .saturating_sub(run.started_unix)
            })
            .sum::<u64>();
        let costs: Vec<f64> = runs
            .iter()
            .filter_map(|run| {
                if run.id == task.id {
                    live.cost_usd
                } else {
                    read(run).and_then(|seen| seen.cost_usd)
                }
            })
            .collect();
        rows.push(Row {
            target: Target::Run(task.id.clone()),
            name: task
                .agent
                .as_ref()
                .map_or_else(|| task.title.clone(), |agent| agent.name.clone()),
            place: place.clone(),
            engine: match task.kind {
                TaskKind::Claude => ENGINE.to_owned(),
                TaskKind::Continue => "Cloud computer".to_owned(),
            },
            standing: standing(task.state, live.paused_until),
            elapsed_seconds,
            cost_usd: (!costs.is_empty()).then(|| costs.iter().sum::<f64>()),
            waiting: task.agent.as_ref().map_or(0, |agent| agent.inbox.len()),
            can_message: task.agent.is_some() && links && !gone,
            transcript: (links && !gone)
                .then(|| format!("/environments/{}/runs/{}", task.environment, task.id)),
        });
    }
    rows
}

/// The panel's rows for the background agents Coder reported for this
/// chat (`session`) from any of the account's computers.
pub(super) fn computer_rows(agents: &phone_api::Agents, session: &str, now_unix: u64) -> Vec<Row> {
    let mut rows = Vec::new();
    for (computer, board) in &agents.boards {
        for item in board
            .items
            .iter()
            .filter(|item| item.kind == "agent" && item.session.as_deref() == Some(session))
        {
            let standing = match item.status.as_str() {
                "working" => Standing::Working,
                "asking" => Standing::WaitingForYou,
                "done" => Standing::Done,
                "failed" => Standing::Failed,
                _ => Standing::Stopped,
            };
            let end = item.finished_unix.unwrap_or(if standing.live() {
                now_unix
            } else {
                item.started_unix
            });
            rows.push(Row {
                target: Target::Computer {
                    computer: computer.clone(),
                    item: item.id.clone(),
                },
                name: item.title.clone(),
                place: computer.clone(),
                engine: item.engine.clone().unwrap_or_else(|| "Coder".to_owned()),
                can_message: true,
                standing,
                elapsed_seconds: end.saturating_sub(item.started_unix),
                cost_usd: item.cost_usd,
                waiting: 0,
                transcript: None,
            });
        }
    }
    rows
}

fn target_fields(target: &Target) -> Markup {
    match target {
        Target::Run(run) => html! { input type="hidden" name="run" value=(run); },
        Target::Computer { computer, item } => html! {
            input type="hidden" name="computer" value=(computer);
            input type="hidden" name="item" value=(item);
        },
    }
}

/// The Agents panel. While an agent works it reloads itself every few
/// seconds; `notice` is a line about the last action.
pub(super) fn render(chat_id: &str, csrf: &str, rows: &[Row], notice: Option<&str>) -> Markup {
    let poll = rows
        .iter()
        .any(|row| row.standing.live() || row.waiting > 0);
    let url = format!("/chat/{chat_id}/agents");
    html! {
        section #chat-agents.oa-agents aria-label="Agents"
            hx-get=[poll.then_some(url.as_str())]
            hx-trigger=[poll.then_some(POLL)]
            hx-swap=[poll.then_some("outerHTML")] {
            @if !rows.is_empty() {
                h2.oa-agents-title { "Agents" }
                @for (index, row) in rows.iter().enumerate() {
                    div.oa-task-row.oa-agents-row id=(format!("chat-agent-{index}")) data-status=(row.standing.key()) {
                        span.oa-task-row-title { (row.name) }
                        span.oa-task-row-detail {
                            (row.engine) " · " (row.place) " · " (agent_fleet::elapsed_words(row.elapsed_seconds))
                            @if let Some(cost) = row.cost_usd { " · " (agent_fleet::dollars(cost)) }
                            @if row.waiting == 1 { " · 1 message waiting" }
                            @else if row.waiting > 1 { " · " (row.waiting) " messages waiting" }
                        }
                        span.oa-chat-status.oa-task-row-status data-status=(row.standing.key()) {
                            @if row.standing == Standing::Working {
                                (openagents_ui::actions::LoadingIndicator::new().decorative())
                            } @else {
                                span.oa-chat-status-dot aria-hidden="true" {}
                            }
                            span.oa-chat-status-label { (row.standing.label()) }
                        }
                    }
                    div.oa-agents-actions {
                        @if row.can_stop() {
                            form method="post" action=(format!("{url}/stop")) hx-post=(format!("{url}/stop")) hx-target="#chat-agents" hx-swap="outerHTML" {
                                input type="hidden" name="csrf" value=(csrf);
                                (target_fields(&row.target))
                                (Button::new("Stop").kind(ButtonType::Submit).size(ControlSize::Sm).variant(ButtonVariant::Outline))
                            }
                        }
                        @if row.can_message {
                            form.oa-agents-message method="post" action=(format!("{url}/message")) hx-post=(format!("{url}/message")) hx-target="#chat-agents" hx-swap="outerHTML" {
                                input type="hidden" name="csrf" value=(csrf);
                                (target_fields(&row.target))
                                (Textarea::new("text").rows(1).maxlength(MAX_AGENT_MESSAGE as u32).aria_label(format!("Message {}", row.name)))
                                (Button::new("Message").kind(ButtonType::Submit).size(ControlSize::Sm).variant(ButtonVariant::Outline))
                            }
                        }
                        @if let Some(href) = &row.transcript {
                            (ButtonLink::new("Open transcript", href.clone()).variant(ButtonVariant::Ghost).size(ControlSize::Sm))
                        }
                    }
                }
            }
            @if let Some(notice) = notice {
                p.oa-agents-notice role="status" { (notice) }
            }
        }
    }
}

/// Where the panel goes in the thread: it loads itself, so a chat with no
/// agents and no runs shows nothing.
pub(super) fn slot(chat: &Conversation) -> Markup {
    if chat.tasks.is_empty() && chat.terminal.is_none() {
        return html! {};
    }
    html! {
        section #chat-agents.oa-agents aria-label="Agents"
            hx-get=(format!("/chat/{}/agents", chat.id)) hx-trigger="load" hx-swap="outerHTML" {}
    }
}

/// The panel's rows for `chat` as this request may see them.
async fn rows(app: &App, headers: &HeaderMap, chat: &Conversation) -> Vec<Row> {
    let links = super::work::links(app, headers).await;
    let now_unix = now();
    let studio = app.config.environments.get();
    let mut rows = run_rows(
        chat,
        |task| {
            let run = studio.as_ref()?.claude_run(&task.environment, &task.id)?;
            Some(Live {
                cost_usd: run.cost_usd,
                paused_until: run.paused_until,
            })
        },
        links,
        now_unix,
    );
    if let Some(terminal) = &chat.terminal
        && let Ok(agents) = phone_api::read_agents(&app.config.chat_store, &chat.owner).await
    {
        rows.extend(computer_rows(&agents, &terminal.session, now_unix));
    }
    rows
}

async fn respond(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
    notice: Option<&str>,
) -> Response {
    if headers.get("HX-Request").is_none_or(|v| v != "true") {
        return crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response());
    }
    let loaded = match load_owned(app, owner, id).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let chat = &loaded.conversation;
    let rows = rows(app, headers, chat).await;
    crate::chat_html::protect(render(&chat.id, &csrf(app, owner), &rows, notice).into_response())
}

async fn panel_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    let loaded = match load_owned(&app, &owner, &id).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let notice = deliver(&app, &headers, &loaded.conversation).await;
    let loaded = if notice.is_some() {
        match load_owned(&app, &owner, &id).await {
            Ok(loaded) => loaded,
            Err(response) => return response,
        }
    } else {
        loaded
    };
    let chat = &loaded.conversation;
    let rows = rows(&app, &headers, chat).await;
    crate::chat_html::protect(
        render(&chat.id, &csrf(&app, &owner), &rows, notice.as_deref()).into_response(),
    )
}

#[derive(Deserialize)]
struct Act {
    csrf: String,
    #[serde(default)]
    run: String,
    #[serde(default)]
    computer: String,
    #[serde(default)]
    item: String,
    #[serde(default)]
    text: String,
}

impl Act {
    fn target(&self) -> Option<Target> {
        if !self.run.is_empty() {
            return Some(Target::Run(self.run.clone()));
        }
        let computer = crate::coder_sync::line(&self.computer, 64);
        (!computer.is_empty() && !self.item.is_empty() && self.item.len() <= 128).then(|| {
            Target::Computer {
                computer,
                item: self.item.clone(),
            }
        })
    }
}

/// Sends `action` (with `text`) for an item Coder reported, the way the
/// phone's Stop and Message do.
async fn to_computer(
    app: &App,
    owner: &str,
    computer: &str,
    item: &str,
    action: &str,
    text: Option<String>,
) -> String {
    let command = Command {
        id: crate::cloud::byo::fresh_request(),
        item: item.to_owned(),
        action: action.to_owned(),
        question: None,
        text,
    };
    match phone_api::queue_action(&app.config.chat_store, owner, computer, command).await {
        Ok(Acted::Queued) if action == "stop" => format!("Asked Coder on {computer} to stop it."),
        Ok(Acted::Queued) => format!("Sent to Coder on {computer}."),
        Ok(Acted::Unknown) => "That agent isn't running on that computer anymore.".to_owned(),
        Ok(Acted::Offline) => format!("Coder on {computer} isn't online now."),
        Err(_) => "That didn't go through. Try again.".to_owned(),
    }
}

async fn stop_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Act>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let notice = match form.target() {
        None => "Pick an agent to stop.".to_owned(),
        Some(Target::Computer { computer, item }) => {
            to_computer(&app, &owner, &computer, &item, "stop", None).await
        }
        Some(Target::Run(run)) => stop_run(&app, &headers, &owner, &id, &run).await,
    };
    respond(&app, &headers, &owner, &id, Some(&notice)).await
}

async fn stop_run(app: &App, headers: &HeaderMap, owner: &str, id: &str, run: &str) -> String {
    let (Some(studio), true) = (
        app.config.environments.get(),
        super::work::links(app, headers).await,
    ) else {
        return "Agents can't be stopped from here.".to_owned();
    };
    let loaded = match load_owned(app, owner, id).await {
        Ok(loaded) => loaded,
        Err(_) => return "That chat couldn't be opened. Reload the page.".to_owned(),
    };
    let Some(task) = loaded.conversation.tasks.iter().find(|task| task.id == run) else {
        return "That agent isn't in this chat.".to_owned();
    };
    if task.state.finished() {
        return "It has already ended.".to_owned();
    }
    match studio.stop_claude(&task.environment, &task.id) {
        Ok(()) => {
            let _ = super::work::sync(app, loaded).await;
            "Stopping it. Its result joins the chat when it has stopped.".to_owned()
        }
        Err(error) => error,
    }
}

async fn message_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<Act>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let text = form.text.trim().to_owned();
    let notice = if text.is_empty() {
        "Write a message first.".to_owned()
    } else if text.chars().count() > MAX_AGENT_MESSAGE {
        format!("Keep a message to {MAX_AGENT_MESSAGE} characters.")
    } else if secret_screen::credential_in(&text).is_some() {
        "This looks like it holds a password or key, so it wasn't sent.".to_owned()
    } else {
        match form.target() {
            None => "Pick an agent to message.".to_owned(),
            Some(Target::Computer { computer, item }) => {
                to_computer(&app, &owner, &computer, &item, "message", Some(text)).await
            }
            Some(Target::Run(run)) => message_run(&app, &headers, &owner, &id, &run, text).await,
        }
    };
    respond(&app, &headers, &owner, &id, Some(&notice)).await
}

/// What adding a message to a run's agent did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Queued {
    /// It waits; `true` when the agent has ended, so its next run can start
    /// now.
    Waiting(bool),
    NotAnAgent,
    Full,
    Missing,
}

/// Adds `text` to the inbox of the agent whose newest run is `run`.
pub(super) fn queue(chat: &mut Conversation, run: &str, text: &str) -> Queued {
    let Some(index) = chat.tasks.iter().position(|task| task.id == run) else {
        return Queued::Missing;
    };
    let finished = chat.tasks[index].state.finished();
    let Some(agent) = chat.tasks[index].agent.as_mut() else {
        return Queued::NotAnAgent;
    };
    if agent.inbox.len() >= MAX_AGENT_INBOX {
        return Queued::Full;
    }
    agent.inbox.push(text.to_owned());
    Queued::Waiting(finished)
}

async fn message_run(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
    run: &str,
    text: String,
) -> String {
    if app.config.environments.is_none() || !super::work::links(app, headers).await {
        return "Agents can't be messaged from here.".to_owned();
    }
    let result = Mutex::new(Queued::Missing);
    let updated = sidebar::update(app, owner, id, |chat| {
        let queued = queue(chat, run, &text);
        let changed = matches!(queued, Queued::Waiting(_));
        if let Ok(mut slot) = result.lock() {
            *slot = queued;
        }
        changed
    })
    .await;
    let Ok(chat) = updated else {
        return "That didn't go through. Try again.".to_owned();
    };
    let queued = result
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match queued {
        Queued::Waiting(true) => deliver(app, headers, &chat)
            .await
            .unwrap_or_else(|| "Its next run starts with your message.".to_owned()),
        Queued::Waiting(false) => {
            "It reads your message when its current run ends, then carries on.".to_owned()
        }
        Queued::NotAnAgent => {
            "Only agents started together take messages; write in the chat instead.".to_owned()
        }
        Queued::Full => format!(
            "It already has {MAX_AGENT_INBOX} messages waiting. Send more when it has read them."
        ),
        Queued::Missing => "That agent isn't in this chat.".to_owned(),
    }
}

/// The agents whose run has ended with messages waiting: the index of
/// each one's newest run.
pub(super) fn due(chat: &Conversation) -> Vec<usize> {
    chat.tasks
        .iter()
        .enumerate()
        .filter(|(index, task)| {
            task.state.finished()
                && task.agent.as_ref().is_some_and(|agent| {
                    !agent.inbox.is_empty()
                        && !chat.tasks[index + 1..].iter().any(|later| {
                            later
                                .agent
                                .as_ref()
                                .is_some_and(|other| other.name == agent.name)
                        })
                })
        })
        .map(|(index, _)| index)
        .collect()
}

/// Starts the next run of every agent that ended with messages waiting,
/// with the viewer's own Claude key. A line about what happened, when
/// anything did.
async fn deliver(app: &App, headers: &HeaderMap, chat: &Conversation) -> Option<String> {
    let due = due(chat);
    if due.is_empty() {
        return None;
    }
    let studio = app.config.environments.get()?;
    if !super::work::links(app, headers).await {
        return None;
    }
    let environment: ChatEnvironment = chat.environment.clone().filter(|env| !env.removed)?;
    let mut lines = Vec::new();
    for index in due {
        let previous = chat.tasks[index].clone();
        // Take the messages in one write, so two open pages start one run.
        let taken = Mutex::new(Vec::<String>::new());
        let claimed = sidebar::update(app, &chat.owner, &chat.id, |next| {
            let Some(task) = next.tasks.iter_mut().find(|task| task.id == previous.id) else {
                return false;
            };
            let Some(agent) = task.agent.as_mut().filter(|agent| !agent.inbox.is_empty()) else {
                return false;
            };
            let messages = std::mem::take(&mut agent.inbox);
            if let Ok(mut slot) = taken.lock() {
                *slot = messages;
            }
            true
        })
        .await;
        let messages = taken
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if claimed.is_err() || messages.is_empty() {
            continue;
        }
        let Some(agent) = previous.agent.clone() else {
            continue;
        };
        let last = studio.claude_run(&previous.environment, &previous.id);
        let prompt = follow_up_prompt(
            last.as_ref()
                .map_or(previous.title.as_str(), |run| run.prompt.as_str()),
            &agent,
            last.as_ref()
                .and_then(|run| run.reply.as_deref().or(run.error.as_deref())),
            &messages,
        );
        let own = crate::cloud::byo::run_key(app, headers).await;
        match studio.run_claude(&previous.environment, &prompt, own) {
            Ok(run) => {
                let version = studio
                    .claude_run(&previous.environment, &run)
                    .and_then(|run| run.version)
                    .or(previous.version);
                let task = ChatTask {
                    id: run,
                    kind: TaskKind::Claude,
                    environment: previous.environment.clone(),
                    title: previous.title.clone(),
                    state: TaskState::Working,
                    started_unix: now(),
                    after_message: 0,
                    version,
                    finished_unix: None,
                    agent: Some(TaskAgent {
                        run: agent.run.saturating_add(1),
                        inbox: Vec::new(),
                        ..agent.clone()
                    }),
                };
                let recorded = sidebar::update(app, &chat.owner, &chat.id, |next| {
                    super::work::record(next, environment.clone(), task.clone());
                    true
                })
                .await;
                if recorded.is_err() {
                    super::work::abandon(app, &task);
                    lines.push(format!("{} couldn't start again. Try again.", agent.name));
                } else {
                    lines.push(format!("{} is working on your message.", agent.name));
                }
            }
            Err(error) => {
                let text = format!(
                    "{} couldn't start again: {error}\n\nYour message, to send again when it can run:\n{}",
                    agent.name,
                    messages.join("\n")
                );
                let _ = sidebar::update(app, &chat.owner, &chat.id, |next| {
                    super::continued::add_message(next, Role::Assistant, &text);
                    true
                })
                .await;
                lines.push(format!("{} couldn't start again.", agent.name));
            }
        }
    }
    if lines.is_empty() {
        return None;
    }
    super::work::watch(app.clone(), chat.owner.clone(), chat.id.clone());
    Some(lines.join(" "))
}

#[cfg(test)]
#[path = "chat_agents_tests.rs"]
mod tests;
