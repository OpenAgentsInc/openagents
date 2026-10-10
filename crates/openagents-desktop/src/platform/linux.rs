//! Linux: the host as a systemd user unit, the session's lock hint, and
//! the desktop's clipboard, folder chooser, and notifications.
//!
//! - **Keys.** The host (`coder host serve --keychain`) keeps its keys in
//!   the Secret Service (GNOME Keyring, KWallet, KeePassXC), under the
//!   service `com.openagents.desktop` and the same accounts as on a Mac
//!   (`coder_host::serve::keys::SecretService`). On a desktop where no
//!   Secret Service answers, it serves with `--keys ~/.openagents/host-keys`
//!   instead: `0600` files in a `0700` directory, as a command-line install
//!   keeps them ([`openagents_desktop::migrate::Keys`]). The window reads
//!   none.
//! - **Login agent.** `coder host serve` runs as the systemd user unit
//!   [`UNIT`], enabled for `default.target`, so it starts at login and
//!   keeps running when the window closes. Registering writes the unit,
//!   reloads the user manager, and enables and starts it. Inside an
//!   AppImage the unit runs the AppImage file itself (`AppRun` dispatches
//!   `coder ...`), because the AppImage's mount point changes every launch.
//!   Outside an installed location (a build directory such as
//!   `target/release`, which the next build overwrites), the unit runs a
//!   copy of `coder` and `microcoder` in `~/.openagents/host/bin/<sha256>/`
//!   ([`stage`]), named by their contents: a new build is a new folder,
//!   picked up the next time the app starts, and never replaced under a
//!   running host.
//! - **Control socket.** `$XDG_RUNTIME_DIR/openagents/control.sock`
//!   ([`openagents_desktop::control::socket_path`]); the host checks every
//!   peer's user ID with `SO_PEERCRED`.
//!
//! None of this reads a secret. The sign-in check looks only at whether a
//! credential file exists, never its contents.

use openagents_desktop::folder::{self, Chosen};
use openagents_desktop::migrate::Keys;
use openagents_desktop::model::{Agent, Agents};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The systemd user unit that runs the host; the same name as the Mac's
/// login agent label.
pub const UNIT: &str = "com.openagents.desktop.host.service";

/// The host binary's name, beside the window's binary when installed.
const CODER: &str = "coder";

/// What the unit runs `coder` with, around the key source: the same as the
/// Mac's login agent (`com.openagents.desktop.host.plist`).
fn host_args(keys: &Keys) -> Vec<String> {
    ["host", "serve"]
        .into_iter()
        .map(String::from)
        .chain(keys.serve_args())
        .chain(["--iroh".into(), "--control".into()])
        .collect()
}

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
    /// `coder host serve ...` with `keys` from an installation: through
    /// the AppImage file when there is one, else the `coder` beside the
    /// window's binary.
    pub fn for_install(appimage: Option<&Path>, exe_dir: &Path, keys: &Keys) -> HostCommand {
        let args = host_args(keys).into_iter();
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
        // A host that could not start itself again after a settings change
        // exits with a failure; it is back within a second. A host that
        // keeps failing waits longer each time, up to 30 seconds (systemd
        // 254 and later; older ones ignore the two lines).
        "RestartSec=1".into(),
        "RestartSteps=5".into(),
        "RestartMaxDelaySec=30".into(),
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
        // A host that failed to start too often is left stopped by the
        // start limit until this clears it.
        let _ = systemctl.run(&["reset-failed", UNIT]);
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
    // For a Linux tray item's status; tested now.
    #[cfg_attr(not(test), allow(dead_code))]
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

/// Where a copy of a build directory's `coder` lives for the unit, under
/// the home folder: one folder a build, named by its SHA-256.
const STAGED: &str = ".openagents/host/bin";

/// The programs copied together: `coder`, and the engine it finds beside
/// itself (`coder host autostart` sets up its first policy with it).
const STAGED_PROGRAMS: [&str; 2] = [CODER, "microcoder"];

/// Whether `dir` is an installed location that no build overwrites: the
/// .deb's `/usr/lib/openagents`, `/opt`, or the Nix store.
pub fn installed(dir: &Path) -> bool {
    ["/usr", "/opt", "/nix/store", "/snap"]
        .iter()
        .any(|root| dir.starts_with(root))
}

/// SHA-256 over the programs in `dir` (`coder` required), each named and
/// sized, so the folder changes when any of them does.
fn programs_digest(dir: &Path) -> Result<String, String> {
    use ring::digest::{Context, SHA256};
    use std::io::Read;
    let mut digest = Context::new(&SHA256);
    for name in STAGED_PROGRAMS {
        let path = dir.join(name);
        if name != CODER && !path.is_file() {
            continue;
        }
        let mut file = std::fs::File::open(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let size = file.metadata().map_err(|error| error.to_string())?.len();
        digest.update(name.as_bytes());
        digest.update(&[0]);
        digest.update(&size.to_be_bytes());
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
    }
    Ok(digest
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Copies `coder` (and `microcoder`, when it is there) from `source` into
/// `root/<sha256>/`, the folder `0700` and each program `0500`, and returns
/// the copy of `coder`. A folder already there with the same contents is
/// used as it is; a copy that changed while it was made is refused.
pub fn stage(source: &Path, root: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let digest = programs_digest(source)?;
    let target = root.join(&digest);
    let program = target.join(CODER);
    if program.is_file() && programs_digest(&target).as_deref() == Ok(digest.as_str()) {
        return Ok(program);
    }
    let io = |error: io::Error| format!("cannot copy Coder for the start-up entry: {error}");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&target)
        .map_err(io)?;
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
    for name in STAGED_PROGRAMS {
        let from = source.join(name);
        if !from.is_file() {
            continue;
        }
        let pending = target.join(format!(".{name}.pending"));
        let _ = std::fs::remove_file(&pending);
        std::fs::copy(&from, &pending).map_err(io)?;
        std::fs::set_permissions(&pending, std::fs::Permissions::from_mode(0o500)).map_err(io)?;
        std::fs::File::open(&pending)
            .and_then(|file| file.sync_all())
            .map_err(io)?;
        std::fs::rename(&pending, target.join(name)).map_err(io)?;
    }
    if programs_digest(&target)? != digest {
        let _ = std::fs::remove_dir_all(&target);
        return Err("Coder changed while it was copied; try again after the build".into());
    }
    Ok(program)
}

/// Removes the copies of earlier builds under `root`, keeping `keep`: only
/// folders named by a SHA-256 that hold nothing but the programs [`stage`]
/// copies, so anything else there (an older setup's launcher) stays.
pub fn prune_staged(root: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let hashed = name.len() == 64
            && name
                .to_str()
                .is_some_and(|name| name.bytes().all(|b| b.is_ascii_hexdigit()));
        if !hashed || path == keep || !path.is_dir() {
            continue;
        }
        let only_ours = std::fs::read_dir(&path).is_ok_and(|files| {
            files.flatten().all(|file| {
                file.file_name()
                    .to_str()
                    .is_some_and(|file| STAGED_PROGRAMS.contains(&file))
            })
        });
        if only_ours {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// Registers the systemd user unit that runs `coder host serve`, and
/// reports whether it may run. Only an app with `coder` beside it (an
/// AppImage, the .deb's directory, or a build directory) registers one.
/// Outside an installed location the unit runs a copy of the build
/// ([`stage`]), so the next build never replaces the file under the
/// running host; a changed build is a changed unit, and the host restarts
/// onto it.
pub fn register_agent(keys: &Keys) -> Agent {
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
    let mut command = HostCommand::for_install(appimage.as_deref(), &dir, keys);
    let mut staged = None;
    if appimage.is_none() && !installed(&dir) {
        let Some(home) = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
        else {
            return Agent::Failed("HOME is not set".into());
        };
        let root = home.join(STAGED);
        match stage(&dir, &root) {
            Ok(program) => {
                command.program = program;
                staged = Some(root);
            }
            Err(message) => return Agent::Failed(message),
        }
    }
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".into());
    match agent.register(&command, &path, &mut UserSystemctl) {
        Ok(()) => {
            if let (Some(root), Some(keep)) = (staged, command.program.parent()) {
                prune_staged(&root, keep);
            }
            Agent::Enabled
        }
        Err(message) => Agent::Failed(message),
    }
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

/// Whether the desktop asks for less motion: the desktop portal's
/// `org.freedesktop.appearance` `reduced-motion` key where the portal has
/// it, else GNOME's animations turned off
/// (`org.gnome.desktop.interface enable-animations`). The backdrop then
/// shows a still frame. `false` when there is no setting to ask.
pub fn reduce_motion() -> bool {
    let portal = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--timeout",
            "1",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.ReadOne",
            "org.freedesktop.appearance",
            "reduced-motion",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            rust_native_desktop::theme::motion::parse_portal_reply(&String::from_utf8_lossy(
                &output.stdout,
            ))
        });
    if portal == Some(true) {
        return true;
    }
    Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.trim_ascii() == b"false")
}

/// Puts `text` on the clipboard: the window's own Wayland selection, else
/// `wl-copy` or `xclip`, else `arboard`
/// ([`rust_native_desktop::input::copy`]).
pub fn copy(text: &str) -> bool {
    rust_native_desktop::input::copy(text)
}

/// Empties the clipboard if it still holds `text`.
pub fn clear_if(text: &str) {
    if rust_native_desktop::input::paste().as_deref() == Some(text) {
        rust_native_desktop::input::clear();
    }
}

// ---------------------------------------------------------- notifications

/// Shows `notice` on a thread of its own, so a slow session bus never holds
/// the window ([`notify_now`]).
pub fn notify(notice: openagents_desktop::notices::Notice) {
    let _ = std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || {
            let _ = notify_now(&notice);
        });
}

/// Shows `notice` and says how: the desktop portal
/// (`org.freedesktop.portal.Notification`), else the notification server
/// (`org.freedesktop.Notifications`). `None` when neither answers.
pub fn notify_now(notice: &openagents_desktop::notices::Notice) -> Option<&'static str> {
    notifications::deliver(notifications::bus()?, notice)
}

/// Listens, on a thread of its own, for clicks on the notices this app
/// showed, and opens each one's chat in the window
/// ([`crate::native::open_chat`]).
pub fn listen_notifications() {
    let _ = std::thread::Builder::new()
        .name("notification-clicks".into())
        .spawn(|| {
            if let Some(connection) = notifications::bus() {
                notifications::listen(connection, crate::native::open_chat);
            }
        });
}

/// Notifications over the session bus.
mod notifications {
    use openagents_desktop::notices::{self, Notice};
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use zbus::blocking::{Connection, Proxy, proxy::Builder};
    use zbus::zvariant::Value;

    /// The session bus connection every notice is sent on. One connection
    /// for the app's life: the portal and the notification server send a
    /// click back to the connection that showed the notice.
    pub(super) fn bus() -> Option<&'static Connection> {
        static BUS: OnceLock<Option<Connection>> = OnceLock::new();
        BUS.get_or_init(|| Connection::session().ok()).as_ref()
    }

    /// Reads clicks on this app's notices from `connection` until it
    /// closes, and calls `open` with each clicked notice's chat: the
    /// portal's `ActionInvoked` for [`notices::OPEN_ACTION`], or the
    /// server's for its default action on a notice this app showed.
    pub(super) fn listen(connection: &Connection, open: impl Fn(String)) {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .member("ActionInvoked")
            .map(|rule| rule.build());
        let Ok(rule) = rule else {
            return;
        };
        let Ok(messages) = zbus::blocking::MessageIterator::for_match_rule(rule, connection, None)
        else {
            return;
        };
        for message in messages.flatten() {
            if let Some(chat) = clicked(&message) {
                open(chat);
            }
        }
    }

    /// The chat a click signal opens, if it is a click on this app's notice.
    pub(super) fn clicked(message: &zbus::Message) -> Option<String> {
        let header = message.header();
        let body = message.body();
        match header.interface()?.as_str() {
            "org.freedesktop.portal.Notification" => {
                let (id, action, _): (String, String, Vec<zbus::zvariant::OwnedValue>) =
                    body.deserialize().ok()?;
                notices::portal_click(&id, &action).map(str::to_owned)
            }
            "org.freedesktop.Notifications" => {
                let (server_id, action): (u32, String) = body.deserialize().ok()?;
                if !notices::server_click(&action) {
                    return None;
                }
                let shown = SHOWN.lock().ok()?;
                let id = shown
                    .as_ref()?
                    .iter()
                    .find(|(_, shown)| **shown == server_id)?
                    .0;
                notices::chat_of(id).map(str::to_owned)
            }
            _ => None,
        }
    }

    /// The app's ID: its desktop file's name, `com.openagents.desktop.desktop`.
    pub(super) const APP_ID: &str = "com.openagents.desktop";

    fn proxy<'a>(
        connection: &Connection,
        destination: &'static str,
        path: &'static str,
        interface: &'static str,
    ) -> zbus::Result<Proxy<'a>> {
        Builder::new(connection)
            .destination(destination)?
            .path(path)?
            .interface(interface)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
    }

    /// The portal first, then the notification server.
    pub(super) fn deliver(connection: &Connection, notice: &Notice) -> Option<&'static str> {
        if portal(connection, notice).is_ok() {
            return Some("the desktop portal (org.freedesktop.portal.Notification)");
        }
        server(connection, notice)
            .ok()
            .map(|_| "the notification server (org.freedesktop.Notifications)")
    }

    /// `AddNotification` on the desktop portal. A host app first names
    /// itself to the portal (`org.freedesktop.host.portal.Registry`, on
    /// portals that have it), so the notice carries the app's name and
    /// icon; a portal without it, or without the desktop file, still shows
    /// the notice.
    pub(super) fn portal(connection: &Connection, notice: &Notice) -> zbus::Result<()> {
        const DESTINATION: &str = "org.freedesktop.portal.Desktop";
        const PATH: &str = "/org/freedesktop/portal/desktop";
        if let Ok(registry) = proxy(
            connection,
            DESTINATION,
            PATH,
            "org.freedesktop.host.portal.Registry",
        ) {
            let options: HashMap<&str, Value<'_>> = HashMap::new();
            let _: zbus::Result<()> = registry.call("Register", &(APP_ID, options));
        }
        let portal = proxy(
            connection,
            DESTINATION,
            PATH,
            "org.freedesktop.portal.Notification",
        )?;
        portal.call::<_, _, ()>(
            "AddNotification",
            &(notice.id.as_str(), portal_fields(notice)),
        )
    }

    /// The portal's notification: title, body, priority, and, for a
    /// chat's notice, a click that opens the chat: the default action
    /// [`notices::OPEN_ACTION`] with the chat's ID as its target. The name
    /// is not an `app.` action, so the portal sends it back as
    /// `ActionInvoked` ([`listen`]) rather than activating the app.
    pub(super) fn portal_fields(notice: &Notice) -> HashMap<&'static str, Value<'_>> {
        let mut fields = HashMap::from([
            ("title", Value::from(notice.title.as_str())),
            ("body", Value::from(notice.body.as_str())),
            (
                "priority",
                Value::from(if notice.urgent { "high" } else { "normal" }),
            ),
        ]);
        if let Some(chat) = notice.chat() {
            fields.insert("default-action", Value::from(notices::OPEN_ACTION));
            fields.insert("default-action-target", Value::from(chat));
        }
        fields
    }

    /// The server's ID of the last notice shown for each chat, so a newer
    /// one replaces it.
    pub(super) static SHOWN: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);

    /// `Notify` on the notification server (mako, dunst, GNOME Shell,
    /// Plasma).
    pub(super) fn server(connection: &Connection, notice: &Notice) -> zbus::Result<u32> {
        let server = proxy(
            connection,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )?;
        let replaces = SHOWN
            .lock()
            .ok()
            .and_then(|shown| shown.as_ref()?.get(&notice.id).copied())
            .unwrap_or(0);
        let hints: HashMap<&str, Value<'_>> = HashMap::from([
            ("desktop-entry", Value::from(APP_ID)),
            (
                "urgency",
                Value::from(if notice.urgent { 2u8 } else { 1u8 }),
            ),
        ]);
        // A click on the notice's body is the default action ([`listen`]).
        let actions: Vec<&str> = if notice.chat().is_some() {
            vec![notices::SERVER_DEFAULT, "Open"]
        } else {
            vec![]
        };
        let id: u32 = server.call(
            "Notify",
            &(
                "OpenAgents",
                replaces,
                APP_ID,
                notice.title.as_str(),
                notice.body.as_str(),
                actions,
                hints,
                -1i32,
            ),
        )?;
        if let Ok(mut shown) = SHOWN.lock() {
            shown
                .get_or_insert_with(HashMap::new)
                .insert(notice.id.clone(), id);
        }
        Ok(id)
    }
}

/// Asks the person for a folder: the desktop portal's chooser first, then
/// `zenity`, then `kdialog` ([`openagents_desktop::folder::choose`]).
/// [`Chosen::Unavailable`] when none of them opens. Blocks until the person
/// answers, so the worker runs it on its own thread.
pub fn choose_folder() -> Chosen {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    folder::choose(&mut SystemChoosers, &home)
}

/// This computer's folder choosers.
struct SystemChoosers;

impl folder::Choosers for SystemChoosers {
    fn portal(&mut self) -> Option<Chosen> {
        portal::choose_folder()
    }

    fn command(&mut self, program: &str, args: &[String]) -> Option<Chosen> {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        folder::from_command(output.status.code(), &output.stdout)
    }
}

/// The desktop portal's file chooser
/// (`org.freedesktop.portal.FileChooser.OpenFile`), over the session bus.
mod portal {
    use openagents_desktop::folder::{self, Chosen};
    use std::collections::HashMap;
    use zbus::blocking::{Connection, Proxy, proxy::Builder};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

    const DESTINATION: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";

    fn proxy<'a>(
        connection: &Connection,
        path: &'a str,
        interface: &'static str,
    ) -> zbus::Result<Proxy<'a>> {
        Builder::new(connection)
            .destination(DESTINATION)?
            .path(path)?
            .interface(interface)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
    }

    /// The request object the portal answers on for `token`
    /// (`/org/freedesktop/portal/desktop/request/SENDER/TOKEN`).
    pub(super) fn request_path(unique_name: &str, token: &str) -> String {
        let sender = unique_name.trim_start_matches(':').replace('.', "_");
        format!("{PATH}/request/{sender}/{token}")
    }

    /// Asks through the portal. `None` when there is no session bus, no
    /// portal, or no chooser behind it, or it ended without asking;
    /// otherwise the person's answer.
    pub(super) fn choose_folder() -> Option<Chosen> {
        let connection = Connection::session().ok()?;
        let unique = connection.unique_name()?.as_str().to_owned();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        let token = format!("openagents_{}_{nanos}", std::process::id());
        let expected = request_path(&unique, &token);
        // Listen before asking, so a fast answer is not missed.
        let request = proxy(&connection, &expected, "org.freedesktop.portal.Request").ok()?;
        let mut responses = request.receive_signal("Response").ok()?;
        let chooser = proxy(&connection, PATH, "org.freedesktop.portal.FileChooser").ok()?;
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        options.insert("handle_token", Value::from(token.as_str()));
        options.insert("modal", Value::from(true));
        options.insert("directory", Value::from(true));
        options.insert("multiple", Value::from(false));
        let handle: OwnedObjectPath = chooser
            .call("OpenFile", &("", folder::PROMPT, options))
            .ok()?;
        // A portal older than handle tokens answers on a path of its own.
        let other;
        if handle.as_str() != expected {
            other = proxy(
                &connection,
                handle.as_str(),
                "org.freedesktop.portal.Request",
            )
            .ok()?;
            responses = other.receive_signal("Response").ok()?;
        }
        let message = responses.next()?;
        let (response, results): (u32, HashMap<String, OwnedValue>) =
            message.body().deserialize().ok()?;
        match response {
            0 => {
                let uris = results.get("uris")?.try_clone().ok()?;
                let uris: Vec<String> = uris.try_into().ok()?;
                Some(
                    uris.first()
                        .and_then(|uri| folder::from_uri(uri))
                        .map_or(Chosen::Cancelled, Chosen::Folder),
                )
            }
            1 => Some(Chosen::Cancelled),
            _ => None,
        }
    }
}

/// Whether Codex and Claude Code are signed in for this user, and Grok
/// Build when it is installed. On Linux Codex and Claude Code keep
/// their sign-in in a file.
pub fn signed_in(home: &Path) -> Agents {
    let claude = home.join(".claude/.credentials.json").exists();
    Agents {
        codex: openagents_desktop::model::codex_login(home).exists(),
        claude,
        grok: openagents_desktop::model::grok(home),
        claude_problem: openagents_desktop::claude_setup::check(home, claude),
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
    fn the_portal_answers_on_the_senders_request_path() {
        assert_eq!(
            portal::request_path(":1.42", "openagents_7_9"),
            "/org/freedesktop/portal/desktop/request/1_42/openagents_7_9"
        );
    }

    #[test]
    fn linux_host_command_runs_coder_from_the_install() {
        let deb = HostCommand::for_install(None, Path::new("/usr/lib/openagents"), &Keys::Keychain);
        assert_eq!(deb.program, PathBuf::from("/usr/lib/openagents/coder"));
        assert_eq!(
            deb.args,
            ["host", "serve", "--keychain", "--iroh", "--control"]
        );
        let appimage = HostCommand::for_install(
            Some(Path::new("/home/kai/Apps/OpenAgents-x86_64.AppImage")),
            Path::new("/tmp/.mount_OpenAgXYZ/usr/lib/openagents"),
            &Keys::Keychain,
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
        let relative = HostCommand::for_install(
            Some(Path::new("x.AppImage")),
            Path::new("/opt/openagents"),
            &Keys::Keychain,
        );
        assert_eq!(relative.program, PathBuf::from("/opt/openagents/coder"));
        // No Secret Service: the keys are private files.
        let files = HostCommand::for_install(
            None,
            Path::new("/usr/lib/openagents"),
            &Keys::Files(PathBuf::from("/home/kai/.openagents/host-keys")),
        );
        assert_eq!(
            files.args,
            [
                "host",
                "serve",
                "--keys",
                "/home/kai/.openagents/host-keys",
                "--iroh",
                "--control"
            ]
        );
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
        // A host that could not start itself again is back within a second.
        assert!(unit.contains("RestartSec=1\n"));
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
        let command =
            HostCommand::for_install(None, Path::new("/usr/lib/openagents"), &Keys::Keychain);
        agent
            .register(&command, "/usr/bin", &mut systemctl)
            .unwrap();
        let path = dir.path().join(".config/systemd/user").join(UNIT);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        let enable = format!("enable --now {UNIT}");
        let reset = format!("reset-failed {UNIT}");
        assert_eq!(
            systemctl.calls,
            ["daemon-reload", reset.as_str(), enable.as_str()]
        );
        assert_eq!(agent.status(&mut systemctl), (true, true));

        // The same command again does not restart the host.
        systemctl.calls.clear();
        agent
            .register(&command, "/usr/bin", &mut systemctl)
            .unwrap();
        assert_eq!(
            systemctl.calls,
            ["daemon-reload", reset.as_str(), enable.as_str()]
        );

        // A moved install rewrites the unit and restarts the host onto it.
        systemctl.calls.clear();
        let moved = HostCommand::for_install(None, Path::new("/opt/openagents"), &Keys::Keychain);
        agent.register(&moved, "/usr/bin", &mut systemctl).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("/opt/openagents/coder")
        );
        let restart = format!("restart {UNIT}");
        assert_eq!(
            systemctl.calls,
            [
                "daemon-reload",
                reset.as_str(),
                enable.as_str(),
                restart.as_str()
            ]
        );

        systemctl.calls.clear();
        agent.unregister(&mut systemctl).unwrap();
        assert!(!path.exists());
        let disable = format!("disable --now {UNIT}");
        assert_eq!(systemctl.calls, [disable.as_str(), "daemon-reload"]);
        assert_eq!(agent.status(&mut systemctl), (false, false));
        agent.unregister(&mut systemctl).unwrap();
    }

    /// A build directory's `coder` runs from a copy named by its contents:
    /// a rebuild never replaces the file under the running host, the same
    /// build is the same copy, and a new build is a new folder that
    /// replaces the old copies (never another setup's files).
    #[test]
    fn a_build_directorys_coder_runs_from_a_copy_named_by_its_contents() {
        use std::os::unix::fs::PermissionsExt;
        assert!(installed(Path::new("/usr/lib/openagents")));
        assert!(installed(Path::new("/nix/store/abc-openagents/lib")));
        assert!(!installed(Path::new("/home/kai/openagents/target/release")));
        assert!(!installed(Path::new("/usrlocal/openagents")));

        let dir = tempfile::tempdir().unwrap();
        let build = dir.path().join("target/release");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(build.join("coder"), b"coder build 1").unwrap();
        std::fs::write(build.join("microcoder"), b"engine build 1").unwrap();
        let root = dir.path().join("home/.openagents/host/bin");
        // An older setup's launcher in the same folder stays.
        let launcher = root.join("f".repeat(64));
        std::fs::create_dir_all(&launcher).unwrap();
        std::fs::write(launcher.join("coder-service"), b"launcher").unwrap();

        let first = stage(&build, &root).unwrap();
        assert_eq!(first.file_name().unwrap(), "coder");
        let folder = first.parent().unwrap().to_path_buf();
        assert_eq!(folder.parent().unwrap(), root);
        assert_eq!(folder.file_name().unwrap().len(), 64);
        assert_eq!(std::fs::read(&first).unwrap(), b"coder build 1");
        assert_eq!(
            std::fs::read(folder.join("microcoder")).unwrap(),
            b"engine build 1"
        );
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&folder), 0o700);
        assert_eq!(mode(&first), 0o500);
        assert_eq!(stage(&build, &root).unwrap(), first, "the same build");

        // The build is replaced, as cargo does: the copy stays as it was.
        std::fs::remove_file(build.join("coder")).unwrap();
        std::fs::write(build.join("coder"), b"coder build 2").unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"coder build 1");
        let second = stage(&build, &root).unwrap();
        assert_ne!(second, first);
        assert_eq!(std::fs::read(&second).unwrap(), b"coder build 2");
        // A new engine alone is a new folder too.
        std::fs::write(build.join("microcoder"), b"engine build 2").unwrap();
        let third = stage(&build, &root).unwrap();
        assert_ne!(third, second);

        prune_staged(&root, third.parent().unwrap());
        assert!(third.is_file());
        assert!(!first.exists() && !second.exists());
        assert!(launcher.join("coder-service").is_file());

        // The unit runs the copy.
        let mut command = HostCommand::for_install(None, &build, &Keys::Keychain);
        command.program = third.clone();
        let unit = render_unit(&command, "/usr/bin").unwrap();
        assert!(unit.contains(&format!("ExecStart=\"{}\"", third.display())));
        assert!(!unit.contains("target/release"));
        // No `coder` in the build: nothing to copy.
        assert!(stage(&dir.path().join("empty"), &root).is_err());
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

    /// A click on this app's notice, from the portal or the notification
    /// server, names the notice's chat; anything else opens nothing.
    #[test]
    fn a_click_on_a_notice_opens_its_chat() {
        use zbus::zvariant::Value;
        let signal =
            |interface: &str, body: &dyn Fn(zbus::message::Builder<'_>) -> zbus::Message| {
                body(
                    zbus::Message::signal(
                        "/org/freedesktop/portal/desktop",
                        interface,
                        "ActionInvoked",
                    )
                    .unwrap(),
                )
            };
        let portal = |id: &'static str, action: &'static str| {
            signal("org.freedesktop.portal.Notification", &move |builder| {
                builder
                    .build(&(id, action, vec![Value::from("c1")]))
                    .unwrap()
            })
        };
        assert_eq!(
            notifications::clicked(&portal("coder-c1", "open")).as_deref(),
            Some("c1")
        );
        assert_eq!(notifications::clicked(&portal("coder-c1", "other")), None);
        assert_eq!(
            notifications::clicked(&portal("someone-else", "open")),
            None
        );

        notifications::SHOWN
            .lock()
            .unwrap()
            .get_or_insert_with(Default::default)
            .insert("coder-c2".into(), 9_001);
        let server = |id: u32, action: &'static str| {
            signal("org.freedesktop.Notifications", &move |builder| {
                builder.build(&(id, action)).unwrap()
            })
        };
        assert_eq!(
            notifications::clicked(&server(9_001, "default")).as_deref(),
            Some("c2")
        );
        assert_eq!(
            notifications::clicked(&server(9_001, "open")).as_deref(),
            Some("c2")
        );
        assert_eq!(notifications::clicked(&server(9_002, "default")), None);
        assert_eq!(notifications::clicked(&server(9_001, "close")), None);
    }

    /// Notifications over a private session bus with a stand-in portal and
    /// notification server: the server when there is no portal, the portal
    /// when there is one, each with the notice's fields, and a newer notice
    /// for a chat replacing the older one. Refuses the login session's bus.
    /// `dbus-run-session -- cargo test -p openagents-desktop --bin
    /// openagents-desktop -- --ignored notifications_over_a_private_bus`
    #[test]
    #[ignore = "needs a private session bus (dbus-run-session)"]
    fn notifications_over_a_private_bus() {
        use openagents_desktop::notices::Notice;
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};
        use zbus::zvariant::OwnedValue;

        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
        assert!(
            !address.is_empty() && !address.contains("/run/user/"),
            "run under dbus-run-session, not on the login session's bus"
        );

        type Seen = Arc<Mutex<Vec<(String, String, String, HashMap<String, OwnedValue>)>>>;
        struct Server(Seen, Mutex<u32>);
        #[zbus::interface(name = "org.freedesktop.Notifications")]
        impl Server {
            #[allow(clippy::too_many_arguments)]
            fn notify(
                &self,
                app_name: String,
                replaces_id: u32,
                _app_icon: String,
                summary: String,
                body: String,
                _actions: Vec<String>,
                hints: HashMap<String, OwnedValue>,
                _expire_timeout: i32,
            ) -> u32 {
                self.0.lock().unwrap().push((
                    format!("{app_name} replaces {replaces_id}"),
                    summary,
                    body,
                    hints,
                ));
                let mut next = self.1.lock().unwrap();
                *next += 1;
                *next
            }
        }
        struct Portal(Seen);
        #[zbus::interface(name = "org.freedesktop.portal.Notification")]
        impl Portal {
            fn add_notification(&self, id: String, notification: HashMap<String, OwnedValue>) {
                self.0
                    .lock()
                    .unwrap()
                    .push((id, String::new(), String::new(), notification));
            }
        }
        let text = |value: &OwnedValue| String::try_from(value.try_clone().unwrap()).unwrap();

        let served: Seen = Arc::default();
        let _server = zbus::blocking::connection::Builder::session()
            .unwrap()
            .name("org.freedesktop.Notifications")
            .unwrap()
            .serve_at(
                "/org/freedesktop/Notifications",
                Server(served.clone(), Mutex::new(40)),
            )
            .unwrap()
            .build()
            .unwrap();
        let notice = Notice {
            id: "coder-c1".into(),
            title: "Fix the login bug".into(),
            body: "Coder asked for approval".into(),
            urgent: true,
        };
        let client = zbus::blocking::Connection::session().unwrap();
        assert_eq!(
            notifications::deliver(&client, &notice),
            Some("the notification server (org.freedesktop.Notifications)")
        );
        assert_eq!(
            notifications::deliver(&client, &notice),
            Some("the notification server (org.freedesktop.Notifications)")
        );
        {
            let seen = served.lock().unwrap();
            assert_eq!(seen.len(), 2);
            assert_eq!(seen[0].0, "OpenAgents replaces 0");
            // The second notice for the chat replaces the first.
            assert_eq!(seen[1].0, "OpenAgents replaces 41");
            assert_eq!(seen[0].1, "Fix the login bug");
            assert_eq!(seen[0].2, "Coder asked for approval");
            assert_eq!(u8::try_from(&seen[0].3["urgency"]).unwrap(), 2);
            assert_eq!(text(&seen[0].3["desktop-entry"]), "com.openagents.desktop");
        }

        let portal: Seen = Arc::default();
        let _portal = zbus::blocking::connection::Builder::session()
            .unwrap()
            .name("org.freedesktop.portal.Desktop")
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop", Portal(portal.clone()))
            .unwrap()
            .build()
            .unwrap();
        let finished = Notice {
            body: "Coder finished".into(),
            urgent: false,
            ..notice
        };
        assert_eq!(
            notifications::deliver(&client, &finished),
            Some("the desktop portal (org.freedesktop.portal.Notification)")
        );
        let seen = portal.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "coder-c1");
        assert_eq!(text(&seen[0].3["title"]), "Fix the login bug");
        assert_eq!(text(&seen[0].3["body"]), "Coder finished");
        assert_eq!(text(&seen[0].3["priority"]), "normal");
        // A click on the body opens the chat.
        assert_eq!(text(&seen[0].3["default-action"]), "open");
        assert_eq!(text(&seen[0].3["default-action-target"]), "c1");
        assert_eq!(served.lock().unwrap().len(), 2, "the server was not asked");
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
