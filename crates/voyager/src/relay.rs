//! The Nostr relay, as a supervised child process.
//!
//! An ensemble world's guild channels live on `nostr-relay`, the same
//! binary that serves `relay.openagents.com`, run against a local
//! Postgres database. Ownership follows [`crate::server`]: the child is
//! the leader of its own process group, readiness is probed rather than
//! assumed, and dropping the handle stops the group and reaps the child.
//!
//! The relay's own keypair and the management key are derived like the
//! agents' — deterministic, re-derived at run time — because this is a
//! demo relay whose groups live for one episode. A relay meant to hold
//! real history needs real key custody; that is an operator question,
//! not a manifest field.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// How long a relay may take to accept its first connection: migrations
/// run on first boot.
const READY_WAIT: Duration = Duration::from_secs(60);
/// How often the ready loop retries the TCP probe.
const POLL: Duration = Duration::from_millis(250);

/// A running relay.
pub struct Relay {
    child: Child,
    /// The process group identifier — the child's own pid.
    group: i32,
    /// The `ws://` URL clients connect to.
    pub url: String,
    /// The `http://` URL the management endpoint answers on.
    pub http: String,
    /// The file the relay's output is appended to.
    pub log: PathBuf,
}

/// A key derived for a relay role — deterministic, never stored.
fn derived_secret(role: &str) -> String {
    let input = format!("voyager-relay-key:{role}");
    Sha256::digest(input.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The relay's own signing key — it signs NIP-29 group metadata.
#[must_use]
pub fn relay_secret() -> String {
    derived_secret("relay")
}

/// The management key — the only pubkey the NIP-86 endpoint accepts.
#[must_use]
pub fn management_secret() -> String {
    derived_secret("management")
}

impl Relay {
    /// Starts `nostr-relay` bound to `port`, storing into
    /// `database_url`. The group machinery needs the relay's secret (it
    /// signs group metadata) and the management pubkey (it owns the
    /// NIP-86 endpoint); both are derived in-process and passed through
    /// the environment, never written.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Relay`] when the binary is missing, the process
    /// will not spawn, or readiness times out.
    pub fn start(binary: &Path, database_url: &str, port: u16, log: &Path) -> Result<Self> {
        if !binary.is_file() {
            return Err(Error::relay(format!(
                "no relay binary at {}; build it with `cargo build -p nostr-relay`",
                binary.display()
            )));
        }
        let management = nostr::domain::RelaySigner::from_secret_hex(&management_secret())
            .map_err(|error| Error::relay(format!("management key: {error}")))?;
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|error| Error::relay(format!("{}: {error}", log.display())))?;
        let log_err = log_file.try_clone()?;
        let child = Command::new(binary)
            .env("DATABASE_URL", database_url)
            .env("NOSTR_RELAY_BIND_ADDR", "127.0.0.1")
            .env("NOSTR_RELAY_PORT", port.to_string())
            .env("NOSTR_RELAY_URL", format!("ws://127.0.0.1:{port}"))
            .env("NOSTR_RELAY_SECRET_KEY", relay_secret())
            .env("NOSTR_RELAY_MANAGEMENT_PUBKEY", management.pubkey())
            .env("NOSTR_RELAY_AUTH_REQUIRED", "false")
            .env("NOSTR_RELAY_LOG_LEVEL", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err))
            .process_group(0)
            .spawn()
            .map_err(|error| {
                Error::relay(format!(
                    "relay at {} did not start: {error}",
                    binary.display()
                ))
            })?;
        let mut relay = Relay {
            group: child.id() as i32,
            child,
            url: format!("ws://127.0.0.1:{port}"),
            http: format!("http://127.0.0.1:{port}"),
            log: log.to_path_buf(),
        };
        relay.wait_ready(port)?;
        Ok(relay)
    }

    /// Waits for the relay's HTTP listener to answer, failing early when
    /// the process exits — a migration error shows up there first.
    fn wait_ready(&mut self, port: u16) -> Result<()> {
        let deadline = Instant::now() + READY_WAIT;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(Error::relay(format!(
                    "the relay exited with {status} before it was ready; see {}",
                    self.log.display()
                )));
            }
            if probe_health(port) {
                return Ok(());
            }
            std::thread::sleep(POLL);
        }
        Err(Error::relay(format!(
            "no listener within {}s; see {}",
            READY_WAIT.as_secs(),
            self.log.display()
        )))
    }

    /// Stops the relay: SIGTERM to the group, then SIGKILL if it has not
    /// exited inside the grace. The child is reaped either way.
    pub fn stop(&mut self) -> Result<()> {
        if matches!(self.child.try_wait(), Ok(None)) {
            unsafe {
                libc::killpg(self.group, libc::SIGTERM);
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                match self.child.try_wait() {
                    Ok(Some(_)) | Err(_) => break,
                    Ok(None) => std::thread::sleep(POLL),
                }
            }
            if matches!(self.child.try_wait(), Ok(None)) {
                unsafe {
                    libc::killpg(self.group, libc::SIGKILL);
                }
            }
        }
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// One `GET /health` over a plain socket. The relay answers on the same
/// port as the websocket.
fn probe_health(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut head = [0u8; 64];
    let Ok(read) = stream.read(&mut head) else {
        return false;
    };
    String::from_utf8_lossy(&head[..read]).starts_with("HTTP/")
}
