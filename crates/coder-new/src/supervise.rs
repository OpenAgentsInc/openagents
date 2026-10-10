//! Supervising this terminal from the phone (#11165).
//!
//! While sync is on, Coder reports what runs here to the website
//! ([`coder_sync::activity`]): the open chat while it replies (or asks for
//! approval), and each delegated agent. The phone lists them, and its
//! **Stop**, **Approve**, **Deny**, and **Message** come back as commands,
//! which this module carries out the way the keys here would: Stop is Esc,
//! Approve and Deny answer the approval question, and a message is answered
//! like a reply typed on openagents.com.
//!
//! Finished work stays on the list for [`KEEP_FINISHED`], so the phone can
//! say it finished. Background agents (#11163) join the list through
//! [`App::background_agents`].

use std::time::{Duration, Instant};

use coder_sync::activity::{Command, Item, MAX_ITEMS, Question};
use serde_json::Value;

use crate::App;

/// How long finished work stays on the phone's list.
pub const KEEP_FINISHED: Duration = Duration::from_secs(30 * 60);
/// What Coder says when the phone stopped a reply.
pub const STOPPED: &str = "Stopped from your phone.";
/// The longest title or line sent.
const TITLE_CHARS: usize = 120;

/// What the phone was last told, and the work that finished lately.
#[derive(Default)]
pub(crate) struct Board {
    /// The open chat's reply: its session and when it started.
    turn: Option<(String, u64)>,
    /// The phone stopped the current reply.
    stopped: bool,
    /// Finished work, newest last, with when it finished here.
    finished: Vec<(Item, Instant)>,
    /// When each delegated agent was first seen running here.
    agent_started: std::collections::BTreeMap<String, u64>,
    /// The items last sent; `None` before the first report.
    pub(crate) sent: Option<Vec<Item>>,
}

/// Items without the counts that change while a reply streams, to tell a
/// change worth reporting at once from one the next report carries.
pub(crate) fn settled(items: Option<&Vec<Item>>) -> Option<Vec<Item>> {
    items.map(|items| {
        items
            .iter()
            .cloned()
            .map(|mut item| {
                item.tokens = None;
                item
            })
            .collect()
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
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

/// The approval question as the phone shows it.
pub(crate) fn question_text(event: &Value) -> String {
    if event["kind"] == "action" {
        crate::ops_tool::question_text(event)
    } else if event["kind"] == "computer" {
        format!(
            "Run this on {}?\n\n{}",
            event["host"].as_str().unwrap_or_default(),
            event["command"].as_str().unwrap_or_default(),
        )
    } else {
        format!(
            "Send this lookup to {}?\n\n{}",
            event["recipient"].as_str().unwrap_or("Brainstorm"),
            serde_json::to_string_pretty(&event["input"]).unwrap_or_default(),
        )
    }
}

/// The id an approval question goes by.
fn question_id(event: &Value) -> String {
    match &event["id"] {
        Value::String(id) => id.clone(),
        other => other.to_string(),
    }
}

impl App {
    /// The open chat's title: its first message, shortened.
    fn chat_title(&self) -> String {
        self.live
            .entries
            .iter()
            .find_map(|entry| match entry {
                crate::live::Entry::User(text) => Some(short(text, TITLE_CHARS)),
                _ => None,
            })
            .unwrap_or_else(|| "Coder chat".into())
    }

    /// Background agents (#11163) to list for the phone, beside the open
    /// chat and the delegated agents: one [`Item`] per agent with its
    /// engine, status, tokens and cost. Their commands are handled in
    /// [`Self::apply_command`].
    pub(crate) fn background_agents(&self) -> Vec<Item> {
        self.fleet
            .list()
            .into_iter()
            .map(|row| Item {
                id: row.id.clone(),
                kind: "agent".into(),
                title: short(&row.name, TITLE_CHARS),
                engine: Some(row.engine.clone()),
                status: match row.status {
                    agent_fleet::Status::Running => "working",
                    agent_fleet::Status::Done => "done",
                    agent_fleet::Status::Failed => "failed",
                    agent_fleet::Status::Stopped => "stopped",
                }
                .into(),
                started_unix: row.started_ms / 1000,
                finished_unix: row.ended_ms.map(|ended| ended / 1000),
                cost_usd: row.cost_usd,
                tokens: (row.tokens > 0).then_some(row.tokens),
                session: row.parent_session.clone(),
                question: None,
                line: Some(short(&row.task, 200)).filter(|t| !t.is_empty()),
            })
            .collect()
    }

    /// What runs here now, for the phone: the open chat's reply, delegated
    /// agents, background agents, then work that finished lately.
    pub(crate) fn activity_items(&mut self) -> Vec<Item> {
        let now_unix = now();
        let session = self.session_id().map(str::to_owned);
        let busy = self.live.busy;
        let title = self.chat_title();
        let engine = Some(self.plugins.model.clone()).filter(|m| !m.is_empty());
        let question = self.disclosure_event.as_ref().map(|event| Question {
            id: question_id(event),
            text: question_text(event),
        });
        let tokens = (self.live.tokens > 0).then_some(self.live.tokens);
        let background = self.background_agents();
        let Some(sync) = &mut self.sync else {
            return Vec::new();
        };
        let board = &mut sync.board;
        let elapsed = self.elapsed_seconds;
        board
            .agent_started
            .retain(|id, _| self.delegations.iter().any(|agent| agent.id == *id));
        let agents: Vec<Item> = self
            .delegations
            .iter()
            .filter(|agent| !agent.background)
            .map(|agent| {
                let started = *board
                    .agent_started
                    .entry(agent.id.clone())
                    .or_insert_with(|| {
                        now_unix.saturating_sub(elapsed.saturating_sub(agent.started_at))
                    });
                Item {
                    id: agent.id.clone(),
                    kind: "agent".into(),
                    title: short(&agent.name, TITLE_CHARS),
                    engine: None,
                    status: if agent.running { "working" } else { "done" }.into(),
                    started_unix: started,
                    finished_unix: (!agent.running)
                        .then_some(started.saturating_add(agent.elapsed_seconds)),
                    cost_usd: None,
                    tokens: (agent.chat.tokens > 0).then_some(agent.chat.tokens),
                    session: None,
                    question: None,
                    line: Some(short(&agent.task, 200)).filter(|t| !t.is_empty()),
                }
            })
            .collect();
        let turn_item = |session: &str, started: u64, status: &str| Item {
            id: session.to_owned(),
            kind: "chat".into(),
            title: title.clone(),
            engine: engine.clone(),
            status: status.into(),
            started_unix: started,
            finished_unix: None,
            cost_usd: None,
            tokens,
            session: Some(session.to_owned()),
            question: None,
            line: None,
        };
        // A reply that ended (or whose chat closed) is finished work.
        let ended = match (&board.turn, &session) {
            (Some((was, _)), Some(open)) => !busy || was != open,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if ended && let Some((was, started)) = board.turn.take() {
            let mut done = turn_item(
                &was,
                started,
                if std::mem::take(&mut board.stopped) {
                    "stopped"
                } else {
                    "done"
                },
            );
            done.finished_unix = Some(now_unix);
            board.finished.retain(|(item, _)| item.id != was);
            board.finished.push((done, Instant::now()));
        }
        let mut items = Vec::new();
        if busy && let Some(open) = &session {
            let started = board.turn.get_or_insert((open.clone(), now_unix)).1;
            let mut item = turn_item(open, started, "working");
            if let Some(question) = question {
                item.status = "asking".into();
                item.question = Some(question);
            }
            board.finished.retain(|(done, _)| done.id != *open);
            items.push(item);
        }
        items.extend(agents);
        items.extend(background);
        board
            .finished
            .retain(|(_, at)| at.elapsed() < KEEP_FINISHED);
        for (done, _) in board.finished.iter().rev() {
            if !items.iter().any(|item| item.id == done.id) {
                items.push(done.clone());
            }
        }
        items.truncate(MAX_ITEMS);
        items
    }

    /// Carry out one command from the phone.
    pub(crate) fn apply_command(&mut self, command: &Command) {
        // A background agent is stopped or messaged through its list.
        if self.fleet.get(&command.item).is_some() {
            match command.action.as_str() {
                "stop" => {
                    if let Err(error) = self.fleet.stop(&command.item) {
                        self.notice = Some(error);
                    }
                }
                "message" => {
                    let text = command.text.as_deref().unwrap_or_default().trim();
                    if !text.is_empty() {
                        let id = command.item.clone();
                        self.deliver_agent_message(&id, text);
                    }
                }
                _ => {}
            }
            return;
        }
        let open = self.session_id().map(str::to_owned);
        let is_open_chat = open.as_deref() == Some(command.item.as_str());
        let agent_running = self
            .delegations
            .iter()
            .any(|agent| agent.id == command.item && agent.running && !agent.background);
        match command.action.as_str() {
            "stop" if (is_open_chat && self.live.busy) || agent_running => {
                self.cancel_request();
                self.live.notice = Some(STOPPED.into());
                if let Some(sync) = &mut self.sync {
                    sync.board.stopped = true;
                }
            }
            "approve" | "deny" if is_open_chat => {
                let matches = self.disclosure_event.as_ref().is_some_and(|event| {
                    command
                        .question
                        .as_deref()
                        .is_none_or(|id| id == question_id(event))
                });
                if matches {
                    // The website's confirm card sends `web` (#11170); the
                    // phone sends nothing.
                    let via = match command.text.as_deref() {
                        Some("web") => "web",
                        _ => "phone",
                    };
                    self.answer_disclosure_from(command.action == "approve", via);
                }
            }
            "message" => {
                let text = command.text.as_deref().unwrap_or_default().trim();
                if text.is_empty() {
                    return;
                }
                let session = if is_open_chat {
                    open
                } else {
                    self.sync.as_ref().and_then(|sync| {
                        sync.board
                            .finished
                            .iter()
                            .find(|(item, _)| item.id == command.item)
                            .and_then(|(item, _)| item.session.clone())
                    })
                };
                match session {
                    Some(session) => self.queue_phone_message(session, text.to_owned()),
                    None => {
                        self.notice =
                            Some("A message from your phone couldn't reach that agent.".into());
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn command(item: &str, action: &str) -> Command {
        Command {
            id: "c1".into(),
            item: item.into(),
            action: action.into(),
            question: None,
            text: None,
        }
    }

    fn signed_in_app(dir: &std::path::Path) -> App {
        let mut app = App {
            account_dir: Some(dir.to_path_buf()),
            account: Some("Octo".into()),
            ..App::default()
        };
        app.attach_session_store(crate::sessions::Store::under(dir));
        app.start_sync();
        app
    }

    #[test]
    fn a_reply_shows_working_then_done_and_stays_listed() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = signed_in_app(dir.path());
        assert!(app.activity_items().is_empty());
        assert!(app.open_new_session("s-phone-1"));
        app.live
            .entries
            .push(crate::live::Entry::User("Fix the flaky test".into()));
        app.live.busy = true;
        let items = app.activity_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "s-phone-1");
        assert_eq!(items[0].status, "working");
        assert_eq!(items[0].title, "Fix the flaky test");
        assert_eq!(items[0].session.as_deref(), Some("s-phone-1"));

        app.disclosure_event = Some(json!({
            "event": "approval", "kind": "computer", "id": 7,
            "host": "Studio", "command": "cargo clean"
        }));
        let items = app.activity_items();
        assert_eq!(items[0].status, "asking");
        let question = items[0].question.as_ref().unwrap();
        assert_eq!(question.id, "7");
        assert!(question.text.contains("cargo clean"));

        app.disclosure_event = None;
        app.live.busy = false;
        let items = app.activity_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "done");
        assert!(items[0].finished_unix.is_some());
    }

    #[test]
    fn background_agents_are_listed_with_cost_and_stopped_from_the_phone() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = signed_in_app(dir.path());
        let control = app
            .fleet
            .start(agent_fleet::Spec {
                engine: "codex".into(),
                task: "fix the login".into(),
                ..agent_fleet::Spec::default()
            })
            .unwrap();
        control.add_usage(1200, Some(0.42));
        control.event(crate::bundled_runtime::RuntimeEvent::Text("working".into()));
        app.poll_fleet();
        let items = app.activity_items();
        let agents: Vec<_> = items.iter().filter(|item| item.kind == "agent").collect();
        assert_eq!(agents.len(), 1, "listed once, from the agent list");
        assert_eq!(agents[0].engine.as_deref(), Some("codex"));
        assert_eq!(agents[0].status, "working");
        assert_eq!(agents[0].cost_usd, Some(0.42));
        app.apply_command(&command(control.id(), "stop"));
        assert!(control.stopped());
    }

    #[test]
    fn stop_from_the_phone_stops_the_reply_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = signed_in_app(dir.path());
        assert!(app.open_new_session("s-stop"));
        app.live.busy = true;
        app.activity_items();
        // A stop for something else changes nothing.
        app.apply_command(&command("other", "stop"));
        assert!(app.live.busy);
        app.apply_command(&command("s-stop", "stop"));
        assert!(!app.live.busy);
        assert_eq!(app.live.notice.as_deref(), Some(STOPPED));
        let items = app.activity_items();
        assert_eq!(items[0].status, "stopped");
    }

    #[test]
    fn approve_answers_only_the_question_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = signed_in_app(dir.path());
        assert!(app.open_new_session("s-ask"));
        app.live.busy = true;
        let desk = crate::approval::Desk::new();
        app.disclosure_desk = Some(desk.clone());
        app.disclosure_event = Some(json!({"event": "approval", "kind": "computer", "id": 3}));
        let mut wrong = command("s-ask", "approve");
        wrong.question = Some("9".into());
        app.apply_command(&wrong);
        assert!(app.disclosure_event.is_some());
        let mut right = command("s-ask", "approve");
        right.question = Some("3".into());
        app.apply_command(&right);
        assert!(app.disclosure_event.is_none());
    }

    #[test]
    fn a_message_from_the_phone_is_answered_like_a_web_reply() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = signed_in_app(dir.path());
        assert!(app.open_new_session("s-msg"));
        let mut message = command("s-msg", "message");
        message.text = Some("Now run the tests".into());
        app.apply_command(&message);
        app.answer_web_reply();
        assert!(matches!(
            app.live.entries.last(),
            Some(crate::live::Entry::User(text)) if text == "Now run the tests"
        ));
        // An item it can't reach is said plainly.
        let mut lost = command("nowhere", "message");
        lost.text = Some("Hi".into());
        app.apply_command(&lost);
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|n| n.contains("couldn't reach"))
        );
    }
}
