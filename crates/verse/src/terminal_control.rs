//! The terminal overlay's control socket: a Unix socket on this computer that takes
//! one JSON request a line and answers one JSON reply a line, so a shell,
//! an agent, or a test can drive the same panes the window draws.
//!
//! The socket sits in a directory of mode `0700` and is itself `0600`, so
//! only Verse's own user reaches it. Connections are read on their own
//! threads; the requests cross to the frame thread, where the overlay
//! applies them between frames and sends the reply back.
//!
//! This module needs neither the `terminal` feature nor a window, so
//! `openagents verse terminal` links it as the client.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

/// The environment variable that names the socket instead of the default.
pub const SOCKET_ENV: &str = "VERSE_TERMINAL_SOCKET";

/// Where the socket is by default: `~/.openagents/verse/terminal.sock`, or
/// what [`SOCKET_ENV`] names.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(SOCKET_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    Some(
        PathBuf::from(home)
            .join(".openagents")
            .join("verse")
            .join("terminal.sock"),
    )
}

/// One request, as a line of JSON: `{"op": "split", "axis": "cols"}`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Request {
    /// The overlay, its tabs, and its panes.
    Status,
    /// Show the overlay with focus, starting the first pane when none runs.
    Open,
    /// Hide the overlay; its panes keep running.
    Hide,
    /// Split the focused pane; `program` is a command line, or empty for
    /// the shell.
    Split {
        axis: String,
        #[serde(default)]
        program: Vec<String>,
    },
    /// Focus a neighbor (`left`, `right`, `up`, `down`) or a pane by id.
    Focus {
        #[serde(default)]
        direction: Option<String>,
        #[serde(default)]
        pane: Option<u64>,
    },
    /// Close the focused pane, ending its program.
    Close,
    /// Type `text` into the focused pane as a paste.
    Send { text: String },
    /// Press a named key in the focused pane: `enter`, `ctrl-c`, `up`, ...
    Key { name: String },
    /// The visible text of the focused pane, or of pane `pane`.
    Read {
        #[serde(default)]
        pane: Option<u64>,
    },
    /// Tabs: `new`, `next`, or `prev`.
    Tab { action: String },
    /// Zoom the focused pane to the whole overlay, or back.
    Zoom,
}

/// A request waiting for the frame thread, with where its reply goes.
pub type Pending = (Request, SyncSender<Value>);

/// A bound socket and the thread accepting on it.
pub struct Listener {
    path: PathBuf,
    inode: u64,
    requests: Receiver<Pending>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener")
            .field("path", &self.path)
            .finish()
    }
}

impl Listener {
    /// Binds `path`, replacing a socket an earlier Verse left behind.
    ///
    /// # Errors
    /// When the directory or the socket cannot be made.
    pub fn bind(path: &Path) -> Result<Self, String> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if let Some(dir) = path.parent() {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder
                .create(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        if path.exists() && UnixStream::connect(path).is_err() {
            let _ = std::fs::remove_file(path);
        }
        let listener = UnixListener::bind(path)
            .map_err(|e| format!("cannot listen on {}: {e}", path.display()))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("cannot set the mode of {}: {e}", path.display()))?;
        let inode = inode_of(path);
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("cannot set the socket nonblocking: {e}"))?;
        let (sender, requests) = mpsc::sync_channel::<Pending>(64);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("verse-terminal-control".into())
            .spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let sender = sender.clone();
                            let _ = std::thread::Builder::new()
                                .name("verse-terminal-peer".into())
                                .spawn(move || serve_peer(stream, &sender));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("cannot start the control thread: {e}"))?;
        Ok(Listener {
            path: path.to_path_buf(),
            inode,
            requests,
            stop,
            thread: Some(thread),
        })
    }

    /// Where the socket is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The next request waiting, if any.
    pub fn next(&self) -> Option<Pending> {
        match self.requests.try_recv() {
            Ok(pending) => Some(pending),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if inode_of(&self.path) == self.inode {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// The inode at `path`, so a listener that ends removes only its own socket
/// and not one a newer Verse bound at the same path.
fn inode_of(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).map_or(0, |m| m.ino())
}

/// Reads requests from one connection until it closes, answering each.
fn serve_peer(stream: UnixStream, sender: &SyncSender<Pending>) {
    let _ = stream.set_nonblocking(false);
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(request) => {
                let (tx, rx) = mpsc::sync_channel(1);
                if sender.send((request, tx)).is_err() {
                    serde_json::json!({ "ok": false, "error": "the overlay is gone" })
                } else {
                    match rx.recv_timeout(Duration::from_secs(10)) {
                        Ok(value) => value,
                        Err(_) => serde_json::json!({
                            "ok": false,
                            "error": "the overlay did not answer in 10 s; is Verse drawing?"
                        }),
                    }
                }
            }
            Err(e) => serde_json::json!({ "ok": false, "error": format!("bad request: {e}") }),
        };
        if writeln!(writer, "{reply}").is_err() || writer.flush().is_err() {
            break;
        }
    }
}

/// Sends one request to the socket at `path` and returns its reply.
///
/// # Errors
/// When nothing listens at `path`, or the reply is not JSON.
pub fn call(path: &Path, request: &Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(path).map_err(|e| {
        format!(
            "no Verse terminal at {}: {e}; start Verse on this computer (or set {SOCKET_ENV})",
            path.display()
        )
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    writeln!(stream, "{request}").map_err(|e| format!("cannot write the request: {e}"))?;
    stream.flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| format!("cannot read the reply: {e}"))?;
    if line.trim().is_empty() {
        return Err("Verse closed the connection without a reply".into());
    }
    serde_json::from_str(&line).map_err(|e| format!("the reply is not JSON: {e}"))
}
