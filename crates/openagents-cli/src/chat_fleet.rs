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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_fleet::{AgentRow, Status};
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
    /// The queue is done: report once more, then stop.
    closed: bool,
}

impl Fleet {
    pub(super) fn new(computer: &str) -> Self {
        Self {
            computer: computer.to_owned(),
            rows: Vec::new(),
            issues: BTreeMap::new(),
            lines: BTreeMap::new(),
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
            task: format!("Issue #{issue}: {}", title.trim()),
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
            CoderEvent::Result(finished) => {
                if let Some(cost) = finished.cost_microusd {
                    row.cost_usd = Some(row.cost_usd.unwrap_or(0.0) + cost as f64 / 1e6);
                }
            }
            _ => {}
        }
        if let Some(text) = coder_events::text(event).filter(|text| !text.trim().is_empty()) {
            self.lines.insert(task.to_owned(), text);
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
            .map(|row| item(row, self.lines.get(&row.id).map(String::as_str)))
            .collect()
    }
}

/// How an issue flow's outcome reads in the list.
pub(super) fn status_of(outcome: &str) -> Status {
    match outcome {
        "landed" | "pull_request" | "unchanged" | "closed" | "skipped" => Status::Done,
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

/// Report `shared` to the account while `chat work` runs, carrying out
/// the commands that come back. `None` when this computer is not signed in.
pub(super) fn spawn(shared: Shared, store: PathBuf) -> Option<tokio::task::JoinHandle<()>> {
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
                Ok(fleet) => (fleet.computer.clone(), fleet.items(), fleet.closed),
                Err(_) => return,
            };
            if let Ok(commands) = activity::report(&http, &saved, &computer, &items).await {
                for command in commands {
                    let (shared, store) = (Arc::clone(&shared), store.clone());
                    let said = tokio::task::spawn_blocking(move || {
                        let fleet = shared.0.lock().ok()?;
                        let local = Local::here(store);
                        carry_out(
                            &fleet,
                            &command,
                            |task| local.stop(task),
                            |task, text| local.steer(task, text).map(|_| ()),
                        )
                    })
                    .await
                    .ok()
                    .flatten();
                    if let Some(said) = said {
                        eprintln!("{said}");
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
        assert_eq!(
            item.title,
            "Issue #11228: Fleet rows for cloud environment runs"
        );
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
}
