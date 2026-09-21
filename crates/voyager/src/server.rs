//! The Minecraft server, as a supervised child process.
//!
//! A server is a daemon, not a bounded job, so this module does not wrap
//! `supervise::Job`; it follows the same ownership rules the supervisor
//! documents in `docs/coder/runtime/subprocesses.md`: the child runs as
//! the leader of its own process group, becoming ready has a deadline,
//! stopping asks politely first (`stop` on the console, the graceful
//! vanilla shutdown) and kills the group only after a grace, and the
//! child is always reaped. A caller that drops a [`Server`] gets the
//! same shutdown; nothing is left to reparent.
//!
//! Readiness is read from `server.log` in the server's own directory —
//! the vanilla line `Done (12.3s)!` — not from a port probe, because a
//! port accepting connections is not the same thing as a world a player
//! can join.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::world::World;

/// How long a server may take to report `Done`. The first boot in a fresh
/// directory also unpacks the bundled libraries, which can take minutes.
pub const READY_WAIT: Duration = Duration::from_secs(300);
/// How long a `stop` command may take before the group is killed.
pub const STOP_GRACE: Duration = Duration::from_secs(15);
/// How often the ready loop re-reads the log.
const POLL: Duration = Duration::from_millis(250);
/// The bytes of log tail reported when readiness fails.
const LOG_TAIL: usize = 4 << 10;

/// The `eula.txt` the server requires. Writing it accepts Mojang's
/// Minecraft EULA for this local server; the file says so itself.
const EULA: &str = "# accepted by voyager on behalf of this local server\neula=true\n";

/// A running Minecraft server.
pub struct Server {
    child: Child,
    stdin: ChildStdin,
    /// The process group identifier — the child's own pid.
    group: i32,
    /// Where the server's files live.
    pub dir: PathBuf,
    /// The file the server's output is appended to.
    pub log: PathBuf,
    /// The port the world answers on.
    pub port: u16,
}

impl Server {
    /// Boots a server for `world` from `jar` in `dir` on `port`.
    ///
    /// Writes `eula.txt` and `server.properties` from the manifest,
    /// spawns `java -jar server.jar nogui` in its own process group, and
    /// waits up to [`READY_WAIT`] for the `Done` line in `server.log`.
    /// Once ready, the manifest's boot commands run through the console.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Server`] when the jar is missing, java will not
    /// spawn, the process exits early, or the deadline passes.
    pub fn start(world: &World, jar: &Path, java: &Path, dir: &Path, port: u16) -> Result<Self> {
        if !jar.is_file() {
            return Err(Error::server(format!(
                "no server jar at {}; run scripts/fetch-mc-server.sh {}",
                jar.display(),
                world.minecraft.version
            )));
        }
        fs::create_dir_all(dir)?;
        fs::write(dir.join("eula.txt"), EULA)?;
        fs::write(dir.join("server.properties"), world.properties(port))?;
        let jar_target = dir.join("server.jar");
        if !jar_target.exists() {
            // The jar lives in the version cache; the server wants it in
            // the working directory. Copy, don't move — other runs share it.
            fs::copy(jar, &jar_target)?;
        }
        // The bundled server unpacks its libraries into the working
        // directory on every boot. Point those directories at the version
        // cache instead, so the unpack happens once across all runs of a
        // version rather than once per run.
        if let Some(cache) = jar.parent() {
            for shared in ["libraries", "versions"] {
                let target = cache.join("unpacked").join(shared);
                fs::create_dir_all(&target)?;
                let link = dir.join(shared);
                if !link.exists() {
                    std::os::unix::fs::symlink(&target, &link)?;
                }
            }
        }
        let log = dir.join("server.log");
        let log_file = OpenOptions::new().create(true).append(true).open(&log)?;
        let log_err = log_file.try_clone()?;
        let mut child = Command::new(java)
            .args(["-Xms1024M", "-Xmx2048M", "-jar", "server.jar", "nogui"])
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err))
            .process_group(0)
            .spawn()
            .map_err(|error| {
                Error::server(format!("java at {} did not start: {error}", java.display()))
            })?;
        let stdin = child.stdin.take().expect("stdin was piped");
        let mut server = Server {
            group: child.id() as i32,
            child,
            stdin,
            dir: dir.to_path_buf(),
            log,
            port,
        };
        server.wait_ready()?;
        for command in world.boot_commands() {
            server.command(&command)?;
        }
        Ok(server)
    }

    /// Sends one console command — `gamerule`, `time set`, `op` — to the
    /// running server.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Server`] when the console pipe is closed.
    pub fn command(&mut self, command: &str) -> Result<()> {
        writeln!(self.stdin, "{command}")
            .and_then(|()| self.stdin.flush())
            .map_err(|error| Error::server(format!("the console closed: {error}")))
    }

    /// Whether the server process still runs.
    #[must_use]
    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// The last [`LOG_TAIL`] bytes of `server.log`.
    #[must_use]
    pub fn log_tail(&self) -> String {
        tail(&self.log)
    }

    /// Stops the server: `stop` on the console, then the group is killed
    /// if it has not exited within [`STOP_GRACE`]. The child is reaped
    /// either way.
    pub fn stop(&mut self) -> Result<()> {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.command("stop");
            let deadline = Instant::now() + STOP_GRACE;
            while Instant::now() < deadline {
                match self.child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => std::thread::sleep(POLL),
                    Err(_) => break,
                }
            }
            if matches!(self.child.try_wait(), Ok(None)) {
                // The group, not just the child: `process_group(0)` made the
                // child its own leader and everything java may spawn sits
                // under it. Same ownership rule supervise documents.
                unsafe {
                    libc::killpg(self.group, libc::SIGKILL);
                }
            }
        }
        let _ = self.child.wait();
        Ok(())
    }

    /// Waits for the `Done` line in `server.log`, failing early when the
    /// process exits.
    fn wait_ready(&mut self) -> Result<()> {
        let deadline = Instant::now() + READY_WAIT;
        let mut offset = 0u64;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(Error::server(format!(
                    "the server exited with {status} before it was ready; log tail: {}",
                    tail(&self.log)
                )));
            }
            if ready_since(&self.log, &mut offset) {
                return Ok(());
            }
            std::thread::sleep(POLL);
        }
        Err(Error::server(format!(
            "no Done line within {}s; log tail: {}",
            READY_WAIT.as_secs(),
            tail(&self.log)
        )))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Reads the log since `offset` and reports whether a `Done` line has
/// arrived. Vanilla writes `Done (12.3s)! For help, type "help"`.
fn ready_since(log: &Path, offset: &mut u64) -> bool {
    let Some(mut file) = File::open(log).ok() else {
        return false;
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return false;
    }
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return false;
    }
    *offset += text.len() as u64;
    // Vanilla writes `[Server thread/INFO]: Done (1.230s)! For help, ...`.
    text.lines().any(|line| line.contains("Done ("))
}

/// The last [`LOG_TAIL`] bytes of a log file.
fn tail(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    if length > LOG_TAIL as u64 {
        let _ = file.seek(SeekFrom::End(-(LOG_TAIL as i64)));
    }
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).trim().to_string()
}
