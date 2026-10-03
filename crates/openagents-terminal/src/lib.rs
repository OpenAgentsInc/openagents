//! OpenAgents Terminal: a full-screen chat with OpenAgents in a terminal
//! (`docs/terminal/README.md`).
//!
//! It is a screen over the shared chat client
//! ([`openagents_chat::client`]), the one `openagents chat` uses: every
//! message goes to the chat router, a coding reply starts Coder on this
//! computer, and the client's typed events draw the transcript. It adds no
//! chat logic and no Coder logic of its own. The only parse of what the
//! person types is a leading `/word` against the closed list of slash
//! commands ([`slash::Slash`]); everything else goes to the router.
//!
//! The pieces:
//!
//! - [`app`]: the state (transcript rows, composer, overlay, the thread and
//!   its run) and what each key and each client event does to it. Pure:
//!   state in, actions and view rows out, testable without a terminal.
//! - [`draw`]: one frame, with `coder-terminal`'s components on the white
//!   ladder.
//! - [`screen`]: the input loop, which owns the terminal (through
//!   `coder-terminal`'s panic-safe guard) and runs each action against the
//!   client.
//!
//! What only this computer's host or the `openagents` program can do
//! (pairing a phone, installing the host service, listing plugins, the
//! settings) comes in through [`Extras`], so this crate links no host or
//! Coder code and its tests drive it with fakes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use openagents_chat::client::{self, Client, Coder};
use tokio::sync::watch;

pub mod app;
pub mod copy;
pub mod draw;
pub mod last;
pub mod picker;
pub mod prompts;
pub mod rail;
pub mod rows;
pub mod screen;
pub mod slash;
pub mod view;

/// A pairing invitation the host issued: what `/connect` shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invite {
    /// The host's handle on the invitation, to cancel it.
    pub invitation: String,
    /// The code a phone redeems (shown only on request; never logged).
    pub code: String,
    /// The QR code as text rows, light modules filled, so it draws in the
    /// ladder's white and scans over SSH.
    pub qr: Vec<String>,
    /// Unix seconds when the code stops working.
    pub expires_at: u64,
}

/// One row of the plugin list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    pub name: String,
    /// What it does.
    pub about: String,
    /// What [`Extras::run_plugin`] is told, when it is installed here and
    /// runs from the screen; `None` for one only published.
    pub key: Option<String>,
    /// Whether it is on on this computer, for one installed here; what
    /// [`Extras::turn_plugin`] is told is `id`.
    pub on: Option<bool>,
    /// The installed plugin's `KEY:SLUG`, or the id a published one
    /// installs by ([`Extras::install_plugin`]).
    pub id: Option<String>,
}

/// What `/settings` shows, and changes in place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// The settings file (`~/.openagents/settings.json`).
    pub path: PathBuf,
    /// Why the file cannot be changed here, when it cannot.
    pub problem: Option<String>,
    /// What the list turns on or off: when Coder starts, then each coding
    /// agent Coder may use.
    pub choices: Vec<Choice>,
    /// Who pays for model calls, in one line ("Running on OpenAgents.",
    /// "Running on your keys."), shown in the list's title.
    pub status: Option<String>,
}

/// One setting the list turns on or off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// What [`Extras::change`] is told; the screen never reads it.
    pub key: String,
    pub label: String,
    pub on: bool,
    /// Why it cannot be turned on here, when it cannot.
    pub blocked: Option<String>,
    /// A secret the person pastes to turn it on (a provider key, BYOK):
    /// Enter on it while off opens a masked field, and the value goes to
    /// [`Extras::set_secret`], never to the transcript, the prompt
    /// history, or the screen.
    pub secret: bool,
    /// The line to show while turning it on runs, when that takes a while
    /// (Connect OpenRouter waits for a sign-in in the browser).
    pub waiting: Option<String>,
}

/// What the screen needs from this computer beyond the chat client. Every
/// method may block; the screen calls them off its draw loop.
pub trait Extras: Send + Sync {
    /// A fresh single-use pairing invitation from this computer's host.
    ///
    /// # Errors
    /// No host runs, or it refused, in words for the person.
    fn invite(&self) -> Result<Invite, String>;
    /// The phone that redeemed `invite`, once one has.
    ///
    /// # Errors
    /// The host stopped answering.
    fn paired(&self, invite: &Invite) -> Result<Option<String>, String>;
    /// Cancel `invite`, so it cannot be redeemed after the screen closes it.
    fn cancel(&self, invite: &Invite);
    /// Install this computer's host as a user service, so its chats sync
    /// with a paired phone. The words to show on success.
    ///
    /// # Errors
    /// Why it could not, and what to do instead.
    fn sync(&self) -> Result<String, String>;
    /// The plugins installed on this computer, which run from the screen,
    /// then the published ones.
    ///
    /// # Errors
    /// Neither could be read.
    fn plugins(&self) -> Result<Vec<Plugin>, String>;
    /// Run the installed plugin `key` once on `folder` with `request`, what
    /// the person asked it; its reply.
    ///
    /// # Errors
    /// It did not run or did not finish, in words for the person.
    fn run_plugin(&self, key: &str, request: &str, folder: Option<&Path>)
    -> Result<String, String>;
    /// Turn the installed plugin `id` on or off on this computer; the
    /// words to show.
    ///
    /// # Errors
    /// It could not (the host refused its background rule, say), in words
    /// for the person.
    fn turn_plugin(&self, _id: &str, _on: bool) -> Result<String, String> {
        Err("Plugins cannot be turned on here.".into())
    }
    /// Install the published plugin `id` on this computer, off; the words
    /// to show.
    ///
    /// # Errors
    /// It could not be fetched, did not match its signed release, or
    /// could not be installed, in words for the person.
    fn install_plugin(&self, _id: &str) -> Result<String, String> {
        Err("Plugins cannot be installed here.".into())
    }
    /// Copy this computer's Claude Code and Codex sessions into the host's
    /// threads, each once. The words to show.
    ///
    /// # Errors
    /// No host runs, or it refused, in words for the person.
    fn import(&self) -> Result<String, String>;
    /// The Coder settings on this computer.
    fn settings(&self) -> Settings;
    /// Turn the choice `key` on or off, and the settings after.
    ///
    /// # Errors
    /// The change is not allowed (the last agent turned off, say) or the
    /// file cannot be written, in words for the person.
    fn change(&self, key: &str, on: bool) -> Result<Settings, String>;
    /// Keep the secret `value` for the choice `key` (a provider key), and
    /// the settings after. `value` is never logged or shown.
    ///
    /// # Errors
    /// The provider refused it or it cannot be kept, in words for the
    /// person.
    fn set_secret(&self, _key: &str, _value: &str) -> Result<Settings, String> {
        Err("Keys cannot be added here.".into())
    }
    /// The host's background rules (`/background`).
    ///
    /// # Errors
    /// They cannot be read here, in words for the person.
    fn background(&self) -> Result<Vec<BackgroundRow>, String> {
        Err("Background rules are not available here.".into())
    }
    /// Do `act` to the rule `id`; the lines of the card that shows it.
    ///
    /// # Errors
    /// It could not, in words for the person.
    fn background_act(&self, _id: &str, _act: BackgroundAct) -> Result<Vec<String>, String> {
        Err("Background rules are not available here.".into())
    }
    /// The newest background notification and when it was sent.
    fn background_notice(&self) -> Option<(u64, String)> {
        None
    }
    /// The efficiency report (`/efficiency`, #10210): routed against raw
    /// delegation, from recorded runs. Only here: result cards never show
    /// cost.
    ///
    /// # Errors
    /// It cannot be read here, in words for the person.
    fn efficiency(&self) -> Result<Efficiency, String> {
        Err("The efficiency report is not available here.".into())
    }
    /// The background watchers running on this computer, by name, for
    /// the welcome card.
    fn watchers(&self) -> Vec<String> {
        Vec::new()
    }
    /// The task worktrees on this computer (`/worktrees`, #10296): a row
    /// per project with how many and how much room, then its worktrees.
    ///
    /// # Errors
    /// They cannot be read here, in words for the person.
    fn worktrees(&self) -> Result<Vec<WorktreeRow>, String> {
        Err("Task worktrees are not shown here.".into())
    }
    /// Archive the worktree of the ended task `task` (what `openagents
    /// worktree archive` does); the words to show.
    ///
    /// # Errors
    /// It holds something unsaved or the task is still going, in words for
    /// the person.
    fn archive_worktree(&self, _task: &str) -> Result<String, String> {
        Err("Worktrees cannot be archived here.".into())
    }
}

/// One row of `/worktrees`: a project (no task) with its count and size,
/// or one task's worktree under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeRow {
    /// The task whose worktree this is; `None` on a project's row.
    pub task: Option<String>,
    pub label: String,
    pub detail: String,
    /// The task is over, so `a` may archive its worktree.
    pub ended: bool,
}

/// The efficiency report as `/efficiency` shows it: a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Efficiency {
    pub title: String,
    /// One row per arm: its name, then passes, cost, and time.
    pub rows: Vec<(String, String)>,
    /// The findings, wins and losses alike, and where the rest is.
    pub body: Vec<String>,
}

/// One background rule in `/background`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundRow {
    pub id: String,
    /// One plain line: on or paused, free space, last result.
    pub line: String,
    pub paused: bool,
}

/// What `/background` does to a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundAct {
    Show,
    /// What a run would delete, changing nothing.
    DryRun,
    Run,
    Pause,
    Resume,
    Log,
}

impl BackgroundAct {
    /// What the act is, for the title of what it shows.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::Show => "rule",
            Self::DryRun => "dry run",
            Self::Run => "run",
            Self::Pause | Self::Resume => "state",
            Self::Log => "log",
        }
    }
}

/// Nothing beyond the chat: every extra says it is not available here.
pub struct NoExtras;

impl Extras for NoExtras {
    fn invite(&self) -> Result<Invite, String> {
        Err("Pairing needs this computer's host; none runs here.".into())
    }
    fn paired(&self, _: &Invite) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn cancel(&self, _: &Invite) {}
    fn sync(&self) -> Result<String, String> {
        Err("This screen cannot install the host service.".into())
    }
    fn plugins(&self) -> Result<Vec<Plugin>, String> {
        Ok(Vec::new())
    }
    fn run_plugin(&self, _: &str, _: &str, _: Option<&Path>) -> Result<String, String> {
        Err("Plugins do not run here.".into())
    }
    fn import(&self) -> Result<String, String> {
        Err("Importing sessions needs this computer's host; none runs here.".into())
    }
    fn settings(&self) -> Settings {
        Settings::default()
    }
    fn change(&self, _: &str, _: bool) -> Result<Settings, String> {
        Err("The settings cannot be changed here.".into())
    }
}

/// What stops receiving a reply: [`Interrupter::interrupt`] goes in the
/// client's options, and the screen fires it on Esc while a reply streams
/// and when it closes. Each operation's interrupt waits for a fire after
/// the operation began, so an earlier Esc never stops a later reply.
#[derive(Clone)]
pub struct Interrupter(Arc<watch::Sender<u64>>);

impl Default for Interrupter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interrupter {
    pub fn new() -> Self {
        Self(Arc::new(watch::channel(0).0))
    }

    /// The client's interrupt (`client::Options::interrupt`).
    pub fn interrupt(&self) -> client::Interrupt {
        let sender = self.0.clone();
        Arc::new(move || {
            let mut receiver = sender.subscribe();
            receiver.mark_unchanged();
            Box::pin(async move {
                let _ = receiver.changed().await;
            })
        })
    }

    /// Stop the operation that is waiting now.
    pub fn fire(&self) {
        self.0.send_modify(|generation| *generation += 1);
    }
}

/// Which thread the screen opens on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resume {
    /// This thread (`--thread ID`).
    Thread(String),
    /// The last thread the screen had open in this folder, else a new one.
    LastForFolder,
    /// The thread an ID, ID prefix, or title names (`--resume ARG`), else
    /// a new one with the picker open on what was asked.
    Find(String),
    /// A new thread; `Some` fixes its ID (a scratch store's own thread).
    New(Option<String>),
}

/// Everything the screen runs with.
pub struct Launch {
    /// The client, already open on its backend.
    pub client: Client,
    /// Coder on this computer, for the welcome card's engines and project.
    pub coder: Arc<dyn Coder>,
    /// The interrupt the client was opened with.
    pub interrupter: Interrupter,
    pub extras: Arc<dyn Extras>,
    pub resume: Resume,
    /// The folder the screen runs in.
    pub folder: Option<PathBuf>,
    /// Where the screen keeps the last thread per folder (the client's
    /// chat home).
    pub home: PathBuf,
    /// Lines to show first: what opening the client said (threads moved
    /// into the host, say).
    pub notices: Vec<String>,
    /// The program's version line, which `/export` records as the
    /// exporter.
    pub version: String,
}

/// How the screen closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exit {
    /// The thread that was open, when it has a message.
    pub thread: Option<String>,
    /// Its Coder run was still going when the screen closed.
    pub running: bool,
}

/// Run the screen on this process's terminal until the person quits.
///
/// # Errors
/// The terminal could not be set up or drawn.
pub async fn run(launch: Launch) -> std::io::Result<Exit> {
    screen::run(launch).await
}
