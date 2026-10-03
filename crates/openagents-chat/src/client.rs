//! The chat client every terminal program uses: `openagents chat` and
//! OpenAgents Terminal (`docs/terminal`).
//!
//! It holds no chat logic of its own. Every operation is a [`Command`] and
//! every answer a [`Snapshot`] of the shared service ([`crate::service`]),
//! the same ones the phone and the desktop use, and the worker's router
//! decides every route. What it adds is where the threads live and how a
//! Coder run on this computer follows a coding reply:
//!
//! - **Backends.** When this computer's host runs, commands go to its
//!   control socket ([`Host`]), so the threads are the desktop app's and a
//!   paired phone's. Otherwise the service runs in this process with the
//!   client's own device key and encrypted store, or in a throwaway scratch
//!   store. [`Client::open`] chooses, and the first connection to a host
//!   asks it to take in the threads kept without one
//!   ([`crate::migrate`]).
//! - **Coder.** A coding reply starts Coder on this computer at once,
//!   unless the settings say to ask first. The run itself is the caller's
//!   [`Coder`] (`coder::task::chat_client::Here` runs
//!   `coder::task::local`), so this crate links no Coder code.
//! - **Surface.** Every turn says which surface and program sent it
//!   ([`Caller`]), in process and through a host alike.
//!
//! Each operation reports what happens as typed [`Event`]s: to a sink the
//! caller passes ([`Client::run`], which blocks until the operation ends),
//! or on a channel ([`Client::stream`]). Nothing here prints.
//!
//! The host's control protocol lives in `openagents-connect`, which
//! depends on this crate, so the host side is a trait ([`Host`], [`Dial`])
//! whose implementation the caller passes in; no Unix socket code links
//! into the phone.

use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use secp256k1::SecretKey;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::basic_chats::{BasicChats, Summary};
use crate::basic_coder::{self, Role, Turn};
use crate::cache::Cache;
use crate::coder_events::{CoderEvent, Line, Runner};
use crate::route::{Bound, Journal, Reading, Situation};
use crate::router::{Caller, ClientWord, CoderRun, Context};
use crate::service::{self, Command, Snapshot};
use crate::thread::{LOCAL_HOST, Thread};
use route_contract::lifecycle::Lifecycle;
use route_contract::record::RouteRecord;
use route_contract::route::{LocalAction, RefusalReason, RouteFamily, RouteResult, RunPolicy};

/// A boxed future, for the traits the caller implements.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What stops receiving a reply, and stops following a Coder run: each
/// call makes a fresh future that completes when the person asks to stop
/// (Ctrl-C in `openagents chat`, Esc in the terminal).
pub type Interrupt = Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>;

/// The words that answer a Coder question from this caller, for the
/// backend and the thread. They travel on the question event.
pub type Hint = fn(Kind, &str) -> String;

/// Where the events of an operation go.
pub type Sink<'a> = dyn FnMut(Event) + Send + 'a;

/// How long a reply is waited for by default: the chat worker's own limit.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// How often a streaming reply is read.
const POLL: Duration = Duration::from_millis(80);
/// How often a running task's trajectory is read.
const CODER_POLL: Duration = Duration::from_millis(300);
/// How often it is read while a run starts, until its first step shows or
/// [`STARTING_FOR`] passes, so the first step is not a poll late (#10115).
const STARTING_POLL: Duration = Duration::from_millis(100);
const STARTING_FOR: Duration = Duration::from_secs(10);
/// The first pause before asking again when the chat cannot be reached.
const OFFLINE_FIRST: Duration = Duration::from_secs(2);
/// The longest pause between tries.
const OFFLINE_MOST: Duration = Duration::from_secs(30);
/// How long the chat stays unreachable before the screen says so. A host
/// that restarts for an update is back well within it, so a blip passes
/// without a word.
pub const QUIET_FOR: Duration = Duration::from_secs(3);

/// One stretch of not reaching the chat, so a short one is not said: the
/// first [`Event::Offline`] comes [`QUIET_FOR`] after it began, and
/// [`Event::Online`] only after an [`Event::Offline`].
#[derive(Debug, Default)]
struct Outage {
    since: Option<tokio::time::Instant>,
    said: bool,
}

impl Outage {
    /// It is over: say so when the screen was told it began.
    fn over(&mut self, id: &str, sink: &mut Sink<'_>) {
        if std::mem::take(self).said {
            sink(Event::Online {
                thread: id.to_owned(),
            });
        }
    }
}
/// The message the apps show when a person stops a reply.
pub const STOPPED: &str = "Stopped showing this reply. OpenAgents may still finish it.";

/// A refusal before or outside an operation's events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The request itself is wrong (a missing thread, a bad ID).
    Usage(String),
    /// The backend or the store failed.
    Failed(String),
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Self::Usage(message) | Self::Failed(message) => message,
        }
    }
}

fn failed(message: impl Into<String>) -> Error {
    Error::Failed(message.into())
}

/// Which backend holds the threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// This computer's host, over its control socket.
    Host,
    /// The service in this process, in the client's own store.
    InProcess,
    /// The service in this process, in a throwaway store.
    Scratch,
    /// Another computer's host, over NIP-HOST.
    Computer,
}

impl Kind {
    /// The word `--json` names this backend with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Scratch => "scratch",
            Self::InProcess => "in_process",
            Self::Computer => "computer",
        }
    }
}

/// Where [`Client::open`] looks for threads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// This computer's host when one answers ([`Dial::socket`], or the
    /// named socket, which must answer), else the client's own store.
    Auto { socket: Option<PathBuf> },
    /// The client's own store, even when a host runs.
    Local,
    /// A throwaway store for one thread, in the system temporary directory.
    Scratch,
}

/// How a client opens.
#[derive(Clone)]
pub struct Options {
    pub place: Place,
    /// Who sends the turns: `openagents chat` or OpenAgents Terminal.
    pub caller: Caller,
    /// The folder the client runs in: the checkout Coder runs in, and the
    /// project the router is told about. `None` when it has none.
    pub dir: Option<PathBuf>,
    /// The client's own chat home ([`home`]).
    pub home: PathBuf,
    /// What stops receiving a reply or following a run.
    pub interrupt: Interrupt,
    /// The words that answer a Coder question from this caller.
    pub hint: Option<Hint>,
}

impl Options {
    /// The defaults for `caller`: wherever the host is, this process's
    /// folder, the default home, and no interrupt.
    pub fn new(caller: Caller) -> Self {
        Self {
            place: Place::Auto { socket: None },
            caller,
            dir: std::env::current_dir().ok(),
            home: home(),
            interrupt: never(),
            hint: None,
        }
    }
}

/// An interrupt that never fires.
pub fn never() -> Interrupt {
    Arc::new(|| Box::pin(std::future::pending()))
}

/// What a host did with the threads kept without one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Migration {
    /// Nothing to say: nothing moved, or an older host.
    Quiet,
    /// This many threads moved into the host.
    Moved(u32),
    /// The host refused; they stay in the client's store.
    Kept(String),
}

/// This computer's host, over its control socket: the implementation lives
/// with the control protocol (`coder::task::chat_client::Control`).
pub trait Host: Send + Sync {
    /// One service command, sent as `caller`.
    fn apply(
        &mut self,
        command: Command,
        caller: Caller,
    ) -> BoxFuture<'_, Result<Snapshot, String>>;

    /// Ask the host to take in the threads kept without one in `home`
    /// (absolute), on a connection of its own.
    fn migrate(&mut self, home: &Path) -> BoxFuture<'_, Migration>;

    /// Whether the last [`Host::apply`] failed because the host did not
    /// answer (it is restarting, or gone), rather than refusing in its
    /// own words. A send that failed so is asked again once it answers.
    fn unanswered(&self) -> bool {
        false
    }
}

/// How a client reaches this computer's host.
pub trait Dial: Send + Sync {
    /// The host's control socket, when this platform has one.
    fn socket(&self) -> Option<PathBuf>;

    /// A connection to the host at `socket`, or `None` when nothing
    /// answers there.
    fn dial<'a>(&'a self, socket: &'a Path) -> BoxFuture<'a, Option<Box<dyn Host>>>;
}

/// No host: every [`Place::Auto`] opens the client's own store.
pub struct NoHost;

impl Dial for NoHost {
    fn socket(&self) -> Option<PathBuf> {
        None
    }

    fn dial<'a>(&'a self, _: &'a Path) -> BoxFuture<'a, Option<Box<dyn Host>>> {
        Box::pin(async { None })
    }
}

/// Where a followed task is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// A turn is running or waiting to be admitted.
    Running,
    /// The last turn asked the person and waits for an answer.
    Waiting,
    /// The last turn ended.
    Ended,
}

/// A started Coder task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub task: String,
    pub project: String,
    pub worktree: String,
}

/// The issue flow Jev judged a message asks for, before it starts.
pub trait Issue: Send {
    /// The issue's number; `0` for a pickup that found no free issue,
    /// whose [`Issue::begin`] says why.
    fn number(&self) -> u64;

    /// The issue's title when Coder chose the issue itself (a pickup), so
    /// the reply names what it picked up.
    fn picked(&self) -> Option<String> {
        None
    }

    /// Claim the issue and start its flow in a worktree of the checkout
    /// `dir` is in, for the thread `chat`. Blocking. On success, `finish`
    /// runs the rest of the flow to its end.
    ///
    /// # Errors
    /// Why the flow did not start.
    fn begin(self: Box<Self>, store: &Path, dir: &Path, chat: &str)
    -> Result<IssueStarted, String>;
}

/// A started issue flow.
pub struct IssueStarted {
    pub started: Started,
    pub url: String,
    /// The rest of the flow: the checks, landing, and closing. Blocking
    /// until the flow is handed to a process of its own, or until it ends
    /// when it is worked here.
    pub finish: Box<dyn FnOnce() + Send>,
}

/// Follows one task's events.
pub trait Follow: Send {
    /// The events recorded since the last poll, and where the task is.
    /// Blocking.
    ///
    /// # Errors
    /// The task store cannot be read or holds no such task.
    fn poll(&mut self) -> Result<(Vec<Line>, Progress), String>;
}

/// Coder on this computer, as the client uses it. Every method but
/// [`Coder::context`], [`Coder::predict`], and [`Coder::asks_first`] may
/// block; the client calls them off the async runtime.
pub trait Coder: Send + Sync {
    /// The task store runs use when the thread is not a scratch thread.
    fn default_store(&self) -> PathBuf;
    /// What a turn tells the worker about this computer (#10077): whether
    /// a run could start, the coding agents' readiness, and the project
    /// folder `dir` is in. The client sets the surface.
    fn context(&self, store: &Path, dir: Option<&Path>) -> Context;
    /// Who a run would use now, and why, for the engine the person asked
    /// for (#10076).
    fn predict(
        &self,
        store: &Path,
        engine: Option<nostr::cj_conversation::Engine>,
    ) -> Option<Runner>;
    /// Whether a coding reply waits to be accepted instead of running at
    /// once (`coder.start: ask_first`).
    fn asks_first(&self) -> bool;
    /// Whether `dir` is a checkout a run can start in, or why not.
    ///
    /// # Errors
    /// Why not, in words for the person.
    fn checkout(&self, dir: &Path) -> Result<(), String>;
    /// Make ready, in the background, what a start in the project `dir` is
    /// in will need (a spare worktree, #10115), so the first start does not
    /// wait on it. Returns at once.
    fn warm(&self, _store: &Path, _dir: &Path) {}
    /// Start a run in a worktree of the checkout `dir` is in.
    ///
    /// # Errors
    /// Why it did not start.
    fn start(
        &self,
        store: &Path,
        dir: &Path,
        title: &str,
        prompt: &str,
        chat: &str,
        requested: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String>;
    /// Start one run of a dispatch plan (#10183) in a worktree of its own:
    /// pinned to `engine`, never falling back to another, and, when
    /// `read_only`, under a boundary that writes nothing in the worktree
    /// and seals Git. A Coder that cannot refuses, so a plan never runs
    /// with less than it promised.
    ///
    /// # Errors
    /// Why it did not start.
    #[allow(clippy::too_many_arguments)]
    fn start_run(
        &self,
        _store: &Path,
        _dir: &Path,
        _title: &str,
        _prompt: &str,
        _chat: &str,
        _engine: nostr::cj_conversation::Engine,
        _read_only: bool,
    ) -> Result<Started, String> {
        Err("Coder here cannot start one run per engine.".into())
    }
    /// The GitHub issue of `dir`'s repository the message asks to work, as
    /// Jev judges it after routing, if any.
    fn issue(&self, request: &str, earlier: &str, dir: &Path) -> Option<Box<dyn Issue>>;
    /// Prepare, in the background, what a run started on `prompt` in the
    /// checkout `dir` reads only from its request, while the router still
    /// judges the message (#10279): the delegate recipe's groundwork, when
    /// the run would start on the lean Claude Code session. The run takes
    /// it from `store` only for that exact prompt. Doing nothing is right.
    fn ahead(&self, _store: &Path, _dir: &Path, _prompt: &str) {}
    /// Follow `task` from its first event. `hint` goes on a question.
    fn follow(&self, store: &Path, task: &str, chat: &str, hint: Option<String>)
    -> Box<dyn Follow>;
    /// Ask the running task to stop.
    ///
    /// # Errors
    /// Why it could not be asked.
    fn stop(&self, store: &Path, task: &str) -> Result<(), String>;
    /// Continue the task with `text` (an answer or the next turn); the turn
    /// it starts.
    ///
    /// # Errors
    /// Why it did not continue.
    fn answer(&self, store: &Path, task: &str, text: &str) -> Result<usize, String>;
    /// Send `text` to the task while it works (steering): the running turn
    /// reads it at its next step, or, when the turn has ended or its engine
    /// reads instructions only as a turn starts, the next turn starts with
    /// it.
    ///
    /// # Errors
    /// Why it could not be sent.
    fn steer(&self, _store: &Path, _task: &str, _text: &str) -> Result<Steering, String> {
        Err("Coder cannot take a message while it works here.".into())
    }
    /// What the task's last turn did, once it ended (#10094).
    fn result(&self, store: &Path, task: &str) -> Option<CoderRun>;
    /// How each of the task's ended turns ended, oldest first (#10332).
    fn endings(&self, _store: &Path, _task: &str) -> Vec<CoderEvent> {
        Vec::new()
    }
    /// Carry the task's change into the checkout it came from, uncommitted
    /// (#10343): that checkout and the files changed.
    ///
    /// # Errors
    /// Why it could not.
    fn apply(&self, _store: &Path, _task: &str) -> Result<(PathBuf, Vec<String>), String> {
        Err("Coder's changes can be applied only on the computer that made them.".into())
    }
    /// How the `openagents` command `argv` (without the program's name)
    /// that a reply proposed may run here (#10170), read from this
    /// computer's own command tree, never from the worker's word.
    fn permit(&self, _argv: &[String]) -> Permit {
        Permit::Never
    }
    /// The effect class of the command `argv` in this computer's own
    /// command tree (13.4), which the route policy reads
    /// ([`crate::route::propose`]); `None` when the tree does not know it.
    /// By default it is what [`Coder::permit`] says.
    fn effect(&self, argv: &[String]) -> Option<route_contract::route::Effect> {
        use route_contract::route::Effect;
        match self.permit(argv) {
            Permit::Now => Some(Effect::ReadOnly),
            Permit::Confirm => Some(Effect::LocalWrite),
            Permit::Never => None,
        }
    }
    /// The task owner's disposition of `task`, with the current turn's cost
    /// and wall time, for the route record (#10207). Blocking.
    fn observe(&self, _store: &Path, _task: &str) -> Option<route_contract::record::Observation> {
        None
    }
    /// Whether `task`'s independent check is still to come or under way
    /// (#10232), so the route record waits for its verdict. Blocking.
    fn checking(&self, _store: &Path, _task: &str) -> bool {
        false
    }
    /// Run the `openagents` command `argv` on this computer. Blocking.
    ///
    /// # Errors
    /// Why it could not run.
    fn run_command(&self, _argv: &[String]) -> Result<Ran, String> {
        Err("Commands do not run here.".into())
    }
    /// Every turn's ATIF trajectory the store holds, for an export.
    fn trajectories(&self, store: &Path, task: &str) -> Vec<Value>;
    /// The worktree `task` works in, where its files are (#10177).
    fn worktree(&self, _store: &Path, _task: &str) -> Option<PathBuf> {
        None
    }
    /// The plugin Coder drafted at `dir` (#10177): its package record
    /// checked as `openagents plugin` checks it, and its tests read as
    /// `openagents plugin test` loads them. Blocking.
    ///
    /// # Errors
    /// Why it is not a plugin with tests here, in words for the person.
    fn plugin_tests(&self, _dir: &Path) -> Result<Vec<crate::plugin_flow::Test>, String> {
        Err("Plugins are not checked here.".into())
    }
    /// Run a step of making a plugin on this computer (#10177): the
    /// `openagents` command `argv` for that step, which may take up to
    /// `timeout`; what it printed on its standard output is the output.
    /// Blocking.
    ///
    /// # Errors
    /// Why it could not run.
    fn plugin_command(&self, _argv: &[String], _timeout: Duration) -> Result<Ran, String> {
        Err("Plugins are not made here.".into())
    }
    /// Whether a background rule compiled from this computer's words waits
    /// under draft `id` to be confirmed (#10157): `openagents background
    /// draft` kept one.
    fn drafted(&self, _id: &str) -> bool {
        false
    }
}

/// How a command a reply proposed may run here ([`Coder::permit`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permit {
    /// It only reads: it runs at once, and the reply shows what it printed.
    Now,
    /// It changes something here: it runs after the person confirms it.
    Confirm,
    /// It moves money, shows a secret, or is unknown here: it never runs
    /// from the chat.
    Never,
}

/// What a command printed ([`Coder::run_command`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    /// It exited with success.
    pub ok: bool,
    /// What it printed, bounded.
    pub output: String,
}

/// Where a message sent to a working run went ([`Coder::steer`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Steering {
    /// The running turn reads it at its next step.
    NextStep,
    /// It starts this turn.
    NextTurn(usize),
}

/// No Coder on this computer: nothing starts, and the router is told so.
pub struct NoCoder;

impl Coder for NoCoder {
    fn default_store(&self) -> PathBuf {
        std::env::temp_dir().join("openagents-chat-no-coder")
    }
    fn context(&self, _: &Path, _: Option<&Path>) -> Context {
        Context::default()
    }
    fn predict(&self, _: &Path, _: Option<nostr::cj_conversation::Engine>) -> Option<Runner> {
        None
    }
    fn asks_first(&self) -> bool {
        false
    }
    fn checkout(&self, _: &Path) -> Result<(), String> {
        Err("Coder does not run here.".into())
    }
    fn start(
        &self,
        _: &Path,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: Option<nostr::cj_conversation::Engine>,
    ) -> Result<Started, String> {
        Err("Coder does not run here.".into())
    }
    fn issue(&self, _: &str, _: &str, _: &Path) -> Option<Box<dyn Issue>> {
        None
    }
    fn follow(&self, _: &Path, task: &str, _: &str, _: Option<String>) -> Box<dyn Follow> {
        struct Gone(String);
        impl Follow for Gone {
            fn poll(&mut self) -> Result<(Vec<Line>, Progress), String> {
                Err(format!("no task {} here", self.0))
            }
        }
        Box::new(Gone(task.to_owned()))
    }
    fn stop(&self, _: &Path, _: &str) -> Result<(), String> {
        Err("Coder does not run here.".into())
    }
    fn answer(&self, _: &Path, _: &str, _: &str) -> Result<usize, String> {
        Err("Coder does not run here.".into())
    }
    fn result(&self, _: &Path, _: &str) -> Option<CoderRun> {
        None
    }
    fn trajectories(&self, _: &Path, _: &str) -> Vec<Value> {
        Vec::new()
    }
}

/// The background-rule draft a thread's words are kept under: the
/// thread's id, as `openagents background draft --id` keeps it.
#[must_use]
pub fn standing_draft(thread: &str) -> String {
    let id: String = thread
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if id.is_empty() { "cli".into() } else { id }
}

/// What happens during an operation, in order. The JSON `openagents chat
/// --json` prints is a rendering of these (`docs/cli/chat.md`).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The host took in this many threads kept without one.
    Migrated { moved: u32 },
    /// The host did not take them in; they stay in `home`.
    Kept { home: PathBuf, message: String },
    /// The message is in the thread and its reply streams next.
    Accepted {
        thread: String,
        request: String,
        new: bool,
        backend: Kind,
        at: String,
    },
    /// The reply so far: `text` whole, and `delta` when it only grew.
    Partial {
        thread: String,
        text: String,
        delta: Option<String>,
    },
    /// The finished reply, with the router's judgment, offers, follow-ups,
    /// cards, and who would run Coder (`reply.meta`). `computer`: the
    /// worker judged the thread a computer's. `running`: Coder starts now.
    Reply {
        thread: String,
        reply: Box<Turn>,
        computer: bool,
        running: bool,
        /// The route the shared policy read the reply as
        /// ([`crate::route::propose`]): what the surface shows (a Coder
        /// offer, a command), never interpreted again. `None` for a
        /// plan's composed summary, which is no new message.
        route: Option<RouteFamily>,
    },
    /// The message has no finished reply: it was stopped, or the worker
    /// failed. `partial` is what came, when a stopped reply kept text.
    ReplyFailed {
        thread: String,
        message: String,
        stopped: bool,
        partial: Option<String>,
    },
    /// An operation failed before anything ran: the send was refused, the
    /// thread has no Coder task, or Coder refused the answer.
    Failure { thread: String, message: String },
    /// A run is starting on `engine` (the provider's word): what a
    /// surface shows at once, before the start's worktree and launch
    /// (#10115).
    Starting { thread: String, engine: String },
    /// Whether Coder took the thread's work, in words, and the task.
    /// `quiet`: the run's own start line says it, so a surface shows no
    /// line for this one; `--json` still carries it.
    Coder {
        thread: String,
        accepted: bool,
        message: String,
        task: Option<Value>,
        quiet: bool,
    },
    /// Coder started, but the thread could not record its task.
    Unbound {
        thread: String,
        why: String,
        issue: bool,
    },
    /// One event of the thread's Coder task (`crate::coder_events`).
    Line(Box<Line>),
    /// The task could not be read.
    TaskUnreadable {
        thread: String,
        task: String,
        message: String,
    },
    /// The task's reader stopped.
    Lost,
    /// Whether the task was asked to stop.
    Stop {
        thread: String,
        task: String,
        requested: bool,
        message: String,
    },
    /// An interrupted issue flow ends at its next step; `why` when the
    /// stop could not be asked.
    Stopping { why: Option<String> },
    /// Following stopped; Coder keeps working.
    Detached { thread: String },
    /// The chat has not been reached for [`QUIET_FOR`] (the relay, or this
    /// computer's host): the reply is asked for again in `retry_in`
    /// seconds. Esc stops waiting. A shorter blip is not said.
    Offline { thread: String, retry_in: u64 },
    /// It can be reached again, and the reply streams on. Only after an
    /// [`Event::Offline`].
    Online { thread: String },
    /// The reply proposed the `openagents` command `argv` (#10170): it
    /// runs here now (`confirm` false, [`Event::Ran`] follows), or waits
    /// for the person to confirm it ([`Op::RunCommand`]).
    Command {
        thread: String,
        argv: Vec<String>,
        confirm: bool,
    },
    /// The command ran here, and what it printed.
    Ran {
        thread: String,
        argv: Vec<String>,
        ok: bool,
        output: String,
    },
    /// A step of making a plugin ran on this computer (#10177): where the
    /// thread's plugin now stands, typed, and what we say about it, which
    /// ends with that step's fixed line when it waits for the person.
    Plugin {
        thread: String,
        flow: crate::plugin_flow::Flow,
        text: String,
        ok: bool,
    },
}

/// How an operation ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// It did what it was asked: a reply came (and its Coder run, if any,
    /// ended with a result or a question).
    Done,
    /// It ran and failed, as its events say.
    Failed,
    /// The message never reached the thread.
    Refused,
}

/// Whether a coding reply runs Coder now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// As the settings say (`coder.start`).
    Settings,
    /// Only show the offer.
    OfferOnly,
    /// Run it, whatever the settings say.
    Now,
}

/// Stops one thread's Coder task, or sends it a message, from outside the
/// operation following it ([`Client::stopper`]).
#[derive(Clone)]
pub struct Stopper {
    coder: Arc<dyn Coder>,
    store: PathBuf,
}

impl Stopper {
    /// Read a task's events and real progress on a connection of its own.
    pub fn follow(&self, task: &str, thread: &str) -> Box<dyn Follow> {
        self.coder.follow(&self.store, task, thread, None)
    }

    /// Ask `task` to stop, in the words `chat stop` uses. Blocking.
    ///
    /// # Errors
    /// Why it could not be asked.
    pub fn stop(&self, task: &str) -> Result<String, String> {
        self.coder
            .stop(&self.store, task)
            .map(|()| format!("Asked Coder to stop task {task}. Its turn ends as stopped."))
    }

    /// Send `text` to `task` while it works ([`Coder::steer`]). Blocking.
    ///
    /// # Errors
    /// Why it could not be sent.
    pub fn steer(&self, task: &str, text: &str) -> Result<Steering, String> {
        self.coder.steer(&self.store, task, text)
    }
}

/// One operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Send `text` to the thread (a `new` one is created first) and stream
    /// its reply; a coding reply then starts Coder as `start` says.
    Send {
        thread: String,
        new: bool,
        text: String,
        start: Start,
        timeout: Duration,
    },
    /// Run Coder for the thread's last offer.
    RunCoder { thread: String },
    /// Run the command the thread's last reply proposed, confirmed.
    RunCommand { thread: String },
    /// Read `text` as a background rule on this computer (#10157), as
    /// the terminal's `/background WORDS` asks: the rule and its dry run
    /// are shown, and [`Op::RunCommand`] saves it.
    Standing { thread: String, text: String },
    /// Replay the thread's task from its first event and keep streaming.
    Follow { thread: String },
    /// Stop the thread's running task.
    Stop { thread: String },
    /// Answer the task's question and follow the turn it starts.
    Answer { thread: String, text: String },
}

impl Op {
    /// The thread the operation is about.
    #[must_use]
    pub fn thread(&self) -> &str {
        match self {
            Op::Send { thread, .. }
            | Op::RunCoder { thread }
            | Op::RunCommand { thread }
            | Op::Follow { thread }
            | Op::Stop { thread }
            | Op::Answer { thread, .. }
            | Op::Standing { thread, .. } => thread,
        }
    }
}

/// An operation running on its own task ([`Client::stream`]).
pub struct Stream {
    /// Its events, in order; the channel closes when it ends.
    pub events: mpsc::UnboundedReceiver<Event>,
    /// The client back, and how the operation ended.
    pub done: tokio::task::JoinHandle<(Client, Result<Ended, Error>)>,
}

enum Backend {
    Host {
        link: Box<dyn Host>,
        socket: PathBuf,
    },
    Local {
        chats: Box<BasicChats>,
        scratch: bool,
        home: PathBuf,
    },
    /// Another computer's host, over NIP-HOST `thread.*` as a paired phone
    /// reads it ([`Client::over_computer`]). Its Coder runs there.
    Computer { link: Box<dyn Host>, label: String },
}

/// A chat client over one backend.
pub struct Client {
    backend: Backend,
    coder: Arc<dyn Coder>,
    caller: Caller,
    dir: Option<PathBuf>,
    interrupt: Interrupt,
    hint: Option<Hint>,
    /// The route record of the message this operation serves (#10207):
    /// written to the thread's journal at each move, and settled with the
    /// task owner's dispositions when the operation ends.
    routing: Option<RouteRecord>,
    /// Jev's issue judgment of the message being sent, started beside the
    /// router's (#10279).
    issue_ahead: Option<IssueAhead>,
}

/// Jev's judgment of whether a message asks to work a GitHub issue,
/// started when the message is sent instead of after the router's reply,
/// so a Coder start waits on one Jev round trip, not two in a row
/// (#10279). It is used only for the request and conversation it was
/// asked of; a start that reads the thread differently asks again.
struct IssueAhead {
    request: String,
    earlier: String,
    judged: tokio::task::JoinHandle<Option<Box<dyn Issue>>>,
}

impl Client {
    /// The backend `options` choose. `thread` names the scratch store, and
    /// `new` creates one. On a host, the threads kept without one move into
    /// it first, which `sink` hears about.
    ///
    /// # Errors
    /// A scratch place without a thread, a scratch thread that is gone, a
    /// named socket nothing answers at, or a store that cannot open.
    pub async fn open(
        options: Options,
        dial: &dyn Dial,
        coder: Arc<dyn Coder>,
        thread: Option<&str>,
        new: bool,
        sink: &mut Sink<'_>,
    ) -> Result<Self, Error> {
        let Options {
            place,
            caller,
            dir,
            home,
            interrupt,
            hint,
        } = options;
        let finish = |backend| {
            Self {
                backend,
                coder: coder.clone(),
                caller,
                dir: dir.clone(),
                interrupt: interrupt.clone(),
                hint,
                routing: None,
                issue_ahead: None,
            }
            .warmed()
        };
        let named = match place {
            Place::Scratch => {
                let id = thread.ok_or_else(|| Error::Usage("--scratch needs a thread".into()))?;
                let home = scratch_dir(id);
                if !new && !home.join("device.key").exists() {
                    return Err(failed(format!(
                        "no scratch thread {id} on this computer ({} is gone)",
                        home.display()
                    )));
                }
                let context = coder.context(&home.join("tasks"), dir.as_deref());
                return Ok(finish(local(home, true, context, caller)?));
            }
            Place::Local => None,
            Place::Auto { socket } => Some(socket),
        };
        if let Some(named) = named
            && let Some(socket) = named.clone().or_else(|| dial.socket())
        {
            match dial.dial(&socket).await {
                Some(link) => {
                    let mut backend = Backend::Host { link, socket };
                    migrate(&mut backend, &home, sink).await;
                    return Ok(finish(backend));
                }
                None if named.is_some() => {
                    return Err(failed(format!(
                        "no host answers at {}; start it with `openagents host serve --control` \
                         or open the OpenAgents app",
                        socket.display()
                    )));
                }
                None => {}
            }
        }
        let context = coder.context(&coder.default_store(), dir.as_deref());
        Ok(finish(local(home, false, context, caller)?))
    }

    /// A client over the service `chats` in this process, whose door the
    /// caller chose (a test's scripted worker, say). The context comes from
    /// `coder`, as [`Client::open`] sets it.
    pub fn in_process(
        chats: BasicChats,
        home: PathBuf,
        scratch: bool,
        options: Options,
        coder: Arc<dyn Coder>,
    ) -> Self {
        let mut chats = Box::new(chats);
        let store = if scratch {
            home.join("tasks")
        } else {
            coder.default_store()
        };
        chats.set_context(surfaced(
            coder.context(&store, options.dir.as_deref()),
            options.caller,
        ));
        Self {
            backend: Backend::Local {
                chats,
                scratch,
                home,
            },
            coder,
            caller: options.caller,
            dir: options.dir,
            interrupt: options.interrupt,
            hint: options.hint,
            routing: None,
            issue_ahead: None,
        }
        .warmed()
    }

    /// A client over a host connection the caller made.
    pub fn over_host(
        link: Box<dyn Host>,
        socket: PathBuf,
        options: Options,
        coder: Arc<dyn Coder>,
    ) -> Self {
        Self {
            backend: Backend::Host { link, socket },
            coder,
            caller: options.caller,
            dir: options.dir,
            interrupt: options.interrupt,
            hint: options.hint,
            routing: None,
            issue_ahead: None,
        }
        .warmed()
    }

    /// A client over another computer's threads: `link` carries the
    /// service commands as NIP-HOST `thread.*` operations
    /// (`openagents_chat_app::host_threads::Remote`), and `label` names the
    /// computer. Its Coder runs there; this computer starts none.
    pub fn over_computer(
        link: Box<dyn Host>,
        label: String,
        options: Options,
        coder: Arc<dyn Coder>,
    ) -> Self {
        Self {
            backend: Backend::Computer { link, label },
            coder,
            caller: options.caller,
            dir: options.dir,
            interrupt: options.interrupt,
            hint: options.hint,
            routing: None,
            issue_ahead: None,
        }
    }

    /// This client, once it asked Coder to make its project's first start
    /// quick ([`Coder::warm`]). A scratch thread's store is its own, and
    /// is left alone.
    fn warmed(self) -> Self {
        if let Some(dir) = &self.dir
            && !matches!(self.backend, Backend::Local { scratch: true, .. })
        {
            self.coder.warm(&self.coder.default_store(), dir);
        }
        self
    }

    /// Which backend holds the threads.
    pub fn kind(&self) -> Kind {
        match &self.backend {
            Backend::Host { .. } => Kind::Host,
            Backend::Local { scratch: true, .. } => Kind::Scratch,
            Backend::Local { .. } => Kind::InProcess,
            Backend::Computer { .. } => Kind::Computer,
        }
    }

    /// Where: the host's socket, or the store's home.
    pub fn place(&self) -> String {
        match &self.backend {
            Backend::Host { socket, .. } => socket.display().to_string(),
            Backend::Local { home, .. } => home.display().to_string(),
            Backend::Computer { label, .. } => label.clone(),
        }
    }

    /// Who sends this client's turns.
    pub fn caller(&self) -> Caller {
        self.caller
    }

    /// The task store the thread's Coder runs use: a scratch thread's own,
    /// else [`Coder::default_store`].
    pub fn store(&self, thread: &str) -> PathBuf {
        match &self.backend {
            Backend::Local { scratch: true, .. } => scratch_dir(thread).join("tasks"),
            _ => self.coder.default_store(),
        }
    }

    /// What stops the thread's Coder task while another operation follows
    /// it: OpenAgents Terminal's Esc during a run, which stops the run
    /// rather than the following (#10111). The follow then reads the
    /// task's `stopped` event and ends.
    pub fn stopper(&self, thread: &str) -> Stopper {
        Stopper {
            coder: self.coder.clone(),
            store: self.store(thread),
        }
    }

    /// Whether a coding reply waits to be accepted (`coder.start`).
    pub fn asks_first(&self) -> bool {
        self.coder.asks_first()
    }

    /// One service command.
    ///
    /// # Errors
    /// The service's or the host's refusal, in its words.
    pub async fn apply(&mut self, command: Command) -> Result<Snapshot, String> {
        match &mut self.backend {
            Backend::Host { link, .. } | Backend::Computer { link, .. } => {
                link.apply(command, self.caller).await
            }
            Backend::Local { chats, .. } => service::apply(chats, command, now()),
        }
    }

    /// The whole thread, read page by page through the shared reader.
    ///
    /// # Errors
    /// The service's refusal, a missing thread, or one that kept changing.
    pub async fn collect(&mut self, id: &str) -> Result<Thread, Error> {
        let handle = tokio::runtime::Handle::current();
        tokio::task::block_in_place(|| {
            crate::thread::collect(id, |command| handle.block_on(self.apply(command)))
        })
        .map_err(failed)
    }

    /// Up to `limit` threads, newest first (`all` includes archived ones),
    /// and how many the store holds.
    ///
    /// # Errors
    /// The service's refusal.
    pub async fn threads(
        &mut self,
        all: bool,
        limit: usize,
    ) -> Result<(Vec<Summary>, usize), Error> {
        let first = self.apply(Command::List {}).await.map_err(failed)?;
        let mut rows: Vec<Summary> = first.chats.clone();
        let mut after = first.list_start + first.chats.len();
        while rows.iter().filter(|row| all || !row.archived).count() < limit
            && after < first.list_total
        {
            let page = self
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
        let rows = rows
            .into_iter()
            .filter(|row| all || !row.archived)
            .take(limit)
            .collect();
        Ok((rows, first.list_total))
    }

    /// Carry the thread's Coder change into the checkout it was made from
    /// (#10343).
    ///
    /// # Errors
    /// The thread has no Coder task, or the change could not be applied.
    pub fn apply_coder(&self, id: &str, thread: &Thread) -> Result<(PathBuf, Vec<String>), String> {
        let Some(coder) = &thread.summary.coder else {
            return Err("This thread has no Coder work to apply.".into());
        };
        self.coder.apply(&self.store(id), &coder.task)
    }

    /// How each of the thread's Coder turns ended, when its task store on
    /// this computer holds them: what `chat read` shows (#10332).
    pub fn coder_endings(&self, id: &str, thread: &Thread) -> Vec<CoderEvent> {
        let Some(coder) = &thread.summary.coder else {
            return Vec::new();
        };
        self.coder.endings(&self.store(id), &coder.task)
    }

    /// The thread's Coder task, every turn's trajectory, when its task
    /// store on this computer holds it: carried inside the thread's export.
    pub fn trajectories(&self, id: &str, thread: &Thread) -> Vec<Value> {
        let Some(coder) = &thread.summary.coder else {
            return Vec::new();
        };
        self.coder.trajectories(&self.store(id), &coder.task)
    }

    /// Run `op`, telling `sink` what happens, until it ends.
    ///
    /// # Errors
    /// The backend failed while a reply streamed.
    pub async fn run(&mut self, op: Op, sink: &mut Sink<'_>) -> Result<Ended, Error> {
        let thread = op.thread().to_owned();
        self.routing = None;
        self.issue_ahead = None;
        let ended = self.operate(op, sink).await;
        self.issue_ahead = None;
        self.settle(&thread).await;
        ended
    }

    async fn operate(&mut self, op: Op, sink: &mut Sink<'_>) -> Result<Ended, Error> {
        match op {
            Op::Send {
                thread,
                new,
                text,
                start,
                timeout,
            } => {
                let run = match start {
                    Start::Now => true,
                    Start::OfferOnly => false,
                    Start::Settings => !self.coder.asks_first(),
                };
                self.send(&thread, new, &text, run, timeout, sink).await
            }
            Op::RunCoder { thread } => {
                let ended = self.run_coder(&thread, sink).await;
                let plugin = match ended {
                    Ended::Done => self.last_plugin(&thread).await,
                    _ => None,
                };
                Ok(match plugin {
                    Some(flow) => self.plugin(&thread, flow, sink).await,
                    None => ended,
                })
            }
            Op::RunCommand { thread } => Ok(self.confirmed(&thread, sink).await),
            Op::Standing { thread, text } => {
                if let Backend::Computer { label, .. } = &self.backend {
                    let message = format!("Background rules for this thread are made on {label}.");
                    sink(Event::Failure { thread, message });
                    return Ok(Ended::Failed);
                }
                Ok(self.standing(&thread, &text, sink).await)
            }
            Op::Follow { thread } | Op::Stop { thread } | Op::Answer { thread, .. }
                if matches!(self.backend, Backend::Computer { .. }) =>
            {
                Ok(self.elsewhere(&thread, sink).await)
            }
            Op::Follow { thread } => Ok(match self.bound(&thread, sink).await {
                Some(task) => {
                    self.routing = self.journal(&thread).of_task(&thread, &task);
                    self.follow_from(&thread, &task, 1, false, sink).await
                }
                None => Ended::Failed,
            }),
            Op::Stop { thread } => Ok(self.stop(&thread, sink).await),
            Op::Answer { thread, text } => Ok(match self.bound(&thread, sink).await {
                Some(task) => {
                    self.routing = self.journal(&thread).of_task(&thread, &task);
                    self.answer_task(&thread, &task, &text, sink).await
                }
                None => Ended::Failed,
            }),
        }
    }

    /// Run `op` on its own task, its events on a channel: the event-stream
    /// form of [`Client::run`]. Needs a multi-threaded runtime.
    pub fn stream(self, op: Op) -> Stream {
        let (sender, events) = mpsc::unbounded_channel();
        let done = tokio::spawn(async move {
            let mut client = self;
            let mut sink = move |event: Event| {
                let _ = sender.send(event);
            };
            let ended = client.run(op, &mut sink).await;
            (client, ended)
        });
        Stream { events, done }
    }

    /// Tell the router what the thread's Coder run did, once its turn has
    /// ended (#10094), so the chat answers about it and a request for more
    /// work continues it. The host does this for its own threads; in this
    /// process the context says it.
    async fn carry_run(&mut self, id: &str) {
        if !matches!(self.backend, Backend::Local { .. }) {
            return;
        }
        let bound = self
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await
            .ok()
            .and_then(|snapshot| snapshot.coder)
            .filter(|coder| coder.host == LOCAL_HOST);
        let run = match bound {
            Some(coder) => self.result(id, &coder.task).await,
            None => None,
        };
        if let Backend::Local { chats, .. } = &mut self.backend {
            let mut context = chats.context().clone();
            context.coder_run = run;
            chats.set_context(context);
        }
    }

    async fn send(
        &mut self,
        id: &str,
        new: bool,
        text: &str,
        run: bool,
        timeout: Duration,
        sink: &mut Sink<'_>,
    ) -> Result<Ended, Error> {
        let mut interrupt = (self.interrupt)();
        if new {
            self.delivered(
                id,
                Command::Create {
                    chat: id.to_owned(),
                },
                &mut interrupt,
                sink,
            )
            .await
            .map_err(failed)?;
        }
        if !new {
            self.carry_run(id).await;
        }
        let request = new_id();
        let sent = self
            .delivered(
                id,
                Command::Send {
                    chat: id.to_owned(),
                    request: request.clone(),
                    text: text.to_owned(),
                },
                &mut interrupt,
                sink,
            )
            .await;
        let sent = match sent {
            Ok(sent) => sent,
            Err(message) => {
                sink(Event::Failure {
                    thread: id.to_owned(),
                    message,
                });
                return Ok(Ended::Refused);
            }
        };
        self.judge_ahead(id, &sent, run);
        sink(Event::Accepted {
            thread: id.to_owned(),
            request: request.clone(),
            new,
            backend: self.kind(),
            at: self.place(),
        });
        let mut shown = String::new();
        let mut snapshot = sent;
        let mut stopped = false;
        // Each try waits `timeout` for its reply; a try the relay never
        // took, or a read the host did not answer, is tried again after a
        // pause that doubles up to `OFFLINE_MOST`, until it goes through or
        // the person stops it. What streamed before stays on the screen.
        let mut offline = 0u32;
        let mut down = 0u32;
        let mut outage = Outage::default();
        'tries: loop {
            let deadline = tokio::time::Instant::now() + timeout;
            while snapshot.busy {
                tokio::select! {
                    () = tokio::time::sleep_until(deadline) => { stopped = true; }
                    () = &mut interrupt => { stopped = true; }
                    () = tokio::time::sleep(POLL) => {}
                }
                if stopped {
                    snapshot = self
                        .apply(Command::Stop {
                            chat: id.to_owned(),
                        })
                        .await
                        .map_err(failed)?;
                    break 'tries;
                }
                let read = self
                    .apply(Command::Read {
                        chat: id.to_owned(),
                        before: None,
                    })
                    .await;
                snapshot = match read {
                    Ok(read) => read,
                    // The host stopped answering while the reply streams:
                    // it keeps answering, so read again once it is back.
                    Err(_) if !matches!(self.backend, Backend::Local { .. }) => {
                        down += 1;
                        if !Self::pause(id, down, &mut outage, &mut interrupt, sink).await {
                            stopped = true;
                            break 'tries;
                        }
                        continue;
                    }
                    Err(message) => return Err(failed(message)),
                };
                // Reached again: the host answered, and the relay took the
                // job (a partial came, or the reply ended another way).
                let took = !snapshot.partial.is_empty() || (!snapshot.busy && !snapshot.offline);
                if std::mem::take(&mut down) > 0 && (offline == 0 || took) || offline > 0 && took {
                    offline = 0;
                    outage.over(id, sink);
                }
                if snapshot.busy && snapshot.partial != shown {
                    let delta = snapshot
                        .partial
                        .strip_prefix(shown.as_str())
                        .map(str::to_owned);
                    sink(Event::Partial {
                        thread: id.to_owned(),
                        text: snapshot.partial.clone(),
                        delta,
                    });
                    shown.clone_from(&snapshot.partial);
                }
            }
            if !snapshot.offline {
                break;
            }
            // The relay could not be reached: ask again after a pause.
            offline += 1;
            if !Self::pause(id, offline, &mut outage, &mut interrupt, sink).await {
                stopped = true;
                break;
            }
            snapshot = self
                .apply(Command::Retry {
                    chat: id.to_owned(),
                })
                .await
                .map_err(failed)?;
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
        match reply {
            Some(reply) if !reply.stopped => {
                let plugin = reply.meta.as_ref().and_then(|meta| meta.plugin.clone());
                let standing = reply
                    .meta
                    .as_ref()
                    .is_some_and(crate::router::Meta::standing);
                // Say who will run it, from what the run itself reads here.
                let mut turns = [reply];
                let store = self.store(id);
                crate::delegation::attach_runner(&mut turns, snapshot.computer, |engine| {
                    self.coder.predict(&store, engine)
                });
                let [reply] = turns;
                // One route policy reads the reply (#10207): its route
                // result and admission are recorded with the thread
                // before anything runs.
                let routed = self
                    .route(
                        id,
                        &request,
                        text,
                        &reply,
                        snapshot.computer,
                        run,
                        &snapshot,
                    )
                    .await;
                let coding = routed.family() == RouteFamily::Coder;
                let command = match &routed {
                    RouteResult::LocalCommand {
                        action: LocalAction::Command { argv, .. },
                    } => Some(argv.clone()),
                    _ => None,
                };
                sink(Event::Reply {
                    thread: id.to_owned(),
                    reply: Box::new(reply),
                    computer: snapshot.computer,
                    running: coding && run,
                    route: Some(routed.family()),
                });
                // The reply arrived. The router judged this is coding:
                // Coder runs here at once, unless the person asked only
                // for the offer, and the reply then succeeds only if Coder
                // does.
                if coding && run {
                    // A plan starts runs of its own (#10183); it never
                    // steers the thread's earlier run.
                    let planned = crate::delegation::plan(&snapshot.turns).is_some();
                    if !planned
                        && let Some(ended) = self.steer_working(id, &snapshot, text, sink).await
                    {
                        return Ok(ended);
                    }
                    let ended = self.run_coder(id, sink).await;
                    return Ok(match plugin {
                        Some(flow) if ended == Ended::Done => self.plugin(id, flow, sink).await,
                        _ => ended,
                    });
                }
                // A step of making a plugin that runs here (#10177).
                if let Some(flow) = plugin
                    && !coding
                    && !matches!(self.backend, Backend::Computer { .. })
                {
                    return Ok(self.plugin(id, flow, sink).await);
                }
                // A standing rule (#10157): this computer compiles the
                // message into a rule and shows it with its dry run; it is
                // saved only once confirmed.
                if !coding && standing && !matches!(self.backend, Backend::Computer { .. }) {
                    return Ok(self.standing(id, text, sink).await);
                }
                // A command the reply proposed runs here: at once when it
                // only reads, else after a confirm (#10170).
                if !coding
                    && let Some(argv) = command
                    && !matches!(self.backend, Backend::Computer { .. })
                {
                    return Ok(self.command(id, argv, run, false, sink).await);
                }
                Ok(Ended::Done)
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
                sink(Event::ReplyFailed {
                    thread: id.to_owned(),
                    message,
                    stopped: stopped || reply.is_some(),
                    partial: reply.map(|reply| reply.text),
                });
                Ok(Ended::Failed)
            }
        }
    }

    /// The router judged `text` more work while the thread's Coder run on
    /// this computer still works: the run takes it, as the run view's
    /// composer sends it (at its next step, or as the turn it starts), and
    /// the client follows. Following alone would leave the message unread
    /// by the session it continues. `None` when there is no such run (a
    /// run that ended continues through [`Client::start`]) or it cannot
    /// take a message here, which then follows it as before.
    async fn steer_working(
        &mut self,
        id: &str,
        snapshot: &Snapshot,
        text: &str,
        sink: &mut Sink<'_>,
    ) -> Option<Ended> {
        if matches!(self.backend, Backend::Computer { .. }) {
            return None;
        }
        let coder = snapshot
            .coder
            .as_ref()
            .filter(|coder| coder.host == LOCAL_HOST)?;
        if self.result(id, &coder.task).await.is_some() {
            return None;
        }
        let (runner, store) = (self.coder.clone(), self.store(id));
        let (task, message) = (coder.task.clone(), text.to_owned());
        let steered = tokio::task::spawn_blocking(move || runner.steer(&store, &task, &message))
            .await
            .ok()?
            .ok()?;
        self.dispatched(id, &coder.task, None);
        let (said, turn) = match steered {
            Steering::NextStep => ("Sent. Coder reads it at its next step.", 1),
            Steering::NextTurn(turn) => ("Sent. Coder starts its next turn with it.", turn),
        };
        coder_report(sink, id, true, said, serde_json::to_value(coder).ok());
        Some(self.follow_from(id, &coder.task, turn, false, sink).await)
    }

    /// This computer's route journal for the thread `id`, beside its task
    /// store ([`Journal::beside`]).
    fn journal(&self, id: &str) -> Journal {
        Journal::beside(&self.store(id))
    }

    /// Keep the current route record in the thread's journal. Best effort:
    /// a journal that cannot be written never stops the chat.
    fn keep(&self, id: &str) {
        if let Some(record) = &self.routing {
            let _ = self.journal(id).write(record);
        }
    }

    /// One of the router's own moves on the current record, kept.
    fn step(&mut self, id: &str, to: Lifecycle, cause: &str) {
        if let Some(record) = &mut self.routing
            && record.step(to, cause, crate::route::now_ms()).is_ok()
        {
            self.keep(id);
        }
    }

    /// The current record is admitted (`cause`), from a route or an offer.
    fn admitted(&mut self, id: &str, cause: &str) {
        if self
            .routing
            .as_ref()
            .is_some_and(|record| matches!(record.state, Lifecycle::Received | Lifecycle::Proposed))
        {
            self.step(id, Lifecycle::Admitted, cause);
        }
    }

    /// The current route ended without running.
    fn refused(&mut self, id: &str, reason: Option<RefusalReason>, cause: &str) {
        if let Some(record) = &mut self.routing
            && record.state.router_owned()
            && record.refuse(reason, cause, crate::route::now_ms()).is_ok()
        {
            self.keep(id);
        }
    }

    /// `task` serves the current route: admitted if it was not yet, and the
    /// task named once in its record.
    fn dispatched(&mut self, id: &str, task: &str, engine: Option<&str>) {
        let offered = self
            .routing
            .as_ref()
            .is_some_and(|record| record.state == Lifecycle::Proposed);
        self.admitted(id, if offered { "accepted" } else { "autostart" });
        if let Some(record) = &mut self.routing
            && record.dispatched(task, engine).is_ok()
        {
            self.keep(id);
        }
    }

    /// The contract's word for where this client's messages come from.
    fn surface(&self) -> route_contract::snapshot::Surface {
        use route_contract::snapshot::Surface as To;
        match (self.caller.client, self.caller.surface) {
            (Some(ClientWord::Terminal), _) => To::Terminal,
            (Some(ClientWord::Cli), _) => To::Cli,
            (Some(ClientWord::Desktop), _) | (None, crate::router::Surface::Desktop) => To::Desktop,
            (Some(ClientWord::Mobile), _) | (None, crate::router::Surface::Phone) => To::Phone,
            (Some(ClientWord::Web), _) | (None, crate::router::Surface::Web) => To::Web,
            (None, crate::router::Surface::Terminal) => To::Terminal,
        }
    }

    /// The thread's Coder task on this computer, as a continuation reads it.
    async fn bound_here(&self, id: &str, snapshot: &Snapshot) -> Option<Bound> {
        if matches!(self.backend, Backend::Computer { .. }) {
            return None;
        }
        let coder = snapshot
            .coder
            .as_ref()
            .filter(|coder| coder.host == LOCAL_HOST)?;
        let working = self.result(id, &coder.task).await.is_none();
        let (runner, store, task) = (self.coder.clone(), self.store(id), coder.task.clone());
        let revision = tokio::task::spawn_blocking(move || runner.observe(&store, &task))
            .await
            .ok()
            .flatten()
            .map(|seen| seen.revision);
        Some(Bound {
            task: coder.task.clone(),
            working,
            revision,
        })
    }

    /// Route the reply to the message `request` (`text`) through the shared
    /// route policy (#10207): its route result and admission snapshot
    /// become the thread's route record, kept before anything runs, and the
    /// router's first move is made: a Coder route that starts at once is
    /// admitted, one that waits for the person is an offer, an answer is
    /// delivered. A command moves when it runs ([`Client::command`]).
    #[allow(clippy::too_many_arguments)]
    async fn route(
        &mut self,
        id: &str,
        request: &str,
        text: &str,
        reply: &Turn,
        computer_lane: bool,
        run: bool,
        snapshot: &Snapshot,
    ) -> RouteResult {
        use route_contract::route::PluginRoute;
        let elsewhere = matches!(self.backend, Backend::Computer { .. });
        let bound = self.bound_here(id, snapshot).await;
        let mut situation = Situation {
            surface: self.surface(),
            caller: format!("local:{}", self.caller.client_word()),
            request: request.to_owned(),
            thread: Some(id.to_owned()),
            computer: match &self.backend {
                Backend::Computer { label, .. } => label.clone(),
                _ => crate::route::THIS_COMPUTER.to_owned(),
            },
            project: None,
            ready: false,
            bound,
            // Local runs have no frozen suites yet: their results are
            // labeled unchecked, never verified.
            check: route_contract::snapshot::CheckScope::ExecutorExit,
        };
        let reading = Reading {
            meta: reply.meta.as_ref(),
            computer_lane,
            text,
            reply: &reply.text,
        };
        let coder = self.coder.clone();
        let result = crate::route::propose(&reading, &situation, &|argv| coder.effect(argv));
        if result.family() == RouteFamily::Coder && !elsewhere {
            let (coder, store, dir) = (self.coder.clone(), self.store(id), self.dir.clone());
            if let Ok(context) =
                tokio::task::spawn_blocking(move || coder.context(&store, dir.as_deref())).await
            {
                situation.ready = context.computer_ready;
                situation.project =
                    context
                        .project
                        .map(|project| route_contract::snapshot::WorkspaceBinding {
                            project: project.name,
                            path: project.path,
                        });
            }
        }
        let parent = match &result {
            RouteResult::Coder { plan } => plan
                .runs
                .iter()
                .find_map(|run| run.continuation.as_ref())
                .and_then(|continuation| self.journal(id).of_task(id, &continuation.task))
                .map(|record| record.snapshot),
            _ => None,
        };
        let admission = crate::route::admit(
            &result,
            &situation,
            reply.meta.as_ref(),
            text,
            parent.as_ref(),
        );
        self.routing = RouteRecord::received(
            request,
            Some(id.to_owned()),
            result.clone(),
            admission,
            crate::route::now_ms(),
        )
        .ok()
        .map(|mut record| {
            // BYOK (#10176): the keys that paid for this message, by
            // fingerprint only.
            record.payer_keys = reply
                .meta
                .as_ref()
                .map(|meta| {
                    meta.payer_keys
                        .iter()
                        .map(|key| route_contract::record::PayerKey {
                            provider: key.provider.word().to_owned(),
                            fingerprint: key.fingerprint.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            record
        });
        self.keep(id);
        match &result {
            RouteResult::Coder { .. } if run && !elsewhere => {
                self.step(id, Lifecycle::Admitted, "autostart");
            }
            RouteResult::LocalCommand {
                action: LocalAction::Command { .. },
            } if !elsewhere => {}
            RouteResult::Coder { .. }
            | RouteResult::LocalCommand { .. }
            | RouteResult::StandingRule { .. }
            | RouteResult::MissingCapability { .. }
            | RouteResult::Plugin {
                plugin: PluginRoute::Run { .. },
            } => self.step(id, Lifecycle::Proposed, "offer"),
            RouteResult::Refusal { reason } => self.refused(id, Some(*reason), "refused"),
            RouteResult::Answer { .. }
            | RouteResult::Clarification { .. }
            | RouteResult::Plugin {
                plugin: PluginRoute::Create { .. },
            } => self.step(id, Lifecycle::Completed, "answered"),
        }
        result
    }

    /// The record of the thread's last message, for an operation that
    /// acts on its reply (`run-coder`, a confirmed command): from the
    /// journal, or routed now for a thread from before the journal.
    async fn last_routed(&mut self, id: &str, snapshot: &Snapshot) {
        if self.routing.is_some() {
            return;
        }
        let Some(at) = snapshot
            .turns
            .iter()
            .rposition(|turn| turn.role == Role::User && turn.request.is_some())
        else {
            return;
        };
        let asked = snapshot.turns[at].clone();
        let request = asked.request.clone().unwrap_or_default();
        if let Some(record) = self.journal(id).latest(id, &request) {
            self.routing = Some(record);
            return;
        }
        if let Some(reply) = snapshot
            .turns
            .get(at + 1)
            .filter(|turn| turn.role == Role::Assistant)
            .cloned()
        {
            self.route(
                id,
                &request,
                &asked.text,
                &reply,
                snapshot.computer,
                false,
                snapshot,
            )
            .await;
        }
    }

    /// Copy the task owner's dispositions of the current route's tasks into
    /// its record, with each run's cost and wall time, and keep it: what an
    /// operation leaves when it ends, whether its runs ended or keep going.
    async fn settle(&mut self, id: &str) {
        let Some(mut record) = self.routing.take() else {
            return;
        };
        let tasks: Vec<String> = record
            .runs
            .iter()
            .filter(|run| !run.task.starts_with("command:"))
            .map(|run| run.task.clone())
            .collect();
        if !tasks.is_empty() {
            // A run's independent check follows its end (#10232): the
            // record keeps its verdict, not the moment before it.
            loop {
                let (coder, store, waiting) = (self.coder.clone(), self.store(id), tasks.clone());
                let checking = tokio::task::spawn_blocking(move || {
                    waiting.iter().any(|task| coder.checking(&store, task))
                })
                .await
                .unwrap_or(false);
                if !checking {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            let (coder, store) = (self.coder.clone(), self.store(id));
            let seen = tokio::task::spawn_blocking(move || {
                tasks
                    .into_iter()
                    .filter_map(|task| {
                        let seen = coder.observe(&store, &task)?;
                        // The engine the turn ran on: a continuation is
                        // dispatched before it is known (#10335).
                        let engine = coder.result(&store, &task).and_then(|run| run.engine);
                        Some((task, seen, engine))
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default();
            for (task, seen, engine) in seen {
                record.observe(&task, seen, crate::route::now_ms());
                if let Some(run) = record.runs.iter_mut().find(|run| run.task == task)
                    && run.engine.is_none()
                {
                    run.engine = engine;
                }
            }
        }
        let _ = self.journal(id).write(&record);
    }

    /// The command `argv` a reply proposed, as this computer's command
    /// tree permits it ([`Coder::permit`]): run now when it only reads and
    /// `now`, or when the person `confirmed` it; otherwise offered.
    async fn command(
        &mut self,
        id: &str,
        argv: Vec<String>,
        now: bool,
        confirmed: bool,
        sink: &mut Sink<'_>,
    ) -> Ended {
        // The route policy's rule for commands (13.4), on the effect this
        // computer's own command tree gives it.
        let effect = self.coder.effect(&argv);
        let runs = match effect.map(route_contract::route::Effect::run_policy) {
            None | Some(RunPolicy::NeverFromChat) => {
                let reason = match effect {
                    Some(route_contract::route::Effect::Spends) => RefusalReason::MoneyMovement,
                    Some(route_contract::route::Effect::Secret) => RefusalReason::AsksForSecret,
                    _ => RefusalReason::RouteNotAllowed,
                };
                self.refused(id, Some(reason), "never_from_chat");
                if confirmed {
                    sink(Event::Failure {
                        thread: id.to_owned(),
                        message: format!(
                            "openagents {} does not run from the chat.",
                            argv.join(" ")
                        ),
                    });
                    return Ended::Failed;
                }
                return Ended::Done;
            }
            Some(RunPolicy::AtOnce) => now || confirmed,
            Some(RunPolicy::Confirm) => confirmed,
        };
        sink(Event::Command {
            thread: id.to_owned(),
            argv: argv.clone(),
            confirm: !runs,
        });
        if !runs {
            self.step(id, Lifecycle::Proposed, "confirm");
            return Ended::Done;
        }
        self.admitted(id, if confirmed { "confirmed" } else { "at_once" });
        let began = std::time::Instant::now();
        let coder = self.coder.clone();
        let words = argv.clone();
        let ran = tokio::task::spawn_blocking(move || coder.run_command(&words))
            .await
            .unwrap_or_else(|error| Err(format!("the command stopped: {error}")));
        let (ok, output) = match ran {
            Ok(ran) => (ran.ok, ran.output),
            Err(why) => (false, why),
        };
        let wall = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
        if let Some(record) = &mut self.routing
            && record.ran(ok, Some(wall), crate::route::now_ms()).is_ok()
        {
            self.keep(id);
        }
        sink(Event::Ran {
            thread: id.to_owned(),
            argv,
            ok,
            output,
        });
        if ok { Ended::Done } else { Ended::Failed }
    }

    /// Compile `text` into a background rule on this computer (#10157):
    /// `openagents background draft` runs here at once (it only shows and
    /// keeps a draft), and when a rule waits, saving it is offered as a
    /// command to confirm ([`Op::RunCommand`]).
    async fn standing(&mut self, id: &str, text: &str, sink: &mut Sink<'_>) -> Ended {
        let draft = standing_draft(id);
        let mut argv = vec![
            "background".to_owned(),
            "draft".to_owned(),
            "--id".to_owned(),
            draft.clone(),
        ];
        if let Some(dir) = &self.dir {
            argv.push("--project".to_owned());
            argv.push(dir.display().to_string());
        }
        argv.push("--".to_owned());
        argv.push(text.to_owned());
        let ended = self.command(id, argv, true, false, sink).await;
        if ended != Ended::Done {
            return ended;
        }
        if self.coder.drafted(&draft) {
            sink(Event::Command {
                thread: id.to_owned(),
                argv: vec!["background".to_owned(), "apply".to_owned(), draft],
                confirm: true,
            });
        }
        Ended::Done
    }

    /// Run the command the thread's last reply proposed, after the person
    /// confirmed it.
    async fn confirmed(&mut self, id: &str, sink: &mut Sink<'_>) -> Ended {
        let refuse = |sink: &mut Sink<'_>, message: &str| {
            sink(Event::Failure {
                thread: id.to_owned(),
                message: message.to_owned(),
            });
            Ended::Failed
        };
        if let Backend::Computer { label, .. } = &self.backend {
            let message = format!("Commands for this thread run on {label}.");
            return refuse(sink, &message);
        }
        // A background rule drafted for this thread waits first: from
        // `/background WORDS`, or from a reply the router read as a
        // standing rule (#10157).
        let draft = standing_draft(id);
        if self.coder.drafted(&draft) {
            let argv = vec!["background".to_owned(), "apply".to_owned(), draft.clone()];
            let ended = self.command(id, argv, true, true, sink).await;
            return ended;
        }
        let argv = match self
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await
        {
            Ok(snapshot) => {
                self.last_routed(id, &snapshot).await;
                snapshot
                    .turns
                    .last()
                    .filter(|turn| turn.role == Role::Assistant)
                    .and_then(|turn| turn.meta.as_ref())
                    .and_then(|meta| meta.command.clone())
            }
            Err(message) => return refuse(sink, &message),
        };
        // A confirmed command runs once for its message (#10207).
        if self.routing.as_ref().is_some_and(|record| {
            record.family() == RouteFamily::LocalCommand && record.dispatched_any()
        }) {
            return refuse(sink, "That command already ran for this message.");
        }
        match argv {
            Some(argv) => self.command(id, argv, true, true, sink).await,
            None => refuse(
                sink,
                "OpenAgents has not proposed a command for this thread's last reply.",
            ),
        }
    }

    /// Another computer's thread: its Coder run is there, so this client
    /// says where instead of following, stopping, or answering it.
    async fn elsewhere(&mut self, id: &str, sink: &mut Sink<'_>) -> Ended {
        let label = self.place();
        let snapshot = self
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await;
        let message = match snapshot.ok().and_then(|snapshot| snapshot.coder) {
            Some(coder) => format!(
                "Coder works on this thread's task {} on {label}. Follow or stop it there, or in the OpenAgents app.",
                coder.task
            ),
            None => "This thread has not started Coder.".to_owned(),
        };
        coder_report(sink, id, true, &message, None);
        Ended::Done
    }

    /// `command`, asked again after a pause while the host does not
    /// answer (it restarted, or is restarting) until it does or the person
    /// stops waiting; a refusal in the host's own words ends it at once. A
    /// send carries its send ID, so the service takes it once however
    /// often it is asked.
    async fn delivered(
        &mut self,
        id: &str,
        command: Command,
        interrupt: &mut BoxFuture<'static, ()>,
        sink: &mut Sink<'_>,
    ) -> Result<Snapshot, String> {
        let mut down = 0u32;
        let mut outage = Outage::default();
        loop {
            let result = self.apply(command.clone()).await;
            let unanswered = match &self.backend {
                Backend::Host { link, .. } | Backend::Computer { link, .. } => link.unanswered(),
                Backend::Local { .. } => false,
            };
            if result.is_ok() {
                outage.over(id, sink);
            }
            if result.is_ok() || !unanswered {
                return result;
            }
            down += 1;
            if !Self::pause(id, down, &mut outage, interrupt, sink).await {
                return result;
            }
        }
    }

    /// Wait before try `attempt`, saying the chat cannot be reached once
    /// `outage` has lasted [`QUIET_FOR`]. `false` when the person stopped
    /// waiting.
    async fn pause(
        id: &str,
        attempt: u32,
        outage: &mut Outage,
        interrupt: &mut BoxFuture<'static, ()>,
        sink: &mut Sink<'_>,
    ) -> bool {
        let now = tokio::time::Instant::now();
        let end = now + backoff(attempt);
        let say_at = *outage.since.get_or_insert(now) + QUIET_FOR;
        if !outage.said && say_at < end {
            tokio::select! {
                () = tokio::time::sleep_until(say_at) => {}
                () = &mut *interrupt => return false,
            }
            outage.said = true;
        }
        if outage.said {
            let left = end.saturating_duration_since(tokio::time::Instant::now());
            sink(Event::Offline {
                thread: id.to_owned(),
                retry_in: u64::try_from((left.as_millis() + 500) / 1000).unwrap_or(u64::MAX),
            });
        }
        tokio::select! {
            () = tokio::time::sleep_until(end) => true,
            () = interrupt => false,
        }
    }

    /// Accept the thread's offer to run Coder: here when the client runs
    /// in a checkout, else through the host's own handoff, the one the
    /// desktop's Run Coder uses.
    /// Start, while the router judges the message just sent, the Jev work
    /// a Coder start on this computer would wait for after it (#10279):
    /// the issue judgment, and the recipe's groundwork on the prompt the
    /// start would hand over ([`Coder::ahead`]). Only when Coder runs at
    /// once (`run`), the thread has no Coder task that would take the
    /// message instead, and this client runs in a folder. A reply that
    /// routes elsewhere leaves them unused: a few Jev requests, a fraction
    /// of a cent; a start whose thread reads differently asks again.
    fn judge_ahead(&mut self, id: &str, sent: &Snapshot, run: bool) {
        if !run || sent.coder.is_some() || matches!(self.backend, Backend::Computer { .. }) {
            return;
        }
        let Some(dir) = self.dir.clone() else {
            return;
        };
        let (request, earlier) = asked_of(&sent.turns);
        if request.trim().is_empty() {
            return;
        }
        // The prompt [`Client::start`] hands over, when the thread holds no
        // more than this snapshot shows and the reply names no engine.
        if sent.start == 0 && plugin_step(&sent.turns).is_none() {
            let (prompt, _) = handoff("", &sent.turns);
            self.coder.ahead(&self.store(id), &dir, &prompt);
        }
        let coder = self.coder.clone();
        let (asked, before) = (request.clone(), earlier.clone());
        let judged = tokio::task::spawn_blocking(move || coder.issue(&asked, &before, &dir));
        self.issue_ahead = Some(IssueAhead {
            request,
            earlier,
            judged,
        });
    }

    async fn run_coder(&mut self, id: &str, sink: &mut Sink<'_>) -> Ended {
        let snapshot = match self
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await
        {
            Ok(snapshot) => snapshot,
            Err(message) => {
                coder_report(sink, id, false, &message, None);
                return Ended::Failed;
            }
        };
        if snapshot.coder.is_some() && matches!(self.backend, Backend::Computer { .. }) {
            return self.elsewhere(id, sink).await;
        }
        // The message's work already started (#10207): its record names
        // its tasks, so they are followed, never started again.
        self.last_routed(id, &snapshot).await;
        let started: Vec<String> = self
            .routing
            .as_ref()
            .filter(|record| record.family() == RouteFamily::Coder)
            .map(|record| record.tasks().into_iter().map(str::to_owned).collect())
            .unwrap_or_default();
        if let [task] = started.as_slice() {
            coder_report(
                sink,
                id,
                true,
                &format!(
                    "Following task {}; it already started for this message.",
                    task.get(..8).unwrap_or(task)
                ),
                None,
            );
            return self.follow_from(id, task, 1, false, sink).await;
        }
        if !started.is_empty() {
            coder_report(
                sink,
                id,
                true,
                &format!(
                    "Following the {} runs already started for this message.",
                    started.len()
                ),
                None,
            );
            return self.follow_many(id, &started, sink).await;
        }
        // A run this computer started that still works: follow it. One
        // whose last turn ended takes the new message as its next turn,
        // as a start does (#10094); following it would only replay what
        // is already on the screen, with nothing coming.
        let planned = crate::delegation::plan(&snapshot.turns).is_some();
        if !planned
            && let Some(coder) = &snapshot.coder
            && !(coder.host == LOCAL_HOST && self.result(id, &coder.task).await.is_some())
        {
            coder_report(
                sink,
                id,
                true,
                &format!(
                    "Following task {}.",
                    coder.task.get(..8).unwrap_or(&coder.task)
                ),
                serde_json::to_value(coder).ok(),
            );
            let task = coder.task.clone();
            self.dispatched(id, &task, None);
            return self.follow_from(id, &task, 1, false, sink).await;
        }
        let meta = snapshot
            .turns
            .iter()
            .rev()
            .find(|turn| turn.role == Role::Assistant)
            .and_then(|turn| turn.meta.as_ref());
        if !crate::delegation::offered(meta, snapshot.computer) {
            coder_report(
                sink,
                id,
                false,
                "OpenAgents has not offered to run Coder for this thread's last reply.",
                None,
            );
            return Ended::Failed;
        }
        // The project is the checkout this client runs in: Coder runs here,
        // with or without a host.
        // Another computer's thread runs Coder there.
        let checkout = match &self.backend {
            Backend::Computer { label, .. } => Err(format!("Coder runs on {label}.")),
            _ => self
                .dir
                .as_deref()
                .map(|dir| self.coder.checkout(dir))
                .unwrap_or_else(|| Err(NO_DIR.into())),
        };
        let why = match checkout {
            Ok(()) => return self.start(id, sink).await,
            Err(why) => why,
        };
        // Outside a checkout, a host with a project of its own still can.
        if let Backend::Local { .. } = self.backend {
            self.refused(id, Some(RefusalReason::HostUnavailable), "no_checkout");
            coder_report(sink, id, false, &why, None);
            return Ended::Failed;
        }
        match self
            .apply(Command::RunCoder {
                chat: id.to_owned(),
            })
            .await
        {
            Ok(snapshot) => match snapshot.coder {
                Some(coder) => {
                    // The host admitted and started it (#10207).
                    self.dispatched(id, &coder.task, None);
                    coder_report(
                        sink,
                        id,
                        true,
                        &format!("Coder started task {} on {}.", coder.task, coder.host),
                        serde_json::to_value(&coder).ok(),
                    );
                    Ended::Done
                }
                None => {
                    self.refused(id, None, "host_refused");
                    coder_report(
                        sink,
                        id,
                        false,
                        "The host did not start a Coder task.",
                        None,
                    );
                    Ended::Failed
                }
            },
            Err(message) => {
                self.refused(id, None, "host_refused");
                coder_report(sink, id, false, &message, None);
                Ended::Failed
            }
        }
    }

    /// Start Coder on this computer for the thread `id`, in the checkout
    /// the client runs in, bind the task to the thread, and follow it. It
    /// starts on the shared handoff prompt ([`handoff`]).
    async fn start(&mut self, id: &str, sink: &mut Sink<'_>) -> Ended {
        let thread = match self.collect(id).await {
            Ok(thread) => thread,
            Err(error) => {
                self.refused(id, None, "thread_unreadable");
                coder_report(sink, id, false, error.message(), None);
                return Ended::Failed;
            }
        };
        // The reply planned several runs (#10183): they start here, each
        // its own task, beside whatever the thread ran before.
        if let Some(plan) = crate::delegation::plan(&thread.turns) {
            return self.fan_out(id, &thread, plan, sink).await;
        }
        if let Some(coder) = &thread.summary.coder {
            // The thread's run ended, and the router judged the latest
            // message more work for it: Coder takes it as the task's next
            // turn, in the same worktree (#10094).
            if coder.host == LOCAL_HOST
                && self.result(id, &coder.task).await.is_some()
                && let Some(text) = thread
                    .turns
                    .iter()
                    .rev()
                    .find(|turn| turn.role == Role::User)
                    .map(|turn| turn.text.clone())
            {
                coder_report(
                    sink,
                    id,
                    true,
                    &format!(
                        "Coder continues task {} with your message.",
                        coder.task.get(..8).unwrap_or(&coder.task)
                    ),
                    serde_json::to_value(coder).ok(),
                );
                // A change to a drafted plugin, or another try at one
                // (#10177), carries what Coder is to do with it.
                let text = match last_flow(&thread.turns) {
                    Some(flow) if flow.step() == Some(crate::plugin_flow::Step::Draft) => {
                        if flow.slug.is_some() {
                            format!("{}\n\n{text}", crate::plugin_flow::REVISE)
                        } else {
                            format!("{}\n\n{text}", crate::plugin_flow::BRIEF)
                        }
                    }
                    _ => text,
                };
                let task = coder.task.clone();
                self.dispatched(id, &task, None);
                return self.answer_task(id, &task, &text, sink).await;
            }
            coder_report(
                sink,
                id,
                true,
                &format!(
                    "Coder already runs task {} for this thread; following it.",
                    coder.task
                ),
                serde_json::to_value(coder).ok(),
            );
            let task = coder.task.clone();
            self.dispatched(id, &task, None);
            return self.follow_from(id, &task, 1, false, sink).await;
        }
        let (mut prompt, requested) = handoff(&thread.summary.title, &thread.turns);
        // Coder drafting a plugin is told what a plugin is here (#10177).
        if plugin_step(&thread.turns) == Some(crate::plugin_flow::Step::Draft) {
            prompt = format!("{prompt}\n\n{}", crate::plugin_flow::BRIEF);
        }
        let Some(here) = self.dir.clone() else {
            self.refused(id, Some(RefusalReason::HostUnavailable), "no_checkout");
            coder_report(sink, id, false, NO_DIR, None);
            return Ended::Failed;
        };
        // Say at once who is starting (#10115): the start's own work, its
        // worktree and the engine's launch, comes after.
        let (coder, store) = (self.coder.clone(), self.store(id));
        let runner = tokio::task::spawn_blocking(move || coder.predict(&store, requested))
            .await
            .ok()
            .flatten();
        let engine = runner
            .as_ref()
            .and_then(Runner::provider)
            .map(str::to_owned);
        if let Some(Runner::Runs { provider, .. }) = runner {
            sink(Event::Starting {
                thread: id.to_owned(),
                engine: provider,
            });
        }
        // The router judged this is coding work; Jev now judges whether it
        // asks to work a GitHub issue, choosing among the references the
        // messages name (a bounded field read only after routing).
        let (request, earlier) = asked_of(&thread.turns);
        let issue = match self.issue_ahead.take() {
            Some(ahead) if ahead.request == request && ahead.earlier == earlier => {
                ahead.judged.await.ok().flatten()
            }
            _ => {
                let (coder, dir) = (self.coder.clone(), here.clone());
                tokio::task::spawn_blocking(move || coder.issue(&request, &earlier, &dir))
                    .await
                    .ok()
                    .flatten()
            }
        };
        if let Some(issue) = issue {
            return self.start_issue(id, &here, issue, sink).await;
        }
        let (coder, store) = (self.coder.clone(), self.store(id));
        let title = thread.summary.title.clone();
        let chat = id.to_owned();
        let started = tokio::task::spawn_blocking(move || {
            coder.start(&store, &here, &title, &prompt, &chat, requested)
        })
        .await
        .unwrap_or_else(|_| Err("Coder could not start.".into()));
        let record = match started {
            Ok(record) => record,
            Err(message) => {
                self.refused(id, None, "executor_refused");
                coder_report(sink, id, false, &message, None);
                return Ended::Failed;
            }
        };
        self.dispatched(id, &record.task, engine.as_deref());
        self.bind(id, &record, false, sink).await;
        // The task and its worktree, for `--json` and an export; the run's
        // own start line says who works (#10115).
        sink(Event::Coder {
            thread: id.to_owned(),
            accepted: true,
            message: "Coder started.".into(),
            task: Some(serde_json::json!({
                "host": LOCAL_HOST,
                "task": record.task,
                "project": record.project,
                "worktree": record.worktree,
            })),
            quiet: true,
        });
        self.follow_from(id, &record.task, 1, false, sink).await
    }

    /// Start a dispatch plan's runs (#10183): one per engine, in
    /// parallel, each pinned to its engine and read-only when the plan
    /// is; the first is bound to the thread and every task's record names
    /// the thread. Each run's events stream as the thread's Coder lines,
    /// so the terminal's rail lists every run; once all end, the thread
    /// gets each run's result, and, when the person asked for one, the
    /// chat model's combined summary of them.
    async fn fan_out(
        &mut self,
        id: &str,
        thread: &Thread,
        plan: nostr::cj_conversation::Plan,
        sink: &mut Sink<'_>,
    ) -> Ended {
        let Some(here) = self.dir.clone() else {
            self.refused(id, Some(RefusalReason::HostUnavailable), "no_checkout");
            coder_report(sink, id, false, NO_DIR, None);
            return Ended::Failed;
        };
        let title = crate::delegation::title(&thread.summary.title, &thread.turns);
        let mut starting = Vec::new();
        for &engine in &plan.runs {
            sink(Event::Starting {
                thread: id.to_owned(),
                engine: agent_word(engine).to_owned(),
            });
            let prompt =
                crate::delegation::plan_prompt(&thread.summary.title, &thread.turns, &plan, engine);
            let (coder, store, dir) = (self.coder.clone(), self.store(id), here.clone());
            let (title, chat, read_only) = (title.clone(), id.to_owned(), plan.read_only);
            starting.push(tokio::task::spawn_blocking(move || {
                let started =
                    coder.start_run(&store, &dir, &title, &prompt, &chat, engine, read_only);
                (engine, started)
            }));
        }
        let mut started = Vec::new();
        let mut refused = Vec::new();
        for (at, handle) in starting.into_iter().enumerate() {
            match handle.await {
                Ok((engine, Ok(record))) => started.push((engine, record)),
                Ok((engine, Err(why))) => refused.push((engine, why)),
                Err(_) => refused.push((plan.runs[at], "Coder could not start.".to_owned())),
            }
        }
        for (engine, why) in &refused {
            coder_report(
                sink,
                id,
                false,
                &format!("{} did not start: {why}", engine.name()),
                None,
            );
        }
        if started.is_empty() {
            self.refused(id, None, "executor_refused");
            return Ended::Failed;
        }
        for (engine, record) in &started {
            self.dispatched(id, &record.task, Some(agent_word(*engine)));
        }
        if thread.summary.coder.is_none() {
            self.bind(id, &started[0].1, false, sink).await;
        }
        let names: Vec<&str> = started.iter().map(|(engine, _)| engine.name()).collect();
        // Verb first, naming the engines, never "Started N runs" (#10212).
        let names = match names.as_slice() {
            [one] => (*one).to_owned(),
            [one, two] => format!("{one} and {two}"),
            [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
            [] => String::new(),
        };
        let kind = if plan.read_only { ", read-only" } else { "" };
        coder_report(
            sink,
            id,
            true,
            &format!("Running {names}{kind}."),
            Some(serde_json::json!(
                started
                    .iter()
                    .map(|(engine, record)| serde_json::json!({
                        "host": LOCAL_HOST,
                        "task": record.task,
                        "engine": engine.word(),
                        "project": record.project,
                        "worktree": record.worktree,
                        "read_only": plan.read_only,
                    }))
                    .collect::<Vec<_>>()
            )),
        );
        let tasks: Vec<String> = started
            .iter()
            .map(|(_, record)| record.task.clone())
            .collect();
        let ended = self.follow_many(id, &tasks, sink).await;
        if ended == Ended::Failed && self.interrupted_all(id, &tasks).await {
            return ended;
        }
        // Each run's result, in the thread, in the plan's order.
        let mut results = Vec::new();
        for (engine, record) in &started {
            let Some(run) = self.result(id, &record.task).await else {
                continue;
            };
            let how = match run.ending {
                crate::router::RunEnding::Finished => "",
                crate::router::RunEnding::Failed => " (failed)",
                crate::router::RunEnding::Stopped => " (stopped)",
                crate::router::RunEnding::Running | crate::router::RunEnding::Waiting => {
                    " (waiting)"
                }
            };
            let summary = cut_note(run.summary.trim());
            let text = format!("**{}**{how}: {summary}", engine.name());
            if let Err(why) = self
                .apply(Command::Note {
                    chat: id.to_owned(),
                    text,
                })
                .await
            {
                sink(Event::Failure {
                    thread: id.to_owned(),
                    message: why,
                });
            }
            let mut wire = run.json();
            wire["engine"] = serde_json::json!(agent_word(*engine));
            results.push(wire);
        }
        if plan.summarize && !results.is_empty() {
            return self.summarize(id, results, sink).await;
        }
        ended
    }

    /// Whether every one of `tasks` is still working: following them was
    /// interrupted, so the person left them running.
    async fn interrupted_all(&self, id: &str, tasks: &[String]) -> bool {
        for task in tasks {
            if self.result(id, task).await.is_some() {
                return false;
            }
        }
        true
    }

    /// Follow every one of `tasks` from its first event until each ends or
    /// asks, their lines interleaved as they come. An interrupt stops
    /// following, not the runs. `Done` when any run ended with a result
    /// or a question.
    async fn follow_many(&self, id: &str, tasks: &[String], sink: &mut Sink<'_>) -> Ended {
        let store = self.store(id);
        let hint = self.hint.map(|hint| hint(self.kind(), id));
        let mut follows: Vec<Option<Box<dyn Follow>>> = tasks
            .iter()
            .map(|task| Some(self.coder.follow(&store, task, id, hint.clone())))
            .collect();
        let mut running = vec![true; tasks.len()];
        let mut answered = false;
        let interrupt = (self.interrupt)();
        tokio::pin!(interrupt);
        while running.iter().any(|running| *running) {
            for at in 0..tasks.len() {
                if !running[at] {
                    continue;
                }
                let Some(mut follow) = follows[at].take() else {
                    running[at] = false;
                    continue;
                };
                let polled = tokio::task::spawn_blocking(move || {
                    let result = follow.poll();
                    (follow, result)
                })
                .await;
                let Ok((back, result)) = polled else {
                    sink(Event::Lost);
                    running[at] = false;
                    continue;
                };
                follows[at] = Some(back);
                let (lines, state) = match result {
                    Ok(polled) => polled,
                    Err(why) => {
                        sink(Event::TaskUnreadable {
                            thread: id.to_owned(),
                            task: tasks[at].clone(),
                            message: why,
                        });
                        running[at] = false;
                        continue;
                    }
                };
                for line in lines {
                    if matches!(
                        line.event,
                        CoderEvent::Result(_) | CoderEvent::Question(_) | CoderEvent::Approval(_)
                    ) {
                        answered = true;
                    }
                    sink(Event::Line(Box::new(line)));
                }
                if state != Progress::Running {
                    running[at] = false;
                }
            }
            if !running.iter().any(|running| *running) {
                break;
            }
            tokio::select! {
                () = tokio::time::sleep(CODER_POLL) => {}
                () = &mut interrupt => {
                    sink(Event::Detached { thread: id.to_owned() });
                    return Ended::Failed;
                }
            }
        }
        if answered { Ended::Done } else { Ended::Failed }
    }

    /// Ask the worker for the combined summary of a plan's ended runs
    /// (#10183) and stream it in as a reply.
    async fn summarize(&mut self, id: &str, runs: Vec<Value>, sink: &mut Sink<'_>) -> Ended {
        let asked = self
            .apply(Command::Summarize {
                chat: id.to_owned(),
                runs,
            })
            .await;
        let mut snapshot = match asked {
            Ok(snapshot) => snapshot,
            Err(message) => {
                sink(Event::Failure {
                    thread: id.to_owned(),
                    message,
                });
                return Ended::Failed;
            }
        };
        let before = snapshot.total;
        let mut shown = String::new();
        let deadline = tokio::time::Instant::now() + DEFAULT_TIMEOUT;
        while snapshot.busy && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(POLL).await;
            snapshot = match self
                .apply(Command::Read {
                    chat: id.to_owned(),
                    before: None,
                })
                .await
            {
                Ok(read) => read,
                Err(message) => {
                    sink(Event::Failure {
                        thread: id.to_owned(),
                        message,
                    });
                    return Ended::Failed;
                }
            };
            if snapshot.busy && snapshot.partial != shown {
                let delta = snapshot
                    .partial
                    .strip_prefix(shown.as_str())
                    .map(str::to_owned);
                sink(Event::Partial {
                    thread: id.to_owned(),
                    text: snapshot.partial.clone(),
                    delta,
                });
                shown.clone_from(&snapshot.partial);
            }
        }
        let reply = (snapshot.total > before)
            .then(|| snapshot.turns.last().cloned())
            .flatten()
            .filter(|turn| turn.role == Role::Assistant);
        match reply {
            Some(reply) => {
                sink(Event::Reply {
                    thread: id.to_owned(),
                    reply: Box::new(reply),
                    computer: snapshot.computer,
                    running: false,
                    route: None,
                });
                Ended::Done
            }
            None => {
                sink(Event::ReplyFailed {
                    thread: id.to_owned(),
                    message: snapshot
                        .failure
                        .clone()
                        .unwrap_or_else(|| basic_coder::Failure::Silent.describe()),
                    stopped: false,
                    partial: None,
                });
                Ended::Failed
            }
        }
    }

    /// Record the started task on the thread, so the apps show the run.
    async fn bind(&mut self, id: &str, record: &Started, issue: bool, sink: &mut Sink<'_>) {
        let bound = self
            .apply(Command::BindCoder {
                chat: id.to_owned(),
                host: LOCAL_HOST.into(),
                task: record.task.clone(),
                project: Some(record.project.clone()),
            })
            .await;
        if let Err(why) = bound {
            sink(Event::Unbound {
                thread: id.to_owned(),
                why,
                issue,
            });
        }
    }

    /// Start the issue flow on this computer for the thread `id`: claim, a
    /// worktree of the fetched default branch, the checks, and landing as
    /// the repository's policy says, all streamed as the thread's Coder
    /// events. `finish` hands the flow to a process of its own when it
    /// can, so it outlives this one.
    async fn start_issue(
        &mut self,
        id: &str,
        here: &Path,
        issue: Box<dyn Issue>,
        sink: &mut Sink<'_>,
    ) -> Ended {
        let store = self.store(id);
        let (sender, receiver) = std::sync::mpsc::channel();
        let dir = here.to_path_buf();
        let chat = id.to_owned();
        let number = issue.number();
        let picked = issue.picked();
        let flow = std::thread::spawn(move || match issue.begin(&store, &dir, &chat) {
            Ok(begun) => {
                let IssueStarted {
                    started,
                    url,
                    finish,
                } = begun;
                let _ = sender.send(Ok((started, url)));
                finish();
            }
            Err(refused) => {
                let _ = sender.send(Err(refused));
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
                let message = if number == 0 {
                    message
                } else {
                    format!("Coder did not take #{number}: {message}")
                };
                self.refused(id, None, "issue_refused");
                coder_report(sink, id, false, &message, None);
                return Ended::Failed;
            }
        };
        // The route is issue work on #number (13.5).
        if let Some(issue) = self
            .routing
            .as_ref()
            .and_then(|routed| crate::route::reissue(routed, number, crate::route::now_ms()))
        {
            self.routing = Some(issue);
            self.keep(id);
        }
        self.dispatched(id, &record.task, None);
        self.bind(id, &record, true, sink).await;
        coder_report(
            sink,
            id,
            true,
            &match &picked {
                Some(title) => format!("Picking up #{number}: {title}."),
                None => format!("Coder took issue #{number} ({url})."),
            },
            Some(serde_json::json!({
                "host": LOCAL_HOST,
                "task": record.task,
                "project": record.project,
                "worktree": record.worktree,
                "issue": number,
                "issue_url": url,
            })),
        );
        let ended = self.follow_from(id, &record.task, 1, true, sink).await;
        let _ = tokio::task::spawn_blocking(move || flow.join()).await;
        ended
    }

    /// The task bound to the thread, or a said failure.
    async fn bound(&mut self, id: &str, sink: &mut Sink<'_>) -> Option<String> {
        let snapshot = self
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
        sink(Event::Failure {
            thread: id.to_owned(),
            message,
        });
        None
    }

    /// Ask the thread's running task to stop.
    async fn stop(&mut self, id: &str, sink: &mut Sink<'_>) -> Ended {
        let Some(task) = self.bound(id, sink).await else {
            return Ended::Failed;
        };
        self.routing = self.journal(id).of_task(id, &task);
        let (coder, store, stopping) = (self.coder.clone(), self.store(id), task.clone());
        let result = tokio::task::spawn_blocking(move || coder.stop(&store, &stopping))
            .await
            .unwrap_or_else(|_| Err("Coder could not be reached.".into()));
        let (requested, message) = match result {
            Ok(()) => (
                true,
                format!("Asked Coder to stop task {task}. Its turn ends as stopped."),
            ),
            Err(why) => (false, why),
        };
        sink(Event::Stop {
            thread: id.to_owned(),
            task,
            requested,
            message,
        });
        if requested {
            Ended::Done
        } else {
            Ended::Failed
        }
    }

    /// What the thread's task's last turn did, once it ended (#10094).
    async fn result(&self, thread: &str, task: &str) -> Option<CoderRun> {
        let (coder, store, task) = (self.coder.clone(), self.store(thread), task.to_owned());
        tokio::task::spawn_blocking(move || coder.result(&store, &task))
            .await
            .ok()
            .flatten()
    }

    /// Continue `task` with `text`, an answer or the next turn, and follow
    /// the turn it starts.
    async fn answer_task(&self, id: &str, task: &str, text: &str, sink: &mut Sink<'_>) -> Ended {
        let (coder, store) = (self.coder.clone(), self.store(id));
        let (answering, text) = (task.to_owned(), text.to_owned());
        let result = tokio::task::spawn_blocking(move || coder.answer(&store, &answering, &text))
            .await
            .unwrap_or_else(|_| Err("Coder could not be reached.".into()));
        match result {
            Ok(turn) => self.follow_from(id, task, turn, false, sink).await,
            Err(message) => {
                sink(Event::Failure {
                    thread: id.to_owned(),
                    message,
                });
                Ended::Failed
            }
        }
    }

    /// Stream `task`'s events from `turn` on until it ends or asks. An
    /// interrupt stops following, not the task; in an issue flow (`flow`)
    /// it asks the flow to stop, which stops the running turn or stops
    /// before landing and says so on the issue, and the stream then shows
    /// how it ended.
    /// The plugin step of the thread's last reply, if it served one.
    async fn last_plugin(&mut self, id: &str) -> Option<crate::plugin_flow::Flow> {
        let snapshot = self
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await
            .ok()?;
        last_flow(&snapshot.turns)
    }

    /// Run the step of making a plugin that `flow`, the last reply's,
    /// leaves to this computer (#10177), and say where the plugin stands:
    ///
    /// - [`Step::Draft`](crate::plugin_flow::Step::Draft), once Coder's
    ///   run ended: the plugin it drafted and its tests, for approval.
    /// - [`Step::Run`](crate::plugin_flow::Step::Run): its tests, with the
    ///   plugin and without it, and the result, then the publish question.
    /// - [`Step::Done`](crate::plugin_flow::Step::Done): publish it, turn
    ///   it on here, both, or neither, as the person chose.
    ///
    /// Every other step waits for the person; nothing runs.
    async fn plugin(
        &mut self,
        id: &str,
        flow: crate::plugin_flow::Flow,
        sink: &mut Sink<'_>,
    ) -> Ended {
        use crate::plugin_flow::{Outcome, Step, said};
        let Some(step) = flow.step() else {
            return Ended::Done;
        };
        if !matches!(step, Step::Draft | Step::Run | Step::Done) {
            return Ended::Done;
        }
        let snapshot = match self
            .apply(Command::Read {
                chat: id.to_owned(),
                before: None,
            })
            .await
        {
            Ok(snapshot) => snapshot,
            Err(_) => return Ended::Failed,
        };
        let Some(task) = snapshot
            .coder
            .as_ref()
            .filter(|coder| coder.host == LOCAL_HOST)
            .map(|coder| coder.task.clone())
        else {
            return Ended::Done;
        };
        let store = self.store(id);
        let (coder, worktree_store, worktree_task) =
            (self.coder.clone(), store.clone(), task.clone());
        let worktree =
            tokio::task::spawn_blocking(move || coder.worktree(&worktree_store, &worktree_task))
                .await
                .ok()
                .flatten();
        let report = |sink: &mut Sink<'_>, flow: crate::plugin_flow::Flow, outcome: &Outcome| {
            let ok = !matches!(outcome, Outcome::Failed { .. } | Outcome::NoPlugin);
            sink(Event::Plugin {
                thread: id.to_owned(),
                text: said(&flow, outcome),
                flow: flow.advanced(outcome),
                ok,
            });
            if ok { Ended::Done } else { Ended::Failed }
        };
        let Some(worktree) = worktree else {
            let outcome = Outcome::Failed {
                why: format!("the worktree of Coder's task {task} is not on this computer"),
            };
            return report(sink, flow, &outcome);
        };
        match step {
            Step::Draft => {
                // A turn still going, or waiting for the person's answer,
                // has drafted nothing yet: its question shows instead.
                let Some(run) = self.result(id, &task).await else {
                    return Ended::Done;
                };
                let found =
                    crate::plugin_flow::drafted(run.files.iter().map(|file| file.path.as_str()));
                let Some((slug, _)) = found else {
                    return report(sink, flow, &Outcome::NoPlugin);
                };
                let dir = worktree.join(crate::plugin_flow::PLUGINS_DIR).join(&slug);
                let coder = self.coder.clone();
                let read = tokio::task::spawn_blocking(move || coder.plugin_tests(&dir))
                    .await
                    .unwrap_or_else(|error| Err(error.to_string()));
                let outcome = match read {
                    Ok(tests) => Outcome::Drafted { slug, tests },
                    Err(why) => Outcome::Failed {
                        why: format!(
                            "the plugin Coder drafted in {}/{slug} doesn't load: {why}",
                            crate::plugin_flow::PLUGINS_DIR
                        ),
                    },
                };
                report(sink, flow, &outcome)
            }
            Step::Run => {
                let Some(dir) = flow.dir().map(|dir| worktree.join(dir)) else {
                    return report(sink, flow, &Outcome::NoPlugin);
                };
                // The run's results go to a folder of their own: a run
                // finished only when it wrote its report there.
                let results = std::env::temp_dir()
                    .join("openagents-plugin-runs")
                    .join(format!(
                        "{id}-{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |since| since.as_millis())
                    ));
                let argv: Vec<String> = vec![
                    "plugin".into(),
                    "test".into(),
                    "run".into(),
                    dir.display().to_string(),
                    "--trust".into(),
                    "--output-dir".into(),
                    results.display().to_string(),
                ];
                sink(Event::Command {
                    thread: id.to_owned(),
                    argv: argv.clone(),
                    confirm: false,
                });
                let coder = self.coder.clone();
                let ran = tokio::task::spawn_blocking(move || {
                    coder.plugin_command(&argv, crate::plugin_flow::TEST_TIMEOUT)
                })
                .await
                .unwrap_or_else(|error| Err(error.to_string()));
                let outcome = match ran {
                    Ok(ran) if wrote_report(&results) => Outcome::Ran {
                        ok: ran.ok,
                        summary: ran.output,
                    },
                    // It stopped before a result: the step stays, and saying
                    // yes again runs it again.
                    Ok(ran) => Outcome::Failed {
                        why: format!("the tests didn't run. {}", ran.output.trim()),
                    },
                    Err(why) => Outcome::Failed { why },
                };
                report(sink, flow, &outcome)
            }
            Step::Done => {
                let (Some(slug), Some(dir)) =
                    (flow.slug.clone(), flow.dir().map(|dir| worktree.join(dir)))
                else {
                    return Ended::Done;
                };
                let dir = dir.display().to_string();
                let mut lines: Vec<String> = Vec::new();
                let mut ok = true;
                let mut steps: Vec<Vec<String>> = Vec::new();
                if flow.publish {
                    let publish = vec!["plugin".to_owned(), "publish".to_owned(), dir.clone()];
                    if self.coder.permit(&publish) == Permit::Never {
                        lines.push(format!(
                            "This build of OpenAgents can't publish a plugin to the registry yet. To publish it once it can, run: {}",
                            crate::router::Offer::command_line(&publish)
                        ));
                    } else {
                        steps.push(publish);
                    }
                }
                if flow.enable {
                    steps.push(vec!["plugin".to_owned(), "install".to_owned(), dir.clone()]);
                    steps.push(vec!["plugin".to_owned(), "enable".to_owned(), slug.clone()]);
                }
                for argv in steps {
                    sink(Event::Command {
                        thread: id.to_owned(),
                        argv: argv.clone(),
                        confirm: false,
                    });
                    let coder = self.coder.clone();
                    let words = argv.clone();
                    let ran = tokio::task::spawn_blocking(move || {
                        coder.plugin_command(&words, crate::plugin_flow::STEP_TIMEOUT)
                    })
                    .await
                    .unwrap_or_else(|error| Err(error.to_string()));
                    let (done, output) = match ran {
                        Ok(ran) => (ran.ok, ran.output),
                        Err(why) => (false, why),
                    };
                    sink(Event::Ran {
                        thread: id.to_owned(),
                        argv: argv.clone(),
                        ok: done,
                        output: output.clone(),
                    });
                    if !done {
                        ok = false;
                        lines.push(format!(
                            "{} failed.",
                            crate::router::Offer::command_line(&argv)
                        ));
                        break;
                    }
                }
                // The reply already ended with the done line; say what ran.
                if ok {
                    if flow.publish && !lines.iter().any(|line| line.contains("can't publish")) {
                        lines.push("It's published.".to_owned());
                    }
                    if flow.enable {
                        lines.push("It's on on this computer.".to_owned());
                    }
                    if lines.is_empty() {
                        lines.push("Nothing else to do.".to_owned());
                    }
                }
                sink(Event::Plugin {
                    thread: id.to_owned(),
                    flow,
                    text: lines.join("\n"),
                    ok,
                });
                if ok { Ended::Done } else { Ended::Failed }
            }
            _ => Ended::Done,
        }
    }

    async fn follow_from(
        &self,
        id: &str,
        task: &str,
        turn: usize,
        flow: bool,
        sink: &mut Sink<'_>,
    ) -> Ended {
        let mut stopping = false;
        let store = self.store(id);
        let hint = self.hint.map(|hint| hint(self.kind(), id));
        let mut follow = self.coder.follow(&store, task, id, hint);
        let interrupt = (self.interrupt)();
        tokio::pin!(interrupt);
        let mut last: Option<CoderEvent> = None;
        let following = tokio::time::Instant::now();
        let mut stepped = false;
        loop {
            let polled = tokio::task::spawn_blocking(move || {
                let result = follow.poll();
                (follow, result)
            })
            .await;
            let Ok((back, result)) = polled else {
                sink(Event::Lost);
                return Ended::Failed;
            };
            follow = back;
            let (lines, state) = match result {
                Ok(polled) => polled,
                Err(why) => {
                    sink(Event::TaskUnreadable {
                        thread: id.to_owned(),
                        task: task.to_owned(),
                        message: why,
                    });
                    return Ended::Failed;
                }
            };
            for line in lines {
                if turn_of(&line.event) < turn {
                    continue;
                }
                if line.event.ends_turn() {
                    last = Some(line.event.clone());
                }
                stepped |= !matches!(line.event, CoderEvent::CoderStarted(_));
                sink(Event::Line(Box::new(line)));
            }
            if state != Progress::Running {
                break;
            }
            let pause = if stepped || following.elapsed() > STARTING_FOR {
                CODER_POLL
            } else {
                STARTING_POLL
            };
            tokio::select! {
                () = tokio::time::sleep(pause) => {}
                () = &mut interrupt, if !stopping => {
                    if flow {
                        stopping = true;
                        let (coder, store, stopped) = (self.coder.clone(), store.clone(), task.to_owned());
                        let asked = tokio::task::spawn_blocking(move || coder.stop(&store, &stopped)).await;
                        sink(Event::Stopping {
                            why: match asked {
                                Ok(Err(why)) => Some(why),
                                _ => None,
                            },
                        });
                        continue;
                    }
                    sink(Event::Detached { thread: id.to_owned() });
                    return Ended::Failed;
                }
            }
        }
        match last {
            Some(CoderEvent::Result(_) | CoderEvent::Question(_) | CoderEvent::Approval(_)) => {
                Ended::Done
            }
            _ => Ended::Failed,
        }
    }
}

const NO_DIR: &str = "This command has no working directory.";

/// The agent word a computer names `engine` by (`codex`, `claude`,
/// `grok`, `opencode`, `devin`): what a run's start and context carry.
pub fn agent_word(engine: nostr::cj_conversation::Engine) -> &'static str {
    use nostr::cj_conversation::Engine;
    match engine {
        Engine::Codex => "codex",
        Engine::ClaudeCode => "claude",
        Engine::GrokBuild => "grok",
        Engine::OpenCode => "opencode",
        Engine::Devin => "devin",
    }
}

/// At most 8 KiB of a run's summary for the thread, cut at a character.
fn cut_note(text: &str) -> String {
    const MOST: usize = 8 * 1024;
    if text.len() <= MOST {
        return text.to_owned();
    }
    let mut end = MOST;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Whether a plugin test run wrote its report (`report.json`) anywhere
/// under `dir`, which only a run that finished does (#10177).
fn wrote_report(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                wrote_report(&path)
            } else {
                path.file_name().is_some_and(|name| name == "report.json")
            }
        })
    })
}

/// The plugin step the last reply in `turns` served (#10177).
fn last_flow(turns: &[Turn]) -> Option<crate::plugin_flow::Flow> {
    turns
        .iter()
        .rev()
        .find(|turn| turn.role == Role::Assistant)
        .and_then(|turn| turn.meta.as_ref())
        .and_then(|meta| meta.plugin.clone())
}

/// The step of [`last_flow`].
fn plugin_step(turns: &[Turn]) -> Option<crate::plugin_flow::Step> {
    last_flow(turns).and_then(|flow| flow.step())
}

fn coder_report(sink: &mut Sink<'_>, id: &str, accepted: bool, message: &str, task: Option<Value>) {
    sink(Event::Coder {
        thread: id.to_owned(),
        accepted,
        message: message.to_owned(),
        task,
        quiet: false,
    });
}

/// `context` as `caller` sends it.
fn surfaced(mut context: Context, caller: Caller) -> Context {
    context.surface = caller.surface;
    context.client = caller.client;
    context
}

/// The service in this process. `context` says whether Coder can run on
/// this computer for this thread now, and that this computer and its
/// checkout are where it runs, which the router is told.
fn local(home: PathBuf, scratch: bool, context: Context, caller: Caller) -> Result<Backend, Error> {
    let secret = device_key(&home, true).map_err(failed)?;
    let store = Cache::open(&home.join("threads"), &secret)
        .map_err(|error| failed(format!("cannot open the chat store: {error}")))?;
    let relay =
        std::env::var("OPENAGENTS_CHAT_RELAY").unwrap_or_else(|_| basic_coder::RELAY.to_owned());
    let worker =
        std::env::var("OPENAGENTS_CHAT_WORKER").unwrap_or_else(|_| basic_coder::WORKER.to_owned());
    let door = basic_coder::Relay::new(&relay, &worker, secret).map_err(failed)?;
    let mut chats = BasicChats::new(
        Some(tokio::runtime::Handle::current()),
        Some(Arc::new(door)),
        Some(store),
    );
    // Coder runs on this computer when it is in a checkout and a coding
    // agent is signed in here with capacity.
    chats.set_context(surfaced(context, caller));
    Ok(Backend::Local {
        chats: Box::new(chats),
        scratch,
        home,
    })
}

/// Ask the host to take in the threads kept without one in `home`, when
/// there are any. The host reads them with the client's device key and
/// re-encrypts them in its own store, keeping every ID; a second ask is a
/// no-op. An older host, or a refusal, leaves them where they are, still
/// readable with `--local`.
async fn migrate(backend: &mut Backend, home: &Path, sink: &mut Sink<'_>) {
    let Backend::Host { link, .. } = backend else {
        return;
    };
    if !crate::migrate::pending(home) || crate::migrate::scratch(home) {
        return;
    }
    let Ok(home) = home.canonicalize() else {
        return;
    };
    match link.migrate(&home).await {
        Migration::Moved(moved) if moved > 0 => sink(Event::Migrated { moved }),
        Migration::Kept(message) => sink(Event::Kept { home, message }),
        _ => {}
    }
}

/// What a Coder run for the thread starts with: the shared handoff prompt
/// ([`crate::delegation::prompt`]), which tells the engine the routing is
/// done (#10084), and the engine the person asked for, from the reply's
/// typed offer: it goes first, and the start card says why when another
/// runs (#10076).
pub fn handoff(title: &str, turns: &[Turn]) -> (String, Option<nostr::cj_conversation::Engine>) {
    (
        crate::delegation::prompt(title, turns),
        crate::delegation::requested(turns),
    )
}

/// The thread's last message, and the conversation before it, for Jev's
/// choice of issue.
fn asked_of(turns: &[Turn]) -> (String, String) {
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

/// The turn an event belongs to.
pub fn turn_of(event: &CoderEvent) -> usize {
    match event {
        CoderEvent::CoderStarted(e) => e.turn,
        CoderEvent::Step(e) => e.turn,
        CoderEvent::Output(e) => e.turn,
        CoderEvent::ProviderSwitched(e) => e.turn,
        CoderEvent::Question(e) | CoderEvent::Approval(e) => e.turn,
        CoderEvent::Progress(e) => e.turn,
        CoderEvent::Status(e) => e.turn,
        CoderEvent::Result(e) => e.turn,
        CoderEvent::Failure(e) => e.turn,
        CoderEvent::Stopped(e) => e.turn,
    }
}

/// The pause before try `attempt` (1 for the first retry): it doubles
/// from [`OFFLINE_FIRST`] up to [`OFFLINE_MOST`].
pub fn backoff(attempt: u32) -> Duration {
    let doubled = OFFLINE_FIRST.saturating_mul(1 << attempt.saturating_sub(1).min(8));
    doubled.min(OFFLINE_MOST)
}

/// A thread or send ID: 32 lowercase hex characters, as the service admits.
pub fn thread_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// A fresh thread or send ID.
pub fn new_id() -> String {
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

/// `~/.openagents/chat` (`%USERPROFILE%\.openagents\chat` on Windows), or
/// `OPENAGENTS_CHAT_HOME`.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENAGENTS_CHAT_HOME") {
        return dir.into();
    }
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        // Windows has no `HOME`: the profile folder.
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents")
        .join("chat")
}

/// The throwaway home of one scratch thread.
pub fn scratch_dir(id: &str) -> PathBuf {
    std::env::temp_dir()
        .join("openagents-chat-scratch")
        .join(id)
}

/// The client's public identity in `home`, when it has a device key.
pub fn identity(home: &Path) -> Option<String> {
    device_key(home, false)
        .ok()
        .map(|secret| crate::public(&secret))
}

/// The client's device key in `home`, created on first use (`0600`). It is
/// never printed.
///
/// # Errors
/// The key cannot be read or created, or is not a key.
pub fn device_key(home: &Path, create: bool) -> Result<SecretKey, String> {
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

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
