//! Threads, and each thread as one ATIF trajectory.
//!
//! A thread is one conversation with OpenAgents: one row of the chat list
//! on the phone, the desktop, and the `openagents` CLI (`docs/glossary.md`,
//! Thread). The code elsewhere says *chat* or *conversation* for the same
//! thing.
//!
//! [`collect`] reads a whole thread through the local chat service
//! ([`crate::service`]), page by page, whether the service runs in this
//! process or behind a host's control socket. [`trajectory`] renders it as
//! an `ATIF-v1.8` document through `crates/atif`, so it can be exported,
//! replayed, and carried as NIP-ATIF:
//!
//! - `session_id` and `trajectory_id` are the thread's ID.
//! - Each turn is one step: the person's messages are `user` steps and
//!   OpenAgents' replies are `agent` steps, in order, stamped with the time
//!   this device saved them (a turn saved before times were kept takes the
//!   time of the turn before it, or the thread's start).
//! - The router's typed judgment of a reply is a decision call on that
//!   reply's step (`openagents.decision-call.v1`), and what the router said
//!   beside the text (tier, route, offers, suggestions, cards) is the step's
//!   `openagents.chat.router` extra. A reply that is a prepared knowledge
//!   answer names its entry as `served_answer`.
//! - Coder work the thread delegated is linked from the reply it followed:
//!   an observation result with `subagent_trajectory_ref` names the Coder
//!   task and its first attempt's trajectory file, which that task keeps on
//!   the computer that ran it.
//!
//! Nothing here sends anything or reads a key; the document holds what the
//! device's encrypted store held, and nothing more.

use crate::basic_chats::Summary;
use crate::basic_coder::{Role, Turn, WORKER};
use crate::router::ROUTER;
use crate::service::{Command, Snapshot};
use serde_json::{Value, json};

/// The agent a thread's trajectory names.
pub const AGENT: &str = "openagents-chat";
/// The model a thread's trajectory header names: the chat worker answers
/// through its own model lanes, and each reply's step names the model the
/// worker reported.
pub const MODEL: &str = "openagents-chat-worker";
/// The extra key under which a reply's router observations are kept.
pub const ROUTER_EXTRA: &str = "openagents.chat.router";
/// The extra key under which a delegated Coder task is kept.
pub const CODER_EXTRA: &str = "openagents.chat.coder";
/// The host a thread's binding names for a Coder run this computer
/// started for the person at it.
pub const LOCAL_HOST: &str = "local";

/// A whole thread: its row in the list and every turn it keeps, oldest
/// first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub summary: Summary,
    pub turns: Vec<Turn>,
    /// A reply is streaming into it now.
    pub busy: bool,
    /// Why the last message has no reply, when it has none.
    pub failure: Option<String>,
}

/// The most pages [`collect`] reads before it gives up on a thread that
/// keeps changing under it.
const ATTEMPTS: usize = 4;

/// Read the thread `id` whole through `apply`, the chat service's
/// operations (in-process [`crate::service::apply`] or a host's control
/// socket): its turns page by page, and its row from the list.
///
/// # Errors
/// The service's refusal, a missing thread, or a thread that kept changing
/// while it was read.
pub fn collect(
    id: &str,
    mut apply: impl FnMut(Command) -> Result<Snapshot, String>,
) -> Result<Thread, String> {
    for _ in 0..ATTEMPTS {
        let last = apply(Command::Read {
            chat: id.to_owned(),
            before: None,
        })?;
        if let Some(error) = &last.storage_error {
            return Err(error.clone());
        }
        let total = last.total;
        let mut start = last.start;
        let mut turns = last.turns.clone();
        let mut moved = false;
        while start > 0 {
            let page = apply(Command::Read {
                chat: id.to_owned(),
                before: Some(start),
            })?;
            if page.total != total || page.turns.is_empty() {
                moved = true;
                break;
            }
            start = page.start;
            let mut older = page.turns;
            older.extend(turns);
            turns = older;
        }
        if moved {
            continue;
        }
        let summary = row(id, &last, &mut apply)?;
        return Ok(Thread {
            summary,
            turns,
            busy: last.busy,
            failure: last.failure,
        });
    }
    Err("This thread kept changing while it was read. Try again.".into())
}

/// The thread's row: in the first page of the list, or a later one.
fn row(
    id: &str,
    first: &Snapshot,
    apply: &mut impl FnMut(Command) -> Result<Snapshot, String>,
) -> Result<Summary, String> {
    if let Some(summary) = first.chats.iter().find(|row| row.id == id) {
        return Ok(summary.clone());
    }
    let mut after = first.list_start + first.chats.len();
    while after < first.list_total {
        let page = apply(Command::ListMore {
            after,
            version: first.list_version,
        })?;
        if let Some(summary) = page.chats.iter().find(|row| row.id == id) {
            return Ok(summary.clone());
        }
        if page.chats.is_empty() {
            break;
        }
        after += page.chats.len();
    }
    Err("Thread not found.".into())
}

/// The door a thread's replies came through: the chat worker, by its
/// public key.
pub fn door() -> String {
    format!("nip-cj:{WORKER}")
}

/// The thread as one `ATIF-v1.8` document, written by `version` of the
/// exporting program.
pub fn trajectory(thread: &Thread, version: &str) -> Value {
    trajectory_with(thread, version, Vec::new())
}

/// [`trajectory`], carrying the delegated Coder task's own trajectories
/// (one ATIF document per turn, oldest first) inside the thread's, as
/// `subagent_trajectories`: the delegating step's reference then names the
/// first one by its `trajectory_id`, so a reader has every step the task
/// took without the computer that ran it.
pub fn trajectory_with(thread: &Thread, version: &str, tasks: Vec<Value>) -> Value {
    let summary = &thread.summary;
    let mut session = atif::Session::opening(&summary.id, MODEL, &door(), "", version);
    session.directive = summary.title.clone();
    session.seconds = summary.updated.saturating_sub(summary.started);
    let complete = !thread.busy
        && thread
            .turns
            .last()
            .is_some_and(|turn| turn.role == Role::Assistant && !turn.stopped);
    session.state = if complete {
        atif::log::ENDED
    } else {
        atif::log::INTERRUPTED
    }
    .to_owned();
    let mut last = summary.started;
    let mut steps: Vec<atif::Step> = Vec::with_capacity(thread.turns.len());
    for (index, turn) in thread.turns.iter().enumerate() {
        let at = turn.at.unwrap_or(last);
        last = at;
        steps.push(step(turn, index + 1, at));
    }
    // The delegating reply: the last one before the task was started.
    let delegated = summary.coder.as_ref().and_then(|coder| {
        let index = thread
            .turns
            .iter()
            .enumerate()
            .filter(|(index, turn)| {
                turn.role == Role::Assistant
                    && coder
                        .at
                        .is_none_or(|at| steps[*index].at <= at.saturating_mul(1_000))
            })
            .map(|(index, _)| index)
            .next_back()?;
        let link = json!({
            "host": coder.host,
            "task": coder.task,
            "project": coder.project,
            "at": coder.at,
        });
        steps[index].extensions.insert(CODER_EXTRA.into(), link);
        Some((index, coder.clone()))
    });
    let mut document = atif::document(&session, &steps);
    document["agent"]["name"] = json!(AGENT);
    document["extra"]["thread"] = json!({
        "title": summary.title,
        "started": summary.started,
        "updated": summary.updated,
        "pinned": summary.pinned,
        "archived": summary.archived,
        "named": summary.named,
    });
    document["extra"]["router"] = json!(ROUTER);
    if let Some(failure) = &thread.failure {
        document["extra"]["failure"] = json!(failure);
    }
    if let Some((index, coder)) = delegated
        && let Some(step) = document["steps"].get_mut(index)
    {
        // ATIF links a delegation from an observation result. The task's
        // first attempt writes `<task>.1.atif.jsonl` on the host that ran it.
        let content = if coder.host == LOCAL_HOST {
            format!("Coder task {} on this computer", coder.task)
        } else {
            format!("Coder task {} on computer {}", coder.task, coder.host)
        };
        let mut reference = json!({
            "session_id": coder.task,
            "trajectory_path": format!("{}.1.atif.jsonl", coder.task),
            "extra": {
                "host": coder.host,
                "task": coder.task,
                "project": coder.project,
            },
        });
        if let Some(first) = tasks.first() {
            reference["session_id"] = first["session_id"].clone();
            reference["trajectory_id"] = first["trajectory_id"].clone();
            reference["extra"]["turns"] = json!(tasks.len());
        }
        let result = json!({
            "content": content,
            "subagent_trajectory_ref": [reference],
        });
        match step["observation"]["results"].as_array_mut() {
            Some(results) => results.push(result),
            None => step["observation"] = json!({ "results": [result] }),
        }
        if !tasks.is_empty() {
            document["subagent_trajectories"] = Value::Array(tasks);
        }
    }
    document
}

/// One turn as one step; `ordinal` numbers its decision call.
fn step(turn: &Turn, ordinal: usize, at: u64) -> atif::Step {
    let source = match turn.role {
        Role::User => atif::Source::User,
        Role::Assistant => atif::Source::Agent,
    };
    let mut step = atif::Step::said(source, &turn.text);
    step.at = at.saturating_mul(1_000);
    if turn.role == Role::Assistant {
        if let Some(model) = &turn.model {
            step = step.by(model);
        }
        if let Some(meta) = &turn.meta {
            if let Some(judgment) = meta.judgment.as_deref() {
                let answers: Value =
                    serde_json::from_str(judgment).unwrap_or_else(|_| json!(judgment));
                let model = answers["model"]
                    .as_str()
                    .map_or_else(|| MODEL.to_owned(), str::to_owned);
                step.call = Some(
                    atif::Decision {
                        id: format!("route-{ordinal}"),
                        name: "chat_router".into(),
                        door: door(),
                        model,
                        request: json!({ "router": ROUTER }),
                        answers,
                        route: meta.route.clone(),
                        error: None,
                        attempts: vec![],
                        review: None,
                        milliseconds: 0,
                    }
                    .call(),
                );
            }
            let mut said = serde_json::to_value(meta).unwrap_or_default();
            if let Some(fields) = said.as_object_mut() {
                fields.remove("judgment");
            }
            if said.as_object().is_some_and(|fields| !fields.is_empty()) {
                step = step.noting(ROUTER_EXTRA, said);
            }
            if let Some(answer) = &meta.answer {
                step = step.noting("served_answer", json!(answer));
            }
        }
    }
    if turn.stopped {
        step = step.noting("stopped", json!(true));
    }
    step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_chats::{BasicChats, Spawned};
    use crate::basic_coder::{Door, Reply, lock};
    use crate::cache::Cache;
    use crate::router::{Context, Meta, Offer};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    fn summary(id: &str) -> Summary {
        Summary {
            id: id.into(),
            title: "How do I connect a phone".into(),
            started: 1_700_000_000,
            updated: 1_700_000_060,
            coder: None,
            archived: false,
            pinned: false,
            named: false,
        }
    }

    fn thread() -> Thread {
        let id = "c".repeat(32);
        let mut first = Turn::user("How do I connect a phone");
        first.at = Some(1_700_000_000);
        let mut answer = Turn::assistant(
            "Scan the QR code the desktop app shows.",
            Some(Meta {
                tier: Some("canned".into()),
                answer: Some("openagents.connect-phone@2".into()),
                route: Some("answer".into()),
                judgment: Some(
                    json!({"v": 2, "type": "judgment", "lane": "chat", "route": "answer", "model": "jev"})
                        .to_string(),
                ),
                offers: vec![Offer::RunCoder],
                ..Meta::default()
            }),
        );
        answer.at = Some(1_700_000_001);
        answer.model = Some("bank:openagents".into());
        let mut second = Turn::user("And then?");
        second.at = Some(1_700_000_050);
        let mut reply = Turn::assistant("Then pick a project.", None);
        reply.at = Some(1_700_000_060);
        reply.model = Some("google/gemini-flash".into());
        let mut summary = summary(&id);
        summary.coder = Some(Spawned {
            host: "a".repeat(64),
            task: "task-1".into(),
            project: Some("openagents".into()),
            at: Some(1_700_000_070),
        });
        Thread {
            summary,
            turns: vec![first, answer, second, reply],
            busy: false,
            failure: None,
        }
    }

    #[test]
    fn a_thread_is_one_atif_trajectory_that_reads_back() {
        let thread = thread();
        let mut document = trajectory(&thread, "test");
        assert!(atif::validate(&document).is_empty(), "{document}");
        assert_eq!(atif::upgrade(&mut document).unwrap(), "ATIF-v1.8");
        assert_eq!(document["schema_version"], "ATIF-v1.8");
        assert_eq!(document["session_id"], "c".repeat(32));
        assert_eq!(document["trajectory_id"], "c".repeat(32));
        assert_eq!(document["agent"]["name"], AGENT);
        let steps = document["steps"].as_array().unwrap();
        assert_eq!(steps.len(), thread.turns.len(), "one step per turn");
        assert_eq!(steps[0]["source"], "user");
        assert_eq!(steps[1]["source"], "agent");
        assert_eq!(steps[1]["model_name"], "bank:openagents");
        let call = &steps[1]["tool_calls"][0];
        assert_eq!(call["function_name"], "chat_router");
        assert_eq!(call["extra"]["schema"], atif::DECISION_CALL_SCHEMA);
        assert_eq!(call["extra"]["route"], "answer");
        assert_eq!(call["extra"]["model"], "jev");
        assert_eq!(
            steps[1]["extra"]["served_answer"],
            "openagents.connect-phone@2"
        );
        assert_eq!(
            steps[1]["extra"][ROUTER_EXTRA]["offers"][0]["offer"],
            "run_coder"
        );
        assert!(steps[1]["extra"][ROUTER_EXTRA].get("judgment").is_none());
        assert_eq!(steps[3]["model_name"], "google/gemini-flash");
        assert_eq!(steps[3]["timestamp"], atif::iso(1_700_000_060_000));
        // The task was started after the last reply, so that reply links it.
        let link = &steps[3]["observation"]["results"][0]["subagent_trajectory_ref"][0];
        assert_eq!(link["session_id"], "task-1");
        assert_eq!(link["trajectory_path"], "task-1.1.atif.jsonl");
        assert_eq!(steps[3]["extra"][CODER_EXTRA]["task"], "task-1");
        assert_eq!(
            document["final_metrics"]["extra"]["decision_calls"]["total"],
            1
        );
        assert_eq!(document["extra"]["state"], "ended");
        assert_eq!(document["extra"]["directive"], "How do I connect a phone");
    }

    #[test]
    fn a_local_task_travels_inside_the_thread() {
        let mut thread = thread();
        if let Some(coder) = thread.summary.coder.as_mut() {
            coder.host = LOCAL_HOST.into();
        }
        let task = json!({
            "schema_version": "ATIF-v1.8",
            "session_id": "task-1-1",
            "trajectory_id": "task-1-1",
            "agent": {"name": "microcoder-repository", "version": "0.1.0"},
            "steps": [{"step_id": 1, "source": "user", "message": "add a test"}],
        });
        let document = trajectory_with(&thread, "test", vec![task.clone()]);
        assert!(atif::validate(&document).is_empty(), "{document}");
        assert_eq!(document["subagent_trajectories"][0], task);
        let result = &document["steps"][3]["observation"]["results"][0];
        assert_eq!(result["content"], "Coder task task-1 on this computer");
        let link = &result["subagent_trajectory_ref"][0];
        assert_eq!(link["trajectory_id"], "task-1-1");
        assert_eq!(link["extra"]["turns"], 1);
        // Without embedded trajectories, nothing is added.
        assert!(
            trajectory(&thread, "test")
                .get("subagent_trajectories")
                .is_none()
        );
    }

    #[test]
    fn an_unanswered_or_untimed_thread_is_interrupted_and_keeps_order() {
        let id = "d".repeat(32);
        let thread = Thread {
            summary: summary(&id),
            turns: vec![
                Turn::user("hello"),
                Turn::assistant("hi", None),
                Turn::user("more"),
            ],
            busy: false,
            failure: Some("We couldn't reply this time. Try again.".into()),
        };
        let document = trajectory(&thread, "test");
        assert!(atif::validate(&document).is_empty());
        assert_eq!(document["extra"]["state"], "interrupted");
        let steps = document["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
        // Turns from before times were kept take the thread's start.
        assert_eq!(steps[0]["timestamp"], atif::iso(1_700_000_000_000));
        assert!(steps[1].get("tool_calls").is_none());
        assert!(steps.iter().all(|step| step.get("observation").is_none()));
    }

    /// Answers with the message's reverse, at once.
    struct Echo;
    impl Door for Echo {
        fn ask(
            &self,
            turns: Vec<Turn>,
            _context: Context,
            reply: Arc<Mutex<Reply>>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let text: String = turns.last().unwrap().text.chars().rev().collect();
                let mut reply = lock(&reply);
                reply.text = text;
                reply.model = Some("echo".into());
                reply.done = true;
            })
        }
    }

    #[test]
    fn collect_reads_every_page_through_the_service() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let key = secp256k1::SecretKey::from_byte_array([3; 32]).unwrap();
        let mut chats = BasicChats::new(
            Some(runtime.handle().clone()),
            Some(Arc::new(Echo)),
            Some(Cache::open(dir.path(), &key).unwrap()),
        );
        let id = "e".repeat(32);
        crate::service::apply(&mut chats, Command::Create { chat: id.clone() }, 1).unwrap();
        for n in 0..20u64 {
            crate::service::apply(
                &mut chats,
                Command::Send {
                    chat: id.clone(),
                    request: format!("{n:032x}"),
                    text: format!("message {n}"),
                },
                10 + n,
            )
            .unwrap();
            for _ in 0..500 {
                chats.settle(10 + n);
                if !chats.busy(&id) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        // Other threads fill the list's first page.
        for n in 0..130 {
            chats.create(&format!("{:032x}", 1_000 + n), 100);
        }
        let mut reads = 0;
        let thread = collect(&id, |command| {
            reads += 1;
            crate::service::apply(&mut chats, command, 200)
        })
        .unwrap();
        assert!(reads > 2, "read {reads} pages");
        assert_eq!(thread.turns.len(), 40);
        assert_eq!(thread.turns[0].text, "message 0");
        assert_eq!(thread.turns[0].at, Some(10));
        assert_eq!(thread.turns[39].text, "91 egassem");
        assert_eq!(thread.turns[39].model.as_deref(), Some("echo"));
        assert_eq!(thread.summary.id, id);
        let document = trajectory(&thread, "test");
        assert_eq!(document["steps"].as_array().unwrap().len(), 40);
        assert!(
            collect(&"f".repeat(32), |command| crate::service::apply(
                &mut chats, command, 201
            ))
            .is_err()
        );
    }
}
