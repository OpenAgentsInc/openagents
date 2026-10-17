//! The engine's side of a keeper: start one, find the ones still running,
//! and connect.

use std::ffi::OsString;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use super::protocol::{self, Frame, FromKeeper, KeeperSignal, ToKeeper};
use super::server::{JOURNAL, LAUNCH, LOG, Launch, META, PtySize, TOKEN};

/// How long a new keeper has to open its socket.
const READY_WITHIN: Duration = Duration::from_secs(10);
/// macOS limits a socket path to 104 bytes, Linux to 108.
const SOCKET_PATH_MAX: usize = 100;

/// What to start under a new keeper.
pub struct Spawn {
    /// The binary that runs `keeper` (the app binary).
    pub keeper_exe: PathBuf,
    /// Arguments before `keeper`. A test binary that runs the keeper passes
    /// its own test filter here; the app passes none.
    pub keeper_args: Vec<OsString>,
    /// The folder that holds run folders.
    pub root: PathBuf,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    /// Environment changes for the agent: `None` removes a variable.
    pub env: Vec<(OsString, Option<OsString>)>,
    /// The engine's record of the run, read back on adoption.
    pub meta: serde_json::Value,
    pub orphan_after: Duration,
}

/// A run folder whose keeper may still be running.
#[derive(Debug, Clone)]
pub struct Found {
    pub dir: PathBuf,
    pub meta: serde_json::Value,
}

/// Every run folder under `root`, oldest first.
pub fn discover(root: &Path) -> Vec<Found> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, Found)> = entries
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let meta = std::fs::read(dir.join(META)).ok()?;
            let meta = serde_json::from_slice(&meta).ok()?;
            let made = entry.metadata().and_then(|m| m.modified()).ok()?;
            Some((made, Found { dir, meta }))
        })
        .collect();
    found.sort_by_key(|(made, _)| *made);
    found.into_iter().map(|(_, found)| found).collect()
}

/// Start `spec.program` under a new keeper and connect to it.
pub async fn spawn(spec: Spawn) -> io::Result<Link> {
    start(spec, None)?.ready().await
}

/// Start `spec.program` on a PTY of `cols` by `rows` under a new terminal
/// keeper. The keeper is starting when this returns; [`Starting::ready`]
/// connects once it serves.
pub fn start_pty(spec: Spawn, cols: u16, rows: u16) -> io::Result<Starting> {
    start(spec, Some(PtySize { cols, rows }))
}

/// A keeper that has been started and may not serve yet.
pub struct Starting {
    pub dir: PathBuf,
    socket: PathBuf,
    exited: Arc<std::sync::atomic::AtomicBool>,
}

fn start(spec: Spawn, pty: Option<PtySize>) -> io::Result<Starting> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&spec.root)?;
    let key = uuid::Uuid::new_v4().simple().to_string();
    let dir = spec.root.join(&key);
    std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    let socket = {
        let inside = dir.join("keeper.sock");
        if inside.as_os_str().len() <= SOCKET_PATH_MAX {
            inside
        } else {
            std::env::temp_dir().join(format!("oa-keeper-{}.sock", &key[..16]))
        }
    };
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    write_private(&dir.join(TOKEN), token.as_bytes())?;
    write_private(
        &dir.join(LAUNCH),
        &serde_json::to_vec(&Launch {
            program: spec.program,
            args: spec.args,
            cwd: spec.cwd,
            socket: socket.clone(),
            pty,
        })?,
    )?;
    write_private(&dir.join(META), &serde_json::to_vec(&spec.meta)?)?;

    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(dir.join(LOG))?;
    let mut command = tokio::process::Command::new(&spec.keeper_exe);
    command
        .args(&spec.keeper_args)
        .arg(super::SUBCOMMAND)
        // Options ride the environment so a test binary's own argument
        // parser never sees them; the keeper removes them from the agent's.
        .env(super::server::DIR_ENV, &dir)
        .env(
            super::server::ORPHAN_ENV,
            spec.orphan_after.as_secs().to_string(),
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log)
        .kill_on_drop(false);
    for (key, value) in spec.env {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    let mut keeper = command.spawn()?;
    // Reap the keeper if it exits while this engine is alive. When the
    // engine exits first, the keeper is reparented and keeps running.
    let exited = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let exited = exited.clone();
        tokio::spawn(async move {
            let _ = keeper.wait().await;
            exited.store(true, std::sync::atomic::Ordering::Release);
        });
    }
    Ok(Starting {
        dir,
        socket,
        exited,
    })
}

impl Starting {
    /// Connect once the keeper serves, or remove its run folder and say why
    /// it didn't start.
    pub async fn ready(self) -> io::Result<Link> {
        let Starting {
            dir,
            socket,
            exited,
        } = self;
        let deadline = tokio::time::Instant::now() + READY_WITHIN;
        loop {
            if socket.exists()
                && let Ok(link) = connect(&dir, None).await
            {
                return Ok(link);
            }
            if exited.load(std::sync::atomic::Ordering::Acquire)
                || tokio::time::Instant::now() >= deadline
            {
                let log = std::fs::read_to_string(dir.join(LOG)).unwrap_or_default();
                let _ = std::fs::remove_dir_all(&dir);
                return Err(io::Error::other(format!(
                    "the agent keeper did not start{}",
                    if log.trim().is_empty() {
                        String::new()
                    } else {
                        format!(": {}", log.trim())
                    }
                )));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// Whether the keeper of the run folder `dir` accepts connections.
pub fn alive(dir: &Path) -> bool {
    let Ok(launch) = std::fs::read(dir.join(LAUNCH)) else {
        return false;
    };
    let Ok(launch) = serde_json::from_slice::<Launch>(&launch) else {
        return false;
    };
    StdUnixStream::connect(launch.socket).is_ok()
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

/// Connect to the keeper of the run folder `dir`. `after` replays frames
/// after that sequence number; `None` uses the keeper's last acknowledged one.
pub async fn connect(dir: &Path, after: Option<u64>) -> io::Result<Link> {
    let launch: Launch = serde_json::from_slice(&std::fs::read(dir.join(LAUNCH))?)?;
    let token = std::fs::read_to_string(dir.join(TOKEN))?;
    let meta = std::fs::read(dir.join(META))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(serde_json::Value::Null);
    let stream = StdUnixStream::connect(&launch.socket)?;
    let writer = stream.try_clone()?;
    let sender = LinkSender {
        stream: Arc::new(Mutex::new(writer)),
    };
    sender.send(&ToKeeper::Hello {
        version: protocol::PROTOCOL,
        token: token.trim().to_owned(),
        after,
    })?;
    stream.set_nonblocking(true)?;
    let stream = tokio::net::UnixStream::from_std(stream)?;
    let mut lines = BufReader::new(stream).lines();
    let welcome = tokio::time::timeout(READY_WITHIN, lines.next_line())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "the agent keeper did not answer"))??
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "the agent keeper closed"))?;
    let (version, pid, last_seq, exited, acked) =
        match serde_json::from_str::<FromKeeper>(&welcome)? {
            FromKeeper::Welcome {
                version,
                pid,
                last_seq,
                exited,
                acked,
            } => (version, pid, last_seq, exited, acked),
            FromKeeper::Refused { reason } => {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, reason));
            }
            _ => return Err(io::Error::other("the agent keeper sent no welcome")),
        };
    // Drop the connection without sending anything else, so the agent runs on.
    if !protocol::speaks(version) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("the agent keeper speaks protocol {version}"),
        ));
    }
    let (frames_tx, frames) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let line = tokio::select! {
                line = lines.next_line() => line,
                // A dropped link closes the socket even while the agent is quiet.
                _ = frames_tx.closed() => return,
            };
            let Ok(Some(line)) = line else { return };
            if let Ok(FromKeeper::Frame(frame)) = serde_json::from_str(&line)
                && frames_tx.send(frame).is_err()
            {
                return;
            }
        }
    });
    Ok(Link {
        dir: dir.to_path_buf(),
        meta,
        pid,
        version,
        last_seq,
        exited,
        acked,
        sender,
        frames,
    })
}

/// A connection to one keeper. Dropping it leaves the agent running.
pub struct Link {
    pub dir: PathBuf,
    pub meta: serde_json::Value,
    /// The agent's process id; it leads the agent's process group.
    pub pid: u32,
    pub version: u32,
    /// The highest sequence number the keeper had journaled at connection.
    pub last_seq: u64,
    /// The agent had already exited when this engine connected.
    pub exited: bool,
    /// The highest sequence number an earlier engine acknowledged.
    pub acked: u64,
    sender: LinkSender,
    frames: mpsc::UnboundedReceiver<Frame>,
}

impl Link {
    /// The next frame, or `None` once the keeper closed the connection.
    pub async fn next_frame(&mut self) -> Option<Frame> {
        self.frames.recv().await
    }

    pub fn sender(&self) -> LinkSender {
        self.sender.clone()
    }

    pub fn into_parts(self) -> (LinkSender, mpsc::UnboundedReceiver<Frame>) {
        (self.sender, self.frames)
    }

    /// The keeper's journal, for diagnostics and tests.
    pub fn journal_path(&self) -> PathBuf {
        self.dir.join(JOURNAL)
    }
}

/// Sends to a keeper. Each call has reached the kernel when it returns, so
/// an acknowledgement survives the engine being killed right after.
#[derive(Clone)]
pub struct LinkSender {
    stream: Arc<Mutex<StdUnixStream>>,
}

impl LinkSender {
    pub fn send(&self, message: &ToKeeper) -> io::Result<()> {
        let mut line = serde_json::to_vec(message)?;
        line.push(b'\n');
        let mut stream = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        let mut written = 0;
        while written < line.len() {
            match stream.write(&line[written..]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => written += n,
                // The reading half made the socket non-blocking.
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn stdin(&self, line: impl Into<String>) -> io::Result<()> {
        self.send(&ToKeeper::Stdin { line: line.into() })
    }

    pub fn close_stdin(&self) -> io::Result<()> {
        self.send(&ToKeeper::CloseStdin)
    }

    pub fn ack(&self, seq: u64) -> io::Result<()> {
        self.send(&ToKeeper::Ack { seq })
    }

    pub fn hold(&self, seq: u64) -> io::Result<()> {
        self.send(&ToKeeper::Hold { seq })
    }

    pub fn release(&self, seq: u64) -> io::Result<()> {
        self.send(&ToKeeper::Release { seq })
    }

    pub fn signal(&self, signal: KeeperSignal) -> io::Result<()> {
        self.send(&ToKeeper::Signal { signal })
    }

    /// Terminal keepers: write `bytes` to the PTY.
    pub fn input(&self, bytes: &[u8]) -> io::Result<()> {
        use base64::Engine as _;
        self.send(&ToKeeper::Input {
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
    }

    /// Terminal keepers: resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        self.send(&ToKeeper::Resize { cols, rows })
    }

    /// Stop the agent if it still runs, remove the run folder, and end the keeper.
    pub fn finish(&self) -> io::Result<()> {
        self.send(&ToKeeper::Finish)
    }
}
