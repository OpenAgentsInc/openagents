//! The keeper process: `openagents keeper`, with the run folder in
//! `OPENAGENTS_KEEPER_DIR` (or `--dir RUN_FOLDER`) and the orphan window in
//! `OPENAGENTS_KEEPER_ORPHAN_SECS` (or `--orphan-secs N`).
//!
//! The run folder holds `launch.json` (what to run and where the socket is),
//! `meta.json` (the engine's record of the run), `token`, and the keeper's
//! `journal.jsonl` and `keeper.log`. Every file is private to this user.
//!
//! A launch with a `pty` size starts a terminal keeper instead (#11951):
//! the program runs on a PTY the keeper holds (see `pty.rs`).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use super::protocol::{self, Frame, FrameBody, FromKeeper, KeeperSignal, ToKeeper};

pub(crate) const LAUNCH: &str = "launch.json";
pub(crate) const META: &str = "meta.json";
pub(crate) const TOKEN: &str = "token";
pub(crate) const JOURNAL: &str = "journal.jsonl";
pub(crate) const LOG: &str = "keeper.log";
pub(crate) const DIR_ENV: &str = "OPENAGENTS_KEEPER_DIR";
pub(crate) const ORPHAN_ENV: &str = "OPENAGENTS_KEEPER_ORPHAN_SECS";

/// What the keeper starts, written by the engine before it starts the keeper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Launch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub socket: PathBuf,
    /// Run the program on a PTY of this size, as a terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pty: Option<PtySize>,
}

/// A terminal keeper's PTY size.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

/// How long an engine has to say hello after it connects.
const HELLO_WITHIN: Duration = Duration::from_secs(5);
/// How long an orphaned agent gets to stop after each signal.
pub(super) const STOP_GRACE: Duration = Duration::from_secs(10);

/// The keeper subcommand's entry point. `args` are the arguments after
/// `keeper`. Returns the process exit code.
pub fn main(args: impl IntoIterator<Item = OsString>) -> i32 {
    let mut dir = std::env::var_os(DIR_ENV).map(PathBuf::from);
    let mut orphan_after = std::env::var(ORPHAN_ENV)
        .ok()
        .and_then(|secs| secs.parse().ok())
        .map_or(super::ORPHAN_AFTER, Duration::from_secs);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--dir") => dir = args.next().map(PathBuf::from),
            Some("--orphan-secs") => {
                if let Some(secs) = args.next().and_then(|s| s.to_str()?.parse::<u64>().ok()) {
                    orphan_after = Duration::from_secs(secs);
                }
            }
            _ => {}
        }
    }
    let Some(dir) = dir else {
        eprintln!("keeper: --dir is required");
        return 2;
    };
    // Leave the engine's session and process group: a signal to the engine's
    // group, or the engine's terminal closing, must not reach the agent.
    // SAFETY: setsid takes no arguments; failure only means we already lead one.
    unsafe { libc::setsid() };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("keeper: {error}");
            return 1;
        }
    };
    match runtime.block_on(serve(&dir, orphan_after)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("keeper: {error}");
            1
        }
    }
}

pub(super) fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
}

pub(super) struct Journal {
    pub file: std::fs::File,
    pub last_seq: u64,
    /// Bytes written since the file was opened or last compacted.
    pub bytes: u64,
}

impl Journal {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            file: private_file(path)?,
            last_seq: 0,
            bytes: 0,
        })
    }

    pub fn append(&mut self, body: FrameBody) -> std::io::Result<Frame> {
        self.last_seq += 1;
        let frame = Frame {
            seq: self.last_seq,
            body,
        };
        let mut line = serde_json::to_vec(&frame)?;
        line.push(b'\n');
        self.file.write_all(&line)?;
        self.bytes += line.len() as u64;
        Ok(frame)
    }
}

pub(super) enum Event {
    Out(Option<String>),
    Err(Option<String>),
    /// Terminal keepers: bytes the PTY wrote, `None` at its end.
    Pty(Option<Vec<u8>>),
    /// Terminal keepers: the shell exited with this code.
    Exited(Option<i32>),
    Accepted(UnixStream),
    Message(u64, ToKeeper),
    Closed(u64),
}

pub(super) struct Connection {
    pub id: u64,
    frames: mpsc::UnboundedSender<FromKeeper>,
}

/// Where a run stands when an engine connects.
pub(super) struct Standing<'a> {
    pub pid: u32,
    pub last_seq: u64,
    pub exited: bool,
    pub acked: u64,
    pub held: &'a BTreeSet<u64>,
}

/// Authenticate a new engine and serve it: the welcome, the replay it asks
/// for, then live frames. `None` when it is refused.
pub(super) async fn greet(
    stream: UnixStream,
    id: u64,
    token: &str,
    standing: Standing<'_>,
    journal: PathBuf,
    events: &mpsc::UnboundedSender<Event>,
) -> Option<Connection> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let hello = tokio::time::timeout(HELLO_WITHIN, lines.next_line()).await;
    let hello = match hello {
        Ok(Ok(Some(line))) => serde_json::from_str::<ToKeeper>(&line).ok(),
        _ => None,
    };
    let refusal = match &hello {
        Some(ToKeeper::Hello {
            version, token: t, ..
        }) => {
            if *t != token {
                Some("wrong token".to_owned())
            } else if protocol::negotiate(*version).is_none() {
                Some(format!("protocol {version} is not supported"))
            } else {
                None
            }
        }
        _ => Some("expected hello".to_owned()),
    };
    if let Some(reason) = refusal {
        let _ = write_line(&mut write, &FromKeeper::Refused { reason }).await;
        return None;
    }
    let Some(ToKeeper::Hello { version, after, .. }) = hello else {
        return None;
    };
    // A new engine replaces the old one: an update hands off this way.
    let after = after.unwrap_or(standing.acked);
    let welcome = FromKeeper::Welcome {
        version: version.min(protocol::PROTOCOL),
        pid: standing.pid,
        last_seq: standing.last_seq,
        exited: standing.exited,
        acked: standing.acked,
    };
    let (frames_tx, frames_rx) = mpsc::unbounded_channel();
    let replay = Replay {
        journal,
        held: standing
            .held
            .iter()
            .copied()
            .filter(|seq| *seq <= after)
            .collect(),
        after,
        upto: standing.last_seq,
    };
    tokio::spawn(write_connection(write, welcome, replay, frames_rx));
    let tx = events.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(message) = serde_json::from_str::<ToKeeper>(&line)
                && tx.send(Event::Message(id, message)).is_err()
            {
                return;
            }
        }
        let _ = tx.send(Event::Closed(id));
    });
    Some(Connection {
        id,
        frames: frames_tx,
    })
}

/// Serve `socket`, private to this user, sending each connection to `events`.
pub(super) fn listen(socket: &Path, events: &mpsc::UnboundedSender<Event>) -> std::io::Result<()> {
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let tx = events.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            if tx.send(Event::Accepted(stream)).is_err() {
                break;
            }
        }
    });
    Ok(())
}

async fn serve(dir: &Path, orphan_after: Duration) -> std::io::Result<()> {
    let launch: Launch = serde_json::from_slice(&std::fs::read(dir.join(LAUNCH))?)?;
    let token = std::fs::read_to_string(dir.join(TOKEN))?.trim().to_owned();
    if let Some(size) = launch.pty {
        #[cfg(feature = "pty")]
        return super::pty::serve(dir, &launch, &token, size, orphan_after).await;
        #[cfg(not(feature = "pty"))]
        {
            let _ = size;
            return Err(std::io::Error::other("this build has no terminal keepers"));
        }
    }

    let mut command = tokio::process::Command::new(&launch.program);
    command
        .args(&launch.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env_remove(DIR_ENV)
        .env_remove(ORPHAN_ENV)
        .process_group(0)
        .kill_on_drop(false);
    if let Some(cwd) = &launch.cwd {
        command.current_dir(cwd);
    }
    let mut journal = Journal::open(&dir.join(JOURNAL))?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            // The engine learns why from the journal: an exit with no code.
            journal.append(FrameBody::Stderr {
                line: format!("could not start {}: {error}", launch.program.display()),
            })?;
            journal.append(FrameBody::Exit {
                code: None,
                signal: None,
            })?;
            return Err(error);
        }
    };
    let pid = child.id().unwrap_or(0);
    let (mut stdin_tx, stdin_rx) = mpsc::unbounded_channel::<Option<String>>();
    if let Some(stdin) = child.stdin.take() {
        tokio::spawn(write_stdin(stdin, stdin_rx));
    }

    let (events_tx, mut events) = mpsc::unbounded_channel::<Event>();
    listen(&launch.socket, &events_tx)?;
    if let Some(stdout) = child.stdout.take() {
        let tx = events_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                let line = lines.next_line().await.ok().flatten();
                let end = line.is_none();
                if tx.send(Event::Out(line)).is_err() || end {
                    break;
                }
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = events_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            loop {
                let line = lines.next_line().await.ok().flatten();
                let end = line.is_none();
                if tx.send(Event::Err(line)).is_err() || end {
                    break;
                }
            }
        });
    }

    let mut acked = 0u64;
    let mut held = BTreeSet::new();
    let mut connection: Option<Connection> = None;
    let mut next_connection = 0u64;
    let mut orphan_since = tokio::time::Instant::now();
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut exit_seq: Option<u64> = None;
    // An orphan's stop in progress: when to escalate, and to what.
    let mut orphan_stop: Option<(tokio::time::Instant, KeeperSignal)> = None;

    loop {
        let orphan_deadline = orphan_since + orphan_after;
        let event = tokio::select! {
            event = events.recv() => match event {
                Some(event) => event,
                None => break,
            },
            status = child.wait(), if !stdout_open && !stderr_open && exit_seq.is_none() => {
                let (code, signal) = match status {
                    Ok(status) => {
                        use std::os::unix::process::ExitStatusExt;
                        (status.code(), status.signal())
                    }
                    Err(_) => (None, None),
                };
                let frame = journal.append(FrameBody::Exit { code, signal })?;
                exit_seq = Some(frame.seq);
                deliver(&mut connection, frame);
                if orphan_stop.is_some() {
                    retire(dir, &launch.socket);
                    return Ok(());
                }
                continue;
            }
            _ = tokio::time::sleep_until(orphan_deadline), if connection.is_none() && orphan_stop.is_none() => {
                // No engine for the whole window: stop the agent, journaling
                // what it says as it stops, and keep the journal.
                if exit_seq.is_some() {
                    retire(dir, &launch.socket);
                    return Ok(());
                }
                signal_group(pid, KeeperSignal::Interrupt);
                orphan_stop = Some((tokio::time::Instant::now() + STOP_GRACE, KeeperSignal::Terminate));
                continue;
            }
            _ = tokio::time::sleep_until(orphan_stop.map_or_else(tokio::time::Instant::now, |(at, _)| at)),
                if orphan_stop.is_some() && exit_seq.is_none() =>
            {
                if let Some((_, signal)) = orphan_stop {
                    signal_group(pid, signal);
                    orphan_stop = Some((tokio::time::Instant::now() + STOP_GRACE, KeeperSignal::Kill));
                }
                continue;
            }
        };
        match event {
            Event::Out(Some(line)) => {
                let frame = journal.append(FrameBody::Stdout { line })?;
                deliver(&mut connection, frame);
            }
            Event::Out(None) => stdout_open = false,
            Event::Err(Some(line)) => {
                let frame = journal.append(FrameBody::Stderr { line })?;
                deliver(&mut connection, frame);
            }
            Event::Err(None) => stderr_open = false,
            Event::Accepted(stream) => {
                next_connection += 1;
                let standing = Standing {
                    pid,
                    last_seq: journal.last_seq,
                    exited: exit_seq.is_some(),
                    acked,
                    held: &held,
                };
                if let Some(c) = greet(
                    stream,
                    next_connection,
                    &token,
                    standing,
                    dir.join(JOURNAL),
                    &events_tx,
                )
                .await
                {
                    connection = Some(c);
                }
            }
            Event::Pty(_) | Event::Exited(_) => {}
            Event::Closed(id) => {
                if connection.as_ref().is_some_and(|c| c.id == id) {
                    connection = None;
                    orphan_since = tokio::time::Instant::now();
                }
            }
            Event::Message(id, message) => {
                if connection.as_ref().is_none_or(|c| c.id != id) {
                    continue; // a replaced engine's late message
                }
                match message {
                    ToKeeper::Stdin { line } => {
                        // Journal first: a frame on disk is a frame the agent was sent.
                        let frame = journal.append(FrameBody::Stdin { line: line.clone() })?;
                        let _ = stdin_tx.send(Some(line));
                        deliver(&mut connection, frame);
                    }
                    ToKeeper::CloseStdin => {
                        let _ = stdin_tx.send(None);
                        stdin_tx = mpsc::unbounded_channel().0;
                    }
                    ToKeeper::Ack { seq } => {
                        acked = acked.max(seq);
                        if exit_seq.is_some_and(|exit| acked >= exit) && held.is_empty() {
                            finish(dir, &launch.socket);
                            return Ok(());
                        }
                    }
                    ToKeeper::Hold { seq } => {
                        held.insert(seq);
                    }
                    ToKeeper::Release { seq } => {
                        held.remove(&seq);
                    }
                    ToKeeper::Signal { signal } => {
                        if exit_seq.is_none() {
                            signal_group(pid, signal);
                        }
                    }
                    ToKeeper::Finish => {
                        if exit_seq.is_none() {
                            stop_group(pid, &mut child).await;
                        }
                        finish(dir, &launch.socket);
                        return Ok(());
                    }
                    ToKeeper::Hello { .. }
                    | ToKeeper::Input { .. }
                    | ToKeeper::Resize { .. }
                    | ToKeeper::Unknown => {}
                }
            }
        }
    }
    Ok(())
}

pub(super) fn deliver(connection: &mut Option<Connection>, frame: Frame) {
    if let Some(c) = connection
        && c.frames.send(FromKeeper::Frame(frame)).is_err()
    {
        // Its writer stopped; the reader reports the close.
    }
}

/// Stop serving but keep the journal for whoever looks later.
pub(super) fn retire(dir: &Path, socket: &Path) {
    let _ = std::fs::remove_file(socket);
    let _ = std::fs::remove_file(dir.join(TOKEN));
}

pub(super) fn finish(dir: &Path, socket: &Path) {
    let _ = std::fs::remove_file(socket);
    let _ = std::fs::remove_dir_all(dir);
}

pub(super) fn signal_group(pid: u32, signal: KeeperSignal) {
    if pid == 0 {
        return;
    }
    let number = match signal {
        KeeperSignal::Interrupt => libc::SIGINT,
        KeeperSignal::Terminate => libc::SIGTERM,
        KeeperSignal::Kill => libc::SIGKILL,
    };
    // SAFETY: the agent leads its own process group, which only it and its
    // children are in.
    unsafe { libc::kill(-(pid as i32), number) };
}

/// Interrupt, then terminate, then kill the agent's group, waiting between.
async fn stop_group(pid: u32, child: &mut tokio::process::Child) {
    for signal in [KeeperSignal::Interrupt, KeeperSignal::Terminate] {
        signal_group(pid, signal);
        if tokio::time::timeout(STOP_GRACE, child.wait()).await.is_ok() {
            return;
        }
    }
    signal_group(pid, KeeperSignal::Kill);
    let _ = child.wait().await;
}

async fn write_stdin(
    mut stdin: tokio::process::ChildStdin,
    mut lines: mpsc::UnboundedReceiver<Option<String>>,
) {
    while let Some(Some(mut line)) = lines.recv().await {
        line.push('\n');
        if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
            return;
        }
    }
}

struct Replay {
    journal: PathBuf,
    held: Vec<u64>,
    after: u64,
    upto: u64,
}

async fn write_line<W: tokio::io::AsyncWrite + Unpin>(
    write: &mut W,
    message: &FromKeeper,
) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(message)?;
    line.push(b'\n');
    write.write_all(&line).await
}

/// Welcome, then held frames, then journaled frames after `after` up to
/// `upto`, then live frames as they come.
async fn write_connection(
    mut write: tokio::net::unix::OwnedWriteHalf,
    welcome: FromKeeper,
    replay: Replay,
    mut live: mpsc::UnboundedReceiver<FromKeeper>,
) -> std::io::Result<()> {
    write_line(&mut write, &welcome).await?;
    if replay.upto > 0 {
        let file = tokio::fs::File::open(&replay.journal).await?;
        let mut lines = BufReader::new(file).lines();
        while let Some(line) = lines.next_line().await? {
            let Ok(frame) = serde_json::from_str::<Frame>(&line) else {
                continue;
            };
            if frame.seq > replay.upto {
                break;
            }
            // Held frames are older than `after`, so file order is seq order.
            if frame.seq > replay.after || replay.held.contains(&frame.seq) {
                write_line(&mut write, &FromKeeper::Frame(frame)).await?;
            }
        }
    }
    while let Some(message) = live.recv().await {
        write_line(&mut write, &message).await?;
    }
    Ok(())
}
