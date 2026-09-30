//! `openagents chat`: talk to OpenAgents, the chat router, from a terminal.
//!
//! This is a thin front over the shared chat service
//! ([`openagents_chat::service`]): every operation is a
//! [`Command`] and every answer a [`Snapshot`], the same ones the phone and
//! the desktop use. There is no chat logic here. When this computer's host
//! runs, the commands go to its control socket, so the threads are the
//! desktop app's threads and a Coder offer is accepted through the host's
//! own handoff. Otherwise the service runs in this process, with this
//! command's own device key and encrypted store. The worker's router decides
//! every route; this command only shows what it said.
//!
//! The user-facing unit is the **thread** (`docs/glossary.md`); the shared
//! code calls it a chat. [`openagents_chat::thread`] renders a thread as its
//! ATIF trajectory.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use openagents_chat::basic_chats::{BasicChats, Summary};
use openagents_chat::basic_coder::{self, Role, Turn};
use openagents_chat::cache::Cache;
use openagents_chat::router::{Context, Meta, Offer, Surface};
use openagents_chat::service::{self, Command, Snapshot};
use openagents_connect::control::{self, Op, Reply, Request};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use tokio::net::UnixStream;

use crate::out::{Output, table};
use crate::{Args, EXIT_FAILURE, runtime};

#[path = "chat_coder.rs"]
mod coder_run;

pub(crate) const USAGE: &str = "usage: openagents chat COMMAND [OPTIONS]
  send MESSAGE [--thread ID] [--no-run] [--timeout SECONDS]
        Send one message to OpenAgents and stream its reply. `openagents chat
        MESSAGE` is the same, and MESSAGE `-` reads it from stdin. Without
        --thread a new thread starts. When OpenAgents judges that the
        message is coding work, Coder runs on this computer at once, in its
        own worktree of the Git checkout this command runs in, with Codex or
        Claude Code, whichever is signed in here and has capacity, and its
        events stream here. --no-run only shows the offer instead.
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
    Declared::computer("follow", Effect::ReadOnly),
    Declared::computer("stop", Effect::Publishes),
    Declared::computer("answer", Effect::Publishes),
];

/// How long a reply is waited for by default: the chat worker's own limit.
const DEFAULT_TIMEOUT: u64 = 120;
/// How often a streaming reply is read.
const POLL: Duration = Duration::from_millis(80);
/// The message the apps show when a person stops a reply.
const STOPPED: &str = "Stopped receiving this reply. The hosted worker may still finish.";
const OPTIONS: &[&str] = &["thread", "timeout", "limit", "socket"];
const SWITCHES: &[&str] = &["scratch", "local", "all", "run-coder", "no-run"];

/// What `send` does when OpenAgents judges the message is coding work.
#[derive(Clone, Copy, Debug)]
struct Run {
    /// Only show the offer (`--no-run`).
    no_run: bool,
}

pub(crate) enum Failure {
    Usage(String),
    Failed(String),
}

fn failed(message: impl Into<String>) -> Failure {
    Failure::Failed(message.into())
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
        "send" | "threads" | "read" | "export" | "run-coder" | "follow" | "stop" | "answer" => {
            (first.as_str(), &words[1..])
        }
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
        && !thread_id(id)
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
    match command {
        "send" => {
            let message = message(args.positional())?;
            let timeout = match args.number("timeout", DEFAULT_TIMEOUT) {
                Ok(timeout) if timeout > 0 => Duration::from_secs(timeout),
                _ => {
                    return Err(Failure::Usage(
                        "--timeout is a positive number of seconds".into(),
                    ));
                }
            };
            let (id, new) = match thread {
                Some(id) => (id, false),
                None => (new_id(), true),
            };
            let mut backend = Backend::open(args, Some(&id), new).await?;
            send(
                output,
                &mut backend,
                &id,
                new,
                &message,
                Run {
                    // `--no-run`, or `coder.start: ask_first` in the
                    // settings, keeps only the offer; `--run-coder` runs.
                    no_run: (args.switch("no-run") || coder_run::asks_first())
                        && !args.switch("run-coder"),
                },
                timeout,
            )
            .await
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
            let mut backend = Backend::open(args, None, false).await?;
            threads(output, &mut backend, args.switch("all"), limit).await
        }
        "read" | "export" => {
            no_positional(args)?;
            let id = needs_thread(&thread)?;
            let mut backend = Backend::open(args, Some(&id), false).await?;
            let whole = backend.collect(&id).await?;
            if command == "read" {
                read(output, &backend, &whole);
            } else {
                let tasks = task_trajectories(&backend, &id, &whole);
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
        "run-coder" | "follow" | "stop" => {
            no_positional(args)?;
            let id = needs_thread(&thread)?;
            let mut backend = Backend::open(args, Some(&id), false).await?;
            let code = match command {
                "run-coder" => run_coder(output, &mut backend, &id).await,
                "follow" => coder_run::follow(output, &mut backend, &id).await,
                _ => coder_run::stop(output, &mut backend, &id).await,
            };
            if !output.json() {
                eprintln!("thread {id}");
            }
            Ok(code)
        }
        "answer" => {
            let id = needs_thread(&thread)?;
            let text = message(args.positional())?;
            let mut backend = Backend::open(args, Some(&id), false).await?;
            let code = coder_run::answer(output, &mut backend, &id, &text).await;
            if !output.json() {
                eprintln!("thread {id}");
            }
            Ok(code)
        }
        _ => Err(Failure::Usage(format!("unknown command `{command}`"))),
    }
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

/// A thread or send ID: 32 lowercase hex characters, as the service admits.
fn thread_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn new_id() -> String {
    hex(&secp256k1::rand::random::<[u8; 16]>())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Where threads live for this run.
pub(crate) enum Backend {
    /// This computer's host, over its control socket: the desktop app's
    /// threads.
    Host {
        stream: UnixStream,
        next: u64,
        socket: PathBuf,
    },
    /// The chat service in this process.
    Local {
        chats: Box<BasicChats>,
        scratch: bool,
        home: PathBuf,
    },
}

impl Backend {
    /// The backend `args` choose. `thread` names the scratch store;
    /// `new` creates one.
    async fn open(args: &Args, thread: Option<&str>, new: bool) -> Result<Self, Failure> {
        if args.switch("scratch") {
            let id = thread.ok_or_else(|| Failure::Usage("--scratch needs a thread".into()))?;
            let home = scratch_dir(id);
            if !new && !home.join("device.key").exists() {
                return Err(failed(format!(
                    "no scratch thread {id} on this computer ({} is gone)",
                    home.display()
                )));
            }
            let ready = !args.switch("no-run") && coder_run::ready(home.join("tasks"));
            return Self::local(home, true, ready);
        }
        let named = args.option("socket").map(PathBuf::from);
        if !args.switch("local")
            && let Some(socket) = named.clone().or_else(control::socket_path)
        {
            match UnixStream::connect(&socket).await {
                Ok(stream) => {
                    let host = Self::Host {
                        stream,
                        next: 1,
                        socket,
                    };
                    host.migrate(&home()).await;
                    return Ok(host);
                }
                Err(_) if named.is_some() => {
                    return Err(failed(format!(
                        "no host answers at {}; start it with `openagents host serve --control` \
                         or open the OpenAgents app",
                        socket.display()
                    )));
                }
                Err(_) => {}
            }
        }
        let ready = !args.switch("no-run") && coder_run::ready(coder::task::local::default_store());
        Self::local(home(), false, ready)
    }

    /// Ask the host to take in the threads this command kept without one
    /// in `home` (`openagents_chat::migrate`), when there are any. The host
    /// reads them with this command's device key and re-encrypts them in
    /// its own store, keeping every ID; a second ask is a no-op. An older
    /// host, or a refusal, leaves them where they are, still readable with
    /// `--local`.
    ///
    /// It asks on a connection of its own: an older host ends a connection
    /// that carried an operation it doesn't know.
    async fn migrate(&self, home: &Path) {
        let Self::Host { socket, .. } = self else {
            return;
        };
        if !openagents_chat::migrate::pending(home) || openagents_chat::migrate::scratch(home) {
            return;
        }
        let (Ok(home), Ok(mut stream)) = (home.canonicalize(), UnixStream::connect(socket).await)
        else {
            return;
        };
        let op = Op::ChatMigrate {
            home: home.display().to_string(),
        };
        match control::call(&mut stream, &Request::new(1, op)).await {
            Ok(Reply::ChatMigrated { moved, .. }) if moved > 0 => eprintln!(
                "moved {moved} thread{} kept without a host into this computer's host",
                if moved == 1 { "" } else { "s" }
            ),
            Ok(Reply::Refused { code, message }) if code != "malformed" => {
                eprintln!(
                    "threads in {} stay there for now: {message}",
                    home.display()
                );
            }
            _ => {}
        }
    }

    /// The service in this process. `ready` says whether Coder can run on
    /// this computer for this thread now, which the router is told.
    fn local(home: PathBuf, scratch: bool, ready: bool) -> Result<Self, Failure> {
        let secret = device_key(&home, true).map_err(failed)?;
        let store = Cache::open(&home.join("threads"), &secret)
            .map_err(|error| failed(format!("cannot open the chat store: {error}")))?;
        let relay = std::env::var("OPENAGENTS_CHAT_RELAY")
            .unwrap_or_else(|_| basic_coder::RELAY.to_owned());
        let worker = std::env::var("OPENAGENTS_CHAT_WORKER")
            .unwrap_or_else(|_| basic_coder::WORKER.to_owned());
        let door = basic_coder::Relay::new(&relay, &worker, secret).map_err(failed)?;
        let mut chats = BasicChats::new(
            Some(tokio::runtime::Handle::current()),
            Some(Arc::new(door)),
            Some(store),
        );
        // Coder runs on this computer when it is in a checkout and a coding
        // agent is signed in here with capacity.
        chats.set_context(Context {
            surface: Surface::Terminal,
            computer_ready: ready,
            ..Context::default()
        });
        Ok(Self::Local {
            chats: Box::new(chats),
            scratch,
            home,
        })
    }

    /// The word `--json` names this backend with.
    fn name(&self) -> &'static str {
        match self {
            Self::Host { .. } => "host",
            Self::Local { scratch: true, .. } => "scratch",
            Self::Local { .. } => "in_process",
        }
    }

    fn place(&self) -> String {
        match self {
            Self::Host { socket, .. } => socket.display().to_string(),
            Self::Local { home, .. } => home.display().to_string(),
        }
    }

    pub(crate) async fn apply(&mut self, command: Command) -> Result<Snapshot, String> {
        match self {
            Self::Host { stream, next, .. } => {
                let id = *next;
                *next += 1;
                match control::call(stream, &Request::new(id, Op::Chat { command })).await {
                    Ok(Reply::Chat { snapshot }) => Ok(snapshot),
                    Ok(Reply::Refused { message, .. }) => Err(message),
                    Ok(_) => Err("the host answered another question".into()),
                    Err(error) => Err(format!("the host did not answer: {error}")),
                }
            }
            Self::Local { chats, .. } => service::apply(chats, command, now()),
        }
    }

    /// The whole thread, read page by page through the shared reader.
    pub(crate) async fn collect(
        &mut self,
        id: &str,
    ) -> Result<openagents_chat::thread::Thread, Failure> {
        let handle = tokio::runtime::Handle::current();
        tokio::task::block_in_place(|| {
            openagents_chat::thread::collect(id, |command| handle.block_on(self.apply(command)))
        })
        .map_err(failed)
    }
}

/// `~/.openagents/chat`, or `OPENAGENTS_CHAT_HOME`.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_CHAT_HOME") {
        return dir.into();
    }
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/chat")
}

/// The throwaway home of one scratch thread.
pub(crate) fn scratch_dir(id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("openagents-chat-scratch")
        .join(id)
}

/// This command's device key in `home`, created on first use (`0600`).
/// It is never printed.
fn device_key(home: &Path, create: bool) -> Result<SecretKey, String> {
    let path = home.join("device.key");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let bytes: Vec<u8> = (0..text.trim().len())
                .step_by(2)
                .filter_map(|at| text.trim().get(at..at + 2))
                .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
                .collect();
            let bytes: [u8; 32] = bytes
                .try_into()
                .map_err(|_| format!("{} is not a chat device key", path.display()))?;
            SecretKey::from_byte_array(bytes)
                .map_err(|_| format!("{} is not a chat device key", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
            std::fs::create_dir_all(home)
                .map_err(|error| format!("cannot create {}: {error}", home.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700));
            }
            let secret = SecretKey::new(&mut secp256k1::rand::rng());
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&path)
                .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
            file.write_all(hex(&secret.secret_bytes()).as_bytes())
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            Ok(secret)
        }
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// What `doctor` says about chat: where threads would live, and this
/// command's public identity when it has one. Reads only.
pub fn doctor() -> Value {
    let socket = control::socket_path();
    let running = socket
        .as_ref()
        .is_some_and(|socket| std::os::unix::net::UnixStream::connect(socket).is_ok());
    let home = home();
    let identity = device_key(&home, false).ok().map(|secret| {
        let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret);
        hex(&key.x_only_public_key().0.serialize())
    });
    json!({
        "backend": if running { "host" } else { "in_process" },
        "host_socket": socket.map(|socket| socket.display().to_string()),
        "host_running": running,
        "home": home.display().to_string(),
        "store_exists": home.join("threads").exists(),
        "identity": identity,
    })
}

/// One `--json` event line, or nothing in text mode.
pub(crate) fn event(output: &Output, value: Value) {
    if output.json() {
        println!("{value}");
        let _ = std::io::stdout().flush();
    }
}

#[allow(clippy::too_many_arguments)]
async fn send(
    output: &Output,
    backend: &mut Backend,
    id: &str,
    new: bool,
    text: &str,
    run: Run,
    timeout: Duration,
) -> Result<u8, Failure> {
    if new {
        backend
            .apply(Command::Create {
                chat: id.to_owned(),
            })
            .await
            .map_err(failed)?;
    }
    let request = new_id();
    let sent = backend
        .apply(Command::Send {
            chat: id.to_owned(),
            request: request.clone(),
            text: text.to_owned(),
        })
        .await;
    let sent = match sent {
        Ok(sent) => sent,
        Err(message) => {
            event(
                output,
                json!({"event": "failure", "thread": id, "message": message, "stopped": false}),
            );
            eprintln!("openagents chat: {message}");
            eprintln!("thread {id}");
            return Ok(EXIT_FAILURE);
        }
    };
    event(
        output,
        json!({
            "event": "accepted",
            "thread": id,
            "request": request,
            "new": new,
            "backend": backend.name(),
            "at": backend.place(),
        }),
    );
    let deadline = tokio::time::Instant::now() + timeout;
    let mut shown = String::new();
    let mut printed = String::new();
    let mut diverged = false;
    let mut snapshot = sent;
    let mut stopped = false;
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    while snapshot.busy {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => { stopped = true; }
            _ = &mut interrupt => { stopped = true; }
            () = tokio::time::sleep(POLL) => {}
        }
        if stopped {
            snapshot = backend
                .apply(Command::Stop {
                    chat: id.to_owned(),
                })
                .await
                .map_err(failed)?;
            break;
        }
        snapshot = backend
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await
            .map_err(failed)?;
        if snapshot.busy && snapshot.partial != shown {
            let delta = snapshot
                .partial
                .strip_prefix(shown.as_str())
                .map(str::to_owned);
            event(
                output,
                json!({"event": "partial", "thread": id, "text": snapshot.partial, "delta": delta}),
            );
            shown.clone_from(&snapshot.partial);
            if !output.json() && !diverged {
                match shown.strip_prefix(printed.as_str()) {
                    Some(more) => {
                        print!("{more}");
                        let _ = std::io::stdout().flush();
                        printed.clone_from(&shown);
                    }
                    None => diverged = true,
                }
            }
        }
    }
    // The reply to this message: the turn after it, when one came.
    let at = snapshot
        .turns
        .iter()
        .rposition(|turn| turn.request.as_deref() == Some(request.as_str()));
    let reply = at
        .and_then(|at| snapshot.turns.get(at + 1))
        .filter(|turn| turn.role == Role::Assistant)
        .cloned();
    let mut coding = false;
    let mut code = match reply {
        Some(reply) if !reply.stopped => {
            // The router judged this is coding: Coder runs here at once,
            // unless the person asked only for the offer.
            coding = openagents_chat::delegation::offered(reply.meta.as_ref(), snapshot.computer);
            finish(
                output,
                id,
                &reply,
                snapshot.computer,
                &mut printed,
                diverged,
                coding && !run.no_run,
            );
            0
        }
        reply => {
            let message = if stopped || reply.is_some() {
                STOPPED.to_owned()
            } else {
                snapshot
                    .failure
                    .clone()
                    .unwrap_or_else(|| basic_coder::Failure::Silent.describe())
            };
            if let Some(reply) = &reply
                && !output.json()
                && reply.text.starts_with(printed.as_str())
            {
                print!("{}", &reply.text[printed.len()..]);
            }
            if !output.json() && !printed.is_empty() {
                println!();
            }
            event(
                output,
                json!({
                    "event": "failure",
                    "thread": id,
                    "message": message,
                    "stopped": stopped || reply.is_some(),
                    "partial": reply.map(|reply| reply.text),
                }),
            );
            eprintln!("openagents chat: {message}");
            EXIT_FAILURE
        }
    };
    // The reply arrived; a coding reply then succeeds only if Coder does.
    if code == 0 && coding && !run.no_run {
        code = run_coder(output, backend, id).await;
    }
    if !output.json() {
        eprintln!("thread {id}");
    }
    Ok(code)
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
        }),
    );
    for offer in &meta.offers {
        event(
            output,
            json!({"event": "offer", "thread": id, "offer": offer, "accept": accept(id, offer)}),
        );
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
    notes(id, &meta, computer && !running, running);
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
            Offer::Cli { argv, .. } => eprintln!("offer: {}", Offer::command_line(argv)),
            Offer::StartEval { .. } => {
                eprintln!("offer: run this test set from the OpenAgents app")
            }
            Offer::PublishEval { .. } => {
                eprintln!("offer: add this result to the Gym from the OpenAgents app")
            }
        }
    }
    if coder {
        eprintln!(
            "offer: run Coder on this computer for this thread: openagents chat run-coder --thread {id}"
        );
    }
    for followup in &meta.followups {
        eprintln!("suggestion: {}", followup.label);
    }
}

/// Accept the thread's offer to run Coder through the host's own handoff,
/// the one the desktop's Run Coder uses.
async fn run_coder(output: &Output, backend: &mut Backend, id: &str) -> u8 {
    let report = |accepted: bool, message: &str, coder: Option<Value>| {
        event(
            output,
            json!({"event": "coder", "thread": id, "accepted": accepted, "message": message, "task": coder}),
        );
        if !output.json() {
            eprintln!("{message}");
        }
    };
    let snapshot = match backend
        .apply(Command::Read {
            chat: id.to_owned(),
            before: None,
        })
        .await
    {
        Ok(snapshot) => snapshot,
        Err(message) => {
            report(false, &message, None);
            return EXIT_FAILURE;
        }
    };
    if let Some(coder) = &snapshot.coder {
        report(
            true,
            &format!(
                "Coder already started task {} for this thread; following it.",
                coder.task
            ),
            serde_json::to_value(coder).ok(),
        );
        return coder_run::follow(output, backend, id).await;
    }
    let meta = snapshot
        .turns
        .iter()
        .rev()
        .find(|turn| turn.role == Role::Assistant)
        .and_then(|turn| turn.meta.as_ref());
    if !openagents_chat::delegation::offered(meta, snapshot.computer) {
        report(
            false,
            "OpenAgents has not offered to run Coder for this thread's last reply.",
            None,
        );
        return EXIT_FAILURE;
    }
    // The project is the checkout this command runs in: Coder runs here,
    // with or without a host.
    let here = std::env::current_dir().ok();
    let checkout = here
        .as_deref()
        .map(coder::task::local::checkout)
        .unwrap_or_else(|| Err("This command has no working directory.".into()));
    let why = match checkout {
        Ok(_) => return coder_run::start(output, backend, id).await,
        Err(why) => why,
    };
    // Outside a checkout, a host with a project of its own still can.
    if let Backend::Local { .. } = backend {
        report(false, &why, None);
        return EXIT_FAILURE;
    }
    match backend
        .apply(Command::RunCoder {
            chat: id.to_owned(),
        })
        .await
    {
        Ok(snapshot) => match snapshot.coder {
            Some(coder) => {
                report(
                    true,
                    &format!("Coder started task {} on {}.", coder.task, coder.host),
                    serde_json::to_value(&coder).ok(),
                );
                0
            }
            None => {
                report(false, "The host did not start a Coder task.", None);
                EXIT_FAILURE
            }
        },
        Err(message) => {
            report(false, &message, None);
            EXIT_FAILURE
        }
    }
}

/// The thread's Coder task, every turn's trajectory, when its task store
/// on this computer holds it: carried inside the thread's export.
fn task_trajectories(
    backend: &Backend,
    id: &str,
    thread: &openagents_chat::thread::Thread,
) -> Vec<Value> {
    let Some(coder) = &thread.summary.coder else {
        return Vec::new();
    };
    let store = coder_run::store(backend, id);
    let Ok(task) = coder::task::Store::open(&store).and_then(|tasks| tasks.show(&coder.task))
    else {
        return Vec::new();
    };
    task.earlier
        .iter()
        .chain(task.run.iter())
        .filter_map(|run| atif::log::read(&store.join(&run.admission.trace_file)).ok())
        .map(|recording| recording.document())
        .filter(|document| atif::validate(document).is_empty())
        .collect()
}

async fn threads(
    output: &Output,
    backend: &mut Backend,
    all: bool,
    limit: usize,
) -> Result<u8, Failure> {
    let first = backend.apply(Command::List {}).await.map_err(failed)?;
    let mut rows: Vec<Summary> = first.chats.clone();
    let mut after = first.list_start + first.chats.len();
    while rows.iter().filter(|row| all || !row.archived).count() < limit && after < first.list_total
    {
        let page = backend
            .apply(Command::ListMore {
                after,
                version: first.list_version,
            })
            .await
            .map_err(failed)?;
        if page.chats.is_empty() {
            break;
        }
        after += page.chats.len();
        rows.extend(page.chats);
    }
    let rows: Vec<Value> = rows
        .into_iter()
        .filter(|row| all || !row.archived)
        .take(limit)
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
            "backend": backend.name(),
            "at": backend.place(),
            "total": first.list_total,
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

fn read(output: &Output, backend: &Backend, thread: &openagents_chat::thread::Thread) {
    output.emit(
        &json!({
            "thread": thread.summary.id,
            "title": thread.summary.title,
            "backend": backend.name(),
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
    fn thread_ids_are_the_services_ids() {
        assert!(thread_id(&new_id()));
        assert!(!thread_id(&"A".repeat(32)));
        assert!(!thread_id("abc"));
    }

    #[test]
    fn a_device_key_is_created_once_private_and_never_printed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(device_key(dir.path(), false).is_err());
        let first = device_key(dir.path(), true).unwrap();
        let again = device_key(dir.path(), false).unwrap();
        assert_eq!(first, again);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("device.key"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
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
}
