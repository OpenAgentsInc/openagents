//! The real [`Host`]: git, the dev-bundle build script, `codesign`, a
//! headless smoke run, and LaunchServices. Every command has a time limit.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::layout::Layout;
use crate::record::{self, LogEntry, Outcome, Pending, Phase};
use crate::upgrade::Host;

const FETCH_LIMIT: Duration = Duration::from_secs(120);
const GIT_LIMIT: Duration = Duration::from_secs(60);
/// A cold build of the app is about 900 crates.
const BUILD_LIMIT: Duration = Duration::from_secs(3 * 60 * 60);
const CODESIGN_LIMIT: Duration = Duration::from_secs(180);
/// How long the staged app's headless engine has to answer.
const SMOKE_LIMIT: Duration = Duration::from_secs(120);
/// How long the new app has to start after the old one quits before the old
/// one is put back.
const REOPEN_LIMIT: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_secs(2);

pub struct SystemHost {
    pub layout: Layout,
    /// How long to wait for the app to install the bundle and reopen.
    pub wait: Duration,
    /// Open bundles through LaunchServices (`open`). Tests run the binary
    /// directly instead.
    pub launch_services: bool,
    /// Progress lines go here (the CLI prints them).
    pub progress: Box<dyn FnMut(&str) + Send>,
}

impl SystemHost {
    #[must_use]
    pub fn new(layout: Layout, wait: Duration) -> SystemHost {
        SystemHost {
            layout,
            wait,
            launch_services: true,
            progress: Box::new(|line| eprintln!("{line}")),
        }
    }

    fn say(&mut self, line: &str) {
        (self.progress)(line);
    }

    fn git(&self, args: &[&str], limit: Duration) -> Result<String, String> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.layout.checkout)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0");
        let output = run(command, limit, None)?;
        if output.ok {
            Ok(output.stdout.trim().to_owned())
        } else {
            Err(format!("git {} failed: {}", args.join(" "), output.tail()))
        }
    }

    fn build_log(&self, id: &str) -> PathBuf {
        self.layout.state_dir().join(format!("build-{id}.log"))
    }

    /// Remove stages other than `keep` and the previous bundle.
    fn prune_stages(&self, keep: &str) {
        let Ok(entries) = std::fs::read_dir(self.layout.work_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name != keep && name != "previous" {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    fn smoke(&mut self, staged: &Path) -> Result<(), String> {
        let binary = Layout::binary(staged);
        let scratch = self.layout.state_dir().join("smoke");
        let _ = std::fs::remove_dir_all(&scratch);
        let home = scratch.join("home");
        let data = scratch.join("data");
        std::fs::create_dir_all(&home).map_err(|e| format!("couldn't make a scratch home: {e}"))?;
        std::fs::create_dir_all(&data).map_err(|e| format!("couldn't make scratch data: {e}"))?;
        let port = free_port()?;
        let env = |command: &mut Command| {
            command
                .env_clear()
                .env("HOME", &home)
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("OPENAGENTS_DATA_DIR", &data)
                .env("OPENAGENTS_IPC_PORT", port.to_string())
                .env("OPENAGENTS_AUTO_UPDATE", "0")
                .env("OPENAGENTS_AGENT_BUILD_GATE", "0")
                .env("OPENAGENTS_WORKOS_CLIENT_ID", "")
                // A named org skips the saved OpenAgents sign-in, and file
                // storage skips Antigravity's: no Keychain, so no prompt.
                .env("OPENAGENTS_ORG_ID", "host-upgrade-smoke")
                .env("AGY_ACP_FORCE_FILE_STORAGE", "1");
        };
        let mut version = Command::new(&binary);
        version.arg("--version");
        env(&mut version);
        let output = run(version, Duration::from_secs(30), None)?;
        if !output.ok {
            return Err(format!("the new app doesn't start: {}", output.tail()));
        }
        let log = std::fs::File::create(scratch.join("headless.log"))
            .map_err(|e| format!("couldn't write the smoke log: {e}"))?;
        let mut headless = Command::new(&binary);
        {
            use std::os::unix::process::CommandExt as _;
            headless.process_group(0);
        }
        headless
            .arg("headless")
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log);
        env(&mut headless);
        let mut child = headless
            .spawn()
            .map_err(|e| format!("couldn't start the new app's engine: {e}"))?;
        let started = Instant::now();
        let mut answered = false;
        while started.elapsed() < SMOKE_LIMIT {
            if let Ok(Some(status)) = child.try_wait() {
                let tail = tail_of(&scratch.join("headless.log"));
                return Err(format!(
                    "the new app's engine exited ({status}) at start: {tail}"
                ));
            }
            let mut probe = Command::new(&binary);
            probe.arg("sync");
            env(&mut probe);
            if run(probe, Duration::from_secs(20), None).is_ok_and(|o| o.ok) {
                answered = true;
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        stop(&mut child);
        if !answered {
            let tail = tail_of(&scratch.join("headless.log"));
            return Err(format!(
                "the new app's engine didn't answer within {} s: {tail}",
                SMOKE_LIMIT.as_secs()
            ));
        }
        let _ = std::fs::remove_dir_all(&scratch);
        Ok(())
    }

    /// An app from before hand-overs can't wait for its own quiet moment, so
    /// this waits for one from outside (no keyboard or mouse input for
    /// [`crate::quiet::IDLE`]), posts a notice, asks the app to quit as ⌘Q
    /// would, swaps the bundle, and opens it. Its keepers keep running.
    fn restart_older_app(&mut self, pending: &Pending, running: &[u32]) -> Result<(), String> {
        self.say("The running app predates hand-overs; waiting for a quiet moment to restart it…");
        let started = Instant::now();
        let mut noticed: Option<Instant> = None;
        loop {
            if started.elapsed() > self.wait {
                return Err("the Mac was never idle long enough to restart the app".into());
            }
            let idle = input_idle().unwrap_or_default();
            match noticed {
                _ if idle < crate::quiet::IDLE => noticed = None,
                None => {
                    notify(&format!(
                        "OpenAgents restarts in {} seconds to install an upgrade ({}). Your chats keep running.",
                        crate::quiet::NOTICE.as_secs(),
                        crate::short(&pending.to)
                    ));
                    noticed = Some(Instant::now());
                }
                Some(at) if at.elapsed() >= crate::quiet::NOTICE => break,
                Some(_) => {}
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        self.say("Quitting the app…");
        quit_app(&self.layout.bundle, running)?;
        crate::swap::swap_in(&pending.staged, &pending.bundle, &pending.previous)?;
        let restarting = Pending {
            phase: Phase::Restarting {
                at_ms: crate::now_ms(),
                chat: None,
            },
            ..pending.clone()
        };
        record::write_pending(&self.layout.state_dir(), &restarting)
            .map_err(|e| format!("couldn't record the restart: {e}"))?;
        self.launch(&pending.bundle, None)
    }

    fn launch(&self, bundle: &Path, chat: Option<&str>) -> Result<(), String> {
        let (program, args) = self.launch_command(bundle, chat);
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("couldn't open {}: {e}", bundle.display()))
    }

    /// The command that opens `bundle` with this app's data folder and port,
    /// onto `chat` (a conversation link) when given.
    #[must_use]
    pub fn launch_command(&self, bundle: &Path, chat: Option<&str>) -> (PathBuf, Vec<OsString>) {
        launch_command(&self.layout, bundle, chat, self.launch_services)
    }

    fn roll_back(&mut self, pending: &Pending, why: &str) -> LogEntry {
        let detail = match crate::swap::roll_back(&pending.bundle, &pending.previous) {
            Ok(()) => {
                let chat = match &pending.phase {
                    Phase::Restarting { chat, .. } => chat.clone(),
                    Phase::Ready => None,
                };
                let opened = self.launch(&pending.bundle, chat.as_deref());
                match opened {
                    Ok(()) => format!("{why}; the previous app is back and open"),
                    Err(e) => format!("{why}; the previous app is back but {e}"),
                }
            }
            Err(e) => format!("{why}; putting the previous app back failed: {e}"),
        };
        record::clear_pending(&self.layout.state_dir());
        let entry = LogEntry::new(
            &pending.id,
            crate::now_ms(),
            &pending.requested_by,
            &pending.reason,
            Outcome::RolledBack,
            detail,
        )
        .commits(pending.from.as_deref(), Some(&pending.to));
        self.log(&entry);
        entry
    }
}

impl Host for SystemHost {
    /// Only the bundle's own stamp counts: the checkout may have moved
    /// since the bundle was built. A bundle from before stamps is unknown,
    /// so it's never up to date.
    fn installed_commit(&mut self) -> Option<String> {
        Layout::bundle_commit(&self.layout.bundle)
    }

    fn checkout_commit(&mut self) -> Option<String> {
        self.git(&["rev-parse", "HEAD"], GIT_LIMIT).ok()
    }

    fn fetch_main(&mut self) -> Result<String, String> {
        self.say("Fetching main…");
        self.git(&["fetch", "--quiet", "origin", "main"], FETCH_LIMIT)?;
        self.git(&["rev-parse", "origin/main"], GIT_LIMIT)
    }

    fn checkout(&mut self, commit: &str) -> Result<(), String> {
        let changes = self.git(
            &["status", "--porcelain", "--untracked-files=no"],
            GIT_LIMIT,
        )?;
        if !changes.is_empty() {
            return Err(format!(
                "{} has uncommitted changes, so it wasn't moved",
                self.layout.checkout.display()
            ));
        }
        self.git(&["checkout", "--quiet", "--detach", commit], GIT_LIMIT)
            .map(|_| ())
    }

    fn build(&mut self, id: &str) -> Result<PathBuf, String> {
        self.prune_stages(id);
        let staged = self.layout.stage(id);
        if let Some(parent) = staged.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
        }
        let log = self.build_log(id);
        self.say(&format!(
            "Building in {} (log: {})…",
            self.layout.target_dir.display(),
            log.display()
        ));
        let mut command = Command::new("/bin/bash");
        command
            .arg(
                self.layout
                    .app_dir()
                    .join("scripts")
                    .join("run-macos-dev.sh"),
            )
            .current_dir(self.layout.app_dir())
            .env("OPENAGENTS_DEV_APP_OUT", &staged)
            .env("OPENAGENTS_DEV_DATA_DIR", &self.layout.data_dir)
            .env("OPENAGENTS_DEV_IPC_PORT", self.layout.ipc_port.to_string())
            .env("CARGO_TARGET_DIR", &self.layout.target_dir)
            // The build gate waits for a build slot but keeps this target.
            .env("OPENAGENTS_KEEP_TARGET_DIR", "1");
        for (key, value) in build_gate_vars() {
            command.env(key, value);
        }
        let output = run(command, BUILD_LIMIT, Some(&log))?;
        if !output.ok {
            return Err(format!(
                "the build failed ({}): {}",
                log.display(),
                first_error(&log).unwrap_or_else(|| tail_of(&log))
            ));
        }
        if !Layout::binary(&staged).is_file() {
            return Err(format!("the build made no app at {}", staged.display()));
        }
        Ok(staged)
    }

    fn verify(&mut self, staged: &Path, commit: &str) -> Result<(), String> {
        self.say("Checking the new app…");
        let mut codesign = Command::new("codesign");
        codesign
            .args(["--verify", "--deep", "--strict"])
            .arg(staged);
        let signed = run(codesign, CODESIGN_LIMIT, None)?;
        if !signed.ok {
            return Err(format!("its signature doesn't verify: {}", signed.tail()));
        }
        let plist = staged.join("Contents").join("Info.plist");
        let data = plist_value(&plist, "LSEnvironment.OPENAGENTS_DATA_DIR")?;
        if Path::new(&data) != self.layout.data_dir {
            return Err(format!(
                "it would open the data folder {data}, not {}",
                self.layout.data_dir.display()
            ));
        }
        let port = plist_value(&plist, "LSEnvironment.OPENAGENTS_IPC_PORT")?;
        if port != self.layout.ipc_port.to_string() {
            return Err(format!(
                "it would listen on port {port}, not {}",
                self.layout.ipc_port
            ));
        }
        match Layout::bundle_commit(staged) {
            Some(built) if built == commit => {}
            Some(built) => return Err(format!("it was built from {built}, not {commit}")),
            None => return Err("it doesn't say which commit it was built from".into()),
        }
        self.smoke(staged)
    }

    fn bundle_paths(&mut self) -> (PathBuf, PathBuf) {
        (self.layout.bundle.clone(), self.layout.previous())
    }

    fn app_running(&mut self) -> bool {
        !app_pids(&self.layout.bundle).is_empty()
    }

    fn hand_over(&mut self, pending: &Pending) -> Result<(), String> {
        let running = app_pids(&self.layout.bundle);
        let takes =
            record::watcher(&self.layout.state_dir()).is_some_and(|pid| running.contains(&pid));
        if !takes {
            return self.restart_older_app(pending, &running);
        }
        self.say("Handing the new app over; it restarts at the next quiet moment…");
        record::write_pending(&self.layout.state_dir(), pending)
            .map_err(|e| format!("couldn't write the hand-over: {e}"))
    }

    fn install_now(&mut self, pending: &Pending) -> Result<(), String> {
        self.say("The app isn't running; installing and opening it…");
        crate::swap::swap_in(&pending.staged, &pending.bundle, &pending.previous)?;
        let restarting = Pending {
            phase: Phase::Restarting {
                at_ms: crate::now_ms(),
                chat: None,
            },
            ..pending.clone()
        };
        record::write_pending(&self.layout.state_dir(), &restarting)
            .map_err(|e| format!("couldn't record the install: {e}"))?;
        self.launch(&pending.bundle, None)
    }

    fn wait_for_outcome(&mut self, pending: &Pending) -> Option<LogEntry> {
        let state_dir = self.layout.state_dir();
        let old: Vec<u32> = app_pids(&pending.bundle);
        let started = Instant::now();
        let mut said_restarting = false;
        while started.elapsed() < self.wait {
            if let Some(entry) = record::outcome_of(&state_dir, &pending.id) {
                return Some(entry);
            }
            match record::read_pending(&state_dir) {
                // Replaced by a newer upgrade.
                Some(current) if current.id != pending.id => return None,
                None => return record::outcome_of(&state_dir, &pending.id),
                Some(current) => {
                    if let Phase::Restarting { at_ms, .. } = current.phase {
                        if !said_restarting {
                            said_restarting = true;
                            self.say("The app is restarting onto the new build…");
                        }
                        let running = app_pids(&pending.bundle);
                        let new_running = running.iter().any(|pid| !old.contains(pid));
                        let since = Duration::from_millis(crate::now_ms().saturating_sub(at_ms));
                        if !new_running && since > REOPEN_LIMIT && running.is_empty() {
                            return Some(self.roll_back(
                                &current,
                                &format!(
                                    "the new app didn't start within {} s",
                                    REOPEN_LIMIT.as_secs()
                                ),
                            ));
                        }
                    }
                }
            }
            std::thread::sleep(POLL);
        }
        None
    }

    fn log(&mut self, entry: &LogEntry) {
        if let Err(e) = record::log(&self.layout.state_dir(), entry) {
            self.say(&format!("couldn't write the upgrade log: {e}"));
        }
    }

    fn now_ms(&mut self) -> u64 {
        crate::now_ms()
    }
}

/// The command that opens `bundle` for `layout`'s data folder and port.
/// LaunchServices (`open`) by default, so macOS privacy attributes the app
/// to itself; `--env` repeats what `scripts/run-macos-dev.sh` passes.
#[must_use]
pub fn launch_command(
    layout: &Layout,
    bundle: &Path,
    chat: Option<&str>,
    launch_services: bool,
) -> (PathBuf, Vec<OsString>) {
    if !launch_services {
        let args = chat
            .map(|link| vec![OsString::from(link)])
            .unwrap_or_default();
        return (Layout::binary(bundle), args);
    }
    let mut args: Vec<OsString> = Vec::new();
    for (key, value) in [
        ("OPENAGENTS_AUTO_UPDATE", "0".to_owned()),
        ("OPENAGENTS_DATA_DIR", layout.data_dir.display().to_string()),
        ("OPENAGENTS_IPC_PORT", layout.ipc_port.to_string()),
    ] {
        args.push("--env".into());
        args.push(format!("{key}={value}").into());
    }
    args.push(bundle.into());
    if let Some(link) = chat {
        args.push("--args".into());
        args.push(link.into());
    }
    (PathBuf::from("/usr/bin/open"), args)
}

/// Start `program args` once process `pid` has exited, from a detached shell
/// that outlives it. The app calls this, then quits.
///
/// # Errors
/// The shell can't be started.
pub fn relaunch_after_exit(pid: u32, program: &Path, args: &[OsString]) -> Result<(), String> {
    use std::os::unix::process::CommandExt as _;
    let script = r#"pid=$1; shift
while /bin/kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
exec "$@""#;
    Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg("openagents-host-upgrade")
        .arg(pid.to_string())
        .arg(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("couldn't start the relauncher: {e}"))
}

/// The pids of the app at `bundle`: its binary with no subcommand (keepers,
/// MCP bridges, and the upgrader are the same binary with one).
#[must_use]
pub fn app_pids(bundle: &Path) -> Vec<u32> {
    let binary = Layout::binary(bundle).display().to_string();
    let Ok(output) = Command::new("/bin/ps")
        .args(["-axo", "pid=,command="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, command) = line.split_once(' ')?;
            let rest = command.strip_prefix(binary.as_str())?;
            let app = rest.is_empty()
                || rest
                    .strip_prefix(' ')
                    .is_some_and(|args| args.starts_with("openagents:") || args.starts_with('-'));
            app.then(|| pid.parse().ok()).flatten()
        })
        .collect()
}

/// How long since the last keyboard or mouse input on this Mac.
#[must_use]
pub fn input_idle() -> Option<Duration> {
    let mut command = Command::new("/usr/sbin/ioreg");
    command.args(["-c", "IOHIDSystem"]);
    let output = run(command, Duration::from_secs(10), None).ok()?;
    let nanos: u64 = output
        .stdout
        .lines()
        .find(|line| line.contains("\"HIDIdleTime\""))?
        .rsplit('=')
        .next()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_nanos(nanos))
}

/// A macOS notification, for an app too old to show the notice itself.
fn notify(text: &str) {
    let mut command = Command::new("/usr/bin/osascript");
    command
        .arg("-e")
        .arg(
            "on run argv\ndisplay notification (item 1 of argv) with title \"OpenAgents\"\nend run",
        )
        .arg(text);
    let _ = run(command, Duration::from_secs(10), None);
}

/// Ask the app to quit as ⌘Q would, then stop it if it hasn't within a
/// minute. Only the app's own pids are signalled, never its keepers.
fn quit_app(bundle: &Path, pids: &[u32]) -> Result<(), String> {
    let id = plist_value(
        &bundle.join("Contents").join("Info.plist"),
        "CFBundleIdentifier",
    )?;
    let mut command = Command::new("/usr/bin/osascript");
    command
        .arg("-e")
        .arg("on run argv\ntell application id (item 1 of argv) to quit\nend run")
        .arg(&id);
    let _ = run(command, Duration::from_secs(30), None);
    let gone = |limit: Duration| {
        let started = Instant::now();
        while started.elapsed() < limit {
            if pids.iter().all(|pid| !alive(*pid)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    };
    if gone(Duration::from_secs(60)) {
        return Ok(());
    }
    for pid in pids {
        if let Ok(pid) = libc::pid_t::try_from(*pid) {
            // SAFETY: the app's own pid, from `app_pids`; keepers run under
            // other pids in sessions of their own.
            unsafe { libc::kill(pid, libc::SIGTERM) };
        }
    }
    if gone(Duration::from_secs(20)) {
        Ok(())
    } else {
        Err("the app didn't quit".into())
    }
}

/// Whether process `pid` is alive.
#[must_use]
pub fn alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// The build gate's variables, so the build waits for a build slot like an
/// agent's would. Plain `PATH` without the gate.
fn build_gate_vars() -> Vec<(OsString, OsString)> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    match coder_lease::agent_builds::Gate::from_env() {
        Some(gate) if gate.install().is_ok() => gate.vars(Some(&path)),
        _ => vec![(OsString::from("PATH"), path)],
    }
}

/// A variable a bundle's `Info.plist` sets for the app (`LSEnvironment`),
/// such as the development bundle's data folder.
#[must_use]
pub fn bundle_env(bundle: &Path, name: &str) -> Option<String> {
    plist_value(
        &bundle.join("Contents").join("Info.plist"),
        &format!("LSEnvironment.{name}"),
    )
    .ok()
    .filter(|value| !value.is_empty())
}

fn plist_value(plist: &Path, key: &str) -> Result<String, String> {
    let mut command = Command::new("/usr/bin/plutil");
    command.args(["-extract", key, "raw", "-o", "-"]).arg(plist);
    let output = run(command, Duration::from_secs(20), None)?;
    if output.ok {
        Ok(output.stdout.trim().to_owned())
    } else {
        Err(format!("its Info.plist has no {key}"))
    }
}

fn free_port() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|e| format!("couldn't find a free port: {e}"))
}

struct Output {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Output {
    fn tail(&self) -> String {
        let text = if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        };
        last_lines(text, 3)
    }
}

/// Run `command` for at most `limit`, killing its process group past it.
/// With `log`, output goes there instead of being captured.
fn run(mut command: Command, limit: Duration, log: Option<&Path>) -> Result<Output, String> {
    use std::os::unix::process::CommandExt as _;
    let name = format!("{:?}", command.get_program());
    let (out_path, err_path, cleanup) = match log {
        Some(path) => (path.to_path_buf(), path.to_path_buf(), false),
        None => {
            let dir = std::env::temp_dir();
            let stamp = format!("{}-{}", std::process::id(), crate::now_ms());
            (
                dir.join(format!("oa-host-upgrade-{stamp}.out")),
                dir.join(format!("oa-host-upgrade-{stamp}.err")),
                true,
            )
        }
    };
    let stdout = std::fs::File::create(&out_path).map_err(|e| format!("{name}: {e}"))?;
    let stderr = if log.is_some() {
        stdout.try_clone().map_err(|e| format!("{name}: {e}"))?
    } else {
        std::fs::File::create(&err_path).map_err(|e| format!("{name}: {e}"))?
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .map_err(|e| format!("couldn't run {name}: {e}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() >= limit => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("{name}: {e}")),
        }
    };
    let Some(status) = status else {
        stop(&mut child);
        return Err(format!(
            "{name} took longer than {} s and was stopped",
            limit.as_secs()
        ));
    };
    let read = |path: &Path| std::fs::read_to_string(path).unwrap_or_default();
    let output = Output {
        ok: status.success(),
        stdout: if log.is_some() {
            String::new()
        } else {
            read(&out_path)
        },
        stderr: if log.is_some() {
            String::new()
        } else {
            read(&err_path)
        },
    };
    if cleanup {
        let _ = std::fs::remove_file(&out_path);
        let _ = std::fs::remove_file(&err_path);
    }
    Ok(output)
}

/// Stop a child's process group: TERM, then KILL after ten seconds.
fn stop(child: &mut std::process::Child) {
    let Ok(group) = libc::pid_t::try_from(child.id()) else {
        let _ = child.kill();
        return;
    };
    // SAFETY: signalling the child's own process group (every child here is
    // started with process_group(0)).
    unsafe { libc::kill(-group, libc::SIGTERM) };
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // SAFETY: as above.
    unsafe { libc::kill(-group, libc::SIGKILL) };
    let _ = child.wait();
}

fn tail_of(path: &Path) -> String {
    last_lines(&std::fs::read_to_string(path).unwrap_or_default(), 3)
}

/// The first compiler error in a build log, with the line after it.
fn first_error(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|line| line.starts_with("error"))?;
    Some(
        lines[at..lines.len().min(at + 2)]
            .iter()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines[lines.len().saturating_sub(count)..]
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(dir: &Path) -> Layout {
        Layout {
            checkout: dir.join("checkout"),
            target_dir: dir.join("target"),
            bundle: dir.join("App.app"),
            data_dir: dir.join("data dir"),
            ipc_port: 49777,
        }
    }

    #[test]
    fn opens_through_launch_services_with_the_data_folder_and_chat() {
        let dir = Path::new("/w");
        let layout = layout(dir);
        let (program, args) = launch_command(
            &layout,
            &layout.bundle,
            Some("openagents://open/chat/c1"),
            true,
        );
        assert_eq!(program, Path::new("/usr/bin/open"));
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--env",
                "OPENAGENTS_AUTO_UPDATE=0",
                "--env",
                "OPENAGENTS_DATA_DIR=/w/data dir",
                "--env",
                "OPENAGENTS_IPC_PORT=49777",
                "/w/App.app",
                "--args",
                "openagents://open/chat/c1",
            ]
        );
        let (program, args) = launch_command(&layout, &layout.bundle, None, false);
        assert_eq!(program, Path::new("/w/App.app/Contents/MacOS/openagents"));
        assert!(args.is_empty());
    }

    #[test]
    fn commands_past_their_limit_are_stopped() {
        let mut slow = Command::new("/bin/sleep");
        slow.arg("30");
        let started = Instant::now();
        let err = run(slow, Duration::from_millis(300), None).err().unwrap();
        assert!(err.contains("was stopped"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(15));
        let mut quick = Command::new("/bin/echo");
        quick.arg("hi");
        let output = run(quick, Duration::from_secs(10), None).unwrap();
        assert!(output.ok);
        assert_eq!(output.stdout.trim(), "hi");
    }

    #[test]
    fn build_logs_name_the_first_error() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("build.log");
        std::fs::write(
            &log,
            "   Compiling a v1\nwarning: unused\nerror[E0425]: cannot find value `x`\n  --> src/lib.rs:1:1\nerror: aborting\n",
        )
        .unwrap();
        assert_eq!(
            first_error(&log).unwrap(),
            "error[E0425]: cannot find value `x` --> src/lib.rs:1:1"
        );
    }
}
