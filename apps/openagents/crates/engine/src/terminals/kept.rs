//! Terminals under keepers (#11951, `docs/desktop/move-running-agents.md`):
//! an open terminal outlives the engine, so it no longer holds back an
//! engine update.
//!
//! When the engine has a keeper folder and a binary that runs keepers, each
//! terminal's shell runs on a PTY held by a terminal keeper (the app binary's
//! `keeper` subcommand). The engine talks to it through a [`KeptLink`]:
//! input, resizes, and closing go to the keeper, and its journaled output
//! becomes the terminal's events, numbered by the keeper's sequence numbers
//! so a viewer resuming after a restart misses nothing. Leaving the engine
//! leaves the shell running; the next engine finds the keeper's folder on
//! boot and registers the terminal under its old id, replaying the journal.
//! `OPENAGENTS_KEEPERS=0` opens terminals directly, as before.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use openagents_harness::keeper::{self, FrameBody};
use openagents_proto::{TerminalEvent, TerminalSession};

use super::{LiveTerminal, Terminals, TerminalsInner, lock};
use crate::new_id;

/// The `kind` in a terminal keeper's `meta.json`. Run keepers have none.
const KIND: &str = "terminal";

/// What the engine records about a terminal its keeper holds.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalMeta {
    kind: String,
    terminal_id: String,
    cwd: String,
    shell: String,
    cols: u16,
    rows: u16,
}

/// The engine's connection to one terminal keeper. Input sent while the
/// keeper is still starting waits here until it serves.
#[derive(Debug, Default)]
pub(super) struct KeptLink {
    state: Mutex<LinkState>,
}

#[derive(Default)]
struct LinkState {
    sender: Option<keeper::LinkSender>,
    pending: Vec<Pending>,
    finished: bool,
}

impl std::fmt::Debug for LinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkState")
            .field("connected", &self.sender.is_some())
            .field("pending", &self.pending.len())
            .field("finished", &self.finished)
            .finish()
    }
}

enum Pending {
    Input(Vec<u8>),
    Resize(u16, u16),
}

impl KeptLink {
    fn send(&self, message: Pending) -> std::io::Result<()> {
        let mut state = lock(&self.state);
        if state.finished {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        match &state.sender {
            Some(sender) => match message {
                Pending::Input(bytes) => sender.input(&bytes),
                Pending::Resize(cols, rows) => sender.resize(cols, rows),
            },
            None => {
                state.pending.push(message);
                Ok(())
            }
        }
    }

    pub(super) fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.send(Pending::Resize(cols, rows))
    }

    /// Close the terminal: the keeper hangs up the shell and removes its folder.
    pub(super) fn finish(&self) {
        let mut state = lock(&self.state);
        state.finished = true;
        state.pending.clear();
        if let Some(sender) = &state.sender {
            let _ = sender.finish();
        }
    }

    fn connected(&self, sender: keeper::LinkSender) {
        let mut state = lock(&self.state);
        if state.finished {
            let _ = sender.finish();
            return;
        }
        for message in std::mem::take(&mut state.pending) {
            let _ = match message {
                Pending::Input(bytes) => sender.input(&bytes),
                Pending::Resize(cols, rows) => sender.resize(cols, rows),
            };
        }
        state.sender = Some(sender);
    }
}

/// The terminal's writer: bytes go to the keeper.
struct KeptWriter(Arc<KeptLink>);

impl std::io::Write for KeptWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.send(Pending::Input(bytes.to_vec()))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The terminal's killer: closing it finishes the keeper.
#[derive(Debug)]
struct KeptKiller(Arc<KeptLink>);

impl portable_pty::ChildKiller for KeptKiller {
    fn kill(&mut self) -> std::io::Result<()> {
        self.0.finish();
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(KeptKiller(self.0.clone()))
    }
}

/// The binary and leading arguments that run keepers, when this engine may
/// put terminals under them.
fn launch() -> Option<(PathBuf, Vec<OsString>)> {
    if !keeper::allowed_by_env() {
        return None;
    }
    crate::sessions::keeper_launch()
}

impl Terminals {
    /// Keep terminals under keepers in `root`, and register the terminals
    /// whose keepers are still running under their old ids.
    pub fn start_keepers(&self, root: PathBuf) {
        if self.inner.keepers.set(root.clone()).is_err() {
            return;
        }
        for found in keeper::client::discover(&root) {
            let Ok(meta) = serde_json::from_value::<TerminalMeta>(found.meta) else {
                continue; // a run keeper's folder
            };
            if meta.kind != KIND {
                continue;
            }
            if !keeper::client::alive(&found.dir) {
                // The shell ended with no engine to see it.
                let _ = std::fs::remove_dir_all(&found.dir);
                continue;
            }
            tracing::info!(terminal = %meta.terminal_id, "picking up a running terminal");
            let link = Arc::new(KeptLink::default());
            let session = register(&self.inner, &meta.terminal_id, link.clone(), None);
            let dir = found.dir;
            tokio::spawn(async move {
                match keeper::client::connect(&dir, Some(0)).await {
                    Ok(keeper_link) => pump(Arc::downgrade(&session), link, keeper_link).await,
                    Err(error) => {
                        tracing::warn!(%error, "could not pick up a running terminal");
                        ended(&session);
                    }
                }
            });
        }
    }

    /// Open a shell under a new keeper. `None` when this engine opens
    /// terminals directly, or the keeper couldn't start.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn open_kept(
        &self,
        cwd: &str,
        cols: u16,
        rows: u16,
        shell: &str,
        shell_name: &str,
        environment: &std::collections::HashMap<String, String>,
        initial_script: &mut Option<tempfile::NamedTempFile>,
        bootstrap: Option<&str>,
    ) -> Option<TerminalSession> {
        let root = self.inner.keepers.get()?.clone();
        let (keeper_exe, keeper_args) = launch()?;
        let id = new_id();
        let mut env: Vec<(OsString, Option<OsString>)> = [
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("TERM_PROGRAM", "OpenAgents"),
        ]
        .into_iter()
        .map(|(name, value)| (name.into(), Some(value.into())))
        .collect();
        env.extend(
            environment
                .iter()
                .map(|(name, value)| (name.into(), Some(value.into()))),
        );
        if let Some(script) = initial_script.as_ref() {
            env.push((
                "OPENAGENTS_ACTION_SCRIPT".into(),
                Some(script.path().as_os_str().to_owned()),
            ));
        }
        let meta = TerminalMeta {
            kind: KIND.into(),
            terminal_id: id.clone(),
            cwd: cwd.into(),
            shell: shell_name.into(),
            cols,
            rows,
        };
        let spec = keeper::Spawn {
            keeper_exe,
            keeper_args,
            root,
            program: shell.into(),
            args: vec!["-l".into()], // login shell — the user's real PATH/profile
            cwd: Some(Path::new(cwd).to_path_buf()),
            env,
            meta: serde_json::to_value(&meta).unwrap_or_default(),
            orphan_after: keeper::ORPHAN_AFTER,
        };
        let starting = match keeper::client::start_pty(spec, cols, rows) {
            Ok(starting) => starting,
            Err(error) => {
                tracing::warn!(%error, "could not start a terminal keeper; opening it directly");
                return None;
            }
        };
        let link = Arc::new(KeptLink::default());
        let session = register(&self.inner, &id, link.clone(), initial_script.take());
        if let Some(bootstrap) = bootstrap {
            let _ = link.send(Pending::Input(bootstrap.as_bytes().to_vec()));
        }
        tokio::spawn(async move {
            match starting.ready().await {
                Ok(keeper_link) => pump(Arc::downgrade(&session), link, keeper_link).await,
                Err(error) => {
                    tracing::warn!(%error, "terminal keeper did not start");
                    ended(&session);
                }
            }
        });
        Some(TerminalSession {
            id,
            cwd: cwd.into(),
            shell: shell_name.into(),
        })
    }
}

fn register(
    inner: &TerminalsInner,
    id: &str,
    link: Arc<KeptLink>,
    initial_script: Option<tempfile::NamedTempFile>,
) -> Arc<Mutex<LiveTerminal>> {
    let session = Arc::new(Mutex::new(LiveTerminal {
        initial_script,
        master: None,
        writer: Some(Box::new(KeptWriter(link.clone()))),
        killer: Box::new(KeptKiller(link.clone())),
        kept: Some(link),
        subscribers: Vec::new(),
        replay: Default::default(),
        replay_bytes: 0,
        seq: 0,
        last_active_at: std::time::Instant::now(),
        exited: false,
    }));
    lock(&inner.sessions).insert(id.to_owned(), session.clone());
    session
}

/// The keeper's frames become the terminal's events, numbered as the keeper
/// numbered them.
async fn pump(session: Weak<Mutex<LiveTerminal>>, link: Arc<KeptLink>, keeper_link: keeper::Link) {
    let (sender, mut frames) = keeper_link.into_parts();
    link.connected(sender);
    while let Some(frame) = frames.recv().await {
        let Some(session) = session.upgrade() else {
            return; // closed; the keeper was told to finish
        };
        let mut session = lock(&session);
        if frame.seq <= session.seq {
            continue;
        }
        match frame.body {
            FrameBody::Stdout { line } => {
                session.seq = frame.seq;
                session.emit(TerminalEvent::Data {
                    seq: frame.seq,
                    data: line,
                });
            }
            // The keeper could not start the shell.
            FrameBody::Stderr { line } => {
                use base64::Engine as _;
                session.seq = frame.seq;
                session.emit(TerminalEvent::Data {
                    seq: frame.seq,
                    data: base64::engine::general_purpose::STANDARD.encode(format!("{line}\r\n")),
                });
            }
            FrameBody::Exit { code, .. } => {
                session.seq = frame.seq;
                session.emit(TerminalEvent::Exit {
                    seq: frame.seq,
                    exit_code: code.unwrap_or(-1),
                    signal: None,
                });
                return;
            }
            FrameBody::Stdin { .. } => {}
        }
    }
    // The keeper went away without saying the shell exited.
    if let Some(session) = session.upgrade() {
        ended(&session);
    }
}

fn ended(session: &Arc<Mutex<LiveTerminal>>) {
    let mut session = lock(session);
    if !session.exited {
        let seq = session.next_seq();
        session.emit(TerminalEvent::Exit {
            seq,
            exit_code: -1,
            signal: None,
        });
    }
}
