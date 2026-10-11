//! Issue runs as fleet rows on openagents.com and the phone (#11228).
//!
//! While `chat work` runs issue flows on this computer (a cloud environment
//! such as `oa-dev-env-1`, or a GCE pool host), each flow is an
//! [`agent_fleet::AgentRow`], reported as an agent item on this computer's
//! list ([`coder_sync::activity`]). The phone's Agents screen and the
//! website's `/settings/agents` read that list. Their **Stop** and
//! **Message** come back as commands: Stop asks the flow to stop as Ctrl-C
//! does ([`Local::stop`]), and a message steers its engine
//! ([`Local::steer`]).
//!
//! Reporting needs the account's sign-in: `OPENAGENTS_APP_TOKEN`, else
//! Coder's (`coder login`). Without one the runs only print here.
//! `chat work --on gce` lends the orchestrator's sign-in to each pool run
//! ([`lend_sign_in`]), so pool hosts report too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_fleet::{AgentRow, Status};
use coder::task::issue_run;
use coder::task::local::Local;
use coder_sync::activity::{self, Command, Item};
use openagents_chat::coder_events::{self, CoderEvent};
use tokio::sync::Notify;

/// The longest title or line sent.
const TITLE_CHARS: usize = 120;
const LINE_CHARS: usize = 200;
/// How long the last report may take once the queue is done.
const LAST_REPORT: Duration = Duration::from_secs(20);

/// The issue runs of one `chat work`.
#[derive(Debug)]
pub(super) struct Fleet {
    /// This computer's name, as the lists show it.
    computer: String,
    rows: Vec<AgentRow>,
    /// The issue each row works, by row id.
    issues: BTreeMap<String, u64>,
    /// What each running row last said, by row id.
    lines: BTreeMap<String, String>,
    /// What each row's flow file says (#11250): its stage, and once it
    /// ends, its commit or pull request.
    flows: BTreeMap<String, FlowSeen>,
    /// Rows a turn of which reported no cost: their cost is unknown, never
    /// a partial sum.
    unpriced: BTreeSet<String>,
    /// A word for a row after a command it couldn't take, such as a
    /// message the briefed agent can't read mid-run.
    notes: BTreeMap<String, String>,
    /// The queue is done: report once more, then stop.
    closed: bool,
}

/// What a row's flow file says (#11250).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FlowSeen {
    /// The stage in the rows' words ([`issue_run::Stage::words`]).
    pub stage: Option<String>,
    /// The commit it landed or queued, newest.
    pub commit: Option<String>,
    pub pull_request: Option<String>,
}

impl FlowSeen {
    fn of(flow: &issue_run::Flow) -> Self {
        Self {
            stage: flow.stage.as_ref().map(issue_run::Stage::words),
            commit: flow.link.commits.last().cloned(),
            pull_request: flow.link.pull_request.clone(),
        }
    }
}

impl Fleet {
    pub(super) fn new(computer: &str) -> Self {
        Self {
            computer: computer.to_owned(),
            rows: Vec::new(),
            issues: BTreeMap::new(),
            lines: BTreeMap::new(),
            flows: BTreeMap::new(),
            unpriced: BTreeSet::new(),
            notes: BTreeMap::new(),
            closed: false,
        }
    }

    /// Issue `issue`'s flow started as `task`.
    pub(super) fn started(&mut self, issue: u64, title: &str, task: &str, now_ms: u64) {
        self.rows.retain(|row| row.id != task);
        self.issues.insert(task.to_owned(), issue);
        self.rows.push(AgentRow {
            id: task.to_owned(),
            name: format!("issue-{issue}"),
            engine: "Coder".into(),
            place: self.computer.clone(),
            status: Status::Running,
            task: format!("#{issue} {}", title.trim()),
            started_ms: now_ms,
            ended_ms: None,
            earlier_seconds: 0,
            tokens: 0,
            cost_usd: None,
            worktree: None,
            branch: None,
            parent_session: None,
            transcript: None,
            report: None,
            error: None,
            pending_messages: 0,
            waiting: None,
            choices: Vec::new(),
            runs: 1,
            run_started_ms: now_ms,
        });
    }

    /// One event of `task`'s flow: its engine, its cost, and what it is doing.
    pub(super) fn seen(&mut self, task: &str, event: &CoderEvent) {
        let Some(row) = self.rows.iter_mut().find(|row| row.id == task) else {
            return;
        };
        match event {
            CoderEvent::CoderStarted(started) => row.engine.clone_from(&started.provider),
            // Each turn's cost (Claude Code's, with Jev's decision calls)
            // and its trace's tokens, added as the turn ends (#11250). A
            // turn with no cost makes the whole cost unknown.
            CoderEvent::Result(finished) => {
                match finished.cost_microusd {
                    Some(cost) if !self.unpriced.contains(task) => {
                        row.cost_usd = Some(row.cost_usd.unwrap_or(0.0) + cost as f64 / 1e6);
                    }
                    _ => {
                        self.unpriced.insert(task.to_owned());
                        row.cost_usd = None;
                    }
                }
                row.tokens += trace_tokens(Path::new(&finished.trajectory));
            }
            _ => {}
        }
        if let Some(text) = coder_events::text(event).filter(|text| !text.trim().is_empty()) {
            self.lines.insert(task.to_owned(), text);
        }
    }

    /// Read each row's flow file in `store` for its stage and, once it
    /// ended, its commit or pull request (#11250).
    pub(super) fn refresh(&mut self, store: &Path) {
        for row in &self.rows {
            let ended = row.status != Status::Running;
            if ended
                && self
                    .flows
                    .get(&row.id)
                    .is_some_and(|seen| seen.commit.is_some())
            {
                continue;
            }
            if let Some(flow) = issue_run::load(store, &row.id) {
                self.flows.insert(row.id.clone(), FlowSeen::of(&flow));
            }
        }
    }

    /// `task`'s flow ended with `outcome`, saying `message`.
    pub(super) fn ended(&mut self, task: &str, outcome: &str, message: &str, now_ms: u64) {
        let Some(row) = self.rows.iter_mut().find(|row| row.id == task) else {
            return;
        };
        row.status = status_of(outcome);
        row.ended_ms = Some(now_ms);
        let message = message.trim();
        if !message.is_empty() {
            if row.status == Status::Failed {
                row.error = Some(message.to_owned());
            } else {
                row.report = Some(message.to_owned());
            }
        }
        self.lines.remove(task);
    }

    /// One progress line the briefed agent's engine
    /// (`scripts/work/work_issue.py`) printed for `task` (#11228): the
    /// issue's title, the stage, the engine, and the cost so far (unknown
    /// stays unknown, never 0).
    pub(super) fn engine_said(&mut self, task: &str, said: &serde_json::Value) {
        let issue = self.issues.get(task).copied().unwrap_or(0);
        let Some(row) = self.rows.iter_mut().find(|row| row.id == task) else {
            return;
        };
        let text = said["text"].as_str().unwrap_or_default().trim();
        let stage = match said["phase"].as_str().unwrap_or_default() {
            "issue" => {
                if let Some(title) = said["title"].as_str() {
                    row.task = format!("#{issue} {}", title.trim());
                }
                None
            }
            "briefing" => Some("finding files".to_owned()),
            "agent" => {
                row.engine = engine_name(said["engine"].as_str()).into();
                Some("coding".to_owned())
            }
            "escalate" => {
                row.engine = engine_name(Some("bare")).into();
                Some(format!("coding (handed to Claude Code: {text})"))
            }
            "verify" | "checks" => Some(format!("checks: {text}")),
            "commit" => Some("landing".to_owned()),
            "land" => Some(match said["entry"].as_str() {
                Some(entry) => format!("queued {entry}"),
                None => "landing".to_owned(),
            }),
            "cost" => {
                row.cost_usd = said["cost_usd"].as_f64();
                if let Some(tokens) = said["tokens"].as_u64() {
                    row.tokens = tokens;
                }
                None
            }
            _ => None,
        };
        if let Some(stage) = stage {
            self.flows.entry(task.to_owned()).or_default().stage = Some(stage);
        }
    }

    /// The briefed agent's engine ended `task` with `result` (#11228).
    pub(super) fn engine_finished(&mut self, task: &str, result: &serde_json::Value, now_ms: u64) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.id == task) {
            row.cost_usd = result["cost_usd"].as_f64();
            if let Some(engine) = result["engine"].as_str() {
                row.engine = engine_name(Some(engine)).into();
            }
        }
        let landed = &result["landed"];
        let commit = result["commit"].as_str().map(str::to_owned);
        let seen = self.flows.entry(task.to_owned()).or_default();
        let (outcome, message) = if !result["ok"].as_bool().unwrap_or(false) {
            let why = result["error"]
                .as_str()
                .unwrap_or("the engine stopped without a result");
            ("failed", why.to_owned())
        } else {
            match landed["how"].as_str() {
                Some("queue") => {
                    let entry = landed["entry"]["id"].as_str().unwrap_or("an entry");
                    seen.stage = Some(format!("queued {entry}"));
                    seen.commit = commit;
                    ("queued", format!("queued {entry}"))
                }
                Some("pr") => {
                    seen.pull_request = landed["url"].as_str().map(str::to_owned);
                    seen.stage = None;
                    ("pull_request", "opened a pull request".to_owned())
                }
                _ => {
                    seen.stage = None;
                    let short = commit
                        .as_deref()
                        .map_or_else(String::new, |c| c[..c.len().min(10)].to_owned());
                    ("committed", format!("committed {short}, not pushed"))
                }
            }
        };
        self.ended(task, outcome, &message, now_ms);
    }

    /// Say `note` on `task`'s row, after a command it couldn't take.
    pub(super) fn noted(&mut self, task: &str, note: &str) {
        if self.rows.iter().any(|row| row.id == task) {
            self.notes.insert(task.to_owned(), note.to_owned());
        }
    }

    /// The task of a running row the phone or the website names.
    fn running(&self, item: &str) -> Option<(String, u64)> {
        self.rows
            .iter()
            .find(|row| row.id == item && row.status == Status::Running)
            .map(|row| {
                (
                    row.id.clone(),
                    self.issues.get(&row.id).copied().unwrap_or(0),
                )
            })
    }

    /// The rows as the list's items, newest first.
    pub(super) fn items(&self) -> Vec<Item> {
        self.rows
            .iter()
            .rev()
            .take(activity::MAX_ITEMS)
            .map(|row| {
                let doing = self.lines.get(&row.id).map(String::as_str);
                let mut item = item(row, doing);
                if let Some(line) = stage_line(row, self.flows.get(&row.id)) {
                    item.line = Some(short(&line, LINE_CHARS));
                }
                if let Some(note) = self.notes.get(&row.id) {
                    let line = match item.line.take() {
                        Some(line) => format!("{line} · {note}"),
                        None => note.clone(),
                    };
                    item.line = Some(short(&line, LINE_CHARS));
                }
                item
            })
            .collect()
    }
}

/// The tokens of a turn's ATIF trace (input and output); 0 when it can't
/// be read.
fn trace_tokens(path: &Path) -> u64 {
    atif::log::read(path).map_or(0, |recording| {
        recording
            .steps
            .iter()
            .filter_map(|step| step.tokens)
            .map(|(input, output)| input + output)
            .sum()
    })
}

/// A row's line from its flow (#11250): the stage while it runs
/// (`finding files`, `coding`, `checks`, `fix turn N`, `landing: rebase`,
/// `queued <entry>`, ...), then how it ended: `landed <commit>`, `queued
/// <entry>`, `pull request <url>`, `failed: <reason>`. `None` when the
/// flow file says nothing more than the row does.
pub(super) fn stage_line(row: &AgentRow, seen: Option<&FlowSeen>) -> Option<String> {
    let seen = seen.cloned().unwrap_or_default();
    let commit = |c: &str| c[..c.len().min(10)].to_owned();
    match row.status {
        Status::Running => seen.stage,
        Status::Failed => {
            let why = row.error.as_deref().unwrap_or("it stopped");
            Some(format!(
                "failed: {}",
                why.lines().next().unwrap_or(why).trim()
            ))
        }
        Status::Stopped => Some("stopped".into()),
        Status::Done => match (&seen.stage, &seen.pull_request, &seen.commit) {
            (Some(stage), _, _) if stage.starts_with("queued ") => Some(stage.clone()),
            (_, Some(url), _) => Some(format!("pull request {url}")),
            (_, None, Some(c)) => Some(format!("landed {}", commit(c))),
            _ => None,
        },
    }
}

/// The engine as the lists name it.
fn engine_name(word: Option<&str>) -> &'static str {
    match word {
        Some("bare") => "Claude Code",
        _ => "briefed agent",
    }
}

/// How an issue flow's outcome reads in the list.
pub(super) fn status_of(outcome: &str) -> Status {
    match outcome {
        // Handed to the landing queue: the flow is done; the queue's own
        // row follows the entry (#11250).
        "landed" | "pull_request" | "queued" | "committed" | "unchanged" | "closed" | "skipped" => {
            Status::Done
        }
        "stopped" => Status::Stopped,
        _ => Status::Failed,
    }
}

fn short(text: &str, limit: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= limit {
        return line;
    }
    let mut cut: String = line.chars().take(limit.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// One row as an item of this computer's list.
pub(super) fn item(row: &AgentRow, doing: Option<&str>) -> Item {
    let line = match row.status {
        Status::Running => doing.map(str::to_owned),
        _ => row.error.clone().or_else(|| row.report.clone()),
    };
    Item {
        id: row.id.clone(),
        kind: "agent".into(),
        title: short(&row.task, TITLE_CHARS),
        engine: Some(row.engine.clone()),
        status: match row.status {
            Status::Running => "working",
            Status::Done => "done",
            Status::Failed => "failed",
            Status::Stopped => "stopped",
        }
        .into(),
        started_unix: row.started_ms / 1000,
        finished_unix: row.ended_ms.map(|ended| ended / 1000),
        cost_usd: row.cost_usd,
        tokens: (row.tokens > 0).then_some(row.tokens),
        session: None,
        question: None,
        line: line
            .map(|line| short(&line, LINE_CHARS))
            .filter(|line| !line.is_empty()),
        choices: row.choices.clone(),
    }
}

/// What `chat work` and its reporter share.
pub(super) type Shared = Arc<(Mutex<Fleet>, Notify)>;

/// Change the fleet and report the change soon.
pub(super) fn update(shared: &Shared, change: impl FnOnce(&mut Fleet)) {
    if let Ok(mut fleet) = shared.0.lock() {
        change(&mut fleet);
    }
    shared.1.notify_one();
}

/// What a command from the phone or the website does to the flows of
/// `fleet`, given how to stop and steer a task. The line says what
/// happened; `None` for a command that names no running flow.
pub(super) fn carry_out(
    fleet: &Fleet,
    command: &Command,
    stop: impl FnOnce(&str) -> Result<(), String>,
    steer: impl FnOnce(&str, &str) -> Result<(), String>,
) -> Option<String> {
    let (task, issue) = fleet.running(&command.item)?;
    Some(match command.action.as_str() {
        "stop" => match stop(&task) {
            Ok(()) => format!("#{issue}: stopping, as asked from openagents.com or the phone."),
            Err(why) => format!("#{issue}: could not stop: {why}"),
        },
        "message" => {
            let text = command.text.as_deref().unwrap_or_default().trim();
            if text.is_empty() {
                return None;
            }
            match steer(&task, text) {
                Ok(()) => format!("#{issue}: a message from openagents.com or the phone: {text}"),
                Err(why) => format!("#{issue}: the message could not be delivered: {why}"),
            }
        }
        _ => return None,
    })
}

/// Lend this computer's sign-in to a run on another host, as private
/// variables, so that host reports its runs as well.
pub(super) fn lend_sign_in(variables: &mut BTreeMap<String, String>) {
    if let Ok(saved) = crate::mac::saved(None) {
        variables.insert("OPENAGENTS_APP_TOKEN".into(), saved.token().to_owned());
        variables.insert("OPENAGENTS_ORIGIN".into(), saved.origin.clone());
    }
}

/// How the reporter stops and steers a row's run.
pub(super) trait Actions: Send + Sync + 'static {
    fn stop(&self, task: &str) -> Result<(), String>;
    fn steer(&self, task: &str, text: &str) -> Result<(), String>;
}

/// Coder's own tasks in `store` (the `--engine bare` issue flow).
struct CoderTasks(PathBuf);

impl Actions for CoderTasks {
    fn stop(&self, task: &str) -> Result<(), String> {
        Local::here(self.0.clone()).stop(task)
    }
    fn steer(&self, task: &str, text: &str) -> Result<(), String> {
        Local::here(self.0.clone()).steer(task, text).map(|_| ())
    }
}

/// Report `shared` to the account while `chat work` runs, carrying out
/// the commands that come back. `None` when this computer is not signed in.
pub(super) fn spawn(shared: Shared, store: PathBuf) -> Option<tokio::task::JoinHandle<()>> {
    let actions = Arc::new(CoderTasks(store.clone()));
    spawn_with(shared, store, actions)
}

/// [`spawn`], with `actions` carrying out Stop and Message.
pub(super) fn spawn_with(
    shared: Shared,
    store: PathBuf,
    actions: Arc<dyn Actions>,
) -> Option<tokio::task::JoinHandle<()>> {
    let saved = match crate::mac::saved(None) {
        Ok(saved) => saved,
        Err(why) => {
            eprintln!("These runs won't show on openagents.com or your phone: {why}");
            return None;
        }
    };
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .ok()?;
    Some(tokio::spawn(async move {
        loop {
            let (computer, items, closed) = match shared.0.lock() {
                Ok(mut fleet) => {
                    fleet.refresh(&store);
                    (fleet.computer.clone(), fleet.items(), fleet.closed)
                }
                Err(_) => return,
            };
            if let Ok(commands) = activity::report(&http, &saved, &computer, &items).await {
                for command in commands {
                    let (inner, actions) = (Arc::clone(&shared), Arc::clone(&actions));
                    let item = command.item.clone();
                    let steering = command.action == "message";
                    let said = tokio::task::spawn_blocking(move || {
                        let fleet = inner.0.lock().ok()?;
                        carry_out(
                            &fleet,
                            &command,
                            |task| actions.stop(task),
                            |task, text| actions.steer(task, text),
                        )
                    })
                    .await
                    .ok()
                    .flatten();
                    if let Some(said) = said {
                        eprintln!("{said}");
                        // A message that couldn't reach the run says why on
                        // its row, where it was sent from.
                        if steering
                            && let Some((_, why)) = said.split_once("could not be delivered: ")
                        {
                            let note = format!("message not delivered: {why}");
                            update(&shared, |fleet| fleet.noted(&item, &note));
                        }
                    }
                }
            }
            if closed {
                return;
            }
            tokio::select! {
                () = tokio::time::sleep(activity::every(&items)) => {}
                () = shared.1.notified() => {}
            }
        }
    }))
}

/// The queue is done: send the last report (each run's ending) and stop.
pub(super) async fn close(shared: &Shared, reporter: Option<tokio::task::JoinHandle<()>>) {
    update(shared, |fleet| fleet.closed = true);
    if let Some(reporter) = reporter {
        let _ = tokio::time::timeout(LAST_REPORT, reporter).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(item: &str, action: &str, text: Option<&str>) -> Command {
        Command {
            id: "c1".into(),
            item: item.into(),
            action: action.into(),
            question: None,
            text: text.map(str::to_owned),
        }
    }

    #[test]
    fn an_issue_run_is_an_agent_row_on_this_computers_list() {
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(
            11228,
            "Fleet rows for cloud environment runs",
            "t1",
            1_000_000,
        );
        let row = &fleet.rows[0];
        assert_eq!(row.place, "oa-dev-env-1");
        assert_eq!(row.name, "issue-11228");
        assert_eq!(row.status, Status::Running);
        let items = fleet.items();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.id, "t1");
        assert_eq!(item.kind, "agent");
        assert_eq!(item.status, "working");
        assert_eq!(item.title, "#11228 Fleet rows for cloud environment runs");
        assert_eq!(item.started_unix, 1_000);
        assert_eq!(item.finished_unix, None);
        assert_eq!(item.session, None);
    }

    #[test]
    fn a_run_shows_its_engine_its_cost_and_how_it_ended() {
        let mut fleet = Fleet::new("oa-pool-p1-a");
        fleet.started(7, "Fix it", "t1", 1_000);
        let started: CoderEvent = serde_json::from_value(serde_json::json!({
            "event": "coder_started", "turn": 1, "project": "openagents", "checkout": "/c",
            "worktree": "/w", "base": "abc", "provider": "claude", "model": "opus",
            "reason": "Claude Code is signed in.", "fallbacks": [], "via": "local"
        }))
        .unwrap();
        fleet.seen("t1", &started);
        let result: CoderEvent = serde_json::from_value(serde_json::json!({
            "event": "result", "turn": 1, "summary": "Done.", "files_changed": [],
            "insertions": 0, "deletions": 0, "worktree": "/w", "trajectory": "/t",
            "cost_microusd": 1_700_000
        }))
        .unwrap();
        fleet.seen("t1", &result);
        fleet.ended("t1", "landed", "Landed on main as abc123.", 61_000);
        let item = &fleet.items()[0];
        assert_eq!(item.engine.as_deref(), Some("claude"));
        assert_eq!(item.cost_usd, Some(1.7));
        assert_eq!(item.status, "done");
        assert_eq!(item.finished_unix, Some(61));
        assert_eq!(item.line.as_deref(), Some("Landed on main as abc123."));
        fleet.started(8, "Other", "t2", 2_000);
        fleet.ended("t2", "failed", "The checks failed.", 3_000);
        assert_eq!(fleet.items()[0].status, "failed");
        assert_eq!(status_of("stopped"), Status::Stopped);
    }

    #[test]
    fn stop_and_message_reach_only_a_running_flow() {
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(5, "Five", "t5", 0);
        fleet.started(6, "Six", "t6", 0);
        fleet.ended("t6", "landed", "", 1_000);
        let stopped = std::cell::RefCell::new(Vec::new());
        let said = carry_out(
            &fleet,
            &command("t5", "stop", None),
            |task| {
                stopped.borrow_mut().push(task.to_owned());
                Ok(())
            },
            |_, _| unreachable!(),
        );
        assert_eq!(stopped.borrow().as_slice(), ["t5"]);
        assert!(said.unwrap().starts_with("#5: stopping"));
        let steered = std::cell::RefCell::new(Vec::new());
        let said = carry_out(
            &fleet,
            &command("t5", "message", Some(" use the lease ")),
            |_| unreachable!(),
            |task, text| {
                steered
                    .borrow_mut()
                    .push((task.to_owned(), text.to_owned()));
                Ok(())
            },
        );
        assert_eq!(
            steered.borrow().as_slice(),
            [("t5".to_owned(), "use the lease".to_owned())]
        );
        assert!(said.unwrap().contains("use the lease"));
        let failed = carry_out(
            &fleet,
            &command("t5", "message", Some("hi")),
            |_| unreachable!(),
            |_, _| Err("it is landing".into()),
        );
        assert_eq!(
            failed.as_deref(),
            Some("#5: the message could not be delivered: it is landing")
        );
        // An ended flow, an unknown one, and an approval do nothing.
        let none = |c: Command| carry_out(&fleet, &c, |_| unreachable!(), |_, _| unreachable!());
        assert_eq!(none(command("t6", "stop", None)), None);
        assert_eq!(none(command("t9", "stop", None)), None);
        assert_eq!(none(command("t5", "approve", None)), None);
        assert_eq!(none(command("t5", "message", Some("  "))), None);
    }

    fn flow_file(task: &str, stage: Option<issue_run::Stage>, commits: &[&str]) -> issue_run::Flow {
        issue_run::Flow {
            schema: issue_run::FLOW_SCHEMA.into(),
            task: task.into(),
            link: openagents_chat::coder_events::IssueLink {
                repository: "OpenAgentsInc/openagents".into(),
                number: 11250,
                url: "https://github.com/OpenAgentsInc/openagents/issues/11250".into(),
                title: "Fleet rows for cloud runs".into(),
                outcome: "working".into(),
                commits: commits.iter().map(|c| (*c).to_owned()).collect(),
                pull_request: None,
                closed: false,
                not_landed: None,
            },
            notes: Vec::new(),
            finished: false,
            process_id: None,
            closing: String::new(),
            files: None,
            stage,
        }
    }

    fn result_event(turn: usize, cost: Option<u64>, trajectory: &str) -> CoderEvent {
        serde_json::from_value(serde_json::json!({
            "event": "result", "turn": turn, "summary": "Done.", "files_changed": [],
            "insertions": 0, "deletions": 0, "worktree": "/w", "trajectory": trajectory,
            "cost_microusd": cost
        }))
        .unwrap()
    }

    /// A row moves through the stages its flow file says (#11250), from a
    /// fixture stream: finding files, coding, checks, fix turn, landing,
    /// queued; its time stays from the start across fix turns, and its
    /// cost and tokens grow after every turn.
    #[test]
    fn a_row_moves_through_the_stages_with_its_time_and_cost() {
        use issue_run::{LandingStep, Stage};
        let store = tempfile::tempdir().unwrap();
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(11250, "Fleet rows for cloud runs", "t1", 1_000_000);
        let line = |fleet: &mut Fleet| {
            fleet.refresh(store.path());
            fleet.items()[0].line.clone()
        };
        let stages = [
            (Stage::FindingFiles, "finding files"),
            (Stage::Coding, "coding"),
            (Stage::Checks, "checks"),
            (Stage::Fix { turn: 1 }, "fix turn 1"),
            (
                Stage::Landing {
                    step: LandingStep::Rebase,
                },
                "landing: rebase",
            ),
            (
                Stage::Landing {
                    step: LandingStep::Recheck,
                },
                "landing: re-check",
            ),
            (
                Stage::Landing {
                    step: LandingStep::Push,
                },
                "landing: push",
            ),
        ];
        let trace = store.path().join("turn.jsonl");
        for (index, (stage, words)) in stages.into_iter().enumerate() {
            issue_run::save(store.path(), &flow_file("t1", Some(stage), &[])).unwrap();
            assert_eq!(line(&mut fleet).as_deref(), Some(words));
            if index == 2 || index == 3 {
                // The first turn ends before the checks, the fix turn
                // before landing.
                fleet.seen(
                    "t1",
                    &result_event(index - 1, Some(500_000), &trace.to_string_lossy()),
                );
            }
            let item = &fleet.items()[0];
            assert_eq!(item.title, "#11250 Fleet rows for cloud runs");
            assert_eq!(item.status, "working");
            assert_eq!(item.started_unix, 1_000, "the start stays across fix turns");
            assert_eq!(item.finished_unix, None);
        }
        assert_eq!(fleet.items()[0].cost_usd, Some(1.0));

        // Landed: the commit, then the ending.
        issue_run::save(
            store.path(),
            &flow_file(
                "t1",
                Some(Stage::Landing {
                    step: LandingStep::Push,
                }),
                &["248c7ce8d6a1b2c3d4e5f60718293a4b5c6d7e8f"],
            ),
        )
        .unwrap();
        fleet.ended("t1", "landed", "Landed 248c7ce8d6 on main.", 1_090_000);
        let ended = {
            fleet.refresh(store.path());
            fleet.items()[0].clone()
        };
        assert_eq!(ended.line.as_deref(), Some("landed 248c7ce8d6"));
        assert_eq!(ended.status, "done");
        assert_eq!(
            (ended.started_unix, ended.finished_unix),
            (1_000, Some(1_090))
        );

        // Queued: the entry it waits as.
        fleet.started(11251, "Queue it", "t2", 2_000_000);
        issue_run::save(
            store.path(),
            &flow_file(
                "t2",
                Some(Stage::Queued {
                    entry: "20261011T080000Z-oa-dev-env-1-ab12".into(),
                }),
                &["0123456789abcdef"],
            ),
        )
        .unwrap();
        fleet.ended("t2", "queued", "Queued.", 2_060_000);
        fleet.refresh(store.path());
        let queued = fleet
            .items()
            .into_iter()
            .find(|item| item.id == "t2")
            .unwrap();
        assert_eq!(
            queued.line.as_deref(),
            Some("queued 20261011T080000Z-oa-dev-env-1-ab12")
        );

        // Failed: one line of why.
        fleet.started(11252, "Break", "t3", 3_000_000);
        fleet.ended(
            "t3",
            "failed",
            "The checks still fail after 2 fix turn(s).\nmore detail",
            3_010_000,
        );
        fleet.refresh(store.path());
        let failed = fleet
            .items()
            .into_iter()
            .find(|item| item.id == "t3")
            .unwrap();
        assert_eq!(
            failed.line.as_deref(),
            Some("failed: The checks still fail after 2 fix turn(s).")
        );
    }

    /// A turn that reports no cost makes the row's cost unknown, never a
    /// partial sum or zero; tokens come from each turn's trace.
    #[test]
    fn an_unknown_turn_cost_leaves_the_cost_unknown() {
        let store = tempfile::tempdir().unwrap();
        let trace = store.path().join("turn.jsonl");
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(7, "Seven", "t7", 0);
        assert_eq!(fleet.items()[0].cost_usd, None);
        fleet.seen(
            "t7",
            &result_event(1, Some(250_000), &trace.to_string_lossy()),
        );
        assert_eq!(fleet.items()[0].cost_usd, Some(0.25));
        fleet.seen("t7", &result_event(2, None, &trace.to_string_lossy()));
        assert_eq!(fleet.items()[0].cost_usd, None);
        fleet.seen(
            "t7",
            &result_event(3, Some(250_000), &trace.to_string_lossy()),
        );
        assert_eq!(fleet.items()[0].cost_usd, None, "still unknown");
        assert_eq!(trace_tokens(&trace), 0, "no trace, no tokens");
    }

    #[test]
    fn a_briefed_agent_run_moves_through_its_stages_and_ends_queued() {
        use serde_json::json;
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(11228, "starting", "briefed-11228-1", 1_000_000);
        fleet.engine_said(
            "briefed-11228-1",
            &json!({"phase": "issue", "text": "#11228 Fleet rows", "title": "Fleet rows"}),
        );
        fleet.engine_said(
            "briefed-11228-1",
            &json!({"phase": "briefing", "text": "finding the files this issue needs"}),
        );
        let item = &fleet.items()[0];
        assert_eq!(item.title, "#11228 Fleet rows");
        assert_eq!(item.line.as_deref(), Some("finding files"));
        fleet.engine_said(
            "briefed-11228-1",
            &json!({"phase": "agent", "text": "the briefed agent is working", "engine": "briefed"}),
        );
        fleet.engine_said(
            "briefed-11228-1",
            &json!({"phase": "cost", "text": "$0.40 so far", "cost_usd": 0.4, "tokens": 1200}),
        );
        let item = &fleet.items()[0];
        assert_eq!(item.line.as_deref(), Some("coding"));
        assert_eq!(item.engine.as_deref(), Some("briefed agent"));
        assert_eq!(item.cost_usd, Some(0.4));
        assert_eq!(item.tokens, Some(1200));
        fleet.engine_said(
            "briefed-11228-1",
            &json!({"phase": "verify", "text": "verify: passed"}),
        );
        assert_eq!(
            fleet.items()[0].line.as_deref(),
            Some("checks: verify: passed")
        );
        fleet.engine_finished(
            "briefed-11228-1",
            &json!({"type": "result", "ok": true, "engine": "briefed", "cost_usd": 0.55,
                "commit": "0123456789abcdef", "landed": {"how": "queue", "entry": {"id": "E1"}}}),
            1_090_000,
        );
        let item = &fleet.items()[0];
        assert_eq!(item.status, "done");
        assert_eq!(item.line.as_deref(), Some("queued E1"));
        assert_eq!(item.cost_usd, Some(0.55));
        assert_eq!(item.finished_unix, Some(1_090));
    }

    #[test]
    fn a_briefed_agent_run_keeps_an_unknown_cost_unknown_and_says_why_it_failed() {
        use serde_json::json;
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(7, "starting", "briefed-7-1", 1_000_000);
        fleet.engine_said(
            "briefed-7-1",
            &json!({"phase": "cost", "text": "cost unknown", "cost_usd": null}),
        );
        assert_eq!(fleet.items()[0].cost_usd, None);
        fleet.engine_finished(
            "briefed-7-1",
            &json!({"type": "result", "ok": false, "cost_usd": null,
                "error": "verify kept failing\nmore detail"}),
            1_010_000,
        );
        let item = &fleet.items()[0];
        assert_eq!(item.status, "failed");
        assert_eq!(item.cost_usd, None);
        assert_eq!(item.line.as_deref(), Some("failed: verify kept failing"));
    }

    #[test]
    fn a_message_a_run_cannot_read_says_so_on_its_row() {
        let mut fleet = Fleet::new("oa-dev-env-1");
        fleet.started(7, "Fix it", "briefed-7-1", 1_000_000);
        let said = carry_out(
            &fleet,
            &command("briefed-7-1", "message", Some("use the other file")),
            |_| Ok(()),
            |_, _| Err("the briefed agent can't read messages while it works".into()),
        )
        .unwrap();
        let (_, why) = said.split_once("could not be delivered: ").unwrap();
        fleet.noted("briefed-7-1", &format!("message not delivered: {why}"));
        let line = fleet.items()[0].line.clone().unwrap();
        assert!(line.contains("message not delivered"), "{line}");
    }
}
