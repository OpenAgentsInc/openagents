//! Terminals on this computer, in process: a `coder-pty` host that only
//! Verse's own user drives, and one emulator per terminal.
//!
//! This is the demo's local path. The next phase reaches a host through
//! NIP-TERM (`coder_computers::terminal::session::Session`) instead, so the
//! same panes can show another computer's terminals.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use coder_pty::host::{self, Config, Host, Right, Rights};
use coder_pty::wire::{
    Attach, Body, Close, Frame, Input, Launch, Mode, Open, Resize, Size, Status, TerminalRef, Value,
};

/// The one principal the in-process host serves.
const PRINCIPAL: &str = "7665727365000000000000000000000000000000000000000000000000000000";
/// The workspace every pane opens in.
const WORKSPACE: &str = "7665727365000000000000000000000000000000000000000000000000000001";
/// Lines each pane keeps after they scroll off.
pub const SCROLLBACK: usize = 5000;
/// Frames waiting per attachment before the host holds output back.
const QUEUE: usize = 4096;

/// Spawns a program as it is.
struct Plain;

impl host::Wrap for Plain {
    fn command(
        &self,
        program: &Path,
        args: &[std::ffi::OsString],
    ) -> Result<std::process::Command, String> {
        let mut command = std::process::Command::new(program);
        command.args(args);
        Ok(command)
    }
}

/// Verse's user holds every right on its own host.
struct Owner;

impl Rights for Owner {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == PRINCIPAL
    }
}

fn request() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("{:032x}{:016x}{:016x}", std::process::id(), nanos, n)
}

/// What a pane runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Program {
    /// The user's login shell.
    Shell,
    /// An absolute program and its arguments, with a short label.
    Command {
        program: PathBuf,
        args: Vec<String>,
        label: String,
    },
}

impl Program {
    /// `openagents terminal`, when an `openagents` binary that has the
    /// command is found. The answer is kept for the process's life.
    #[must_use]
    pub fn openagents_terminal() -> Option<Program> {
        static FOUND: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
        FOUND
            .get_or_init(|| {
                candidates("openagents")
                    .into_iter()
                    .find(|path| has_terminal(path))
            })
            .clone()
            .map(|program| Program::Command {
                program,
                args: vec!["terminal".into()],
                label: "openagents terminal".into(),
            })
    }

    /// A short name for the title bar.
    #[must_use]
    pub fn label(&self, shell: &Path) -> String {
        match self {
            Program::Shell => shell
                .file_name()
                .map_or_else(|| "shell".into(), |n| n.to_string_lossy().into_owned()),
            Program::Command { label, .. } => label.clone(),
        }
    }
}

/// Where `name` may be, in order: beside this executable (a workspace
/// build, the newest), on `PATH`, then in `~/.openagents/bin`.
#[must_use]
pub fn candidates(name: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        // An example or test binary sits one level below the build's own.
        if dir.ends_with("examples") || dir.ends_with("deps") {
            dirs.extend(dir.parent().map(Path::to_path_buf));
        }
        dirs.push(dir);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".openagents/bin"));
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for path in dirs.into_iter().map(|dir| dir.join(name)) {
        if path.is_absolute() && path.is_file() && !found.contains(&path) {
            found.push(path);
        }
    }
    found
}

/// Whether `program terminal --help` succeeds: older `openagents` builds
/// have no `terminal` command.
fn has_terminal(program: &Path) -> bool {
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(program)
        .args(["terminal", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// The in-process host and what it needs to open terminals.
pub struct Sessions {
    host: Host,
    shell: PathBuf,
}

impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessions")
            .field("shell", &self.shell)
            .finish_non_exhaustive()
    }
}

impl Sessions {
    /// A host whose terminals start in `root` and run `shell` as a login
    /// shell.
    #[must_use]
    pub fn new(root: &Path, shell: PathBuf) -> Self {
        Sessions::build(root, shell, None)
    }

    fn build(root: &Path, shell: PathBuf, home: Option<&Path>) -> Self {
        let mut config = Config::new().workspace(WORKSPACE, root);
        if let Some(home) = home {
            config.base_env.retain(|(name, _)| name != "HOME");
            config
                .base_env
                .push(("HOME".into(), home.display().to_string()));
        }
        config.shell.clone_from(&shell);
        config.shell_args = vec!["-l".into()];
        config
            .base_env
            .push(("COLORTERM".into(), "truecolor".into()));
        config
            .base_env
            .push(("SHELL".into(), shell.display().to_string()));
        config.rate_max = 64 * 1024 * 1024;
        config.idle = Duration::from_secs(7 * 24 * 3600);
        config.terminals_max = 64;
        config.ring_bytes = 4 * 1024 * 1024;
        // The person at this computer drives these terminals, so they run
        // unwrapped: the privacy boundary the resident host applies is for
        // terminals nobody at the Mac would answer a prompt for.
        config.wrap = Some(Arc::new(Plain));
        Sessions {
            host: Host::new(config, Arc::new(Owner)),
            shell,
        }
    }

    /// A host whose terminals see `root` as their home too, so nothing
    /// they run reads or writes the real one. Tests use it.
    #[must_use]
    pub fn isolated(root: &Path, shell: PathBuf) -> Self {
        Sessions::build(root, shell, Some(root))
    }

    /// The host for the user's home and login shell.
    #[must_use]
    pub fn for_user() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_dir())
            .unwrap_or_else(|| PathBuf::from("/"));
        let shell = std::env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|shell| shell.is_absolute() && shell.is_file())
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        Sessions::new(&home, shell)
    }

    #[must_use]
    pub fn shell(&self) -> &Path {
        &self.shell
    }

    /// Opens a terminal running `program` at `rows` by `cols` and attaches
    /// to it.
    pub fn open(&self, program: &Program, rows: u16, cols: u16) -> Result<Session, String> {
        let launch = match program {
            Program::Shell => Launch::Shell,
            Program::Command { program, args, .. } => Launch::Command {
                program: program.display().to_string(),
                args: args.clone(),
            },
        };
        let size = Size::new(rows.max(1), cols.max(1));
        let open = Open::new(request(), WORKSPACE, "", launch, size);
        let terminal = match self.host.open(PRINCIPAL, &open) {
            Ok((Status::Accepted, Value::Opened { terminal, .. })) => terminal,
            Ok(other) => return Err(format!("unexpected open result: {other:?}")),
            Err(refusal) => return Err(format!("{refusal:?}")),
        };
        let (sink, frames) = host::channel(QUEUE);
        let attach = Attach::new(request(), terminal.clone(), Mode::Interact, 0, 1 << 26);
        if let Err(refusal) = self.host.attach(PRINCIPAL, &attach, Box::new(sink)) {
            let _ = self
                .host
                .close(PRINCIPAL, &Close::new(request(), terminal.clone()));
            return Err(format!("{refusal:?}"));
        }
        Ok(Session {
            vt: coder_vt::Terminal::new(rows.into(), cols.into(), SCROLLBACK),
            terminal,
            frames,
            exited: None,
            group: None,
            cwd: None,
            checked: None,
        })
    }

    /// Writes `bytes` to the session's program, in pieces the wire allows.
    pub fn input(&self, session: &Session, bytes: &[u8]) {
        if session.exited.is_some() {
            return;
        }
        for piece in bytes.chunks(coder_pty::wire::INPUT_MAX) {
            let input = Input::new(request(), session.terminal.clone(), piece);
            let _ = self.host.input(PRINCIPAL, &input);
        }
    }

    /// Resizes the session's grid and the program's terminal.
    pub fn resize(&self, session: &mut Session, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        if session.vt.rows() == usize::from(rows) && session.vt.cols() == usize::from(cols) {
            return;
        }
        session.vt.resize(rows.into(), cols.into());
        let resize = Resize::new(request(), session.terminal.clone(), Size::new(rows, cols));
        let _ = self.host.resize(PRINCIPAL, &resize);
    }

    /// Ends the session's process group.
    pub fn close(&self, session: &Session) {
        let _ = self
            .host
            .close(PRINCIPAL, &Close::new(request(), session.terminal.clone()));
    }

    /// Applies waiting output to the session's grid, up to about
    /// `max_bytes` and until `deadline`, and answers the program's device
    /// queries. Output past either stays queued for the next call; the
    /// host's ring holds it, and a gap marks what overflowed. Returns
    /// whether anything changed and how many output bytes it parsed.
    pub fn pump(&self, session: &mut Session, max_bytes: usize, deadline: Instant) -> (bool, u64) {
        let mut changed = false;
        let mut bytes = 0u64;
        loop {
            if bytes as usize >= max_bytes || (bytes > 0 && Instant::now() >= deadline) {
                break;
            }
            match session.frames.try_recv() {
                Ok(frame) => {
                    changed = true;
                    if let Body::Output { data, .. } = &frame.body {
                        bytes += data.len() as u64;
                    }
                    session.apply(&frame);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if session.exited.is_none() {
                        session.exited = Some("detached".into());
                        changed = true;
                    }
                    break;
                }
            }
        }
        let replies = session.vt.take_replies();
        if !replies.is_empty() {
            self.input(session, &replies);
        }
        let now = Instant::now();
        if session
            .checked
            .is_none_or(|at| now.duration_since(at) > Duration::from_secs(1))
        {
            session.checked = Some(now);
            if session.group.is_none() {
                session.group = self.host.process_group(&session.terminal);
            }
            session.cwd = session.group.and_then(cwd_of);
        }
        (changed, bytes)
    }

    /// Ends every terminal's process group.
    pub fn shutdown(&self) {
        self.host.shutdown();
    }
}

/// One terminal and its emulator.
pub struct Session {
    pub vt: coder_vt::Terminal,
    terminal: TerminalRef,
    frames: Receiver<Frame>,
    /// How the program ended, once it has.
    pub exited: Option<String>,
    group: Option<i32>,
    /// The shell's working directory, when the system says.
    pub cwd: Option<String>,
    checked: Option<Instant>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("vt", &self.vt)
            .field("exited", &self.exited)
            .finish_non_exhaustive()
    }
}

impl Session {
    fn apply(&mut self, frame: &Frame) {
        match &frame.body {
            Body::Output { data, .. } => self.vt.feed(data),
            Body::Gap { .. } => self.vt.mark("[output skipped]"),
            Body::Exit { exit, .. } => {
                self.exited = Some(match (exit.code, exit.signal) {
                    (Some(code), _) => format!("exited {code}"),
                    (_, Some(signal)) => format!("signal {signal}"),
                    _ => "ended".into(),
                });
            }
            Body::Detached { .. } => {
                if self.exited.is_none() {
                    self.exited = Some("detached".into());
                }
            }
        }
    }
}

/// The working directory of process `pid`, home shortened to `~`.
fn cwd_of(pid: i32) -> Option<String> {
    let path = raw_cwd(pid)?;
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());
    Some(match home {
        Some(home) if path == home => "~".into(),
        Some(home) if path.starts_with(&format!("{home}/")) => format!("~{}", &path[home.len()..]),
        _ => path,
    })
}

#[cfg(target_os = "macos")]
fn raw_cwd(pid: i32) -> Option<String> {
    // SAFETY: `info` is a plain C struct the call fills, sized as passed.
    unsafe {
        let mut info: libc::proc_vnodepathinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
        let got = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&raw mut info).cast(),
            size,
        );
        if got != size {
            return None;
        }
        let path = std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast());
        Some(path.to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "linux")]
fn raw_cwd(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()
        .map(|p| p.display().to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn raw_cwd(_: i32) -> Option<String> {
    None
}
