//! Linux: the host as a systemd user unit, the session's lock hint, and
//! the desktop's clipboard and folder chooser.
//!
//! - **Keys.** The host (`coder host serve --keychain`) keeps its keys in
//!   the Secret Service (GNOME Keyring, KWallet, KeePassXC), under the
//!   service `com.openagents.desktop` and the same accounts as on a Mac
//!   (`coder_host::serve::keys::SecretService`). The window reads none.
//! - **Login agent.** `coder host serve` runs as the systemd user unit
//!   [`UNIT`], enabled for `default.target`, so it starts at login and
//!   keeps running when the window closes. Registering writes the unit,
//!   reloads the user manager, and enables and starts it. Inside an
//!   AppImage the unit runs the AppImage file itself (`AppRun` dispatches
//!   `coder ...`), because the AppImage's mount point changes every launch.
//! - **Control socket.** `$XDG_RUNTIME_DIR/openagents/control.sock`
//!   ([`openagents_desktop::control::socket_path`]); the host checks every
//!   peer's user ID with `SO_PEERCRED`.
//!
//! None of this reads a secret. The sign-in check looks only at whether a
//! credential file exists, never its contents.

use openagents_desktop::model::{Agent, Agents};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The systemd user unit that runs the host; the same name as the Mac's
/// login agent label.
pub const UNIT: &str = "com.openagents.desktop.host.service";

/// The host binary's name, beside the window's binary when installed.
const CODER: &str = "coder";

/// What the unit runs `coder` with: the same as the Mac's login agent
/// (`com.openagents.desktop.host.plist`).
pub const HOST_ARGS: [&str; 5] = ["host", "serve", "--keychain", "--iroh", "--control"];

/// The directory this executable runs from.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// The AppImage file this process runs from, when it runs from one.
fn appimage() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
}

/// The `coder` the app runs: the installed one beside this executable
/// (`/usr/lib/openagents/` from the .deb, `usr/lib/openagents/` inside the
/// AppImage), else one on `PATH`, else the one an earlier setup installed.
pub fn coder_path() -> Option<PathBuf> {
    if let Some(bundled) = exe_dir().map(|dir| dir.join(CODER))
        && bundled.is_file()
    {
        return Some(bundled);
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(CODER);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let installed = home.join(".openagents/bin/coder");
    installed.exists().then_some(installed)
}

// ------------------------------------------------------------ login agent

/// How the unit starts the host: a program and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl HostCommand {
    /// `coder host serve ...` ([`HOST_ARGS`]) from an installation:
    /// through the AppImage file when there is one, else the `coder` beside
    /// the window's binary.
    pub fn for_install(appimage: Option<&Path>, exe_dir: &Path) -> HostCommand {
        let args = HOST_ARGS.iter().map(|arg| arg.to_string());
        match appimage.filter(|path| path.is_absolute()) {
            Some(appimage) => HostCommand {
                program: appimage.to_path_buf(),
                args: std::iter::once(CODER.to_string()).chain(args).collect(),
            },
            None => HostCommand {
                program: exe_dir.join(CODER),
                args: args.collect(),
            },
        }
    }
}

/// Quotes one `ExecStart=` or `Environment=` word for systemd: in double
/// quotes, with `\`, `"`, and systemd's `%` and `$` expansions escaped.
fn systemd_quote(value: &str) -> Result<String, String> {
    if value.chars().any(char::is_control) {
        return Err("a start-up argument contains a control character".into());
    }
    Ok(format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    ))
}

/// The unit file for `command`, with `path` as the host's `PATH`, so
/// Codex and Claude Code installed for this user are found at login.
pub fn render_unit(command: &HostCommand, path: &str) -> Result<String, String> {
    if !command.program.is_absolute() {
        return Err("the host program must be an absolute path".into());
    }
    let program = command
        .program
        .to_str()
        .ok_or("the host program's path is not UTF-8")?;
    let exec = std::iter::once(program)
        .chain(command.args.iter().map(String::as_str))
        .map(systemd_quote)
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    Ok([
        "[Unit]".to_string(),
        "Description=OpenAgents (lets your phone reach this computer)".into(),
        "StartLimitIntervalSec=300".into(),
        "StartLimitBurst=5".into(),
        String::new(),
        "[Service]".into(),
        "Type=simple".into(),
        format!("ExecStart={exec}"),
        "Restart=on-failure".into(),
        "RestartSec=10".into(),
        "KillMode=control-group".into(),
        "UMask=0077".into(),
        "NoNewPrivileges=yes".into(),
        format!("Environment={}", systemd_quote(&format!("PATH={path}"))?),
        String::new(),
        "[Install]".into(),
        "WantedBy=default.target".into(),
        String::new(),
    ]
    .join("\n"))
}

/// The user unit directory: `$XDG_CONFIG_HOME/systemd/user`, else
/// `~/.config/systemd/user`.
pub fn unit_dir(config_home: Option<&Path>, home: &Path) -> PathBuf {
    config_home
        .filter(|dir| dir.is_absolute())
        .map_or_else(|| home.join(".config"), Path::to_path_buf)
        .join("systemd/user")
}

/// Runs `systemctl --user`; a trait so tests can record the calls.
pub trait Systemctl {
    /// Whether the call succeeded, and its standard output.
    fn run(&mut self, args: &[&str]) -> io::Result<(bool, String)>;
}

/// The real `systemctl --user`.
#[derive(Debug, Default)]
pub struct UserSystemctl;

impl Systemctl for UserSystemctl {
    fn run(&mut self, args: &[&str]) -> io::Result<(bool, String)> {
        let output = Command::new("systemctl")
            .arg("--user")
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        Ok((
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ))
    }
}

fn checked(systemctl: &mut dyn Systemctl, args: &[&str]) -> Result<String, String> {
    match systemctl.run(args) {
        Ok((true, stdout)) => Ok(stdout),
        Ok((false, _)) => Err(format!("systemctl --user {} failed", args.join(" "))),
        Err(error) => Err(format!("systemctl did not run: {error}")),
    }
}

/// The host's systemd user unit.
#[derive(Debug, Clone)]
pub struct LoginAgent {
    unit_dir: PathBuf,
}

impl LoginAgent {
    pub fn new(unit_dir: PathBuf) -> LoginAgent {
        LoginAgent { unit_dir }
    }

    /// The agent for this user's environment.
    pub fn current() -> Option<LoginAgent> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())?;
        let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        Some(LoginAgent::new(unit_dir(config.as_deref(), &home)))
    }

    pub fn unit_path(&self) -> PathBuf {
        self.unit_dir.join(UNIT)
    }

    /// Writes the unit (`0644`, replaced atomically), reloads the user
    /// manager, and enables and starts the host. A changed command (the
    /// AppImage moved) rewrites the unit and restarts the running host.
    pub fn register(
        &self,
        command: &HostCommand,
        path: &str,
        systemctl: &mut dyn Systemctl,
    ) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let text = render_unit(command, path)?;
        let io = |error: io::Error| format!("cannot write the start-up entry: {error}");
        std::fs::create_dir_all(&self.unit_dir).map_err(io)?;
        let target = self.unit_path();
        let previous = std::fs::read_to_string(&target).ok();
        let changed = previous.as_deref() != Some(text.as_str());
        if changed {
            let temp = self.unit_dir.join(format!(".{UNIT}.tmp"));
            std::fs::write(&temp, &text).map_err(io)?;
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o644)).map_err(io)?;
            std::fs::rename(&temp, &target).map_err(io)?;
        }
        checked(systemctl, &["daemon-reload"])?;
        checked(systemctl, &["enable", "--now", UNIT])?;
        // `enable --now` leaves a running host on the old unit.
        if changed && previous.is_some() {
            checked(systemctl, &["restart", UNIT])?;
        }
        Ok(())
    }

    /// Stops and disables the host and removes the unit. Unregistering an
    /// agent that is not there succeeds.
    // For **Stop Coder** once Linux has a tray item; tested now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn unregister(&self, systemctl: &mut dyn Systemctl) -> Result<(), String> {
        let target = self.unit_path();
        if target.exists() {
            // A unit the manager never loaded cannot be disabled; that is fine.
            let _ = systemctl.run(&["disable", "--now", UNIT]);
            std::fs::remove_file(&target)
                .map_err(|error| format!("cannot remove the start-up entry: {error}"))?;
        }
        checked(systemctl, &["daemon-reload"]).map(drop)
    }

    /// Whether the unit is enabled, and whether it runs now.
    pub fn status(&self, systemctl: &mut dyn Systemctl) -> (bool, bool) {
        if !self.unit_path().exists() {
            return (false, false);
        }
        let answer = |systemctl: &mut dyn Systemctl, verb: &str| {
            systemctl
                .run(&[verb, UNIT])
                .map(|(_, stdout)| stdout.trim().to_string())
                .unwrap_or_default()
        };
        (
            answer(systemctl, "is-enabled") == "enabled",
            answer(systemctl, "is-active") == "active",
        )
    }
}

/// Registers the systemd user unit that runs `coder host serve`, and
/// reports whether it may run. Only an installed app (an AppImage, or the
/// .deb's directory with `coder` beside the window) registers one; a
/// development build reports `NotRegistered`, as on a Mac outside a bundle.
pub fn register_agent() -> Agent {
    let Some(dir) = exe_dir() else {
        return Agent::NotRegistered;
    };
    let appimage = appimage();
    if appimage.is_none() && !dir.join(CODER).is_file() {
        return Agent::NotRegistered;
    }
    let Some(agent) = LoginAgent::current() else {
        return Agent::Failed("HOME is not set".into());
    };
    let command = HostCommand::for_install(appimage.as_deref(), &dir);
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".into());
    match agent.register(&command, &path, &mut UserSystemctl) {
        Ok(()) => Agent::Enabled,
        Err(message) => Agent::Failed(message),
    }
}

/// Whether this app's unit is already enabled, without registering it.
pub fn agent_enabled() -> bool {
    LoginAgent::current().is_some_and(|agent| agent.status(&mut UserSystemctl).0)
}

/// Linux asks no one to allow a user unit; nothing to open.
pub fn open_login_items() {}

// ------------------------------------------------------------ the session

/// Whether this session's screen is locked, from logind's `LockedHint`.
/// `false` when there is no logind session to ask.
pub fn screen_locked() -> bool {
    let session = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "self".into());
    Command::new("loginctl")
        .args(["show-session", &session, "--property=LockedHint", "--value"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.trim_ascii() == b"yes")
}

/// Which clipboard tool this session uses: `wl-copy` on Wayland, `xclip`
/// on X11.
fn clipboard(wayland: bool, copy: bool) -> (&'static str, &'static [&'static str]) {
    match (wayland, copy) {
        (true, true) => ("wl-copy", &[]),
        (true, false) => ("wl-paste", &["--no-newline"]),
        (false, true) => ("xclip", &["-selection", "clipboard", "-in"]),
        (false, false) => ("xclip", &["-selection", "clipboard", "-out"]),
    }
}

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Puts `text` on the clipboard.
pub fn copy(text: &str) -> bool {
    let (program, args) = clipboard(wayland(), true);
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok_and(|status| status.success()) && wrote
}

/// Empties the clipboard if it still holds `text`.
pub fn clear_if(text: &str) {
    let (program, args) = clipboard(wayland(), false);
    let holds = Command::new(program)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| output.stdout == text.as_bytes());
    if holds {
        copy("");
    }
}

/// Asks the person for a folder with the desktop's chooser (`zenity`,
/// else `kdialog`). `None` when they cancel or neither is installed.
pub fn choose_folder() -> Option<PathBuf> {
    const PROMPT: &str = "Choose the folder that holds your code";
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let attempts: [(&str, Vec<&str>); 2] = [
        (
            "zenity",
            vec!["--file-selection", "--directory", "--title", PROMPT],
        ),
        (
            "kdialog",
            vec!["--getexistingdirectory", &home, "--title", PROMPT],
        ),
    ];
    for (program, args) in attempts {
        let Ok(output) = Command::new(program)
            .args(&args)
            .stderr(Stdio::null())
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8(output.stdout).ok()?;
        let path = path.trim().trim_end_matches('/');
        return (!path.is_empty()).then(|| PathBuf::from(path));
    }
    None
}

/// Whether Codex and Claude Code are signed in for this user. On Linux
/// both keep their sign-in in a file.
pub fn signed_in(home: &Path) -> Agents {
    Agents {
        codex: home.join(".codex/auth.json").exists(),
        claude: home.join(".claude/.credentials.json").exists(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        calls: Vec<String>,
        enabled: bool,
    }

    impl Systemctl for Recorder {
        fn run(&mut self, args: &[&str]) -> io::Result<(bool, String)> {
            self.calls.push(args.join(" "));
            match args.first().copied() {
                Some("enable") => self.enabled = true,
                Some("disable") => self.enabled = false,
                _ => {}
            }
            let stdout = match (args.first().copied(), self.enabled) {
                (Some("is-enabled"), true) => "enabled",
                (Some("is-enabled"), false) => "disabled",
                (Some("is-active"), true) => "active",
                (Some("is-active"), false) => "inactive",
                _ => "",
            };
            Ok((true, stdout.into()))
        }
    }

    #[test]
    fn linux_host_command_runs_coder_from_the_install() {
        let deb = HostCommand::for_install(None, Path::new("/usr/lib/openagents"));
        assert_eq!(deb.program, PathBuf::from("/usr/lib/openagents/coder"));
        assert_eq!(
            deb.args,
            ["host", "serve", "--keychain", "--iroh", "--control"]
        );
        let appimage = HostCommand::for_install(
            Some(Path::new("/home/kai/Apps/OpenAgents-x86_64.AppImage")),
            Path::new("/tmp/.mount_OpenAgXYZ/usr/lib/openagents"),
        );
        assert_eq!(
            appimage.program,
            PathBuf::from("/home/kai/Apps/OpenAgents-x86_64.AppImage")
        );
        assert_eq!(
            appimage.args,
            [
                "coder",
                "host",
                "serve",
                "--keychain",
                "--iroh",
                "--control"
            ]
        );
        // A relative $APPIMAGE is ignored rather than trusted.
        let relative =
            HostCommand::for_install(Some(Path::new("x.AppImage")), Path::new("/opt/openagents"));
        assert_eq!(relative.program, PathBuf::from("/opt/openagents/coder"));
    }

    #[test]
    fn linux_unit_quotes_every_word() {
        let command = HostCommand {
            program: PathBuf::from("/home/kai/My Apps/Open$Agents%.AppImage"),
            args: vec!["coder".into(), "host".into(), "serve".into()],
        };
        let unit = render_unit(&command, "/usr/bin:/home/kai/.local/bin").unwrap();
        assert!(unit.contains(
            "ExecStart=\"/home/kai/My Apps/Open$$Agents%%.AppImage\" \"coder\" \"host\" \"serve\"\n"
        ));
        assert!(unit.contains("Environment=\"PATH=/usr/bin:/home/kai/.local/bin\"\n"));
        assert!(unit.contains("WantedBy=default.target\n"));
        assert!(unit.contains("Restart=on-failure\n"));
        assert!(unit.contains("UMask=0077\n"));
    }

    #[test]
    fn linux_unit_refuses_what_it_cannot_quote() {
        let newline = HostCommand {
            program: PathBuf::from("/opt/openagents/coder"),
            args: vec!["host\nExecStartPre=/bin/sh".into()],
        };
        assert!(render_unit(&newline, "/usr/bin").is_err());
        let relative = HostCommand {
            program: PathBuf::from("coder"),
            args: vec![],
        };
        assert!(render_unit(&relative, "/usr/bin").is_err());
    }

    #[test]
    fn linux_login_agent_registers_and_unregisters() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let agent = LoginAgent::new(unit_dir(None, dir.path()));
        let mut systemctl = Recorder::default();
        assert_eq!(agent.status(&mut systemctl), (false, false));
        let command = HostCommand::for_install(None, Path::new("/usr/lib/openagents"));
        agent
            .register(&command, "/usr/bin", &mut systemctl)
            .unwrap();
        let path = dir.path().join(".config/systemd/user").join(UNIT);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        let enable = format!("enable --now {UNIT}");
        assert_eq!(systemctl.calls, ["daemon-reload", enable.as_str()]);
        assert_eq!(agent.status(&mut systemctl), (true, true));

        // The same command again does not restart the host.
        systemctl.calls.clear();
        agent
            .register(&command, "/usr/bin", &mut systemctl)
            .unwrap();
        assert_eq!(systemctl.calls, ["daemon-reload", enable.as_str()]);

        // A moved install rewrites the unit and restarts the host onto it.
        systemctl.calls.clear();
        let moved = HostCommand::for_install(None, Path::new("/opt/openagents"));
        agent.register(&moved, "/usr/bin", &mut systemctl).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("/opt/openagents/coder")
        );
        let restart = format!("restart {UNIT}");
        assert_eq!(
            systemctl.calls,
            ["daemon-reload", enable.as_str(), restart.as_str()]
        );

        systemctl.calls.clear();
        agent.unregister(&mut systemctl).unwrap();
        assert!(!path.exists());
        let disable = format!("disable --now {UNIT}");
        assert_eq!(systemctl.calls, [disable.as_str(), "daemon-reload"]);
        assert_eq!(agent.status(&mut systemctl), (false, false));
        agent.unregister(&mut systemctl).unwrap();
    }

    #[test]
    fn linux_paths_follow_xdg() {
        assert_eq!(
            unit_dir(Some(Path::new("/cfg")), Path::new("/home/kai")),
            PathBuf::from("/cfg/systemd/user")
        );
        assert_eq!(
            unit_dir(Some(Path::new("relative")), Path::new("/home/kai")),
            PathBuf::from("/home/kai/.config/systemd/user")
        );
        assert_eq!(
            openagents_desktop::control::socket_path_for(
                "linux",
                None,
                Some(Path::new("/run/user/1000"))
            ),
            Some(PathBuf::from("/run/user/1000/openagents/control.sock"))
        );
    }

    #[test]
    fn linux_clipboard_follows_the_display_server() {
        assert_eq!(clipboard(true, true).0, "wl-copy");
        assert_eq!(clipboard(true, false).0, "wl-paste");
        assert_eq!(clipboard(false, true).1, ["-selection", "clipboard", "-in"]);
        assert_eq!(
            clipboard(false, false).1,
            ["-selection", "clipboard", "-out"]
        );
    }

    /// The real user manager: registers a harmless stand-in (`sleep`) as
    /// the unit, checks that it runs, and removes it. Refuses to touch a
    /// real OpenAgents unit. `cargo test -p openagents-desktop --bin
    /// openagents-desktop -- --ignored linux_login_agent_with_systemd`
    #[test]
    #[ignore = "needs a systemd user manager; writes and removes the real unit"]
    fn linux_login_agent_with_systemd() {
        let agent = LoginAgent::current().unwrap();
        assert!(
            !agent.unit_path().exists(),
            "an OpenAgents unit is installed; not touching it"
        );
        let sleep = [
            "/run/current-system/sw/bin/sleep",
            "/usr/bin/sleep",
            "/bin/sleep",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
        .unwrap();
        let command = HostCommand {
            program: sleep,
            args: vec!["300".into()],
        };
        let mut systemctl = UserSystemctl;
        agent
            .register(&command, "/usr/bin:/bin", &mut systemctl)
            .unwrap();
        let status = agent.status(&mut systemctl);
        agent.unregister(&mut systemctl).unwrap();
        assert_eq!(status, (true, true));
        assert!(!agent.unit_path().exists());
    }
}
