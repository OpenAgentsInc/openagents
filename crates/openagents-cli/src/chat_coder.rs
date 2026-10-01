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
/// `engine` is the engine the person asked for, from the reply's typed
/// offer (#10076).
pub(super) fn predict(
    backend: &Backend,
    thread: &str,
    engine: Option<nostr::cj_conversation::Engine>,
) -> Option<openagents_chat::coder_events::Runner> {
    runner(backend, thread).predict(engine.map(coder::task::settings::provider_of))
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

/// What a turn from this command tells the chat worker about this computer
/// (#10077): the terminal surface, whether a Coder run could start
/// ([`ready`]), that this computer is where Coder runs with its coding
/// agents' readiness, and the project folder: the checkout this command
/// runs in. `--no-run` says the same: it only keeps this command from
/// starting the run, so the offer it prints, with the engine the person
/// asked for, is the one a run would take (#10076). Reads only.
pub(super) fn context(store: PathBuf) -> openagents_chat::router::Context {
    use openagents_chat::router::{Computer, Context, Engine, Project, Surface};
    let engines = Local::here(store.clone())
        .predict(None)
        .map(|runner| Engine::from_runner(&runner))
        .unwrap_or_default();
    let project = std::env::current_dir()
        .ok()
        .and_then(|dir| local::checkout(&dir).ok())
        .and_then(|checkout| Project::at(&checkout.top.display().to_string()));
    Context {
        surface: Surface::Terminal,
        computer_ready: ready(store),
        computer: Some(Computer::Here {
            name: None,
            engines,
        }),
        project,
        ..Context::default()
    }
}

/// Start Coder on this computer for the thread `id`, in the checkout the
/// command runs in, bind the task to the thread, and follow it.
/// What `chat run-coder` starts Coder with: the shared handoff prompt
/// ([`openagents_chat::delegation::prompt`]), which tells the engine the
/// routing is done (#10084), and the engine the person asked for, from the
/// reply's typed offer: it goes first, and the start card says why when
/// another runs (#10076).
fn handoff(
    title: &str,
    turns: &[openagents_chat::basic_coder::Turn],
) -> (String, Option<coder::task::capacity::Provider>) {
    (
        openagents_chat::delegation::prompt(title, turns),
        openagents_chat::delegation::requested(turns).map(coder::task::settings::provider_of),
    )
}

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
        // The thread's run ended, and the router judged the latest message
        // more work for it: Coder takes it as the task's next turn, in the
        // same worktree (#10094).
        if coder.host == local::LOCAL_HOST
            && result(backend, id, &coder.task).await.is_some()
            && let Some(text) = thread
                .turns
                .iter()
                .rev()
                .find(|turn| turn.role == openagents_chat::basic_coder::Role::User)
                .map(|turn| turn.text.clone())
        {
            report(
                true,
                &format!("Coder continues task {} with your message.", coder.task),
                serde_json::to_value(coder).ok(),
            );
            return answer_task(output, backend, id, &coder.task, &text).await;
        }
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
    let (prompt, requested) = handoff(&thread.summary.title, &thread.turns);
    let here = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            report(false, "This command has no working directory.", None);
            return EXIT_FAILURE;
        }
    };
    // The router judged this is coding work; Jev now judges whether it
    // asks to work a GitHub issue, choosing among the references the
    // messages name (a bounded field read only after routing).
    let (request, earlier) = asked_of(&thread.turns);
    let dir = here.clone();
    let reference = tokio::task::spawn_blocking(move || {
        coder::task::issue_run::asked_blocking(&request, &earlier, &dir)
    })
    .await
    .ok()
    .flatten();
    if let Some(reference) = reference {
        return start_issue(output, backend, id, &here, reference).await;
    }
    let run = runner(backend, id);
    let title = thread.summary.title.clone();
    let chat = id.to_owned();
    let started = tokio::task::spawn_blocking(move || {
        run.start_requested(&here, &title, &prompt, Some(&chat), &[], requested)
    })
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

/// The thread's last message, and the conversation before it, for Jev's
/// choice of issue.
fn asked_of(turns: &[openagents_chat::basic_coder::Turn]) -> (String, String) {
    use openagents_chat::basic_coder::Role;
    let last = turns.iter().rposition(|turn| turn.role == Role::User);
    let Some(last) = last else {
        return (String::new(), String::new());
    };
    let earlier = turns[..last]
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|turn| {
            let who = if turn.role == Role::User {
                "user"
            } else {
                "assistant"
            };
            format!("{who}: {}", turn.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    (turns[last].text.clone(), earlier)
}

/// Start the issue flow for `reference` on this computer for the thread
/// `id`: claim, a worktree of the fetched default branch, the checks, and
/// landing as the repository's policy says, all streamed as the thread's
/// Coder events. The flow runs on a thread of this process until it ends.
async fn start_issue(
    output: &Output,
    backend: &mut Backend,
    id: &str,
    here: &std::path::Path,
    reference: coder::task::issue_run::Reference,
) -> u8 {
    let report = |accepted: bool, message: &str, task: Option<serde_json::Value>| {
        event(
            output,
            json!({"event": "coder", "thread": id, "accepted": accepted, "message": message, "task": task}),
        );
        if !output.json() {
            eprintln!("{message}");
        }
    };
    let store = store(backend, id);
    let (sender, receiver) = std::sync::mpsc::channel();
    let dir = here.to_path_buf();
    let chat = id.to_owned();
    let number = reference.number;
    let flow = std::thread::spawn(move || {
        let runner = coder::task::issue_run::Runner::new(store);
        match runner.begin(&dir, &reference, Some(&chat)) {
            Ok(started) => {
                let _ = sender.send(Ok((started.record.clone(), started.issue.url.clone())));
                Some(started.finish())
            }
            Err(refused) => {
                let _ = sender.send(Err(refused.to_string()));
                None
            }
        }
    });
    let begun = tokio::task::spawn_blocking(move || receiver.recv())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_else(|| Err("Coder could not start the issue flow.".into()));
    let (record, url) = match begun {
        Ok(begun) => begun,
        Err(message) => {
            let _ = flow.join();
            report(
                false,
                &format!("Coder did not take #{number}: {message}"),
                None,
            );
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
            "openagents chat: Coder started, but the thread could not record its task ({why})."
        );
    }
    report(
        true,
        &format!(
            "Coder took issue #{number} ({url}) as task {} in a worktree of {}.",
            record.task, record.project
        ),
        Some(json!({
            "host": local::LOCAL_HOST,
            "task": record.task,
            "project": record.project,
            "worktree": record.worktree,
            "issue": number,
            "issue_url": url,
        })),
    );
    let code = follow_flow(output, backend, id, &record.task).await;
    let _ = tokio::task::spawn_blocking(move || flow.join()).await;
    code
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
    answer_task(output, backend, id, &task, text).await
}

/// What the thread's task's last turn did, once it ended (#10094), read
/// from the thread's store.
pub(super) async fn result(
    backend: &Backend,
    thread: &str,
    task: &str,
) -> Option<openagents_chat::router::CoderRun> {
    let store = store(backend, thread);
    let task = task.to_owned();
    tokio::task::spawn_blocking(move || local::result_in(Some(&store), &task))
        .await
        .ok()
        .flatten()
}

/// Continue `task` with `text`, an answer or the next turn, and follow the
/// turn it starts.
async fn answer_task(output: &Output, backend: &Backend, id: &str, task: &str, text: &str) -> u8 {
    let task = task.to_owned();
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
    follow_from(output, backend, id, task, turn, false).await
}

/// Stream an issue flow's task to its end. Ctrl-C asks the flow to stop:
/// it stops the running turn, or stops before landing, and says so on the
/// issue; the stream then shows how it ended.
async fn follow_flow(output: &Output, backend: &Backend, id: &str, task: &str) -> u8 {
    follow_from(output, backend, id, task, 1, true).await
}

async fn follow_from(
    output: &Output,
    backend: &Backend,
    id: &str,
    task: &str,
    turn: usize,
    flow: bool,
) -> u8 {
    let mut stopping = false;
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
            _ = &mut interrupt, if !stopping => {
                if flow {
                    stopping = true;
                    let run = runner(backend, id);
                    let stopped = task.to_owned();
                    let asked = tokio::task::spawn_blocking(move || run.stop(&stopped)).await;
                    eprintln!(
                        "Stopping: Coder ends the issue flow at its next step and says so on the \
                         issue.{}",
                        match asked {
                            Ok(Err(why)) => format!(" ({why})"),
                            _ => String::new(),
                        }
                    );
                    continue;
                }
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
            if let Some(issue) = &result.issue {
                println!("{}", issue.line());
            }
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

#[cfg(test)]
mod tests {
    use openagents_chat::basic_coder::Turn;
    use openagents_chat::router::{Meta, Offer};

    /// The CLI's run starts on the shared handoff (#10084): the engine is
    /// told the person's request for Claude Code is done and its job is
    /// the task, and the run asks for Claude Code.
    #[test]
    fn the_cli_handoff_tells_the_engine_the_routing_is_done() {
        let turns = vec![
            Turn::user("do a test delegation to claude"),
            Turn::assistant(
                "We'll dispatch Coder, asking for Claude Code, to take this on.",
                Some(Meta {
                    offers: vec![Offer::RunCoder],
                    engine: Some(nostr::cj_conversation::Engine::ClaudeCode),
                    ..Meta::default()
                }),
            ),
        ];
        let (prompt, requested) = super::handoff("Chat", &turns);
        assert_eq!(requested, Some(coder::task::capacity::Provider::Claude));
        assert!(
            prompt.starts_with("do a test delegation to claude\n\n"),
            "{prompt}"
        );
        for needle in [
            "The person asked for this to run on Claude Code.",
            "never start another coding engine's command line",
            "a small, harmless check of this project",
        ] {
            assert!(prompt.contains(needle), "{needle:?} missing from {prompt}");
        }
    }
}
