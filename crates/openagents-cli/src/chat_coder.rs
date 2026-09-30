//! `openagents chat`'s Coder runs on this computer: start one for a thread,
//! follow its events, answer it, and stop it.
//!
//! Nothing here runs Coder itself. The run is [`coder::task::local`], the
//! shared start the host's auto-start also uses, and the events are
//! [`openagents_chat::coder_events`], the stream the desktop and the phone
//! render. This module prints them: a compact live view on stderr and the
//! result on stdout, or every event as one NDJSON line under `--json`.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use coder::task::local::{self, Follow, Local, State};
use openagents_chat::coder_events::{self, CoderEvent, Line, StepKind};
use openagents_chat::service::Command;
use serde_json::json;

use super::{Backend, event};
use crate::EXIT_FAILURE;
use crate::out::Output;

/// How often a running task's trajectory is read.
const POLL: Duration = Duration::from_millis(300);

/// The task store a thread's Coder runs use: the scratch thread's own,
/// or [`local::default_store`].
pub(super) fn store(backend: &Backend, thread: &str) -> PathBuf {
    match backend {
        Backend::Local { scratch: true, .. } => super::scratch_dir(thread).join("tasks"),
        _ => local::default_store(),
    }
}

/// The runner over `backend`'s store, with the person's settings
/// (`coder::task::settings`).
fn runner(backend: &Backend, thread: &str) -> Local {
    Local::here(store(backend, thread))
}

/// Who a Coder run for this thread would use now, and why: the runner's
/// own choice ([`Local::predict`]) over the thread's store and settings.
pub(super) fn predict(
    backend: &Backend,
    thread: &str,
) -> Option<openagents_chat::coder_events::Runner> {
    runner(backend, thread).predict()
}

/// Whether a coding reply waits for `openagents chat run-coder` instead of
/// running at once: the settings' `coder.start` is `ask_first`, or they
/// cannot be read (the refusal then shows when the person accepts).
pub(super) fn asks_first() -> bool {
    Local::here(PathBuf::new()).asks_first()
}

/// The words that answer a thread's question from this terminal.
fn answer_hint(backend: &Backend, thread: &str) -> String {
    format!(
        "openagents chat answer{} --thread {thread} \"YOUR ANSWER\"",
        flag(backend)
    )
}

/// The switch that reaches the thread's store again: `--scratch` for a
/// scratch thread, `--local` for one in this command's own store (so a
/// host started later is not asked for it).
fn flag(backend: &Backend) -> &'static str {
    match backend {
        Backend::Local { scratch: true, .. } => " --scratch",
        Backend::Local { .. } => " --local",
        Backend::Host { .. } => "",
    }
}

/// Whether a Coder run could start from here now: the directory is a
/// checkout that counts as a project in the person's settings, and an
/// allowed provider is signed in with capacity. Reads only.
pub(super) fn ready(store: PathBuf) -> bool {
    let run = Local::here(store);
    let here = std::env::current_dir().ok();
    here.is_some_and(|dir| run.project(&dir).is_ok()) && run.ready()
}

/// Start Coder on this computer for the thread `id`, in the checkout the
/// command runs in, bind the task to the thread, and follow it.
pub(super) async fn start(output: &Output, backend: &mut Backend, id: &str) -> u8 {
    let report = |accepted: bool, message: &str, task: Option<serde_json::Value>| {
        event(
            output,
            json!({"event": "coder", "thread": id, "accepted": accepted, "message": message, "task": task}),
        );
        if !output.json() {
            eprintln!("{message}");
        }
    };
    let thread = match backend.collect(id).await {
        Ok(thread) => thread,
        Err(super::Failure::Failed(message) | super::Failure::Usage(message)) => {
            report(false, &message, None);
            return EXIT_FAILURE;
        }
    };
    if let Some(coder) = &thread.summary.coder {
        report(
            true,
            &format!(
                "Coder already runs task {} for this thread; following it.",
                coder.task
            ),
            serde_json::to_value(coder).ok(),
        );
        return follow_task(output, backend, id, &coder.task, 1).await;
    }
    let prompt = openagents_chat::delegation::prompt(&thread.summary.title, &thread.turns);
    let here = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            report(false, "This command has no working directory.", None);
            return EXIT_FAILURE;
        }
    };
    let run = runner(backend, id);
    let title = thread.summary.title.clone();
    let chat = id.to_owned();
    let started =
        tokio::task::spawn_blocking(move || run.start(&here, &title, &prompt, Some(&chat)))
            .await
            .unwrap_or_else(|_| Err("Coder could not start.".into()));
    let record = match started {
        Ok(record) => record,
        Err(message) => {
            report(false, &message, None);
            return EXIT_FAILURE;
        }
    };
    let bound = backend
        .apply(Command::BindCoder {
            chat: id.to_owned(),
            host: local::LOCAL_HOST.into(),
            task: record.task.clone(),
            project: Some(record.project.clone()),
        })
        .await;
    if let Err(why) = bound {
        eprintln!(
            "openagents chat: Coder started, but the thread could not record its task ({why}); \
             follow it with `openagents chat follow --thread {id}` from this store."
        );
    }
    report(
        true,
        &format!(
            "Coder started task {} in a worktree of {}.",
            record.task, record.project
        ),
        Some(json!({
            "host": local::LOCAL_HOST,
            "task": record.task,
            "project": record.project,
            "worktree": record.worktree,
        })),
    );
    follow_task(output, backend, id, &record.task, 1).await
}

/// `chat follow`: replay the thread's task from its first event and keep
/// streaming until it ends or asks.
pub(super) async fn follow(output: &Output, backend: &mut Backend, id: &str) -> u8 {
    let Some(task) = bound(output, backend, id).await else {
        return EXIT_FAILURE;
    };
    follow_task(output, backend, id, &task, 1).await
}

/// The task bound to the thread, or a said failure.
async fn bound(output: &Output, backend: &mut Backend, id: &str) -> Option<String> {
    let snapshot = backend
        .apply(Command::Read {
            chat: id.to_owned(),
            before: None,
        })
        .await;
    let message = match snapshot {
        Ok(snapshot) => match snapshot.coder {
            Some(coder) => return Some(coder.task),
            None => "This thread has not started Coder.".to_owned(),
        },
        Err(message) => message,
    };
    event(
        output,
        json!({"event": "failure", "thread": id, "message": message, "stopped": false}),
    );
    eprintln!("openagents chat: {message}");
    None
}

/// `chat stop`: ask the thread's running task to stop.
pub(super) async fn stop(output: &Output, backend: &mut Backend, id: &str) -> u8 {
    let Some(task) = bound(output, backend, id).await else {
        return EXIT_FAILURE;
    };
    let run = runner(backend, id);
    let stopping = task.clone();
    let result = tokio::task::spawn_blocking(move || run.stop(&stopping))
        .await
        .unwrap_or_else(|_| Err("Coder could not be reached.".into()));
    let (stopped, message) = match result {
        Ok(()) => (
            true,
            format!("Asked Coder to stop task {task}. Its turn ends as stopped."),
        ),
        Err(why) => (false, why),
    };
    event(
        output,
        json!({"event": "stop", "thread": id, "task": task, "requested": stopped, "message": message}),
    );
    if !output.json() {
        eprintln!("{message}");
    }
    if stopped { 0 } else { EXIT_FAILURE }
}

/// `chat answer`: answer the thread's task's question, and follow the turn
/// the answer starts.
pub(super) async fn answer(output: &Output, backend: &mut Backend, id: &str, text: &str) -> u8 {
    let Some(task) = bound(output, backend, id).await else {
        return EXIT_FAILURE;
    };
    let run = runner(backend, id);
    let answering = task.clone();
    let text = text.to_owned();
    let result = tokio::task::spawn_blocking(move || run.answer(&answering, &text))
        .await
        .unwrap_or_else(|_| Err("Coder could not be reached.".into()));
    match result {
        Ok(record) => {
            let turn = record.turns.last().map_or(1, |start| start.turn);
            follow_task(output, backend, id, &task, turn).await
        }
        Err(message) => {
            event(
                output,
                json!({"event": "failure", "thread": id, "message": message, "stopped": false}),
            );
            eprintln!("openagents chat: {message}");
            EXIT_FAILURE
        }
    }
}

/// Stream `task`'s events from `turn` on until it ends or asks. Ctrl-C
/// stops following, not the task.
async fn follow_task(output: &Output, backend: &Backend, id: &str, task: &str, turn: usize) -> u8 {
    let mut follow: Follow =
        runner(backend, id).follow(task, Some(id), Some(answer_hint(backend, id)));
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let mut last: Option<CoderEvent> = None;
    loop {
        let polled = tokio::task::spawn_blocking(move || {
            let result = follow.poll();
            (follow, result)
        })
        .await;
        let Ok((back, result)) = polled else {
            eprintln!("openagents chat: the task could not be read");
            return EXIT_FAILURE;
        };
        follow = back;
        let (lines, state) = match result {
            Ok(polled) => polled,
            Err(why) => {
                event(
                    output,
                    json!({"event": "failure", "thread": id, "task": task, "message": why, "stopped": false}),
                );
                eprintln!("openagents chat: cannot read task {task}: {why}");
                return EXIT_FAILURE;
            }
        };
        for line in lines {
            if turn_of(&line.event) < turn {
                continue;
            }
            show(output, &line);
            if line.event.ends_turn() {
                last = Some(line.event.clone());
            }
        }
        if state != State::Running {
            break;
        }
        tokio::select! {
            () = tokio::time::sleep(POLL) => {}
            _ = &mut interrupt => {
                eprintln!(
                    "Stopped following. Coder keeps working: follow it with `openagents chat follow{} --thread {id}`, or stop it with `openagents chat stop{} --thread {id}`.",
                    flag(backend), flag(backend)
                );
                return EXIT_FAILURE;
            }
        }
    }
    match last {
        Some(CoderEvent::Result(_) | CoderEvent::Question(_) | CoderEvent::Approval(_)) => 0,
        _ => EXIT_FAILURE,
    }
}

fn turn_of(event: &CoderEvent) -> usize {
    match event {
        CoderEvent::CoderStarted(e) => e.turn,
        CoderEvent::Step(e) => e.turn,
        CoderEvent::Output(e) => e.turn,
        CoderEvent::ProviderSwitched(e) => e.turn,
        CoderEvent::Question(e) | CoderEvent::Approval(e) => e.turn,
        CoderEvent::Progress(e) => e.turn,
        CoderEvent::Result(e) => e.turn,
        CoderEvent::Failure(e) => e.turn,
        CoderEvent::Stopped(e) => e.turn,
    }
}

/// One event: an NDJSON line, or the live view on stderr and the result on
/// stdout.
fn show(output: &Output, line: &Line) {
    if output.json() {
        if let Ok(text) = serde_json::to_string(line) {
            println!("{text}");
            let _ = std::io::stdout().flush();
        }
        return;
    }
    match &line.event {
        CoderEvent::Result(result) => {
            if let Some(text) = coder_events::text(&line.event) {
                eprintln!("{text}");
            }
            println!("{}", result.summary.trim());
            for file in &result.files_changed {
                println!(
                    "  {} {} (+{} -{})",
                    file.status,
                    file.path,
                    file.added.map_or("?".into(), |n| n.to_string()),
                    file.removed.map_or("?".into(), |n| n.to_string())
                );
            }
            println!("worktree: {}", result.worktree);
            let _ = std::io::stdout().flush();
        }
        CoderEvent::Question(asked) | CoderEvent::Approval(asked) => {
            println!("{}", asked.text.trim());
            if let Some(how) = &asked.answer {
                eprintln!("answer: {how}");
            }
        }
        CoderEvent::Step(step) if step.kind == StepKind::Reply => {}
        event => {
            if let Some(text) = coder_events::text(event) {
                eprintln!("{text}");
            }
        }
    }
}
