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

#[path = "chat_boat.rs"]
mod boat;
#[path = "chat_coder.rs"]
mod coder_run;
#[path = "chat_fleet.rs"]
mod fleet;
#[path = "chat_gce.rs"]
mod gce;
#[path = "chat_placement.rs"]
mod placement;
#[path = "chat_shell.rs"]
mod shell;
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
  shell-result -
        Record one exact approved shell result on its existing thread.
  shell-request -
        Submit a typed live-shell request from stdin. Commands remain pending.
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
  apply --thread ID
        Bring the thread's Coder change into the checkout it was made from,
        uncommitted, so you can review and commit it there. The checkout
        must have no changes of its own.
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
  work --issues NUMBERS|LABEL [--parallel N] [--land main|pr] [--on boat|gce]
        Hand several issues to Coder, one issue flow each, each in its own
        thread: NUMBERS such as 10050,10051, or a LABEL's open issues.
        --parallel (1 to 4, default 1) runs that many at once, each in its
        own worktree. Issues claimed in the last hours (the repository's
        claim window) and closed issues are skipped. --land overrides the
        repository's policy.
        --on boat runs each issue on a Boat sandbox of its own instead of
        this computer (--parallel up to 16; starts are queued under Boat's
        start limits): started from the newest oa-coder-main-<date>
        template (--template NAME picks one; with none, a seed is set up
        once and forked), the same issue flow runs and lands there, its
        events stream here, and the sandbox's machine time and cost go in
        a comment on the issue. The sandbox stops when its run ends and is
        deleted once the issue landed. --engine-logins api-keys (default:
        Codex on this computer's ChatGPT login, sent as a copy that
        can't refresh; with --engine-fallback, else an OpenAI key from OA_CODER_OPENAI_API_KEY or
        Secret Manager coder-openai-api-key; and Grok Build's XAI_API_KEY)
        or boat (subscriptions connected on Boat's dashboard). Needs BOAT_API_KEY or Secret Manager
        boat-api-key, and a GitHub token (OA_BOAT_GH_TOKEN, Secret Manager
        coder-pool-git-token, or `gh auth token`); docs/cloud/boat-chat-work.md.
        With --engine-logins api-keys, cloud runs require a ChatGPT access token with at least 2 h left.
        If it is short, open Codex on the Mac once to refresh it, then rerun.
        --engine-fallback allows an OpenAI API key or Grok Build instead,
        and allows switching engines during the run. Without it, runs use
        only Codex on the ChatGPT login and refuse if it is unavailable.
        --on gce runs each issue on the GCE pool this computer granted with
        `openagents cloud up` (--parallel up to 32, two runs per host; the
        pool grows within the grant's --max-hosts): the same issue flow
        runs and lands on a pool host, its events stream here, and its wall
        time and estimated cost go in a comment on the issue. Hosts delete
        themselves when idle. A lost host resumes on another pool host from
        its pushed progress or stranded branch, or from scratch. When spot
        capacity is unavailable, new hosts fall back to on demand. Preemptions
        go in the run record. Without a live grant it refuses; it never
        runs elsewhere. Engine logins use the ChatGPT copy first, or with
        --engine-fallback, Codex's OpenAI key or Grok Build's XAI_API_KEY,
        as for boat; docs/cloud/gce-pool.md.
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
reply; OpenAgents may still finish it. Every run prints the thread ID
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
    Declared::computer("shell-request", Effect::Publishes),
    Declared::computer("shell-result", Effect::Publishes),
    Declared::computer("threads", Effect::ReadOnly),
    Declared::computer("read", Effect::ReadOnly),
    Declared::computer("export", Effect::ReadOnly),
    Declared::computer("run-coder", Effect::Publishes),
    Declared::computer("run-command", Effect::LocalWrite),
    Declared::computer("follow", Effect::ReadOnly),
    Declared::computer("stop", Effect::Publishes),
    Declared::computer("answer", Effect::Publishes),
    Declared::computer("work", Effect::Publishes),
    Declared::computer("apply", Effect::LocalWrite),
];

const OPTIONS: &[&str] = &[
    "thread",
    "timeout",
    "limit",
    "socket",
    "issues",
    "parallel",
    "land",
    "on",
    "engine-logins",
    "template",
];
const SWITCHES: &[&str] = &[
    "scratch",
    "local",
    "all",
    "run-coder",
    "no-run",
    "engine-fallback",
];

pub(crate) enum Failure {
    Usage(String),
    /// A usage error the whole usage would only bury: one line, exit 64.
    Refused(String),
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

/// The `openagents chat` commands; any other first word starts a message.
const COMMANDS: &[&str] = &[
    "shell-request",
    "shell-result",
    "send",
    "threads",
    "read",
    "export",
    "run-coder",
    "run-command",
    "follow",
    "stop",
    "answer",
    "work",
    "apply",
];

/// The command `word` is a typo of: within one edit of a name of up to six
/// letters or two of a longer one. Only the command names are compared,
/// nothing about what the message means.
fn near_command(word: &str) -> Option<&'static str> {
    if word.contains(char::is_whitespace) || word.len() < 3 {
        return None;
    }
    let word = word.to_lowercase();
    COMMANDS
        .iter()
        .map(|name| (*name, edit_distance(&word, name)))
        .filter(|(name, distance)| {
            *distance > 0 && *distance <= if name.len() <= 6 { 1 } else { 2 }
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(name, _)| name)
}

/// Edit distance between two short words, counting a swap of neighbours
/// as one edit (`sned` is one from `send`).
pub(super) fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
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
        name if COMMANDS.contains(&name) => (name, &words[1..]),
        // A word one or two edits from a command's name is a typo of it,
        // not the start of a message: `chat sned hi`.
        word if near_command(word).is_some() => {
            let near = near_command(word).unwrap_or_default();
            let message = words.join(" ");
            return output.refuse(
                "chat",
                &format!(
                    "unknown command `{word}`; did you mean `openagents chat {near}`? \
                     To send this as a message: openagents chat send \"{message}\""
                ),
            );
        }
        // `openagents chat MESSAGE` is `openagents chat send MESSAGE`.
        _ => ("send", words),
    };
    if rest.first().is_some_and(|word| word == "--help") {
        if let Some(usage) = crate::argv::command_usage("chat", command, USAGE) {
            println!("{usage}");
            return 0;
        }
    }
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
        Err(Failure::Refused(message)) => output.refuse("chat", &message),
        Err(Failure::Failed(message)) => output.fail("chat", &message),
    }
}

async fn dispatch(output: &Output, command: &str, args: &Args) -> Result<u8, Failure> {
    let mut printer = Printer::new(output);
    let thread = match args.option("thread") {
        Some(id) if !client::thread_id(id) => Some(named_thread(args, id, &mut printer).await?),
        other => other.map(str::to_owned),
    };
    let needs_thread = |thread: &Option<String>| {
        thread
            .clone()
            .ok_or_else(|| Failure::Usage(format!("`chat {command}` needs --thread ID")))
    };
    match command {
        "shell-request" => shell::request(output, args).await,
        "shell-result" => shell::result(output, args).await,
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
        "apply" => {
            no_positional(args)?;
            let id = needs_thread(&thread)?;
            let mut client = open(args, Some(&id), false, &mut printer).await?;
            let whole = client.collect(&id).await?;
            match client.apply_coder(&id, &whole) {
                Ok((checkout, files)) => {
                    output.emit(
                        &json!({"thread": id, "checkout": checkout, "files": files}),
                        |_| {
                            let mut out = format!(
                                "Applied Coder's change to {}, uncommitted:",
                                checkout.display()
                            );
                            for file in &files {
                                out.push_str(&format!("\n  {file}"));
                            }
                            out
                        },
                    );
                    Ok(0)
                }
                Err(why) => Err(failed(why)),
            }
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

/// The thread `arg` names when it isn't a whole ID: a unique ID prefix or
/// a title, as `openagents terminal --resume` takes.
async fn named_thread(
    args: &Args,
    arg: &str,
    printer: &mut Printer<'_>,
) -> Result<String, Failure> {
    if args.switch("scratch") {
        return Err(Failure::Refused(
            "a --scratch thread is named by its whole 32-character ID".into(),
        ));
    }
    let mut client = open(args, None, false, printer).await?;
    let (rows, _) = client.threads(true, NAMED_THREADS_MAX).await?;
    openagents_terminal::picker::resolve(&rows, arg)
        .map(|row| row.id.clone())
        .ok_or_else(|| {
            Failure::Refused(format!(
                "no single thread is named `{arg}`; give its ID, a unique ID prefix, \
                 or its title (`openagents chat threads` lists them)"
            ))
        })
}

/// How many threads a prefix or title is matched against.
const NAMED_THREADS_MAX: usize = 500;

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
    open_as(args, thread, new, printer, Caller::CLI).await
}

/// [`open`] as `caller`: OpenAgents Terminal's live shell opens as
/// [`Caller::TERMINAL`], so its turns carry the terminal's instructions.
pub(crate) async fn open_as(
    args: &Args,
    thread: Option<&str>,
    new: bool,
    printer: &mut Printer<'_>,
    caller: Caller,
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
    let mut options = client::Options::new(caller);
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
    /// "Reconnecting" was printed and not yet answered by "Connected
    /// again".
    offline: bool,
}

impl<'a> Printer<'a> {
    pub(crate) fn new(output: &'a Output) -> Self {
        Self {
            output,
            kind: Kind::InProcess,
            printed: String::new(),
            diverged: false,
            tools: openagents_chat::tool_groups::Stream::default(),
            offline: false,
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
                route,
            } => finish(
                output,
                &thread,
                &reply,
                computer,
                &mut self.printed,
                self.diverged,
                running,
                route,
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
                // A start's own line (`coder_started`) says who works; a
                // refusal the runner line already said is not said twice.
                let said = !accepted
                    && SAID
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .as_deref()
                        == Some(message.as_str());
                if !output.json() && !quiet && !said {
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
                // The client says so only once the outage has lasted
                // `QUIET_FOR`; the line is printed once per outage.
                if !output.json() && !std::mem::replace(&mut self.offline, true) {
                    eprintln!("Reconnecting to OpenAgents… (Ctrl-C stops)");
                }
            }
            Event::Online { thread } => {
                event(output, json!({"event": "online", "thread": thread}));
                if !output.json() && std::mem::take(&mut self.offline) {
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
            Event::Plugin {
                thread,
                flow,
                text,
                ok,
            } => {
                // A step of making a plugin, typed, and in plain words (#10177).
                event(
                    output,
                    json!({"event": "plugin", "thread": thread, "plugin": flow.wire(), "text": text, "ok": ok}),
                );
                if !output.json() {
                    println!("\n{text}");
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
#[allow(clippy::too_many_arguments)]
fn finish(
    output: &Output,
    id: &str,
    reply: &Turn,
    computer: bool,
    printed: &mut String,
    diverged: bool,
    running: bool,
    route: Option<openagents_chat::route::RouteFamily>,
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
            // The shared route policy's family for this reply (#10207).
            "family": route,
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
    // The shared route policy read the reply (#10207): a Coder route is
    // the offer `run-coder` accepts (#10170).
    let offered = route == Some(openagents_chat::route::RouteFamily::Coder);
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
/// The runner line [`notes`] last showed: a start refused for the same
/// reason says nothing more (#10314).
static SAID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Which product answer replied is the router's own bookkeeping: `--json`
/// and the thread's export carry it, not the person's screen (#10315).
fn notes(id: &str, meta: &Meta, computer: bool, running: bool) {
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
        eprintln!("Nothing is running yet. Start it with: openagents chat run-coder --thread {id}");
        if let Some(engine) = meta.engine {
            eprintln!("asked for: {}", engine.name());
        }
    }
    if let Some(runner) = &meta.runner {
        let text = runner.text();
        eprintln!("coder: {text}");
        *SAID
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text);
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
            let total = value["total"].as_u64().unwrap_or(0);
            let noun = if total == 1 { "thread" } else { "threads" };
            let place = match value["backend"].as_str() {
                Some("host") => "in this computer's host",
                _ => "stored on this computer",
            };
            let summary = format!(
                "{total} {noun} {place} at {}",
                value["at"].as_str().unwrap_or("")
            );
            if table_rows.len() > 1 {
                format!("{}\n{summary}", table(&table_rows))
            } else {
                summary
            }
        },
    );
    Ok(0)
}

fn read(output: &Output, client: &Client, thread: &openagents_chat::thread::Thread) {
    // What Coder did on each of its turns (#10332): the thread's own turns
    // hold only the messages and acknowledgements.
    let endings = client.coder_endings(&thread.summary.id, thread);
    output.emit(
        &json!({
            "thread": thread.summary.id,
            "title": thread.summary.title,
            "backend": client.kind().word(),
            "busy": thread.busy,
            "failure": thread.failure,
            "coder": thread.summary.coder,
            "turns": thread.turns,
            "coder_turns": endings,
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
            if !endings.is_empty() {
                out.push_str("\nCoder:\n");
                for ending in &endings {
                    out.push_str(&coder_turn(ending));
                }
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

/// One ended Coder turn as `chat read` shows it: how it ended, then its
/// answer, indented.
fn coder_turn(ending: &openagents_chat::coder_events::CoderEvent) -> String {
    use openagents_chat::coder_events::CoderEvent;
    let Some(text) = openagents_chat::coder_events::text(ending) else {
        return String::new();
    };
    let mut out = String::new();
    let mut lines = text.lines();
    if let Some(header) = lines.next() {
        out.push_str(&format!("\n{header}\n"));
    }
    for line in lines {
        out.push_str(&format!("{line}\n"));
    }
    if let CoderEvent::Result(result) = ending
        && !result.summary.trim().is_empty()
    {
        for line in result.summary.trim().lines() {
            if line.trim().is_empty() {
                out.push('\n');
            } else {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_coder_turn_reads_as_its_outcome_and_answer() {
        use openagents_chat::coder_events::{CoderEvent, FileChange, Finished};
        let shown = coder_turn(&CoderEvent::Result(Finished {
            turn: 2,
            summary: "Committed as 5ab07fa; 3 tests passed.".into(),
            files_changed: vec![FileChange {
                path: "calc.py".into(),
                status: "modified".into(),
                added: Some(2),
                removed: Some(2),
                patch: None,
                patch_cut: 0,
            }],
            insertions: 2,
            deletions: 2,
            worktree: "/w".into(),
            trajectory: "/t".into(),
            issue: None,
            pushed_to: None,
            cost_microusd: None,
        }));
        assert!(
            shown.contains("Coder finished turn 2: 1 file changed"),
            "{shown}"
        );
        assert!(shown.contains("modified calc.py (+2 -2)"), "{shown}");
        assert!(
            shown.contains("  Committed as 5ab07fa; 3 tests passed."),
            "{shown}"
        );
    }

    #[test]
    fn a_typo_of_a_command_name_is_caught_and_messages_are_not() {
        assert_eq!(near_command("sned"), Some("send"));
        assert_eq!(near_command("thread"), Some("threads"));
        assert_eq!(near_command("folow"), Some("follow"));
        assert_eq!(near_command("expotr"), Some("export"));
        // Exact names are commands, not typos; ordinary first words pass.
        assert_eq!(near_command("send"), None);
        for word in [
            "How", "hello", "explain", "what", "fix", "Can", "reply", "sned hi",
        ] {
            assert_eq!(near_command(word), None, "{word}");
        }
    }

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

    /// "Reconnecting" prints once per outage, and "Connected again" only
    /// after it. (The client sends `offline` only once an outage has
    /// lasted three seconds, so a restart's blip prints nothing.)
    #[test]
    fn reconnecting_is_printed_once_and_answered() {
        let output = Output::new(false);
        let mut printer = Printer::new(&output);
        let thread = "a".repeat(32);
        printer.print(Event::Online {
            thread: thread.clone(),
        });
        assert!(!printer.offline);
        for retry_in in [3, 8] {
            printer.print(Event::Offline {
                thread: thread.clone(),
                retry_in,
            });
            assert!(printer.offline);
        }
        printer.print(Event::Online { thread });
        assert!(!printer.offline);
    }
}
