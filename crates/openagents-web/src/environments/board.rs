//! A person's Claude Code runs on their environments as fleet rows
//! (#11307): the `Environments` list on `/settings/agents` and the phone's
//! `GET /v1/agents`, beside the account's computers and the work runs on
//! openagents.com's own machines ([`crate::work_runs`]).
//!
//! Every run of an environment the person owns is a row, whether it was
//! started on the environment's page or from a chat, while it works and
//! for a day after it ends: its title (the request's first line), its stage
//! (Starting, Claude Code is working, Paused, Done, Failed, Stopped), its
//! cost so far, and a link to its page. A working row takes Stop and
//! Message, the same as the run's own page: a message waits for the run's
//! next turn ([`coder_environment_operator::studio::claude::Runs::message`]),
//! and a run that has just ended starts a follow-up with it instead.
//!
//! A row's item id is `ENVIRONMENT/RUN`. A run a usage limit paused in a
//! chat keeps its chat row (`CHAT/RUN`, [`crate::pages::chat::paused_items`])
//! for its pause choices; Stop and Message take either id.

use axum::http::HeaderMap;
use coder_environment_operator::studio::Studio;
use coder_environment_operator::studio::claude::{self, RunState, Sent};

use crate::App;
use crate::cloud::session::Viewer;
use crate::coder_sync::line;
use crate::phone_api::{self, Item};

/// The list the rows show on, beside the account's computers.
pub(crate) const BOARD: &str = phone_api::PAUSED_BOARD;
/// How long an ended run stays on the list, in seconds.
const ENDED_SECONDS: u64 = 24 * 3600;

/// The person's environment runs as rows: the ones going and the ones
/// that ended in the last day, newest first. `paused` are the chat rows of
/// paused chat runs ([`crate::pages::chat::paused_items`]), which stand in
/// for those runs' own rows.
pub(crate) fn items(studio: &Studio, account: &str, paused: Vec<Item>, now: u64) -> Vec<Item> {
    let mut rows: Vec<(u64, Item)> = Vec::new();
    let mut stand_ins = paused;
    for env in studio
        .list()
        .into_iter()
        .filter(|env| env.account.as_deref() == Some(account))
    {
        for run in studio.claude_runs(&env.id) {
            let ended_at = run.updated_ms / 1000;
            if run.state.finished() && now.saturating_sub(ended_at) >= ENDED_SECONDS {
                continue;
            }
            let at = if run.state.finished() { ended_at } else { now };
            let chat = stand_ins
                .iter()
                .position(|item| item.id.rsplit_once('/').is_some_and(|(_, id)| id == run.id));
            let row = match chat {
                Some(index) => stand_ins.remove(index),
                None => {
                    let waiting = studio.claude_waiting(&env.id, &run.id).len();
                    item(&run, waiting, now)
                }
            };
            rows.push((at, row));
        }
    }
    // Paused chat runs of environments listed under no account here.
    rows.extend(stand_ins.into_iter().map(|item| (now, item)));
    rows.sort_by_key(|(at, item)| std::cmp::Reverse((*at, item.started_unix)));
    rows.into_iter()
        .take(phone_api::MAX_ITEMS)
        .map(|(_, item)| item)
        .collect()
}

/// The rows of the signed-in `account` (with the phone's or the page's
/// `viewer`, for a paused chat run's choices), when this site has
/// environments.
pub(crate) async fn rows(app: &App, account: &str, viewer: Option<&Viewer>) -> Vec<Item> {
    let Some(studio) = app.config.environments.get() else {
        return Vec::new();
    };
    let owner = crate::chat_store::account_owner(account);
    let paused = crate::pages::chat::paused_items(app, &owner, viewer).await;
    items(&studio, account, paused, crate::chat_store::now_unix())
}

/// The page a row opens: its run's page, or the chat a paused chat run
/// is in.
pub(crate) fn row_path(item: &str) -> Option<String> {
    run_path(item).or_else(|| {
        let (chat, run) = item.split_once('/')?;
        (super::valid_id(chat) && super::valid_id(run)).then(|| format!("/chat/{chat}"))
    })
}

/// One run as a row; `waiting` messages wait for its next turn.
pub(crate) fn item(run: &claude::Run, waiting: usize, now: u64) -> Item {
    let status = match run.state {
        RunState::Starting | RunState::Running | RunState::Paused => "working",
        RunState::Done => "done",
        RunState::Failed => "failed",
        RunState::Stopped => "stopped",
    };
    let engine = if run.openagents {
        "OpenAgents"
    } else {
        "Claude Code"
    };
    let mut stage = stage(run);
    if waiting > 0 && !run.state.finished() {
        stage.push_str(&match waiting {
            1 => " 1 message waits for its next turn.".to_owned(),
            n => format!(" {n} messages wait for its next turn."),
        });
    }
    Item {
        id: format!("{}/{}", run.environment, run.id),
        kind: "agent".into(),
        title: line(first_line(&run.prompt), 120),
        engine: Some(engine.into()),
        status: status.into(),
        started_unix: run.created_ms / 1000,
        finished_unix: run
            .state
            .finished()
            .then(|| (run.updated_ms / 1000).min(now)),
        cost_usd: run.cost_usd,
        tokens: None,
        session: None,
        question: None,
        line: Some(line(&stage, 200)),
        choices: Vec::new(),
    }
}

/// Where a run is, in the rows' words.
pub(crate) fn stage(run: &claude::Run) -> String {
    match run.state {
        RunState::Starting => "Starting.".into(),
        RunState::Running if run.openagents => "OpenAgents is working.".into(),
        RunState::Running => "Claude Code is working.".into(),
        RunState::Paused => {
            let when = run.paused_until.map_or_else(
                || "until the limit resets".to_owned(),
                |at| format!("until {:02}:{:02} UTC", at / 3600 % 24, at / 60 % 60),
            );
            format!("Paused {when} on the Claude plan limit.")
        }
        RunState::Done => "Done.".into(),
        RunState::Failed => {
            let why = run.error.as_deref().unwrap_or("The run failed.");
            format!("Failed: {}", first_line(why))
        }
        RunState::Stopped => "Stopped.".into(),
    }
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("Claude Code run")
}

/// The run page a row's id names.
pub(crate) fn run_path(item: &str) -> Option<String> {
    let (env, run) = item.split_once('/')?;
    (env.starts_with("env-") && super::valid_id(env) && super::valid_id(run))
        .then(|| format!("/environments/{env}/runs/{run}"))
}

/// The environment and run a row's id names, when the run is `account`'s:
/// `ENVIRONMENT/RUN`, or a paused chat row's `CHAT/RUN`.
pub(crate) async fn resolve(
    app: &App,
    studio: &Studio,
    account: &str,
    owner: &str,
    item: &str,
) -> Option<(String, String)> {
    let (first, run) = item.split_once('/')?;
    if !super::valid_id(first) || !super::valid_id(run) {
        return None;
    }
    let env = if studio.account(first).is_some() {
        first.to_owned()
    } else {
        let chat = app.config.chat_store.load(owner, first).await.ok()??;
        chat.conversation
            .tasks
            .iter()
            .find(|task| task.id == run)?
            .environment
            .clone()
    };
    (studio.account(&env).as_deref() == Some(account) && studio.claude_run(&env, run).is_some())
        .then(|| (env, run.to_owned()))
}

/// Whose Claude key and GitHub access a follow-up run takes.
pub(crate) enum Asker<'a> {
    /// The website: the signed-in session in these headers.
    Web(&'a HeaderMap),
    /// The phone: its own signed-in view, and its request's headers.
    App(&'a Viewer, &'a HeaderMap),
}

/// What a message to a run did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Messaged {
    /// It waits for the run's next turn.
    NextTurn,
    /// The run had ended; this new run starts with the message.
    FollowUp(String),
}

impl Messaged {
    /// What the person reads after sending.
    pub(crate) fn words(&self) -> &'static str {
        match self {
            Self::NextTurn => {
                "Sent. Claude Code can't read messages in the middle of a turn, so it reads yours when its current turn ends, then carries on."
            }
            Self::FollowUp(_) => "That run had ended, so a new run started with your message.",
        }
    }
}

/// Send `text` to run `run` of environment `env`: for its next turn, or,
/// when it has ended, as a follow-up run on a fresh computer.
pub(crate) async fn message(
    app: &App,
    studio: &Studio,
    asker: Asker<'_>,
    env: &str,
    run: &str,
    text: &str,
) -> Result<Messaged, String> {
    if secret_screen::credential_in(text).is_some() {
        return Err("This looks like it holds a password or key, so it wasn't sent.".into());
    }
    match studio.message_claude(env, run, text)? {
        Sent::NextTurn => Ok(Messaged::NextTurn),
        Sent::Ended => {
            let previous = studio
                .claude_run(env, run)
                .ok_or("That run isn't there anymore.")?;
            let (own, headers) = match asker {
                Asker::Web(headers) => (crate::cloud::byo::run_key(app, headers).await, headers),
                Asker::App(viewer, headers) => {
                    (crate::cloud::byo::run_key_for(app, viewer), headers)
                }
            };
            let push = super::push::access(app, headers, studio, env).await;
            let prompt = follow_up_prompt(&previous, text);
            studio
                .run_claude_pushing(env, &prompt, own, Vec::new(), push)
                .map(Messaged::FollowUp)
        }
    }
}

/// The longest report of the previous run a follow-up carries.
const REPORT_CHARS: usize = 3000;

/// A follow-up run's request: the person's message, after what the run it
/// follows was asked and reported.
pub(crate) fn follow_up_prompt(previous: &claude::Run, text: &str) -> String {
    let mut prompt = format!(
        "{}\n\nThis follows an earlier Claude Code run on this environment. It was asked:\n{}",
        text.trim(),
        previous.prompt.trim()
    );
    let report = previous
        .reply
        .as_deref()
        .or(previous.error.as_deref())
        .unwrap_or("")
        .trim();
    if !report.is_empty() {
        let cut: String = report.chars().take(REPORT_CHARS).collect();
        prompt.push_str("\n\nWhat it reported:\n");
        prompt.push_str(&cut);
    }
    prompt.push_str(
        "\n\nIts changes are on this computer only if it pushed them: if it named a branch, fetch that branch and check it out first.",
    );
    prompt.chars().take(claude::MAX_PROMPT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: RunState) -> claude::Run {
        claude::Run {
            id: "claude-env-abc-1".into(),
            environment: "env-abc".into(),
            prompt: "Fix the login bug\nwith care".into(),
            version: Some(1),
            state,
            events: Vec::new(),
            reply: None,
            error: None,
            created_ms: 1_000_000,
            updated_ms: 2_000_000,
            cost_usd: Some(0.42),
            paused_until: None,
            openagents: false,
        }
    }

    #[test]
    fn a_run_reads_as_a_row_with_its_stage_and_cost() {
        let working = item(&run(RunState::Running), 0, 5_000);
        assert_eq!(working.id, "env-abc/claude-env-abc-1");
        assert_eq!(working.title, "Fix the login bug");
        assert_eq!(working.status, "working");
        assert_eq!(working.engine.as_deref(), Some("Claude Code"));
        assert_eq!(working.line.as_deref(), Some("Claude Code is working."));
        assert_eq!(working.cost_usd, Some(0.42));
        assert_eq!(working.finished_unix, None);
        let waiting = item(&run(RunState::Running), 2, 5_000);
        assert!(
            waiting
                .line
                .as_deref()
                .is_some_and(|l| l.ends_with("2 messages wait for its next turn.")),
            "{waiting:?}"
        );
        let starting = item(&run(RunState::Starting), 0, 5_000);
        assert_eq!(starting.line.as_deref(), Some("Starting."));
        let done = item(&run(RunState::Done), 0, 5_000);
        assert_eq!(done.status, "done");
        assert_eq!(done.finished_unix, Some(2_000));
        let mut failed = run(RunState::Failed);
        failed.error = Some("Claude Code is not logged in.\nmore".into());
        let failed = item(&failed, 0, 5_000);
        assert_eq!(failed.status, "failed");
        assert_eq!(
            failed.line.as_deref(),
            Some("Failed: Claude Code is not logged in.")
        );
        let stopped = item(&run(RunState::Stopped), 1, 5_000);
        assert_eq!(stopped.status, "stopped");
        assert_eq!(stopped.line.as_deref(), Some("Stopped."));
        let mut paused = run(RunState::Paused);
        paused.paused_until = Some(50_700);
        let paused = item(&paused, 0, 5_000);
        assert_eq!(paused.status, "working");
        assert_eq!(
            paused.line.as_deref(),
            Some("Paused until 14:05 UTC on the Claude plan limit.")
        );
        for row in [working, done, failed, stopped, paused] {
            assert!(
                oa_copy::violations(row.line.as_deref().unwrap_or(""), &[]).is_empty(),
                "{row:?}"
            );
        }
    }

    #[test]
    fn a_row_links_to_its_run_page() {
        assert_eq!(
            run_path("env-abc/claude-env-abc-1").as_deref(),
            Some("/environments/env-abc/runs/claude-env-abc-1")
        );
        assert_eq!(run_path("chat-1/claude-env-abc-1"), None);
        assert_eq!(run_path("env-abc/../x"), None);
        assert_eq!(run_path("wrk0123"), None);
    }

    #[test]
    fn a_follow_up_carries_the_message_then_what_came_before() {
        let mut previous = run(RunState::Done);
        previous.reply = Some("Pushed branch fix-login.".into());
        let prompt = follow_up_prompt(&previous, "  Also add a test. ");
        assert!(prompt.starts_with("Also add a test.\n\n"), "{prompt}");
        assert!(prompt.contains("Fix the login bug"));
        assert!(prompt.contains("What it reported:\nPushed branch fix-login."));
        assert!(prompt.contains("fetch that branch"));
    }
}
