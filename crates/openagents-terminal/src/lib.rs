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

use std::path::PathBuf;
use std::sync::Arc;

use openagents_chat::client::{self, Client, Coder};
use tokio::sync::watch;

pub mod app;
pub mod copy;
pub mod draw;
pub mod last;
pub mod prompts;
pub mod rows;
pub mod screen;
pub mod slash;

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

/// What `/settings` shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// The settings file (`~/.openagents/settings.json`).
    pub path: PathBuf,
    /// A few lines saying what it holds now.
    pub lines: Vec<String>,
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
    /// The published plugins, one `(name, what it does)` row each.
    ///
    /// # Errors
    /// The catalog could not be read.
    fn plugins(&self) -> Result<Vec<(String, String)>, String>;
    /// The Coder settings on this computer.
    fn settings(&self) -> Settings;
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
    fn plugins(&self) -> Result<Vec<(String, String)>, String> {
        Ok(Vec::new())
    }
    fn settings(&self) -> Settings {
        Settings::default()
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
