//! The Grid Foreman's commands to agents that open draft pull requests
//! (#12448).
//!
//! | Route | Caller | What |
//! | --- | --- | --- |
//! | `POST /v1/foreman/commands` | the Foreman's host, `Authorization: Nostr <base64 event>` | One signed command: `agent.start` an issue, or `agent.draft_pr` an agent it started |
//! | `GET /v1/foreman/commands` | anyone | Whether commands are on, today's counts, the agents it runs, open requests, and the audit log |
//! | `POST /v1/foreman/commands/pause` `{paused}` | an admin | Pause or resume every command at once |
//! | `POST /v1/foreman/commands/requests` `{issue}` | an admin | Ask the Foreman to start an agent on an issue |
//!
//! The authority is narrow by construction. A command is a NIP-98 event
//! (kind 27235) over this route's exact URL and `POST`, with a `payload`
//! tag holding the body's SHA-256, signed by the Foreman's pinned key
//! within [`grid_foreman::COMMAND_FRESH_SECONDS`], and never seen before.
//! Its body names one of [`grid_foreman::ACTIONS`] and an issue number;
//! there is no word for merging, closing, deploying, paying, steering, or
//! free text. Before acting, the site checks the issue (open, an issue and
//! not a pull request, unassigned, no denied label, no running Foreman
//! agent on it), the caps (5 starts and 10 draft asks a UTC day, 3 running
//! at once, one command a minute), the kill switch (on unless
//! `OPENAGENTS_FOREMAN_COMMANDS=off`, owner 2026-10-22: nothing ships turned
//! off; and an admin's pause), and the worker's definition (no standing
//! authority). Every command, accepted or refused, goes to the audit log.
//!
//! A start is an agent assignment on a computer of the worker account, so
//! it runs as an app chat of its own that the desktop Foreman, the phone,
//! and the website can see and steer, with the fixed brief
//! [`grid_foreman::brief`]: claim the issue, work in a worktree, open a
//! draft pull request, and never merge, deploy, or spend. A draft ask sends
//! that chat [`grid_foreman::DRAFT_PR_TEXT`]. Merging, deploying, and
//! spending stay with an admin.
//!
//! The worker account is `OPENAGENTS_FOREMAN_OWNER` (an account ID) when
//! set, else the admin who last published the desktop Foreman's record or
//! asked for an agent. Its worker agent is `_WORKER_AGENT` when set, else
//! one the site saves in that account ([`worker_spec`]: Claude Code, the
//! computer's own access, no standing authority). Its computer is
//! `_WORKER_HOST` (and `_WORKER_PROJECT`) when set, else the account's online
//! computer that runs the worker, preferring the `openagents` project.

mod objectives;

use std::collections::BTreeSet;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use base64::Engine as _;
use nostr::domain::Event;
use openagents_sdk::agents::assignments::{
    AssignmentAction, AssignmentCommand, AssignmentSpec, AssignmentState, CreateAssignment,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::agent_assignments::Change;
use crate::chat_store::Store;
use crate::coder_sync::{answer, refused};

/// Commands run only when this is `on`.
const SWITCH_ENV: &str = "OPENAGENTS_FOREMAN_COMMANDS";
/// The account whose computer runs the Foreman's agents.
const OWNER_ENV: &str = "OPENAGENTS_FOREMAN_OWNER";
/// The owner's saved agent that does the work.
const WORKER_AGENT_ENV: &str = "OPENAGENTS_FOREMAN_WORKER_AGENT";
/// The owner's computer it runs on.
const WORKER_HOST_ENV: &str = "OPENAGENTS_FOREMAN_WORKER_HOST";
/// The app project it runs in, when one is named.
const WORKER_PROJECT_ENV: &str = "OPENAGENTS_FOREMAN_WORKER_PROJECT";
/// Origins a command's `u` tag may name; `https://openagents.com` unless set.
const ORIGINS_ENV: &str = "OPENAGENTS_FOREMAN_ORIGINS";
/// The NIP-98 event kind.
const HTTP_AUTH: u16 = 27_235;
/// The longest body or header accepted.
const MAX_BYTES: usize = 8 * 1024;
/// The most log entries a day keeps.
const LOG_MAX: usize = 400;
/// The most open requests kept.
const REQUESTS_MAX: usize = 20;
/// An agent the Foreman started counts as running for at most this long
/// when its assignment can't be read, s.
const RUNNING_FOR: u64 = 24 * 60 * 60;

/// One command's body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Command {
    pub schema: String,
    pub action: String,
    #[serde(default)]
    pub issue: u64,
    #[serde(default)]
    pub exploration: Option<objectives::Exploration>,
    /// For `agent.draft_pr`: the assignment it started.
    #[serde(default)]
    pub assignment: Option<String>,
}

/// One audit log entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub at: u64,
    /// The command event's ID, when the command was signed well enough to
    /// have one.
    #[serde(default)]
    pub event: Option<String>,
    pub action: String,
    pub issue: u64,
    #[serde(default)]
    pub objective: Option<String>,
    /// `started`, `asked`, or `refused`.
    pub result: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub assignment: Option<String>,
    #[serde(default)]
    pub chat: Option<String>,
}

/// An admin's request for an agent on an issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Request {
    pub issue: u64,
    pub at: u64,
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(grid_foreman::COMMANDS_PATH, get(overview).post(command))
        .route("/v1/foreman/commands/pause", post(pause))
        .route("/v1/foreman/commands/requests", post(request))
}

fn day(now: u64) -> u64 {
    now / 86_400
}

fn log_key(day: u64) -> String {
    format!("grid-foreman/commands/log-{day}.json")
}

const PAUSE_KEY: &str = "grid-foreman/commands/paused.json";
const REQUESTS_KEY: &str = "grid-foreman/commands/requests.json";

async fn read<T: for<'de> Deserialize<'de> + Default>(
    store: &Store,
    key: &str,
) -> (T, Option<String>) {
    match store.read_key(key).await {
        Ok(Some((bytes, generation))) => (
            serde_json::from_slice(&bytes).unwrap_or_default(),
            Some(generation),
        ),
        _ => (T::default(), None),
    }
}

/// Changes the JSON document at `key` with `f`, retrying a concurrent write.
async fn edit<T, R>(store: &Store, key: &str, mut f: impl FnMut(&mut T) -> R) -> Option<R>
where
    T: for<'de> Deserialize<'de> + Serialize + Default,
{
    for _ in 0..8 {
        let (mut value, generation) = read::<T>(store, key).await;
        let out = f(&mut value);
        let bytes = serde_json::to_vec(&value).ok()?;
        if store
            .write_key(key, bytes, generation.as_deref())
            .await
            .is_ok()
        {
            return Some(out);
        }
    }
    None
}

/// Today's and yesterday's log, oldest first.
pub(crate) async fn recent(store: &Store, now: u64) -> Vec<Entry> {
    let (mut out, _) = read::<Vec<Entry>>(store, &log_key(day(now).saturating_sub(1))).await;
    let (today, _) = read::<Vec<Entry>>(store, &log_key(day(now))).await;
    out.extend(today);
    if let Ok(entries) = objectives::entries(store).await {
        for entry in entries {
            if !out
                .iter()
                .any(|e| e.result == "started" && e.assignment == entry.assignment)
            {
                out.push(entry);
            }
        }
    }
    out
}

async fn log(store: &Store, entry: Entry) {
    let key = log_key(day(entry.at));
    let _ = edit::<Vec<Entry>, ()>(store, &key, |log| {
        log.push(entry.clone());
        if log.len() > LOG_MAX {
            let extra = log.len() - LOG_MAX;
            log.drain(..extra);
        }
    })
    .await;
}

/// Commands are on unless the switch says `off`.
fn enabled() -> bool {
    !std::env::var(SWITCH_ENV).is_ok_and(|v| v.trim().eq_ignore_ascii_case("off"))
}

/// Where the worker account the admins chose is kept.
const OWNER_KEY: &str = "grid-foreman/commands/owner.json";

/// Remembers `account_id`, an admin's, as the worker account, unless the
/// environment names one.
pub(crate) async fn remember_owner(store: &Store, account_id: &str) {
    if account_id.is_empty() {
        return;
    }
    let account = account_id.to_owned();
    let _ = edit::<Value, ()>(store, OWNER_KEY, |v| {
        *v = json!({"account_id": account});
    })
    .await;
}

/// The worker agent the site saves when none is named.
pub(super) fn dispatch_brief(brief: String) -> Result<String, &'static str> {
    match std::env::var("OPENAGENTS_FOREMAN_KILN_STATE") {
        Ok(state) => {
            grid_foreman::objectives::kiln_brief(brief, state.trim()).map_err(|_| "foreman_unsetup")
        }
        Err(std::env::VarError::NotPresent) => Ok(brief),
        Err(_) => Err("foreman_unsetup"),
    }
}

pub(crate) fn worker_spec() -> openagents_sdk::agents::AgentSpec {
    openagents_sdk::agents::AgentSpec {
        name: "Grid Foreman worker".into(),
        description: "Works one issue or standing objective and opens a draft pull request.".into(),
        instructions:
            "I work the one GitHub issue or objective my first message names, in this chat, and \
follow that message exactly: claim the issue, work in a git worktree of my own, open a draft pull \
request, and reply with its link. I never push to main, merge, deploy, spend money, or test on \
the owner's account. If I stop before a pull request, I release the claim."
                .into(),
        runtime: "claude-code".into(),
        model: None,
        access: openagents_sdk::agents::Access::HostDefault,
        fallbacks: vec![],
        autonomy: None,
    }
}

/// The worker for this command: the environment's, else the remembered
/// account with its saved (or newly saved) worker on an online computer.
async fn resolve_worker(store: &Store) -> Option<Worker> {
    let env = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let account = match env(OWNER_ENV) {
        Some(account) => account,
        None => read::<Value>(store, OWNER_KEY).await.0["account_id"]
            .as_str()?
            .to_owned(),
    };
    let owner = crate::chat_store::account_owner(&account);
    let agent = match env(WORKER_AGENT_ENV) {
        Some(agent) => agent,
        None => {
            let id = to_hex(&Sha256::digest(format!(
                "openagents.grid-foreman.worker.v1\0{owner}"
            )));
            match crate::custom_agents::definition(store, &owner, &id, None).await {
                Ok(Ok(d)) if !d.deleted => id,
                _ => {
                    crate::custom_agents::mutate(
                        store,
                        &owner,
                        crate::custom_agents::Mutation::Create {
                            request_id: id.clone(),
                            spec: worker_spec(),
                        },
                    )
                    .await
                    .ok()?
                    .ok()?;
                    id
                }
            }
        }
    };
    if let Some(host) = env(WORKER_HOST_ENV) {
        return Some((owner, agent, host, env(WORKER_PROJECT_ENV)));
    }
    let definition = match crate::custom_agents::definition(store, &owner, &agent, None).await {
        Ok(Ok(d)) => d,
        _ => return None,
    };
    let hosts = crate::agent_lab::online_hosts(store, &owner).await.ok()?;
    let (host, project) =
        openagents_sdk::agents::place::place(&hosts, &definition, None, None, Some("openagents"))
            .ok()?;
    Some((owner, agent, host.id, Some(project)))
}

async fn paused(store: &Store) -> bool {
    let (value, _) = read::<Value>(store, PAUSE_KEY).await;
    value["paused"].as_bool().unwrap_or(false)
}

fn origins() -> Vec<String> {
    std::env::var(ORIGINS_ENV)
        .unwrap_or_else(|_| "https://openagents.com".into())
        .split(',')
        .map(|o| o.trim().trim_end_matches('/').to_owned())
        .filter(|o| !o.is_empty())
        .collect()
}

/// The Foreman's key commands must be signed by.
fn foreman_key() -> String {
    std::env::var("OPENAGENTS_FOREMAN_PUBKEY")
        .ok()
        .map(|k| k.trim().to_ascii_lowercase())
        .filter(|k| k.len() == 64)
        .unwrap_or_else(|| grid_foreman::PUBKEY.to_owned())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks the command's NIP-98 signature in `authorization` over `body` at
/// `now`, for `key` and one of `origins`, returning its event ID or the
/// refusal code.
pub(crate) fn verify(
    authorization: Option<&str>,
    body: &[u8],
    now: u64,
    key: &str,
    origins: &[String],
) -> Result<String, &'static str> {
    let token = authorization
        .and_then(|h| h.strip_prefix("Nostr "))
        .filter(|t| t.len() <= MAX_BYTES)
        .ok_or("foreman_unsigned")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(token.trim())
        .map_err(|_| "foreman_unsigned")?;
    let event: Event = serde_json::from_slice(&bytes).map_err(|_| "foreman_unsigned")?;
    if event.kind != HTTP_AUTH || event.validate_id().is_err() || event.validate_crypto().is_err() {
        return Err("foreman_bad_signature");
    }
    if event.pubkey != key {
        return Err("foreman_wrong_key");
    }
    if event.created_at.abs_diff(now) > grid_foreman::COMMAND_FRESH_SECONDS {
        return Err("foreman_stale");
    }
    let url: Vec<&str> = event.tag_values("u").collect();
    let method: Vec<&str> = event.tag_values("method").collect();
    let payload: Vec<&str> = event.tag_values("payload").collect();
    let url_ok = url.len() == 1
        && origins
            .iter()
            .any(|o| url[0] == format!("{o}{}", grid_foreman::COMMANDS_PATH));
    if !url_ok || method != ["POST"] {
        return Err("foreman_bad_signature");
    }
    if payload != [to_hex(&Sha256::digest(body)).as_str()] {
        return Err("foreman_bad_signature");
    }
    Ok(event.id)
}

/// What the site reads about an issue before starting an agent on it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct IssueFacts {
    pub open: bool,
    pub pull_request: bool,
    pub assigned: bool,
    pub labels: Vec<String>,
}

/// Why an issue can't take a new agent, if it can't.
pub(crate) fn issue_refusal(facts: &IssueFacts) -> Option<&'static str> {
    if !facts.open {
        Some("foreman_issue_closed")
    } else if facts.pull_request {
        Some("foreman_not_an_issue")
    } else if facts.assigned {
        Some("foreman_issue_claimed")
    } else if facts.labels.iter().any(|l| {
        grid_foreman::DENY_LABELS
            .iter()
            .any(|d| l.eq_ignore_ascii_case(d))
    }) {
        Some("foreman_issue_denied")
    } else {
        None
    }
}

async fn issue_facts(issue: u64) -> Result<IssueFacts, &'static str> {
    let token = std::env::var("OPENAGENTS_FOREMAN_GITHUB_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());
    let body = crate::grid_foreman::get_json(
        &reqwest::Client::new(),
        &format!(
            "https://api.github.com/repos/{}/issues/{issue}",
            crate::github_refs::DEFAULT_REPOSITORY
        ),
        &[],
        token.as_deref(),
    )
    .await
    .map_err(|_| "foreman_issue_unreadable")?;
    Ok(IssueFacts {
        open: body["state"] == "open",
        pull_request: body.get("pull_request").is_some(),
        assigned: body["assignees"].as_array().is_some_and(|a| !a.is_empty()),
        labels: body["labels"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|l| l["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Today's counts and the gap since the last accepted command, from `log`.
pub(crate) fn caps(log: &[Entry], now: u64, action: &str) -> Option<&'static str> {
    let today: Vec<&Entry> = log
        .iter()
        .filter(|e| day(e.at) == day(now) && (e.result == "started" || e.result == "asked"))
        .collect();
    let started = today
        .iter()
        .filter(|e| {
            matches!(
                e.action.as_str(),
                grid_foreman::START | grid_foreman::EXPLORE
            )
        })
        .count();
    let asked = today
        .iter()
        .filter(|e| e.action == grid_foreman::DRAFT_PR)
        .count();
    if log
        .iter()
        .filter(|e| e.result == "started" || e.result == "asked")
        .any(|e| now.saturating_sub(e.at) < grid_foreman::COMMAND_GAP_SECONDS)
    {
        return Some("foreman_too_soon");
    }
    match action {
        grid_foreman::START | grid_foreman::EXPLORE if started >= grid_foreman::STARTS_PER_DAY => {
            Some("foreman_cap")
        }
        grid_foreman::DRAFT_PR if asked >= grid_foreman::DRAFT_PRS_PER_DAY => Some("foreman_cap"),
        _ => None,
    }
}

/// The agents the Foreman started that are still going: their log entries
/// whose assignment `state` says so (or, unread, that are under a day old).
pub(crate) fn running<'a>(
    log: &'a [Entry],
    now: u64,
    state: impl Fn(&str) -> Option<AssignmentState>,
) -> Vec<&'a Entry> {
    log.iter()
        .filter(|e| e.result == "started")
        .filter(|e| {
            let Some(id) = &e.assignment else {
                return false;
            };
            match state(id) {
                Some(s) => matches!(s, AssignmentState::Queued | AssignmentState::Running),
                None => now.saturating_sub(e.at) < RUNNING_FOR,
            }
        })
        .collect()
}

/// Where the Foreman's agents run: the owner, the worker agent, the
/// computer, and the project.
type Worker = (String, String, String, Option<String>);

/// What a command is checked against, from the environment.
pub(crate) struct Setup {
    pub enabled: bool,
    pub key: String,
    pub origins: Vec<String>,
    pub worker: Option<Worker>,
}

impl Setup {
    fn from_env() -> Self {
        Self {
            enabled: enabled(),
            key: foreman_key(),
            origins: origins(),
            worker: None,
        }
    }
}

fn no(code: &'static str) -> Response {
    let (status, message) = match code {
        "foreman_off" | "foreman_paused" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The Foreman's commands are off right now.",
        ),
        "foreman_unsigned"
        | "foreman_bad_signature"
        | "foreman_wrong_key"
        | "foreman_stale"
        | "foreman_replayed" => (StatusCode::UNAUTHORIZED, "Only the Foreman can send this."),
        "foreman_cap" | "foreman_too_soon" | "foreman_running" => (
            StatusCode::TOO_MANY_REQUESTS,
            "The Foreman has started enough agents for now.",
        ),
        "foreman_unsetup" | "foreman_worker_unsafe" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The Foreman's worker isn't set up.",
        ),
        "foreman_backlog_unknown" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The Foreman couldn't check GitHub. Try again later.",
        ),
        "foreman_catalog_invalid"
        | "foreman_catalog_conflict"
        | "foreman_objectives_unreadable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "The Foreman couldn't read its objectives. Try again later.",
        ),
        "foreman_objective_conflict" => (
            StatusCode::CONFLICT,
            "That task changed. Refresh and try again.",
        ),
        "foreman_issue_available" => (
            StatusCode::CONFLICT,
            "The Foreman has an issue to work on first.",
        ),
        _ => (
            StatusCode::CONFLICT,
            "The Foreman can't do that for this issue.",
        ),
    };
    refused(status, code, message)
}

async fn command(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let store = app.config.chat_store.clone();
    let now = crate::chat_store::now_unix();
    let parsed: Option<Command> = serde_json::from_slice(&body).ok();
    let mut entry = Entry {
        at: now,
        action: parsed
            .as_ref()
            .map(|c| c.action.chars().take(32).collect())
            .unwrap_or_default(),
        issue: parsed.as_ref().map_or(0, |c| c.issue),
        result: "refused".into(),
        ..Entry::default()
    };
    let outcome = act(
        &store,
        &Setup::from_env(),
        &headers,
        &body,
        parsed,
        now,
        None,
        &mut entry,
    )
    .await;
    if let Err(code) = &outcome {
        entry.code = Some((*code).to_owned());
        app.config.analytics.event("foreman_command", "refused");
    } else {
        app.config.analytics.event(
            "foreman_command",
            if matches!(
                entry.action.as_str(),
                grid_foreman::START | grid_foreman::EXPLORE
            ) {
                "start"
            } else {
                "draft_pr"
            },
        );
    }
    // Unsigned noise isn't logged; everything with a valid signature is.
    if entry.event.is_some() || outcome.is_ok() {
        log(&store, entry.clone()).await;
    }
    match outcome {
        Ok(()) => answer(StatusCode::OK, json!({"ok": true, "entry": entry})),
        Err(code) => no(code),
    }
}

/// Checks and runs one command; `facts`, when given, stands in for reading
/// the issue from GitHub.
#[allow(clippy::too_many_arguments)]
async fn act(
    store: &Store,
    setup: &Setup,
    headers: &HeaderMap,
    body: &[u8],
    parsed: Option<Command>,
    now: u64,
    facts: Option<IssueFacts>,
    entry: &mut Entry,
) -> Result<(), &'static str> {
    if body.len() > MAX_BYTES {
        return Err("foreman_invalid");
    }
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let id = verify(authorization, body, now, &setup.key, &setup.origins)?;
    entry.event = Some(id.clone());
    let log = recent(store, now).await;
    if log.iter().any(|e| e.event.as_deref() == Some(id.as_str())) {
        return Err("foreman_replayed");
    }
    if !setup.enabled {
        return Err("foreman_off");
    }
    if paused(store).await {
        return Err("foreman_paused");
    }
    let command = parsed.ok_or("foreman_invalid")?;
    if command.schema != grid_foreman::COMMAND_SCHEMA
        || !grid_foreman::ACTIONS.contains(&command.action.as_str())
        || (command.issue == 0 && command.action == grid_foreman::START)
        || (command.action == grid_foreman::EXPLORE && command.issue != 0)
        || (command.exploration.is_some() != (command.action == grid_foreman::EXPLORE))
    {
        return Err("foreman_invalid");
    }
    let reserved = command.exploration.as_ref().is_some_and(|r| {
        log.iter()
            .any(|e| e.assignment.as_deref() == Some(r.request_id.as_str()))
    });
    if !reserved && let Some(code) = caps(&log, now, &command.action) {
        return Err(code);
    }
    let (owner, agent, host, project) = match &setup.worker {
        Some(worker) => worker.clone(),
        None => resolve_worker(store).await.ok_or("foreman_unsetup")?,
    };
    let (book, _) = crate::agent_assignments::load(store, &owner)
        .await
        .map_err(|_| "foreman_unsetup")?;
    let state = |id: &str| book.assignments.get(id).map(|a| a.state);
    let running = running(&log, now, state);
    match command.action.as_str() {
        grid_foreman::EXPLORE => {
            if !reserved && running.len() >= grid_foreman::RUNNING_AT_ONCE {
                return Err("foreman_running");
            }
            let reference = command.exploration.as_ref().ok_or("foreman_invalid")?;
            let allow_new = if reserved {
                false
            } else {
                objectives::eligible_issues().await?.is_empty()
            };
            if !reserved && !allow_new {
                return Err("foreman_issue_available");
            }
            let (assignment, chat) = objectives::start(
                store,
                reference,
                (owner, agent, host, project),
                now,
                allow_new,
            )
            .await?;
            entry.result = if reserved { "recovered" } else { "started" }.into();
            entry.objective = Some(reference.objective.clone());
            entry.assignment = Some(assignment);
            entry.chat = Some(chat);
            Ok(())
        }
        grid_foreman::START => {
            if grid_foreman::objectives::Catalog::parse(
                grid_foreman::objectives::CATALOG.as_bytes(),
            )
            .map_err(|_| "foreman_catalog_invalid")?
            .selection
            .excluded_issues
            .contains(&command.issue)
            {
                return Err("foreman_issue_denied");
            }
            if running.len() >= grid_foreman::RUNNING_AT_ONCE {
                return Err("foreman_running");
            }
            if running.iter().any(|e| e.issue == command.issue) {
                return Err("foreman_issue_claimed");
            }
            let facts = match facts {
                Some(facts) => facts,
                None => issue_facts(command.issue).await?,
            };
            if let Some(code) = issue_refusal(&facts) {
                return Err(code);
            }
            let definition =
                match crate::custom_agents::definition(store, &owner, &agent, None).await {
                    Ok(Ok(d)) if !d.deleted => d,
                    _ => return Err("foreman_unsetup"),
                };
            // The worker never holds standing authority to coordinate.
            if definition
                .spec
                .autonomy
                .as_ref()
                .is_some_and(|a| a.authority)
            {
                return Err("foreman_worker_unsafe");
            }
            let body = CreateAssignment {
                request_id: id.clone(),
                spec: AssignmentSpec {
                    agent: agent.clone(),
                    agent_revision: definition.revision,
                    host,
                    project,
                    task: dispatch_brief(grid_foreman::brief(
                        crate::github_refs::DEFAULT_REPOSITORY,
                        command.issue,
                    ))?,
                },
                chat: None,
            };
            let made = crate::agent_assignments::mutate(
                store,
                &owner,
                Change::Create { body, definition },
            )
            .await
            .map_err(|_| "foreman_unsetup")?
            .map_err(|_| "foreman_invalid")?;
            entry.result = "started".into();
            entry.assignment = Some(made.id);
            entry.chat = Some(made.chat);
            Ok(())
        }
        _ => {
            let assignment = command.assignment.clone().ok_or("foreman_invalid")?;
            let ours = running.iter().any(|e| {
                e.assignment.as_deref() == Some(assignment.as_str()) && e.issue == command.issue
            });
            if !ours {
                return Err("foreman_not_ours");
            }
            let made = crate::agent_assignments::mutate(
                store,
                &owner,
                Change::Command {
                    id: assignment.clone(),
                    body: AssignmentCommand {
                        request_id: id.clone(),
                        action: AssignmentAction::FollowUp {
                            text: grid_foreman::DRAFT_PR_TEXT.into(),
                        },
                    },
                },
            )
            .await
            .map_err(|_| "foreman_unsetup")?
            .map_err(|_| "foreman_invalid")?;
            entry.result = "asked".into();
            entry.assignment = Some(made.id);
            entry.chat = Some(made.chat);
            Ok(())
        }
    }
}

async fn overview(State(app): State<App>) -> Response {
    let store = app.config.chat_store.clone();
    let now = crate::chat_store::now_unix();
    let log = recent(&store, now).await;
    let account = match std::env::var(OWNER_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(account) => Some(account.trim().to_owned()),
        None => read::<Value>(&store, OWNER_KEY).await.0["account_id"]
            .as_str()
            .map(str::to_owned),
    };
    let book = match &account {
        Some(account) => {
            crate::agent_assignments::load(&store, &crate::chat_store::account_owner(account))
                .await
                .ok()
                .map(|(b, _)| b)
        }
        None => None,
    };
    let state = |id: &str| {
        book.as_ref()
            .and_then(|b| b.assignments.get(id))
            .map(|a| a.state)
    };
    let running: Vec<Value> = running(&log, now, state)
        .into_iter()
        .map(|e| {
            let asked = log.iter().any(|a| {
                a.result == "asked" && a.assignment == e.assignment
            });
            json!({"issue": e.issue, "assignment": e.assignment, "chat": e.chat, "started_at": e.at, "asked_draft": asked})
        })
        .collect();
    let today: Vec<&Entry> = log
        .iter()
        .filter(|e| day(e.at) == day(now) && (e.result == "started" || e.result == "asked"))
        .collect();
    let (requests, _) = read::<Vec<Request>>(&store, REQUESTS_KEY).await;
    let handled: BTreeSet<u64> = log
        .iter()
        .filter(|e| e.result == "started")
        .map(|e| e.issue)
        .collect();
    let requests: Vec<&Request> = requests
        .iter()
        .filter(|r| !handled.contains(&r.issue))
        .collect();
    let eligible = objectives::eligible_issues().await.ok();
    let next = objectives::next(&store, now, state).await.ok().flatten();
    let mut response = answer(
        StatusCode::OK,
        json!({
            "schema": "openagents.grid-foreman.commands.v1",
            "enabled": enabled(),
            "worker": account.is_some(),
            "paused": paused(&store).await,
            "today": {
                "started": today.iter().filter(|e| matches!(e.action.as_str(), grid_foreman::START | grid_foreman::EXPLORE)).count(),
                "draft_prs": today.iter().filter(|e| e.action == grid_foreman::DRAFT_PR).count(),
            },
            "caps": {
                "starts_per_day": grid_foreman::STARTS_PER_DAY,
                "draft_prs_per_day": grid_foreman::DRAFT_PRS_PER_DAY,
                "running_at_once": grid_foreman::RUNNING_AT_ONCE,
            },
            "running": running,
            "requests": requests,
            "eligible_issues": eligible,
            "next_exploration": next,
            "objective_catalog_revision": grid_foreman::objectives::Catalog::parse(grid_foreman::objectives::CATALOG.as_bytes()).ok().map(|c| c.revision()),
            "log": log.iter().rev().take(50).collect::<Vec<_>>(),
        }),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

/// The admin's account ID, when the caller is a site admin.
async fn admin(app: &App, headers: &HeaderMap) -> Option<String> {
    let service = app.config.cloud.as_deref()?;
    service
        .authenticate(headers)
        .await
        .ok()
        .filter(|v| v.admin)
        .map(|v| v.account_id)
}

async fn pause(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    if admin(&app, &headers).await.is_none() {
        return refused(
            StatusCode::FORBIDDEN,
            "admin_only",
            "Only an admin can pause the Foreman.",
        );
    }
    let Some(paused) = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v["paused"].as_bool())
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {\"paused\": true} or {\"paused\": false}.",
        );
    };
    let now = crate::chat_store::now_unix();
    match edit::<Value, ()>(&app.config.chat_store, PAUSE_KEY, |v| {
        *v = json!({"paused": paused, "at": now});
    })
    .await
    {
        Some(()) => answer(StatusCode::OK, json!({"paused": paused})),
        None => refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_failed",
            "That didn't save. Try again.",
        ),
    }
}

async fn request(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(account) = admin(&app, &headers).await else {
        return refused(
            StatusCode::FORBIDDEN,
            "admin_only",
            "Only an admin can ask the Foreman to start an agent.",
        );
    };
    remember_owner(&app.config.chat_store, &account).await;
    let Some(issue) = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v["issue"].as_u64())
        .filter(|n| *n > 0)
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send the issue's number as {\"issue\": N}.",
        );
    };
    let now = crate::chat_store::now_unix();
    match edit::<Vec<Request>, ()>(&app.config.chat_store, REQUESTS_KEY, |list| {
        list.retain(|r| r.issue != issue && now.saturating_sub(r.at) < 7 * 86_400);
        list.push(Request { issue, at: now });
        if list.len() > REQUESTS_MAX {
            let extra = list.len() - REQUESTS_MAX;
            list.drain(..extra);
        }
    })
    .await
    {
        Some(()) => answer(StatusCode::OK, json!({"requested": issue})),
        None => refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_failed",
            "That didn't save. Try again.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::{RelaySigner, Tag};

    const ORIGIN: &str = "https://openagents.com";

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap()
    }

    fn header_for(signer: &RelaySigner, body: &[u8], at: u64, url: &str, method: &str) -> String {
        let event = signer.sign(
            at,
            HTTP_AUTH,
            vec![
                Tag::new(vec!["u".into(), url.into()]),
                Tag::new(vec!["method".into(), method.into()]),
                Tag::new(vec!["payload".into(), to_hex(&Sha256::digest(body))]),
            ],
            String::new(),
        );
        format!(
            "Nostr {}",
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&event).unwrap())
        )
    }

    fn url() -> String {
        format!("{ORIGIN}{}", grid_foreman::COMMANDS_PATH)
    }

    #[test]
    fn only_a_fresh_exact_signature_from_the_pinned_key_passes() {
        let s = signer();
        let key = s.pubkey().to_owned();
        let origins = vec![ORIGIN.to_owned()];
        let body =
            br#"{"schema":"openagents.grid-foreman.command.v1","action":"agent.start","issue":5}"#;
        let ok = header_for(&s, body, 1_000, &url(), "POST");
        assert!(verify(Some(&ok), body, 1_010, &key, &origins).is_ok());
        // Another key.
        assert_eq!(
            verify(Some(&ok), body, 1_010, &"22".repeat(32), &origins),
            Err("foreman_wrong_key")
        );
        // Old.
        assert_eq!(
            verify(Some(&ok), body, 1_100, &key, &origins),
            Err("foreman_stale")
        );
        // Another body.
        assert_eq!(
            verify(Some(&ok), b"{}", 1_010, &key, &origins),
            Err("foreman_bad_signature")
        );
        // Another URL or method.
        let elsewhere = header_for(
            &s,
            body,
            1_000,
            "https://evil.example/v1/foreman/commands",
            "POST",
        );
        assert_eq!(
            verify(Some(&elsewhere), body, 1_010, &key, &origins),
            Err("foreman_bad_signature")
        );
        let get = header_for(&s, body, 1_000, &url(), "GET");
        assert_eq!(
            verify(Some(&get), body, 1_010, &key, &origins),
            Err("foreman_bad_signature")
        );
        // A tampered event.
        let mut event: Event = serde_json::from_slice(
            &base64::engine::general_purpose::STANDARD
                .decode(ok.strip_prefix("Nostr ").unwrap())
                .unwrap(),
        )
        .unwrap();
        event.created_at += 1;
        let tampered = format!(
            "Nostr {}",
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&event).unwrap())
        );
        assert_eq!(
            verify(Some(&tampered), body, 1_010, &key, &origins),
            Err("foreman_bad_signature")
        );
        assert_eq!(
            verify(None, body, 1_010, &key, &origins),
            Err("foreman_unsigned")
        );
    }

    fn entry(at: u64, action: &str, result: &str, issue: u64, assignment: Option<&str>) -> Entry {
        Entry {
            at,
            action: action.into(),
            issue,
            result: result.into(),
            assignment: assignment.map(str::to_owned),
            ..Entry::default()
        }
    }

    #[test]
    fn caps_hold_per_day_and_per_minute() {
        let day0 = 86_400 * 20_000;
        let mut log: Vec<Entry> = (0..grid_foreman::STARTS_PER_DAY as u64)
            .map(|i| entry(day0 + i * 600, grid_foreman::START, "started", i + 1, None))
            .collect();
        let later = day0 + 10_000;
        assert_eq!(caps(&log, later, grid_foreman::START), Some("foreman_cap"));
        assert_eq!(caps(&log, later, grid_foreman::DRAFT_PR), None);
        // Refused commands don't count.
        log.push(entry(later - 5, grid_foreman::START, "refused", 9, None));
        assert_eq!(caps(&log, later, grid_foreman::DRAFT_PR), None);
        // One a minute.
        log.push(entry(later - 30, grid_foreman::DRAFT_PR, "asked", 1, None));
        assert_eq!(
            caps(&log, later, grid_foreman::DRAFT_PR),
            Some("foreman_too_soon")
        );
        // A new day starts the counts again.
        assert_eq!(caps(&log, day0 + 86_400 + 100, grid_foreman::START), None);
        for _ in 0..grid_foreman::DRAFT_PRS_PER_DAY {
            log.push(entry(day0, grid_foreman::DRAFT_PR, "asked", 1, None));
        }
        assert_eq!(
            caps(&log, later + 600, grid_foreman::DRAFT_PR),
            Some("foreman_cap")
        );
    }

    #[test]
    fn running_agents_follow_their_assignments() {
        let now = 1_000_000;
        let log = vec![
            entry(now - 100, grid_foreman::START, "started", 1, Some("a")),
            entry(now - 100, grid_foreman::START, "started", 2, Some("b")),
            entry(now - 100, grid_foreman::START, "started", 3, Some("c")),
            entry(
                now - 2 * RUNNING_FOR,
                grid_foreman::START,
                "started",
                4,
                Some("d"),
            ),
            entry(now - 100, grid_foreman::START, "refused", 5, None),
        ];
        let state = |id: &str| match id {
            "a" => Some(AssignmentState::Running),
            "b" => Some(AssignmentState::Completed),
            _ => None,
        };
        let running: Vec<u64> = running(&log, now, state).iter().map(|e| e.issue).collect();
        assert_eq!(running, [1, 3]);
    }

    #[test]
    fn issues_are_checked_before_an_agent_starts() {
        let open = IssueFacts {
            open: true,
            ..IssueFacts::default()
        };
        assert_eq!(issue_refusal(&open), None);
        let mut closed = open.clone();
        closed.open = false;
        assert_eq!(issue_refusal(&closed), Some("foreman_issue_closed"));
        let mut pr = open.clone();
        pr.pull_request = true;
        assert_eq!(issue_refusal(&pr), Some("foreman_not_an_issue"));
        let mut claimed = open.clone();
        claimed.assigned = true;
        assert_eq!(issue_refusal(&claimed), Some("foreman_issue_claimed"));
        let mut denied = open.clone();
        denied.labels = vec!["Security".into()];
        assert_eq!(issue_refusal(&denied), Some("foreman_issue_denied"));
    }

    #[test]
    fn a_command_carries_only_its_fields() {
        let text =
            r#"{"schema":"openagents.grid-foreman.command.v1","action":"agent.merge","issue":5}"#;
        let command: Command = serde_json::from_str(text).unwrap();
        assert!(!grid_foreman::ACTIONS.contains(&command.action.as_str()));
        let extra = r#"{"schema":"x","action":"agent.start","issue":5,"text":"merge it"}"#;
        assert!(serde_json::from_str::<Command>(extra).is_err());
    }

    #[test]
    fn the_refusals_are_plain() {
        for code in [
            "foreman_off",
            "foreman_unsigned",
            "foreman_cap",
            "foreman_unsetup",
            "foreman_issue_closed",
        ] {
            let response = no(code);
            assert!(!response.status().is_success());
        }
        for text in [
            "The Foreman's commands are off right now.",
            "Only the Foreman can send this.",
            "The Foreman has started enough agents for now.",
            "The Foreman's worker isn't set up.",
            "The Foreman can't do that for this issue.",
            "Only an admin can pause the Foreman.",
            "Only an admin can ask the Foreman to start an agent.",
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }

    fn test_worker(authority: bool) -> openagents_sdk::agents::AgentSpec {
        openagents_sdk::agents::AgentSpec {
            name: "Foreman worker".into(),
            description: String::new(),
            instructions: "Work the issue you are given.".into(),
            runtime: "claude-code".into(),
            model: None,
            access: openagents_sdk::agents::Access::HostDefault,
            fallbacks: vec![],
            autonomy: authority.then(|| openagents_sdk::agents::Autonomy {
                authority: true,
                ..Default::default()
            }),
        }
    }

    fn signed(s: &RelaySigner, body: &[u8], at: u64) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header_for(s, body, at, &url(), "POST").parse().unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn a_start_makes_one_assignment_and_a_draft_ask_follows_it() {
        let root = std::env::temp_dir().join(format!("foreman-act-{}", std::process::id()));
        let store = Store::local(root.clone());
        let owner = crate::chat_store::account_owner("foreman-act-test");
        let saved = crate::custom_agents::mutate(
            &store,
            &owner,
            crate::custom_agents::Mutation::Create {
                request_id: format!("{:064x}", 1),
                spec: test_worker(false),
            },
        )
        .await
        .unwrap()
        .unwrap();
        let s = signer();
        let setup = Setup {
            enabled: true,
            key: s.pubkey().to_owned(),
            origins: vec![ORIGIN.to_owned()],
            worker: Some((owner.clone(), saved.id.clone(), "test-host".into(), None)),
        };
        let open = IssueFacts {
            open: true,
            ..IssueFacts::default()
        };
        let now = 86_400 * 20_000 + 100;
        let body = br#"{"schema":"openagents.grid-foreman.command.v1","action":"agent.start","issue":4242}"#;
        let headers = signed(&s, body, now);
        let parsed = serde_json::from_slice(body).ok();
        let mut entry = Entry::default();
        act(
            &store,
            &setup,
            &headers,
            body,
            parsed.clone(),
            now,
            Some(open.clone()),
            &mut entry,
        )
        .await
        .unwrap();
        assert_eq!(entry.result, "started");
        log(
            &store,
            Entry {
                at: now,
                action: grid_foreman::START.into(),
                issue: 4242,
                ..entry.clone()
            },
        )
        .await;
        let (book, _) = crate::agent_assignments::load(&store, &owner)
            .await
            .unwrap();
        assert_eq!(book.assignments.len(), 1);
        let made = book.assignments.values().next().unwrap();
        assert_eq!(
            made.spec.task,
            grid_foreman::brief(crate::github_refs::DEFAULT_REPOSITORY, 4242)
        );
        assert_eq!(made.spec.host, "test-host");
        assert_eq!(Some(made.id.clone()), entry.assignment);

        // The same signed command again is a replay.
        let mut again = Entry::default();
        assert_eq!(
            act(
                &store,
                &setup,
                &headers,
                body,
                parsed,
                now + 5,
                Some(open.clone()),
                &mut again
            )
            .await,
            Err("foreman_replayed")
        );

        // A draft ask for that agent, a minute later.
        let later = now + 61;
        let ask = format!(
            r#"{{"schema":"openagents.grid-foreman.command.v1","action":"agent.draft_pr","issue":4242,"assignment":"{}"}}"#,
            made.id
        );
        let mut asked = Entry::default();
        act(
            &store,
            &setup,
            &signed(&s, ask.as_bytes(), later),
            ask.as_bytes(),
            serde_json::from_str(&ask).ok(),
            later,
            None,
            &mut asked,
        )
        .await
        .unwrap();
        assert_eq!(asked.result, "asked");
        let (book, _) = crate::agent_assignments::load(&store, &owner)
            .await
            .unwrap();
        let commands = &book.assignments[&made.id].commands;
        assert_eq!(commands.len(), made.commands.len() + 1);
        assert_eq!(
            commands.last().unwrap().action,
            AssignmentAction::FollowUp {
                text: grid_foreman::DRAFT_PR_TEXT.into()
            }
        );

        // A draft ask for an agent it didn't start is refused.
        let other = r#"{"schema":"openagents.grid-foreman.command.v1","action":"agent.draft_pr","issue":4242,"assignment":"0000000000000000000000000000000000000000000000000000000000000009"}"#;
        let mut refused = Entry::default();
        log(
            &store,
            Entry {
                at: later,
                action: grid_foreman::DRAFT_PR.into(),
                issue: 4242,
                result: "asked".into(),
                ..Entry::default()
            },
        )
        .await;
        let at = later + 61;
        assert_eq!(
            act(
                &store,
                &setup,
                &signed(&s, other.as_bytes(), at),
                other.as_bytes(),
                serde_json::from_str(other).ok(),
                at,
                None,
                &mut refused
            )
            .await,
            Err("foreman_not_ours")
        );

        // An unknown action, a claimed issue, and a worker with standing
        // authority are refused.
        let merge =
            br#"{"schema":"openagents.grid-foreman.command.v1","action":"agent.merge","issue":1}"#;
        let at = at + 61;
        assert_eq!(
            act(
                &store,
                &setup,
                &signed(&s, merge, at),
                merge,
                serde_json::from_slice(merge).ok(),
                at,
                Some(open.clone()),
                &mut Entry::default()
            )
            .await,
            Err("foreman_invalid")
        );
        let start =
            br#"{"schema":"openagents.grid-foreman.command.v1","action":"agent.start","issue":77}"#;
        let claimed = IssueFacts {
            assigned: true,
            ..open.clone()
        };
        assert_eq!(
            act(
                &store,
                &setup,
                &signed(&s, start, at),
                start,
                serde_json::from_slice(start).ok(),
                at,
                Some(claimed),
                &mut Entry::default()
            )
            .await,
            Err("foreman_issue_claimed")
        );
        let unsafe_worker = crate::custom_agents::mutate(
            &store,
            &owner,
            crate::custom_agents::Mutation::Create {
                request_id: format!("{:064x}", 2),
                spec: test_worker(true),
            },
        )
        .await
        .unwrap()
        .unwrap();
        let risky = Setup {
            worker: Some((owner.clone(), unsafe_worker.id, "test-host".into(), None)),
            ..setup
        };
        assert_eq!(
            act(
                &store,
                &risky,
                &signed(&s, start, at),
                start,
                serde_json::from_slice(start).ok(),
                at,
                Some(open.clone()),
                &mut Entry::default()
            )
            .await,
            Err("foreman_worker_unsafe")
        );
        // Off, nothing runs.
        let off = Setup {
            enabled: false,
            ..risky
        };
        assert_eq!(
            act(
                &store,
                &off,
                &signed(&s, start, at),
                start,
                serde_json::from_slice(start).ok(),
                at,
                Some(open),
                &mut Entry::default()
            )
            .await,
            Err("foreman_off")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn the_site_saves_a_safe_worker_for_the_remembered_admin() {
        let root = std::env::temp_dir().join(format!("foreman-worker-{}", std::process::id()));
        let store = Store::local(root.clone());
        assert_eq!(resolve_worker(&store).await, None);
        remember_owner(&store, "acct_test_admin").await;
        // No computer online yet: no worker, but the worker agent is saved.
        assert_eq!(resolve_worker(&store).await, None);
        let owner = crate::chat_store::account_owner("acct_test_admin");
        let id = to_hex(&Sha256::digest(format!(
            "openagents.grid-foreman.worker.v1\0{owner}"
        )));
        let saved = crate::custom_agents::definition(&store, &owner, &id, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.spec, worker_spec());
        assert!(saved.spec.autonomy.is_none());
        assert!(
            saved
                .spec
                .instructions
                .contains("never push to main, merge, deploy, spend money")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn commands_are_on_unless_switched_off() {
        // Unset, the switch is on.
        if std::env::var(SWITCH_ENV).is_err() {
            assert!(enabled());
        }
    }

    #[tokio::test]
    async fn the_log_keeps_every_entry_by_day() {
        let dir = std::env::temp_dir().join(format!("foreman-log-{}", std::process::id()));
        let store = Store::local(dir.clone());
        let now = 86_400 * 20_000 + 50;
        log(
            &store,
            entry(now - 86_400, grid_foreman::START, "started", 1, Some("a")),
        )
        .await;
        log(&store, entry(now, grid_foreman::START, "refused", 2, None)).await;
        let found = recent(&store, now).await;
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].issue, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
