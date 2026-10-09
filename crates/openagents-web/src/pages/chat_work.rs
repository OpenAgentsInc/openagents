//! A chat's environment and tasks (#11037, `docs/web/sidebar.md` phase 5).
//!
//! A chat about a repository that has a saved environment (`/environments`,
//! [`crate::environments`]) can run Claude Code there: the chat's header
//! offers **Run Claude Code** (`/chat/{id}/claude`). Starting a run records
//! the environment and the task on the chat (`environment`, `tasks` in
//! [`crate::chat_store`]), so
//! - the header names the environment and its version, linking to it;
//! - the thread shows each task as a compact row ([`TaskRow`]) with its
//!   status, linking to the run, after the message it followed;
//! - the sidebar row's line 2 ends with "Environment v3", and its status is
//!   Working while a task runs and Failed when the newest task failed (until
//!   a new message is sent).
//!
//! The run's own record (the environments studio's job store) is where its
//! state lives; the chat keeps the last state it saw. [`sync`] writes a
//! change into the chat when it reads one: a watcher while the run goes
//! ([`watch`]), the chat page and its stream (every load), and the
//! sidebar stream's checks on Working chats. The chat store announces the
//! write, so open pages update without polling the run themselves.
//!
//! Environments and runs answer only the local address, so the header link,
//! the run page, and task links show only there ([`crate::local_request`]),
//! and the site guard keeps `/chat/{id}/claude` local too.
//!
//! A task that ran over a minute and finished makes the row say Done until
//! the chat is opened: the chat keeps when it saw the task finish
//! (`finished_unix`) and, only while such a Done waits, when it was last
//! open (`opened_unix`, written by [`mark_opened`] when the chat page or its
//! stream loads it).
//!
//! Not wired, because nothing records them yet: Coder tasks on connected
//! computers are not started from chats, runs report no steps ("3 of 7"),
//! and no run asks the person anything or pauses for a usage limit (runs
//! here use the person's API key and are driven without the operator that
//! records sign-in prompts and limit pauses), so no Waiting for you and no
//! Paused until.

use coder_environment_operator::studio::claude::{MAX_PROMPT, RUN_SECONDS, RunState};
use coder_environment_operator::studio::{Studio, Summary};
use openagents_ui::actions::{ButtonLink, ButtonVariant, ControlSize};
use openagents_ui::forms::{Field, Textarea};
use openagents_ui::shell::{TaskRow, TaskStatus};

use crate::chat_store::{ChatEnvironment, ChatTask, MAX_TASKS, TaskKind, TaskState};
use crate::projects::Sidebar;

use super::*;

/// The longest task title kept on the chat (the prompt's first line).
const TASK_TITLE_CHARS: usize = 120;
/// A task that ran at least this long says Done on its chat's row until
/// the chat is opened; quicker ones just finish.
const DONE_AFTER_SECONDS: u64 = 60;
/// How often the watcher reads a running task.
const WATCH_EVERY: Duration = Duration::from_secs(3);

pub(super) fn routes() -> Router<App> {
    Router::new().route("/chat/{id}/claude", get(run_page).post(run))
}

fn studio(app: &App) -> Option<&Arc<Studio>> {
    app.config.environments.as_ref()
}

/// Whether this request may show environment and run links: environments
/// are set up here and the request came to the local address.
pub(super) fn links(app: &App, headers: &HeaderMap) -> bool {
    studio(app).is_some() && crate::local_request(headers)
}

/// The environment a chat page offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Offer {
    pub id: String,
    pub repository: String,
    /// The version the chat's tasks ran on, else the newest saved one.
    pub version: Option<u64>,
    /// Claude Code can start there now (a saved version and a key).
    pub runnable: bool,
}

impl Offer {
    pub(super) fn label(&self) -> String {
        match self.version {
            Some(n) => format!("{} v{n}", self.repository),
            None => self.repository.clone(),
        }
    }
}

/// The repository a chat is about: its project's, else the one its
/// composer selected.
pub(super) fn repository<'a>(
    chat: &'a Conversation,
    projects: Option<&'a Sidebar>,
) -> Option<&'a str> {
    chat.project
        .as_deref()
        .and_then(|id| projects?.project(id))
        .map(|project| project.repository.as_str())
        .or_else(|| {
            chat.selection
                .as_ref()?
                .repository
                .as_ref()
                .map(|source| source.repository.as_str())
        })
}

/// The chat's environment among `rows`: the one it is tied to while it
/// exists, else the most recently changed saved one for its repository.
pub(super) fn pick<'a>(
    rows: &'a [Summary],
    chat: &Conversation,
    repository: Option<&str>,
) -> Option<&'a Summary> {
    if let Some(linked) = chat.environment.as_ref().filter(|e| !e.removed)
        && let Some(row) = rows.iter().find(|row| row.id == linked.id)
    {
        return Some(row);
    }
    let repository = repository?;
    rows.iter()
        .find(|row| row.saved.is_some() && row.repository.eq_ignore_ascii_case(repository))
}

/// What the chat page offers: its environment, when this request may link
/// it. A tied environment that no longer exists is marked removed on the
/// chat (its line 2 then says so).
pub(super) async fn offer(app: &App, headers: &HeaderMap, chat: &Conversation) -> Option<Offer> {
    // A chat synced from a terminal is read-only here.
    if !links(app, headers) || chat.terminal.is_some() {
        return None;
    }
    let studio = studio(app)?.clone();
    let rows = studio.list();
    if let Some(linked) = chat.environment.as_ref().filter(|e| !e.removed)
        && !rows.iter().any(|row| row.id == linked.id)
    {
        mark_removed(app, &chat.owner, &chat.id, &linked.id).await;
    }
    let projects = crate::projects::sidebar(app).await;
    let row = pick(&rows, chat, repository(chat, projects.as_deref()))?;
    let version = chat
        .environment
        .as_ref()
        .filter(|e| e.id == row.id)
        .and_then(|e| e.version)
        .or(row.saved);
    Some(Offer {
        id: row.id.clone(),
        repository: row.repository.clone(),
        version,
        runnable: row.saved.is_some() && studio.claude_ready(),
    })
}

async fn mark_removed(app: &App, owner: &str, id: &str, environment: &str) {
    let _ = sidebar::update(app, owner, id, |chat| {
        let Some(linked) = chat
            .environment
            .as_mut()
            .filter(|e| e.id == environment && !e.removed)
        else {
            return false;
        };
        linked.removed = true;
        for task in &mut chat.tasks {
            if task.environment == environment && !task.state.finished() {
                task.state = TaskState::Stopped;
            }
        }
        true
    })
    .await;
}

/// The header's breadcrumb: the environment (linked) before the title.
pub(super) fn breadcrumb(chat: &Conversation, offer: Option<&Offer>) -> Breadcrumb {
    let crumb = Breadcrumb::new(chat.title.clone());
    match offer {
        Some(offer) => crumb.crumb(offer.label(), format!("/environments/{}", offer.id)),
        None => crumb,
    }
}

/// The header's chat actions: Run Claude Code when it can start. `oob`
/// replaces the page's copy when another chat opens in place.
pub(super) fn actions(chat: &Conversation, offer: Option<&Offer>, oob: bool) -> Markup {
    html! {
        span #chat-actions hx-swap-oob=[oob.then_some("outerHTML")] {
            @if offer.is_some_and(|offer| offer.runnable) {
                (ButtonLink::new("Run Claude Code", format!("/chat/{}/claude", chat.id))
                    .color(Color::Secondary)
                    .variant(ButtonVariant::Outline)
                    .size(ControlSize::Sm)
                    .pill(true))
            }
        }
    }
}

/// Line 2's environment part: "Environment v3", or that it was removed.
pub(super) fn detail(chat: &Conversation) -> Option<String> {
    let environment = chat.environment.as_ref()?;
    Some(if environment.removed {
        "Environment removed".to_owned()
    } else {
        match environment.version {
            Some(n) => format!("Environment v{n}"),
            None => "Environment".to_owned(),
        }
    })
}

/// A task is running (or paused, and continues by itself).
pub(super) fn running(chat: &Conversation) -> bool {
    chat.tasks.iter().any(|task| !task.state.finished())
}

/// The newest task failed and no message was sent since.
pub(super) fn failed(chat: &Conversation) -> bool {
    chat.tasks.last().is_some_and(|task| {
        task.state == TaskState::Failed && task.after_message >= chat.messages.len()
    })
}

/// The newest task ran at least [`DONE_AFTER_SECONDS`], finished Done,
/// and the chat wasn't opened since: the row says Done until it is.
pub(super) fn unseen_done(chat: &Conversation) -> bool {
    chat.tasks.last().is_some_and(|task| {
        task.state == TaskState::Done
            && task.finished_unix.is_some_and(|at| {
                at.saturating_sub(task.started_unix) >= DONE_AFTER_SECONDS
                    && chat.opened_unix.is_none_or(|opened| opened < at)
            })
    })
}

/// Records that the owner has the chat open when a Done waits to be seen,
/// so the row's Done clears; any other time it writes nothing. The chat as
/// stored after.
pub(super) async fn mark_opened(app: &App, loaded: Loaded) -> Loaded {
    if !unseen_done(&loaded.conversation) {
        return loaded;
    }
    let mut next = loaded.conversation.clone();
    next.opened_unix = Some(now());
    next.revision += 1;
    match app.config.chat_store.compare_and_swap(&loaded, &next).await {
        Ok(saved) => saved,
        // Another write won; the next load tries again.
        Err(_) => loaded,
    }
}

fn status(state: TaskState) -> TaskStatus {
    match state {
        TaskState::Working => TaskStatus::Working,
        TaskState::Paused => TaskStatus::Paused,
        TaskState::Done => TaskStatus::Done,
        TaskState::Failed => TaskStatus::Failed,
        TaskState::Stopped => TaskStatus::Stopped,
    }
}

/// What a task row is called.
fn task_label(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Claude => "Claude Code",
        TaskKind::Continue => "Cloud computer",
    }
}

/// The tasks that started after the chat's first `at` messages, as rows.
/// `links` adds each run's address (see [`links`]).
pub(super) fn rows(chat: &Conversation, at: usize, links: bool) -> Markup {
    let count = chat.messages.len();
    let gone = |task: &ChatTask| {
        chat.environment
            .as_ref()
            .is_some_and(|e| e.removed && e.id == task.environment)
    };
    html! {
        @for task in chat.tasks.iter().filter(|task| task.after_message.min(count) == at) {
            @let row = TaskRow::new(task_label(task.kind), status(task.state))
                .detail(task.title.clone())
                .id(format!("chat-task-{}", task.id));
            @if links && !gone(task) {
                (row.href(format!("/environments/{}/runs/{}", task.environment, task.id)))
            } @else {
                (row)
            }
        }
    }
}

/// A run's state as the chat keeps it.
pub(super) fn state(run: RunState) -> TaskState {
    match run {
        RunState::Starting | RunState::Running => TaskState::Working,
        RunState::Paused => TaskState::Paused,
        RunState::Done => TaskState::Done,
        RunState::Failed => TaskState::Failed,
        RunState::Stopped => TaskState::Stopped,
    }
}

/// What reading a run found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Seen {
    pub state: TaskState,
    pub version: Option<u64>,
    /// Claude Code's answer, once it has one.
    pub reply: Option<String>,
}

/// The chat with its unfinished tasks' states as `read` reports them, or
/// `None` when nothing changed. A run `read` can't find keeps its last
/// state. A Coder chat continued on a Cloud computer (#11050) gets the
/// run's answer as its next message when the run is done, in the same
/// write, so the answer lands once.
pub(super) fn observe(
    chat: &Conversation,
    read: impl Fn(&str, &str) -> Option<Seen>,
) -> Option<Conversation> {
    let mut next = chat.clone();
    let mut changed = false;
    let mut answers = Vec::new();
    for task in next.tasks.iter_mut().filter(|task| !task.state.finished()) {
        let Some(seen) = read(&task.environment, &task.id) else {
            continue;
        };
        if task.state != seen.state {
            task.state = seen.state;
            if seen.state.finished() && task.finished_unix.is_none() {
                task.finished_unix = Some(now());
            }
            changed = true;
            if task.kind == TaskKind::Continue
                && seen.state == TaskState::Done
                && let Some(reply) = seen.reply.filter(|reply| !reply.trim().is_empty())
            {
                answers.push(reply);
            }
        }
        if seen.version.is_some() && task.version != seen.version {
            task.version = seen.version;
            changed = true;
        }
    }
    for answer in answers {
        super::continued::add_message(&mut next, Role::Assistant, &answer);
    }
    changed.then_some(next)
}

/// Writes the chat's running tasks' new states, when there are any; the
/// chat as stored after.
pub(super) async fn sync(app: &App, loaded: Loaded) -> Loaded {
    let Some(studio) = studio(app) else {
        return loaded;
    };
    if !running(&loaded.conversation) {
        return loaded;
    }
    let Some(mut next) = observe(&loaded.conversation, |environment, run| {
        studio.claude_run(environment, run).map(|run| Seen {
            state: state(run.state),
            version: run.version,
            reply: run.reply.or_else(|| said(&run.events)),
        })
    }) else {
        return loaded;
    };
    next.revision += 1;
    next.updated_unix = now();
    match app.config.chat_store.compare_and_swap(&loaded, &next).await {
        Ok(saved) => saved,
        // Another write won; the next read tries again.
        Err(_) => loaded,
    }
}

/// What Claude Code said last in a run's events, when the run recorded no
/// answer of its own.
fn said(events: &[serde_json::Value]) -> Option<String> {
    use coder_environment_operator::studio::claude::{Step, transcript};
    transcript(events)
        .into_iter()
        .rev()
        .find_map(|step| match step {
            Step::Said(text) if !text.trim().is_empty() => Some(text),
            _ => None,
        })
}

/// Follows the chat's running tasks until they finish, writing each change
/// (so the sidebar and the chat update with nobody watching the run).
pub(super) fn watch(app: App, owner: String, id: String) {
    tokio::spawn(async move {
        let ends = tokio::time::Instant::now() + Duration::from_secs(RUN_SECONDS + 900);
        while tokio::time::Instant::now() < ends {
            tokio::time::sleep(WATCH_EVERY).await;
            let loaded = match app.config.chat_store.load(&owner, &id).await {
                Ok(Some(loaded)) => loaded,
                Ok(None) => return,
                Err(_) => continue,
            };
            if !running(&sync(&app, loaded).await.conversation) {
                return;
            }
        }
    });
}

/// A task's title: the prompt's first line, cut.
pub(super) fn title(prompt: &str) -> String {
    let line = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Claude Code");
    let mut title: String = line.chars().take(TASK_TITLE_CHARS).collect();
    if line.chars().count() > TASK_TITLE_CHARS {
        title.push('…');
    }
    title
}

/// Ties `chat` to `environment` and adds `task` after its messages.
pub(super) fn record(chat: &mut Conversation, environment: ChatEnvironment, mut task: ChatTask) {
    task.after_message = chat.messages.len();
    chat.environment = Some(environment);
    chat.tasks.push(task);
    let extra = chat.tasks.len().saturating_sub(MAX_TASKS);
    chat.tasks.drain(..extra);
    chat.updated_unix = now();
}

#[derive(Deserialize)]
struct RunForm {
    csrf: String,
    environment: String,
    prompt: String,
}

/// The chat and its runnable offer, for the run page and its post.
async fn runnable(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    id: &str,
) -> Result<(Conversation, Offer), Response> {
    let loaded = load_owned(app, owner, id).await?;
    let chat = loaded.conversation;
    match offer(app, headers, &chat).await {
        Some(offer) if offer.runnable => Ok((chat, offer)),
        _ => Err(missing()),
    }
}

fn run_form(
    app: &App,
    headers: &HeaderMap,
    chat: &Conversation,
    offer: &Offer,
    prompt: &str,
    error: Option<&str>,
) -> Response {
    let id = &chat.id;
    let field = Field::new("chat-claude-prompt", "What should Claude Code do?").error_opt(error);
    let aria = field.aria();
    let field = field.control(
        Textarea::new("prompt")
            .id("chat-claude-prompt")
            .rows(4)
            .maxlength(MAX_PROMPT as u32)
            .value(prompt)
            .aria(aria),
    );
    let page = UiPage::new("Run Claude Code")
        .path(format!("/chat/{id}/claude"))
        .head(crate::chat_html::head())
        .breadcrumb(
            Breadcrumb::new("Run Claude Code").crumb(chat.title.clone(), format!("/chat/{id}")),
        )
        .content(crate::ui_page::prose(html! {
            h1 { "Run Claude Code" }
            p {
                "In " a href=(format!("/environments/{}", offer.id)) { (offer.label()) }
                ", on a fresh computer. It shows in this chat."
            }
            form method="post" action=(format!("/chat/{id}/claude")) {
                input type="hidden" name="csrf" value=(csrf(app, &chat.owner));
                input type="hidden" name="environment" value=(offer.id);
                (field)
                (Button::new("Run").kind(ButtonType::Submit))
            }
        }))
        .status(if error.is_some() {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::OK
        });
    crate::chat_html::protect(page.respond(headers))
}

async fn run_page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    match runnable(&app, &headers, &owner, &id).await {
        Ok((chat, offer)) => run_form(&app, &headers, &chat, &offer, "", None),
        Err(response) => response,
    }
}

async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RunForm>,
) -> Response {
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let (chat, offer) = match runnable(&app, &headers, &owner, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if offer.id != form.environment {
        return refusal(
            StatusCode::CONFLICT,
            "This chat's environment changed. Reload the page.",
        );
    }
    let Some(studio) = studio(&app).cloned() else {
        return missing();
    };
    let prompt = form.prompt.trim();
    if prompt.is_empty() {
        return run_form(
            &app,
            &headers,
            &chat,
            &offer,
            "",
            Some("Write what Claude Code should do."),
        );
    }
    let run = match studio.run_claude(&offer.id, prompt) {
        Ok(run) => run,
        Err(error) => return run_form(&app, &headers, &chat, &offer, prompt, Some(error.as_str())),
    };
    let version = studio
        .claude_run(&offer.id, &run)
        .and_then(|run| run.version)
        .or(offer.version);
    let environment = ChatEnvironment {
        id: offer.id.clone(),
        repository: offer.repository.clone(),
        version,
        removed: false,
    };
    let task = ChatTask {
        id: run,
        kind: TaskKind::Claude,
        environment: offer.id.clone(),
        title: title(prompt),
        state: TaskState::Working,
        started_unix: now(),
        after_message: 0,
        version,
        finished_unix: None,
    };
    let recorded = sidebar::update(&app, &owner, &id, |chat| {
        record(chat, environment.clone(), task.clone());
        true
    })
    .await;
    if let Err(response) = recorded {
        return response;
    }
    watch(app, owner, id.clone());
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_environment_operator::studio::Status;

    fn summary(id: &str, repository: &str, saved: Option<u64>) -> Summary {
        Summary {
            id: id.into(),
            repository: repository.into(),
            branch: "main".into(),
            commit: "0".repeat(40),
            status: Status::Saved,
            saved,
            updated_ms: 0,
        }
    }

    fn chat() -> Conversation {
        Conversation {
            id: "11111111-1111-4111-8111-111111111111".into(),
            owner: "v_owner".into(),
            revision: 1,
            title: "Fix the login".into(),
            messages: vec![Message {
                role: Role::User,
                text: "hi".into(),
                request_id: None,
            }],
            pending: None,
            requests: Vec::new(),
            selection: None,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            terminal: None,
            environment: None,
            tasks: Vec::new(),
            opened_unix: None,
        }
    }

    fn task(id: &str, state: TaskState, after: usize) -> ChatTask {
        ChatTask {
            id: id.into(),
            kind: TaskKind::Claude,
            environment: "env-1".into(),
            title: "Fix the login redirect".into(),
            state,
            started_unix: 1,
            after_message: after,
            version: Some(3),
            finished_unix: None,
        }
    }

    fn linked() -> ChatEnvironment {
        ChatEnvironment {
            id: "env-1".into(),
            repository: "acme/app".into(),
            version: Some(3),
            removed: false,
        }
    }

    #[test]
    fn a_chat_picks_its_tied_environment_else_a_saved_one_for_its_repository() {
        let rows = [
            summary("env-0", "acme/app", None),
            summary("env-2", "Acme/App", Some(2)),
            summary("env-1", "acme/other", Some(1)),
        ];
        let mut chat = chat();
        assert_eq!(pick(&rows, &chat, None), None);
        assert_eq!(
            pick(&rows, &chat, Some("acme/app")).map(|r| r.id.as_str()),
            Some("env-2")
        );
        chat.environment = Some(linked());
        assert_eq!(
            pick(&rows, &chat, Some("acme/app")).map(|r| r.id.as_str()),
            Some("env-1")
        );
        chat.environment.as_mut().unwrap().removed = true;
        assert_eq!(
            pick(&rows, &chat, Some("acme/app")).map(|r| r.id.as_str()),
            Some("env-2")
        );
    }

    #[test]
    fn recording_a_task_ties_the_environment_and_keeps_the_newest_tasks() {
        let mut chat = chat();
        record(
            &mut chat,
            linked(),
            task("claude-env-1-1", TaskState::Working, 0),
        );
        assert_eq!(chat.environment, Some(linked()));
        assert_eq!(chat.tasks[0].after_message, 1);
        assert!(running(&chat));
        for n in 2..=(MAX_TASKS + 3) {
            record(
                &mut chat,
                linked(),
                task(&format!("claude-env-1-{n}"), TaskState::Done, 0),
            );
        }
        assert_eq!(chat.tasks.len(), MAX_TASKS);
        assert_eq!(chat.tasks[0].id, "claude-env-1-4");
    }

    #[test]
    fn observing_runs_changes_only_what_moved() {
        let mut chat = chat();
        chat.tasks = vec![
            task("a", TaskState::Done, 1),
            task("b", TaskState::Working, 1),
            task("c", TaskState::Working, 1),
        ];
        let seen = |state, version| {
            Some(Seen {
                state,
                version,
                reply: None,
            })
        };
        let same = observe(&chat, |_, run| match run {
            "b" => seen(TaskState::Working, Some(3)),
            _ => None,
        });
        assert!(same.is_none());
        let next = observe(&chat, |_, run| match run {
            "a" => seen(TaskState::Failed, None),
            "b" => seen(TaskState::Failed, Some(4)),
            _ => None,
        })
        .expect("b moved");
        assert_eq!(next.tasks[0].state, TaskState::Done, "finished tasks stay");
        assert_eq!(next.tasks[0].finished_unix, None);
        assert_eq!(next.tasks[1].state, TaskState::Failed);
        assert!(next.tasks[1].finished_unix.is_some(), "the finish is timed");
        assert_eq!(next.tasks[1].version, Some(4));
        assert_eq!(
            next.tasks[2].state,
            TaskState::Working,
            "unread runs keep theirs"
        );
        assert_eq!(state(RunState::Starting), TaskState::Working);
        assert_eq!(state(RunState::Paused), TaskState::Paused);
        assert_eq!(state(RunState::Stopped), TaskState::Stopped);
    }

    #[test]
    fn line_two_and_the_row_status_follow_the_tasks() {
        let mut chat = chat();
        assert_eq!(detail(&chat), None);
        chat.environment = Some(linked());
        assert_eq!(detail(&chat).as_deref(), Some("Environment v3"));
        chat.tasks = vec![task("a", TaskState::Working, 1)];
        assert_eq!(row_status(&chat), Some(ChatStatus::Working));
        chat.tasks[0].state = TaskState::Paused;
        assert_eq!(row_status(&chat), Some(ChatStatus::Working));
        chat.tasks[0].state = TaskState::Failed;
        assert_eq!(row_status(&chat), Some(ChatStatus::Failed));
        chat.messages.push(Message {
            role: Role::User,
            text: "again".into(),
            request_id: None,
        });
        assert_eq!(row_status(&chat), None, "a new message clears Failed");
        chat.tasks[0].state = TaskState::Done;
        assert_eq!(row_status(&chat), None, "no finish time, no Done");
        chat.tasks[0].finished_unix = Some(30);
        assert_eq!(row_status(&chat), None, "a quick task just finishes");
        chat.tasks[0].finished_unix = Some(61);
        assert_eq!(
            row_status(&chat),
            Some(ChatStatus::Done),
            "a long one says Done"
        );
        chat.opened_unix = Some(61);
        assert_eq!(row_status(&chat), None, "until the chat is opened");
        chat.opened_unix = Some(60);
        assert!(unseen_done(&chat), "an earlier opening doesn't count");
        chat.environment.as_mut().unwrap().removed = true;
        assert_eq!(detail(&chat).as_deref(), Some("Environment removed"));
    }

    #[test]
    fn task_rows_sit_after_their_message_and_link_only_when_allowed() {
        let mut chat = chat();
        chat.environment = Some(linked());
        chat.tasks = vec![task("claude-env-1-1", TaskState::Working, 1)];
        assert_eq!(rows(&chat, 0, true).into_string(), "");
        let html = rows(&chat, 1, true).into_string();
        assert!(
            html.contains(r#"href="/environments/env-1/runs/claude-env-1-1""#),
            "{html}"
        );
        assert!(html.contains("Claude Code") && html.contains("Fix the login redirect"));
        assert!(html.contains(">Working<"));
        assert!(!rows(&chat, 1, false).into_string().contains("href="));
        chat.environment.as_mut().unwrap().removed = true;
        assert!(!rows(&chat, 1, true).into_string().contains("href="));
        crate::copy_guard::assert_plain("/chat/x", &html);
    }

    #[test]
    fn the_header_names_the_environment_and_offers_a_run_only_when_it_can_start() {
        let chat = chat();
        let offer = Offer {
            id: "env-1".into(),
            repository: "acme/app".into(),
            version: Some(3),
            runnable: true,
        };
        let crumb = breadcrumb(&chat, Some(&offer)).render().into_string();
        assert!(crumb.contains(r#"href="/environments/env-1""#) && crumb.contains("acme/app v3"));
        assert!(
            !breadcrumb(&chat, None)
                .render()
                .into_string()
                .contains("href")
        );
        let on = actions(&chat, Some(&offer), false).into_string();
        assert!(
            on.contains(&format!(r#"href="/chat/{}/claude""#, chat.id)),
            "{on}"
        );
        let off = actions(
            &chat,
            Some(&Offer {
                runnable: false,
                ..offer
            }),
            true,
        )
        .into_string();
        assert!(!off.contains("href") && off.contains(r#"hx-swap-oob="outerHTML""#));
    }

    #[tokio::test]
    async fn running_claude_code_from_a_chat_stays_on_the_local_address() {
        use tower::ServiceExt;
        let dir = tempfile::tempdir().unwrap();
        let mut config = crate::Config::development(dir.path().join("tasks"));
        config.public_hosts = vec!["openagents.com".into()];
        let path = format!("/chat/{}/claude", chat().id);
        let send = |host: &'static str| {
            let config = config.clone();
            let path = path.clone();
            async move {
                let request = axum::http::Request::builder()
                    .uri(path)
                    .header(header::HOST, host)
                    .header("x-openagents-local", "1")
                    .body(axum::body::Body::empty())
                    .unwrap();
                crate::router(config)
                    .oneshot(request)
                    .await
                    .unwrap()
                    .status()
            }
        };
        assert_eq!(send("openagents.com").await, StatusCode::FORBIDDEN);
        // Locally, without environments set up, there is nothing to run.
        assert_eq!(send("127.0.0.1:4300").await, StatusCode::NOT_FOUND);
    }

    #[test]
    fn task_titles_are_one_short_line() {
        assert_eq!(title("\n  Fix it\nmore"), "Fix it");
        assert_eq!(title("   "), "Claude Code");
        assert_eq!(
            title(&"x".repeat(200)).chars().count(),
            TASK_TITLE_CHARS + 1
        );
    }
}
