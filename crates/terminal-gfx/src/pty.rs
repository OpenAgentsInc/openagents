//! Terminals on this computer, in process: a `coder-pty` host that only
//! this process's user drives, and one emulator per terminal.
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
use terminal_core::pty::{Attachment, Event, Transport};
pub use terminal_core::pty::{Program, Session, Sessions};

/// The one principal the in-process host serves.
const PRINCIPAL: &str = "7665727365000000000000000000000000000000000000000000000000000000";
/// The workspace every pane opens in.
const WORKSPACE: &str = "7665727365000000000000000000000000000000000000000000000000000001";
/// Lines each pane keeps after they scroll off.
pub const SCROLLBACK: usize = 5000;
/// Frames waiting per attachment before the host holds output back.
const QUEUE: usize = 4096;

/// Spawns a program as it is.
struct Plain {
    _integration: Option<super::integration::Integration>,
}

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

/// The local user holds every right on this in-process host.
struct Owner;

impl Rights for Owner {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == PRINCIPAL
    }
}

pub(super) fn request() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("{:032x}{:016x}{:016x}", std::process::id(), nanos, n)
}

/// What a pane runs.
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
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        dirs.push(PathBuf::from(home).join(".openagents/bin"));
    }
    let mut found: Vec<PathBuf> = Vec::new();
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    for path in dirs.into_iter().map(|dir| dir.join(&executable)) {
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
pub struct Local {
    host: Arc<Host>,
    shell: PathBuf,
    helper_home: Option<PathBuf>,
    #[cfg(test)]
    test_home: Option<tempfile::TempDir>,
}

impl std::fmt::Debug for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessions")
            .field("shell", &self.shell)
            .finish_non_exhaustive()
    }
}

impl Local {
    /// A host whose terminals start in `root` and run `shell` as a login
    /// shell.
    #[must_use]
    pub fn new(root: &Path, shell: PathBuf) -> Self {
        Local::build(root, shell, None)
    }

    fn build(root: &Path, shell: PathBuf, home: Option<&Path>) -> Self {
        let helper_home = home.map(Path::to_path_buf);
        let mut config = Config::new().workspace(WORKSPACE, root);
        config.emulator = Some(coder_vt::Authority::factory(500));
        if let Some(home) = home {
            config.base_env.retain(|(name, _)| name != "HOME");
            config
                .base_env
                .push(("HOME".into(), home.display().to_string()));
            if cfg!(windows) {
                for name in ["USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP"] {
                    config.base_env.retain(|(key, _)| key != name);
                    config
                        .base_env
                        .push((name.into(), home.display().to_string()));
                }
                config
                    .base_env
                    .retain(|(key, _)| !matches!(key.as_str(), "HOMEDRIVE" | "HOMEPATH"));
            }
        }
        config.shell.clone_from(&shell);
        config.shell_args = if cfg!(windows) {
            Vec::new()
        } else {
            vec!["-l".into()]
        };
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
        let home = home.map(Path::to_path_buf).unwrap_or_else(|| {
            std::env::var_os("ZDOTDIR")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| root.to_path_buf())
        });
        let name = shell.file_name().and_then(|name| name.to_str());
        let integration = match name {
            Some("zsh") => super::integration::Integration::zsh()
                .ok()
                .inspect(|hooks| config.base_env.extend(hooks.zsh_environment(&home))),
            Some("bash") => super::integration::Integration::bash()
                .ok()
                .inspect(|hooks| {
                    // `--rcfile` holds for an interactive shell that is not a
                    // login shell, so the file reads the login profile itself.
                    let start = hooks.bash_start(true);
                    config.shell_args = start.args;
                    config.base_env.extend(start.env);
                }),
            Some("fish") => super::integration::Integration::fish()
                .ok()
                .inspect(|hooks| {
                    config.shell_args = hooks.fish_start();
                }),
            _ => None,
        };
        config.wrap = Some(Arc::new(Plain {
            _integration: integration,
        }));
        Local {
            helper_home,
            #[cfg(test)]
            test_home: None,
            host: Arc::new(Host::new(config, Arc::new(Owner))),
            shell,
        }
    }

    /// A host whose terminals see `root` as their home too, so nothing
    /// they run reads or writes the real one. Tests use it.
    #[must_use]
    pub fn isolated(root: &Path, shell: PathBuf) -> Self {
        Local::build(root, shell, Some(root))
    }

    /// The host for the user's home and login shell.
    #[must_use]
    #[cfg(not(test))]
    pub fn for_user() -> Self {
        let home = user_home();
        let shell = user_shell();
        Local::new(&home, shell)
    }

    #[cfg(test)]
    pub fn for_user() -> Self {
        let home = tempfile::tempdir().expect("isolated terminal home");
        let mut local = Self::isolated(home.path(), PathBuf::from("/bin/sh"));
        local.test_home = Some(home);
        local
    }

    #[must_use]
    pub fn shell(&self) -> &Path {
        &self.shell
    }
}

impl Transport for Local {
    fn shell(&self) -> &Path {
        &self.shell
    }
    fn open(&self, program: &Program, rows: u16, cols: u16) -> Result<Box<dyn Attachment>, String> {
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
        let attach =
            Attach::new(request(), terminal.clone(), Mode::Interact, 0, 1 << 26).with_effects();
        let attachment = match self.host.attach(PRINCIPAL, &attach, Box::new(sink)) {
            Ok((_, Value::Attached { attachment, .. })) => attachment,
            other => {
                let _ = self
                    .host
                    .close(PRINCIPAL, &Close::new(request(), terminal.clone()));
                return Err(format!("terminal attach failed: {other:?}"));
            }
        };
        let group = self.host.process_group(&terminal);
        Ok(Box::new(LocalAttachment {
            attachment,
            terminal,
            frames,
            group,
            host: self.host.clone(),
            ended: false,
        }))
    }

    fn shutdown(&self) {
        self.host.shutdown();
    }
    fn thread_program(&self) -> Option<Program> {
        openagents_terminal()
    }
    fn resolve(&self, name: &str) -> Option<PathBuf> {
        candidates(name).into_iter().next()
    }
    fn request(
        &self,
        request: &terminal_core::bridge::Request,
    ) -> Result<terminal_core::bridge::Connection, String> {
        super::helpers::request(request, self.helper_home.as_deref())
    }
    fn read_thread(&self, thread: &str) -> Receiver<terminal_core::thread::Read> {
        super::helpers::read_thread(thread, self.helper_home.as_deref())
    }
    fn read_run(&self, task: &str) -> Receiver<terminal_core::run::Read> {
        super::helpers::read_run(task, self.helper_home.as_deref())
    }
    fn read_artifact(
        &self,
        task: &str,
        path: &str,
        digest: &str,
    ) -> Receiver<terminal_core::files::Read> {
        super::helpers::read_artifact(task, path, digest, self.helper_home.as_deref())
    }
    fn read_rules(&self) -> Receiver<terminal_core::rules::Read> {
        super::helpers::read_rules(self.helper_home.as_deref())
    }
    fn rule_command(&self, verb: &str, id: &str) -> Receiver<Result<(), String>> {
        super::helpers::rule_command(verb, id, self.helper_home.as_deref())
    }
    fn read_studies(&self, root: &str) -> Receiver<terminal_core::gym::ListRead> {
        super::helpers::read_studies(root, self.helper_home.as_deref())
    }
    fn read_components(&self, root: &str) -> Receiver<terminal_core::gym::ComponentsRead> {
        super::helpers::read_components(root, self.helper_home.as_deref())
    }
    fn plugin_use(
        &self,
        id: &str,
        version: &str,
        digest: &str,
        request: &str,
        workspace: &str,
    ) -> Receiver<terminal_core::gym::UseRead> {
        super::helpers::plugin_use(
            [id, version, digest, request, workspace],
            self.helper_home.as_deref(),
        )
    }
    fn search_knowledge(&self, query: &str) -> Receiver<terminal_core::knowledge::HitsRead> {
        super::helpers::search_knowledge(query, self.helper_home.as_deref())
    }
    fn read_entry(&self, id: &str) -> Receiver<terminal_core::knowledge::ShownRead> {
        super::helpers::read_entry(id, self.helper_home.as_deref())
    }
    fn read_goals(&self) -> Receiver<terminal_core::knowledge::GoalsRead> {
        super::helpers::read_goals(self.helper_home.as_deref())
    }
    fn read_study(&self, dir: &str) -> Receiver<terminal_core::gym::Read> {
        super::helpers::read_study(dir, self.helper_home.as_deref())
    }
    fn task_command(&self, verb: &str, bytes: &[u8]) -> Receiver<terminal_core::run::Sent> {
        super::helpers::task_command(verb, bytes, self.helper_home.as_deref())
    }
    fn ask_agent(
        &self,
        agent: &str,
        text: &str,
        directory: Option<&str>,
    ) -> Receiver<Result<String, String>> {
        super::helpers::ask_agent(agent, text, directory, self.helper_home.as_deref())
    }
    fn git_summary(&self, pane: u64, directory: String) -> Receiver<(u64, String, String)> {
        super::helpers::git_summary(pane, directory, self.helper_home.as_deref())
    }
    fn open_link(&self, target: &str) -> Result<(), String> {
        #[cfg(test)]
        {
            let _ = target;
            return Ok(());
        }
        #[cfg(not(test))]
        {
            #[cfg(windows)]
            {
                return windows_open_link(target);
            }
            #[cfg(not(windows))]
            let opener = if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            };
            #[cfg(not(windows))]
            std::process::Command::new(opener)
                .arg(target)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|_| "The link opener did not start.".into())
        }
    }
    fn clipboard(&self) -> Option<String> {
        #[cfg(test)]
        {
            None
        }
        #[cfg(not(test))]
        {
            arboard::Clipboard::new().ok()?.get_text().ok()
        }
    }
    fn copy(&self, text: &str) -> Result<(), String> {
        #[cfg(test)]
        {
            let _ = text;
            Ok(())
        }
        #[cfg(not(test))]
        {
            arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.set_text(text))
                .map_err(|_| "Could not copy to the clipboard.".into())
        }
    }
}

struct LocalAttachment {
    attachment: String,
    host: Arc<Host>,
    terminal: TerminalRef,
    frames: Receiver<Frame>,
    group: Option<i32>,
    ended: bool,
}
impl Attachment for LocalAttachment {
    fn sharing(
        &self,
        action: terminal_core::sharing::Action,
    ) -> Option<std::sync::mpsc::Receiver<Result<Value, String>>> {
        use coder_pty::share::{SharePause, ShareRequest, Unshare};
        use terminal_core::sharing::Action;
        let result = match action {
            Action::Read => self.host.owner_viewers(PRINCIPAL, &self.terminal),
            Action::Handoff { agent, thread, run } => self.host.hand_off(
                PRINCIPAL,
                &coder_pty::ext::Handoff::new(
                    request(),
                    self.terminal.clone(),
                    self.attachment.clone(),
                    agent,
                    thread,
                    run,
                ),
            ),
            Action::Issue {
                grantee,
                mode,
                expires_at,
            } => self.host.share(
                PRINCIPAL,
                &ShareRequest::new(request(), self.terminal.clone(), grantee, mode, expires_at),
            ),
            Action::Pause(paused) => self.host.pause(
                PRINCIPAL,
                &SharePause::new(request(), self.terminal.clone(), paused),
            ),
            Action::Revoke(Some(share)) => self.host.unshare(
                PRINCIPAL,
                &Unshare::one(request(), self.terminal.clone(), share),
            ),
            Action::Revoke(None) => self
                .host
                .unshare(PRINCIPAL, &Unshare::all(request(), self.terminal.clone())),
        }
        .map(|(_, value)| value)
        .map_err(|why| why.detail);
        let (send, recv) = std::sync::mpsc::channel();
        let _ = send.send(result);
        Some(recv)
    }
    fn host_answers(&self) -> bool {
        true
    }

    fn offer_proposal(
        &self,
        proposal: &terminal_core::proposals::Proposal,
    ) -> Option<Result<(), String>> {
        if !self.host.features().proposals {
            return None;
        }
        Some(
            self.host
                .proposal(
                    PRINCIPAL,
                    &coder_pty::proposal::Request::new(
                        request(),
                        self.terminal.clone(),
                        coder_pty::proposal::Action::Offer {
                            proposal: proposal.clone(),
                        },
                    ),
                )
                .map(|_| ())
                .map_err(|e| e.detail),
        )
    }
    fn decide_proposal(
        &self,
        proposal: &terminal_core::proposals::Proposal,
        approve: bool,
    ) -> Option<Result<coder_pty::proposal::State, String>> {
        if !self.host.features().proposals {
            return None;
        }
        Some(
            self.host
                .proposal(
                    PRINCIPAL,
                    &coder_pty::proposal::Request::new(
                        request(),
                        self.terminal.clone(),
                        coder_pty::proposal::Action::Decide {
                            thread: proposal.thread.clone(),
                            proposal: proposal.id.clone(),
                            revision: proposal.revision,
                            approve,
                            attachment: self.attachment.clone(),
                        },
                    ),
                )
                .map_err(|e| e.detail)
                .and_then(|(_, value)| match value {
                    Value::Proposals { page } => page
                        .entries
                        .first()
                        .map(|e| e.state.clone())
                        .ok_or_else(|| "The host returned no proposal disposition.".into()),
                    _ => Err("The host returned another proposal result.".into()),
                }),
        )
    }

    fn input(&self, bytes: &[u8]) {
        for piece in bytes.chunks(coder_pty::wire::INPUT_MAX) {
            let _ = self.host.input(
                PRINCIPAL,
                &Input::new(request(), self.terminal.clone(), piece),
            );
        }
    }
    fn resize(&self, rows: u16, cols: u16) {
        let _ = self.host.resize(
            PRINCIPAL,
            &Resize::new(request(), self.terminal.clone(), Size::new(rows, cols)),
        );
    }
    fn close(&self) {
        let _ = self
            .host
            .close(PRINCIPAL, &Close::new(request(), self.terminal.clone()));
    }
    fn poll(&mut self) -> Option<Event> {
        if self.ended {
            return None;
        }
        if self.group.is_none() {
            self.group = self.host.process_group(&self.terminal);
        }
        match self.frames.try_recv() {
            Ok(frame) => match frame.body {
                Body::Output { data, .. } => Some(Event::Output(data)),
                Body::Gap { .. } => Some(Event::Gap),
                Body::Exit { exit, .. } => {
                    self.ended = true;
                    Some(Event::End(match (exit.code, exit.signal) {
                        (Some(code), _) => format!("exited {code}"),
                        (_, Some(signal)) => format!("signal {signal}"),
                        _ => "ended".into(),
                    }))
                }
                Body::Detached { .. } => {
                    self.ended = true;
                    Some(Event::End("detached".into()))
                }
                // The host answers queries; the pane projects the original
                // output into glyphs and shell marks.
                Body::Effect { .. } | Body::Typist { .. } | Body::Paused { .. } => self.poll(),
            },
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.ended = true;
                Some(Event::End("detached".into()))
            }
        }
    }
    fn target(&self) -> Option<terminal_core::proposals::Binding> {
        Some(terminal_core::proposals::Binding {
            terminal: self.terminal.terminal.clone(),
            generation: self.terminal.generation.clone(),
            cwd: raw_cwd(self.group?)?,
            shell_directory: None,
            context_digest: String::new(),
        })
    }
    fn directory(&self) -> Option<String> {
        cwd_of(self.group?)
    }
}

pub fn for_user() -> Sessions {
    Sessions(Arc::new(Local::for_user()))
}
pub fn isolated(root: &Path, shell: PathBuf) -> Sessions {
    Sessions(Arc::new(Local::isolated(root, shell)))
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

/// The native profile directory, or this process's current directory.
pub fn user_home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// The configured shell, with the host's native default as a fallback.
pub fn user_shell() -> PathBuf {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| Config::new().shell)
}

#[cfg(windows)]
fn windows_open_link(target: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let target: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
    // SAFETY: both strings are terminated and live throughout the OS call.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize > 32 {
        Ok(())
    } else {
        Err("The link opener did not start.".into())
    }
}
