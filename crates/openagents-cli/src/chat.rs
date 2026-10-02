//! `openagents chat`: talk to OpenAgents, the chat router, from a terminal.
//!
//! This is a thin front over the shared chat client
//! ([`openagents_chat::client`]), which OpenAgents Terminal uses too: every
//! operation is a service [`Command`] and every answer a typed client
//! [`Event`], printed here as text or `--json` NDJSON. There is no chat
//! logic here. When this computer's host runs, the commands go to its
//! control socket, so the threads are the desktop app's threads and a Coder
//! offer is accepted through the host's own handoff. Otherwise the service
//! runs in this process, with this command's own device key and encrypted
//! store. The worker's router decides every route; this command only shows
//! what it said. Coder runs on this computer through
//! [`coder::task::chat_client::Here`].
//!
//! The user-facing unit is the **thread** (`docs/glossary.md`); the shared
//! code calls it a chat. [`openagents_chat::thread`] renders a thread as its
//! ATIF trajectory.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::chat_client::{Control, Here};
use openagents_chat::basic_coder::Turn;
use openagents_chat::client::{self, Client, Ended, Event, Kind, Op, Place, Start};
use openagents_chat::router::{Caller, Meta, Offer};
use openagents_connect::control;
use serde_json::{Value, json};

use crate::out::{Output, table};
use crate::{Args, EXIT_FAILURE, runtime};

#[path = "chat_coder.rs"]
mod coder_run;
#[path = "chat_work.rs"]
mod work;

pub(crate) const USAGE: &str = "usage: openagents chat COMMAND [OPTIONS]
  send MESSAGE [--thread ID] [--no-run] [--timeout SECONDS]
        Send one message to OpenAgents and stream its reply. `openagents chat
        MESSAGE` is the same, and MESSAGE `-` reads it from stdin. Without
        --thread a new thread starts. When OpenAgents judges that the
        message is coding work, Coder runs on this computer at once, in its
        own worktree of the Git checkout this command runs in, with Codex,
        Claude Code, or Grok Build, the first signed in here with capacity,
        and its events stream here. --no-run only shows the offer instead.
        `openagents settings` chooses the providers, whether Coder asks
        first, the project folders, and what its commands may reach.
  follow --thread ID
        Replay the thread's Coder task from its first event and keep
        streaming until it ends. Ctrl-C stops following, not the task.
  stop --thread ID
        Stop the thread's running Coder task.
  answer --thread ID TEXT
        Answer the question Coder asked, and follow the turn it starts.
  threads [--all] [--limit N]
        List threads, newest first; --all includes archived threads.
  read --thread ID
        Print a thread's turns.
  export --thread ID
        Print the thread as an ATIF-v1.8 trajectory whose session_id is ID.
  run-coder --thread ID
        Run Coder on this computer for the thread's last offer, as send does.
        When the message asks Coder to work a GitHub issue of this
        checkout's repository (\"work on #10034\"), Coder runs the issue
        flow: it claims the issue, works in a worktree of origin/main, runs
        the checks for what it touched, lands as the repository's
        .openagents/coder-issues.json says (this repository: rebase and push
        main when the checks pass; others: a pull request), comments the
        evidence, and closes the issue. Ctrl-C stops the flow.
  run-command --thread ID
        Run the openagents command the thread's last reply proposed, on
        this computer. send runs a command that only reads at once, as this
        build's command tree declares it; one that changes something here
        waits for this; one that moves money or shows a secret never runs
        from the chat.
  work --issues NUMBERS|LABEL [--parallel N] [--land main|pr]
        Hand several issues to Coder, one issue flow each, each in its own
        thread: NUMBERS such as 10050,10051, or a LABEL's open issues.
        --parallel (1 to 4, default 1) runs that many at once, each in its
        own worktree. Issues claimed in the last hours (the repository's
        claim window) and closed issues are skipped. --land overrides the
        repository's policy.
Every command also takes --scratch, --local, and --socket PATH. When this
computer's host runs (the OpenAgents app, or `openagents host serve
--control`), threads live in the host and the desktop app shows them;
--socket names another control socket and --local skips the host. Without a
host, threads live in ~/.openagents/chat/ under this command's own device
key (OPENAGENTS_CHAT_HOME overrides the directory); once a host runs here,
they move into it, keeping their IDs (docs/cli/chat.md). Coder tasks live in
~/.openagents/tasks (OPENAGENTS_TASKS overrides it), the store this
computer's host serves. --scratch uses a throwaway identity, thread store,
and task store in the system temporary directory; continue that thread with
--scratch --thread ID. --timeout (default 120) and Ctrl-C stop receiving a
reply; the hosted worker may still finish it. Every run prints the thread ID
it used. Under --json, send prints NDJSON events: accepted, partial, route,
offer, result, coder, and failure, then the Coder task's events
(coder_started, step, output, provider_switched, progress, question,
approval, result, failure, stopped; docs/cli/chat.md). No model key is
needed, and nothing prints a key.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("send", Effect::Publishes),
    Declared::computer("threads", Effect::ReadOnly),
    Declared::computer("read", Effect::ReadOnly),
    Declared::computer("export", Effect::ReadOnly),
    Declared::computer("run-coder", Effect::Publishes),
    Declared::computer("run-command", Effect::LocalWrite),
    Declared::computer("follow", Effect::ReadOnly),
    Declared::computer("stop", Effect::Publishes),
    Declared::computer("answer", Effect::Publishes),
    Declared::computer("work", Effect::Publishes),
];

const OPTIONS: &[&str] = &[
    "thread", "timeout", "limit", "socket", "issues", "parallel", "land",
];
const SWITCHES: &[&str] = &["scratch", "local", "all", "run-coder", "no-run"];

pub(crate) enum Failure {
    Usage(String),
    Failed(String),
}

fn failed(message: impl Into<String>) -> Failure {
    Failure::Failed(message.into())
}

impl From<client::Error> for Failure {
    fn from(error: client::Error) -> Self {
        match error {
            client::Error::Usage(message) => Self::Usage(message),
            client::Error::Failed(message) => Self::Failed(message),
        }
    }
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(first) = words.first() else {
        return output.usage("chat", "a message or a command is required", USAGE);
    };
    let (command, rest) = match first.as_str() {
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            return 0;
        }
        "send" | "threads" | "read" | "export" | "run-coder" | "run-command" | "follow"
        | "stop" | "answer" | "work" => (first.as_str(), &words[1..]),
        // `openagents chat MESSAGE` is `openagents chat send MESSAGE`.
        _ => ("send", words),
    };
    let args = match Args::parse(rest, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("chat", &message, USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !OPTIONS.contains(name))
    {
        return output.usage("chat", &format!("unknown option `--{name}`"), USAGE);
    }
    let result = runtime().block_on(dispatch(output, command, &args));
    match result {
        Ok(code) => code,
        Err(Failure::Usage(message)) => output.usage("chat", &message, USAGE),
        Err(Failure::Failed(message)) => output.fail("chat", &message),
    }
}

async fn dispatch(output: &Output, command: &str, args: &Args) -> Result<u8, Failure> {
    let thread = args.option("thread").map(str::to_owned);
    if let Some(id) = &thread
        && !client::thread_id(id)
    {
        return Err(Failure::Usage(
            "a thread ID is 32 lowercase hex characters".into(),
        ));
    }
    let needs_thread = |thread: &Option<String>| {
        thread
            .clone()
            .ok_or_else(|| Failure::Usage(format!("`chat {command}` needs --thread ID")))
    };
    let mut printer = Printer::new(output);
    match command {
        "work" => {
            no_positional(args)?;
            work::work(output, args).await
        }
        "send" => {
            let message = message(args.positional())?;
            let timeout = match args.number("timeout", client::DEFAULT_TIMEOUT.as_secs()) {
                Ok(timeout) if timeout > 0 => Duration::from_secs(timeout),
                _ => {
                    return Err(Failure::Usage(
                        "--timeout is a positive number of seconds".into(),
                    ));
                }
            };
            let (id, new) = match thread {
                Some(id) => (id, false),
                None => (client::new_id(), true),
            };
            let mut client = open(args, Some(&id), new, &mut printer).await?;
            // `--no-run`, or `coder.start: ask_first` in the settings, keeps
            // only the offer; `--run-coder` runs.
            let start = if args.switch("run-coder") {
                Start::Now
            } else if args.switch("no-run") {
                Start::OfferOnly
            } else {
                Start::Settings
            };
            let op = Op::Send {
                thread: id.clone(),
                new,
                text: message,
                start,
                timeout,
            };
            let ended = client.run(op, &mut |event| printer.print(event)).await?;
            if ended == Ended::Refused {
                eprintln!("thread {id}");
                return Ok(EXIT_FAILURE);
            }
            if !output.json() {
                eprintln!("thread {id}");
            }
            Ok(code(ended))
        }
        "threads" => {
            no_positional(args)?;
            if args.switch("scratch") {
                return Err(Failure::Usage(
                    "each --scratch thread has its own store; read one with --scratch --thread ID"
                        .into(),
                ));
            }
            let limit: usize = args.number("limit", 50).map_err(Failure::Usage)?;
            let mut client = open(args, None, false, &mut printer).await?;
            threads(output, &mut client, args.switch("all"), limit).await
        }
        "read" | "export" => {
            no_positional(args)?;
            let id = needs_thread(&thread)?;
            let mut client = open(args, Some(&id), false, &mut printer).await?;
            let whole = client.collect(&id).await?;
            if command == "read" {
                read(output, &client, &whole);
            } else {
                let tasks = client.trajectories(&id, &whole);
                let document =
                    openagents_chat::thread::trajectory_with(&whole, &crate::version_line(), tasks);
                // Only a document `crates/atif` reads back leaves this command.
                if let Some(problem) = atif::validate(&document).into_iter().next() {
                    return Err(failed(format!(
                        "the trajectory is not valid ATIF: {problem}"
                    )));
                }
                output.emit(&document, |document| {
                    serde_json::to_string_pretty(document).unwrap_or_default()
                });
                eprintln!("thread {id}");
            }
            Ok(0)
        }
        "run-coder" | "run-command" | "follow" | "stop" | "answer" => {
            if command != "answer" {
                no_positional(args)?;
            }
            let id = needs_thread(&thread)?;
            let op = match command {
                "answer" => Op::Answer {
                    thread: id.clone(),
                    text: message(args.positional())?,
                },
                "run-coder" => Op::RunCoder { thread: id.clone() },
                "run-command" => Op::RunCommand { thread: id.clone() },
                "follow" => Op::Follow { thread: id.clone() },
                _ => Op::Stop { thread: id.clone() },
            };
            let mut client = open(args, Some(&id), false, &mut printer).await?;
            let ended = client.run(op, &mut |event| printer.print(event)).await?;
            if !output.json() {
                eprintln!("thread {id}");
            }
            Ok(code(ended))
        }
        _ => Err(Failure::Usage(format!("unknown command `{command}`"))),
    }
}

/// The exit code an operation ends with.
fn code(ended: Ended) -> u8 {
    match ended {
        Ended::Done => 0,
        Ended::Failed | Ended::Refused => EXIT_FAILURE,
    }
}

/// The client `args` choose: `--scratch`, `--local`, or this computer's
/// host when one answers (`--socket` names another). `thread` names the
/// scratch store; `new` creates one.
pub(crate) async fn open(
    args: &Args,
    thread: Option<&str>,
    new: bool,
    printer: &mut Printer<'_>,
) -> Result<Client, Failure> {
    let place = if args.switch("scratch") {
        Place::Scratch
    } else if args.switch("local") {
        Place::Local
    } else {
        Place::Auto {
            socket: args.option("socket").map(PathBuf::from),
        }
    };
    let mut options = client::Options::new(Caller::CLI);
    options.place = place;
    options.interrupt = Arc::new(|| {
        Box::pin(async {
            let _ = tokio::signal::ctrl_c().await;
        })
    });
    options.hint = Some(coder_run::answer_hint);
    let client = Client::open(
        options,
        &Control,
        Arc::new(Here),
        thread,
        new,
        &mut |event| printer.print(event),
    )
    .await?;
    printer.kind = client.kind();
    Ok(client)
}

fn no_positional(args: &Args) -> Result<(), Failure> {
    match args.positional().first() {
        None => Ok(()),
        Some(word) => Err(Failure::Usage(format!("unexpected argument `{word}`"))),
    }
}

/// The message: the words given, or stdin for `-`.
fn message(words: &[String]) -> Result<String, Failure> {
    let text = if words.len() == 1 && words[0] == "-" {
        let mut text = String::new();
        std::io::stdin()
            .take(64 * 1024)
            .read_to_string(&mut text)
            .map_err(|error| failed(format!("cannot read the message from stdin: {error}")))?;
        text
    } else {
        words.join(" ")
    };
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(Failure::Usage("the message is empty".into()));
    }
    if text.len() > 32 * 1024 {
        return Err(Failure::Usage("the message is longer than 32 KiB".into()));
    }
    Ok(text)
}

/// What `doctor` says about chat: where threads would live, and this
/// command's public identity when it has one. Reads only.
pub fn doctor() -> Value {
    let socket = control::socket_path();
    let running = socket
        .as_ref()
        .is_some_and(|socket| crate::host_answers_at(socket));
    let home = client::home();
    json!({
        "backend": if running { "host" } else { "in_process" },
        "host_socket": socket.map(|socket| socket.display().to_string()),
        "host_running": running,
        "home": home.display().to_string(),
        "store_exists": home.join("threads").exists(),
        "identity": client::identity(&home),
    })
}

/// One `--json` event line, or nothing in text mode.
pub(crate) fn event(output: &Output, value: Value) {
    if output.json() {
        println!("{value}");
        let _ = std::io::stdout().flush();
    }
}

/// Prints a client's events: text on stdout and stderr, or NDJSON under
/// `--json` (`docs/cli/chat.md`).
pub(crate) struct Printer<'a> {
    output: &'a Output,
    /// The backend, once open: it names the switch that reaches a thread
    /// again.
    kind: Kind,
    /// The reply text printed so far.
    printed: String,
    /// The finished reply replaced the preview.
    diverged: bool,
    /// The Coder run's tool calls not yet final (#10117).
    tools: openagents_chat::tool_groups::Stream,
}

impl<'a> Printer<'a> {
    pub(crate) fn new(output: &'a Output) -> Self {
        Self {
            output,
            kind: Kind::InProcess,
            printed: String::new(),
            diverged: false,
            tools: openagents_chat::tool_groups::Stream::default(),
        }
    }

    pub(crate) fn print(&mut self, happened: Event) {
        let output = self.output;
        match happened {
            Event::Migrated { moved } => eprintln!(
                "moved {moved} thread{} kept without a host into this computer's host",
                if moved == 1 { "" } else { "s" }
            ),
            Event::Kept { home, message } => eprintln!(
                "threads in {} stay there for now: {message}",
                home.display()
            ),
            Event::Accepted {
                thread,
                request,
                new,
                backend,
                at,
            } => event(
                output,
                json!({
                    "event": "accepted",
                    "thread": thread,
                    "request": request,
                    "new": new,
                    "backend": backend.word(),
                    "at": at,
                }),
            ),
            Event::Partial {
                thread,
                text,
                delta,
            } => {
                event(
                    output,
                    json!({"event": "partial", "thread": thread, "text": text, "delta": delta}),
                );
                if !output.json() && !self.diverged {
                    match text.strip_prefix(self.printed.as_str()) {
                        Some(more) => {
                            print!("{more}");
                            let _ = std::io::stdout().flush();
                            self.printed = text;
                        }
                        None => self.diverged = true,
                    }
                }
            }
            Event::Reply {
                thread,
                reply,
                computer,
                running,
            } => finish(
                output,
                &thread,
                &reply,
                computer,
                &mut self.printed,
                self.diverged,
                running,
            ),
            Event::ReplyFailed {
                thread,
                message,
                stopped,
                partial,
            } => {
                if let Some(text) = &partial
                    && !output.json()
                    && text.starts_with(self.printed.as_str())
                {
                    print!("{}", &text[self.printed.len()..]);
                }
                if !output.json() && !self.printed.is_empty() {
                    println!();
                }
                event(
                    output,
                    json!({
                        "event": "failure",
                        "thread": thread,
                        "message": message,
                        "stopped": stopped,
                        "partial": partial,
                    }),
                );
                eprintln!("openagents chat: {message}");
            }
            Event::Failure { thread, message } => {
                event(
                    output,
                    json!({"event": "failure", "thread": thread, "message": message, "stopped": false}),
                );
                eprintln!("openagents chat: {message}");
            }
            Event::Starting { thread, engine } => {
                let text = openagents_chat::coder_events::starting(&engine);
                event(
                    output,
                    json!({"event": "starting", "thread": thread, "engine": engine, "text": text}),
                );
                if !output.json() {
                    eprintln!("{text}");
                }
            }
            Event::Coder {
                thread,
                accepted,
                message,
                task,
                quiet,
            } => {
                event(
                    output,
                    json!({"event": "coder", "thread": thread, "accepted": accepted, "message": message, "task": task}),
                );
                // A start's own line (`coder_started`) says who works.
                if !output.json() && !quiet {
                    eprintln!("{message}");
                }
            }
            Event::Unbound {
                thread,
                why,
                issue: false,
            } => eprintln!(
                "openagents chat: Coder started, but the thread could not record its task ({why}); \
                 follow it with `openagents chat follow --thread {thread}` from this store."
            ),
            Event::Unbound { why, .. } => eprintln!(
                "openagents chat: Coder started, but the thread could not record its task ({why})."
            ),
            Event::Line(line) => coder_run::show(output, &mut self.tools, &line),
            Event::TaskUnreadable {
                thread,
                task,
                message,
            } => {
                event(
                    output,
                    json!({"event": "failure", "thread": thread, "task": task, "message": message, "stopped": false}),
                );
                eprintln!("openagents chat: cannot read task {task}: {message}");
            }
            Event::Lost => eprintln!("openagents chat: the task could not be read"),
            Event::Stop {
                thread,
                task,
                requested,
                message,
            } => {
                event(
                    output,
                    json!({"event": "stop", "thread": thread, "task": task, "requested": requested, "message": message}),
                );
                if !output.json() {
                    eprintln!("{message}");
                }
            }
            Event::Stopping { why } => eprintln!(
                "Stopping: Coder ends the issue flow at its next step and says so on the \
                 issue.{}",
                why.map(|why| format!(" ({why})")).unwrap_or_default()
            ),
            Event::Offline { thread, retry_in } => {
                event(
                    output,
                    json!({"event": "offline", "thread": thread, "retry_in": retry_in}),
                );
                if !output.json() {
                    eprintln!(
                        "OpenAgents cannot be reached; trying again in {retry_in}s. Ctrl-C stops."
                    );
                }
            }
            Event::Online { thread } => {
                event(output, json!({"event": "online", "thread": thread}));
                if !output.json() {
                    eprintln!("Connected again.");
                }
            }
            Event::Command {
                thread,
                argv,
                confirm,
            } => {
                let line = Offer::command_line(&argv);
                event(
                    output,
                    json!({"event": "command", "thread": thread, "argv": argv, "confirm": confirm}),
                );
                if !output.json() {
                    if confirm {
                        let flag = coder_run::flag(self.kind);
                        eprintln!(
                            "offer: {line} changes something on this computer; run it with `openagents chat run-command{flag} --thread {thread}`"
                        );
                    } else {
                        eprintln!("running: {line}");
                    }
                }
            }
            Event::Ran {
                thread,
                argv,
                ok,
                output: printed,
            } => {
                event(
                    output,
                    json!({"event": "ran", "thread": thread, "argv": argv, "ok": ok, "output": printed}),
                );
                if !output.json() {
                    println!("\n{printed}");
                    if !ok {
                        eprintln!("{} failed", Offer::command_line(&argv));
                    }
                }
            }
            Event::Detached { thread } => {
                let flag = coder_run::flag(self.kind);
                eprintln!(
                    "Stopped following. Coder keeps working: follow it with `openagents chat follow{flag} --thread {thread}`, or stop it with `openagents chat stop{flag} --thread {thread}`."
                );
            }
        }
    }
}

/// Print the finished reply and what the router said beside it.
fn finish(
    output: &Output,
    id: &str,
    reply: &Turn,
    computer: bool,
    printed: &mut String,
    diverged: bool,
    running: bool,
) {
    let meta = reply.meta.clone().unwrap_or_default();
    let judgment: Value = meta
        .judgment
        .as_deref()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null);
    event(
        output,
        json!({
            "event": "route",
            "thread": id,
            "tier": meta.tier,
            "route": meta.route,
            "bank": meta.bank,
            "served_answer": meta.answer,
            "judgment": judgment,
            "computer": computer,
            "followups": meta.followups,
            "cards": meta.cards,
            // Who would run Coder on this computer for this reply.
            "runner": meta.runner,
            "runner_text": meta.runner.as_ref().map(|runner| runner.text()),
            // The engine the person asked for, from the typed offer.
            "engine": meta.engine,
        }),
    );
    for offer in &meta.offers {
        let mut line =
            json!({"event": "offer", "thread": id, "offer": offer, "accept": accept(id, offer)});
        // Who will run Coder on this computer, typed and in words.
        if *offer == Offer::RunCoder
            && let Some(runner) = &meta.runner
        {
            line["runner"] = json!(runner);
            line["runner_text"] = json!(runner.text());
        }
        // The engine the person asked for, as the worker's typed offer
        // named it (#10076).
        if *offer == Offer::RunCoder
            && let Some(engine) = meta.engine
        {
            line["engine"] = json!(engine);
        }
        event(output, line);
    }
    event(
        output,
        json!({
            "event": "result",
            "thread": id,
            "text": reply.text,
            "model": reply.model,
            "served_answer": meta.answer,
        }),
    );
    if output.json() {
        return;
    }
    if !diverged && let Some(rest) = reply.text.strip_prefix(printed.as_str()) {
        print!("{rest}");
    } else {
        // The finished reply replaced the preview; show it whole.
        println!("\n");
        print!("{}", reply.text);
    }
    println!();
    printed.clone_from(&reply.text);
    // The offer is what `run-coder` accepts, read the same way (#10170).
    let offered = openagents_chat::delegation::offered(reply.meta.as_ref(), computer);
    notes(id, &meta, offered && !running, running);
}

/// The command that accepts `offer`, when this command can.
fn accept(id: &str, offer: &Offer) -> Option<String> {
    match offer {
        Offer::RunCoder => Some(format!("openagents chat run-coder --thread {id}")),
        Offer::Cli { argv, .. } => Some(Offer::command_line(argv)),
        _ => None,
    }
}

/// The router's observations, on stderr so stdout stays the reply.
fn notes(id: &str, meta: &Meta, computer: bool, running: bool) {
    if let Some(answer) = &meta.answer {
        eprintln!("answered from product knowledge: {answer}");
    }
    let mut coder = computer;
    for offer in &meta.offers {
        match offer {
            Offer::RunCoder => coder = !running,
            Offer::OpenScreen { screen } => {
                eprintln!("offer: open {screen:?} in the OpenAgents app");
            }
            // The client runs or offers the reply's command itself.
            Offer::Cli { argv, .. } if meta.command.is_none() => {
                eprintln!("offer: {}", Offer::command_line(argv))
            }
            Offer::Cli { .. } => {}
            Offer::StartEval { .. } => {
                eprintln!("offer: run this test set from the OpenAgents app")
            }
            Offer::PublishEval { .. } => {
                eprintln!("offer: add this result to the Gym from the OpenAgents app")
            }
            Offer::OpenPresentation { .. } => {
                eprintln!("{}", openagents_chat::router::PRESENTATION_ELSEWHERE)
            }
        }
    }
    if coder {
        eprintln!(
            "offer: run Coder on this computer for this thread: openagents chat run-coder --thread {id}"
        );
        if let Some(engine) = meta.engine {
            eprintln!("asked for: {}", engine.name());
        }
    }
    if let Some(runner) = &meta.runner {
        eprintln!("coder: {}", runner.text());
    }
    for followup in &meta.followups {
        eprintln!("suggestion: {}", followup.label);
    }
}

async fn threads(
    output: &Output,
    client: &mut Client,
    all: bool,
    limit: usize,
) -> Result<u8, Failure> {
    let (rows, total) = client.threads(all, limit).await?;
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "thread": row.id,
                "title": row.title,
                "started": row.started,
                "updated": row.updated,
                "pinned": row.pinned,
                "archived": row.archived,
                "coder": row.coder,
            })
        })
        .collect();
    output.emit(
        &json!({
            "backend": client.kind().word(),
            "at": client.place(),
            "total": total,
            "threads": rows,
        }),
        |value| {
            let mut table_rows = vec![vec!["THREAD".into(), "UPDATED".into(), "TITLE".into()]];
            for row in value["threads"].as_array().into_iter().flatten() {
                let mut title = row["title"].as_str().unwrap_or("").to_owned();
                if row["archived"].as_bool() == Some(true) {
                    title.push_str(" (archived)");
                }
                if !row["coder"].is_null() {
                    title.push_str(" (Coder)");
                }
                table_rows.push(vec![
                    row["thread"].as_str().unwrap_or("").to_owned(),
                    atif::iso(row["updated"].as_u64().unwrap_or(0) * 1_000),
                    title,
                ]);
            }
            format!(
                "{}\n{} threads in {} ({})",
                table(&table_rows),
                value["total"],
                value["backend"].as_str().unwrap_or(""),
                value["at"].as_str().unwrap_or("")
            )
        },
    );
    Ok(0)
}

fn read(output: &Output, client: &Client, thread: &openagents_chat::thread::Thread) {
    output.emit(
        &json!({
            "thread": thread.summary.id,
            "title": thread.summary.title,
            "backend": client.kind().word(),
            "busy": thread.busy,
            "failure": thread.failure,
            "coder": thread.summary.coder,
            "turns": thread.turns,
        }),
        |value| {
            let mut out = format!("{}\n", value["title"].as_str().unwrap_or(""));
            for turn in value["turns"].as_array().into_iter().flatten() {
                let who = if turn["role"] == "user" {
                    "you"
                } else {
                    "openagents"
                };
                out.push_str(&format!(
                    "\n{who}{}:\n{}\n",
                    if turn["stopped"] == true {
                        " (stopped)"
                    } else {
                        ""
                    },
                    turn["text"].as_str().unwrap_or("")
                ));
            }
            if value["busy"] == true {
                out.push_str("\n(a reply is streaming)\n");
            }
            if let Some(failure) = value["failure"].as_str() {
                out.push_str(&format!("\n({failure})\n"));
            }
            out.push_str(&format!(
                "\nthread {}",
                value["thread"].as_str().unwrap_or("")
            ));
            out
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_come_from_words_and_are_bounded() {
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            message(&words("How do I connect a phone")).ok().unwrap(),
            "How do I connect a phone"
        );
        assert!(matches!(message(&[]), Err(Failure::Usage(_))));
        assert!(matches!(
            message(&["x".repeat(33 * 1024)]),
            Err(Failure::Usage(_))
        ));
    }
}
