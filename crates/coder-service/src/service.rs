//! The host's background service: a systemd user unit on Linux and a
//! launchd agent on macOS.
//!
//! The rendered definition lives under the host root, in
//! `<root>/service/`. The operating system reads definitions from its own
//! directory, so installation adds one symbolic link there, the
//! *registration*, pointing at the canonical file. That link is the only
//! thing this crate writes outside the host root. Uninstall removes the
//! link only when it still points at this host root's file.
//!
//! Both definitions run `coder-service --root <root> run`, the launcher,
//! from a copy installed under `<root>/bin/`. The service restarts the
//! launcher only when it fails: a clean stop stays stopped.
//!
//! On Linux a user unit starts at login and stops at logout unless the
//! user has *linger* enabled, which starts the user's manager at boot and
//! keeps it after logout. Status reports linger, and install enables it
//! only when asked. On macOS a launchd agent in the `gui` domain starts at
//! login and stops at logout; status reports that it does not survive
//! logout.
//!
//! Every service manager command goes through [`Runner`], so tests check
//! the exact commands without touching the machine's services.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::descriptor::HostDescriptor;
use crate::launcher::{Config, LauncherState, Layout};
use crate::{Error, Result, fsx, launcher};

/// The service platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// A launchd agent in the user's `gui` domain.
    Macos,
    /// A systemd user unit.
    Linux,
}

impl Platform {
    /// The platform this process runs on, if it has a supported service
    /// manager.
    #[must_use]
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Platform::Macos)
        } else if cfg!(target_os = "linux") {
            Some(Platform::Linux)
        } else {
            None
        }
    }

    /// The default registration directory for `home`.
    #[must_use]
    pub fn default_registration_dir(self, home: &Path) -> PathBuf {
        match self {
            Platform::Macos => home.join("Library/LaunchAgents"),
            Platform::Linux => std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| home.join(".config"))
                .join("systemd/user"),
        }
    }
}

/// Checks a service label: 1 to 80 characters of letters, digits, dots,
/// underscores, and hyphens, starting with a letter or digit.
pub fn validate_label(label: &str) -> Result<()> {
    let valid = !label.is_empty()
        && label.len() <= 80
        && label.as_bytes()[0].is_ascii_alphanumeric()
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(Error::refused(
            "a service label is 1 to 80 letters, digits, dots, underscores, or hyphens",
        ))
    }
}

/// The definition's file name.
#[must_use]
pub fn file_name(config: &Config) -> String {
    match config.platform {
        Platform::Macos => format!("{}.plist", config.label),
        Platform::Linux => format!("{}.service", config.label),
    }
}

/// The canonical definition under the host root.
#[must_use]
pub fn canonical_path(layout: &Layout, config: &Config) -> PathBuf {
    layout.service_dir().join(file_name(config))
}

/// The registration link in the operating system's directory.
#[must_use]
pub fn registration_path(config: &Config) -> PathBuf {
    config.registration_dir.join(file_name(config))
}

fn launcher_args(layout: &Layout, config: &Config) -> Vec<String> {
    vec![
        config.launcher.display().to_string(),
        "--root".into(),
        layout.root().display().to_string(),
        "run".into(),
    ]
}

/// Renders the service definition for the configured platform.
pub fn render(layout: &Layout, config: &Config) -> Result<String> {
    config.validate(layout)?;
    let args = launcher_args(layout, config);
    match config.platform {
        Platform::Linux => render_systemd(config, &args),
        Platform::Macos => render_launchd(layout, config, &args),
    }
}

fn render_systemd(config: &Config, args: &[String]) -> Result<String> {
    let exec = args
        .iter()
        .map(|arg| systemd_quote(arg))
        .collect::<Result<Vec<_>>>()?
        .join(" ");
    Ok([
        "[Unit]".to_string(),
        format!("Description=OpenAgents Coder host ({})", config.label),
        "StartLimitIntervalSec=300".into(),
        "StartLimitBurst=5".into(),
        String::new(),
        "[Service]".into(),
        "Type=simple".into(),
        format!("ExecStart={exec}"),
        "Restart=on-failure".into(),
        "RestartSec=5".into(),
        "KillMode=control-group".into(),
        format!("TimeoutStopSec={}", config.stop_grace_secs + 10),
        "UMask=0077".into(),
        "NoNewPrivileges=yes".into(),
        format!(
            "Environment={}",
            systemd_quote(&format!("PATH={}", config.search_path))?
        ),
        String::new(),
        "[Install]".into(),
        "WantedBy=default.target".into(),
        String::new(),
    ]
    .join("\n"))
}

fn systemd_quote(value: &str) -> Result<String> {
    if value.chars().any(char::is_control) {
        return Err(Error::refused(
            "a service argument contains a control character",
        ));
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

fn render_launchd(layout: &Layout, config: &Config, args: &[String]) -> Result<String> {
    let log = xml(&layout.logs().join("launcher.log").display().to_string())?;
    let mut program = String::new();
    for arg in args {
        program.push_str(&format!("\t\t<string>{}</string>\n", xml(arg)?));
    }
    Ok(format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
            "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n",
            "<dict>\n",
            "\t<key>AbandonProcessGroup</key>\n\t<false/>\n",
            "\t<key>EnvironmentVariables</key>\n\t<dict>\n",
            "\t\t<key>PATH</key>\n\t\t<string>{path}</string>\n\t</dict>\n",
            "\t<key>ExitTimeOut</key>\n\t<integer>{exit}</integer>\n",
            "\t<key>KeepAlive</key>\n\t<dict>\n",
            "\t\t<key>SuccessfulExit</key>\n\t\t<false/>\n\t</dict>\n",
            "\t<key>Label</key>\n\t<string>{label}</string>\n",
            "\t<key>ProcessType</key>\n\t<string>Standard</string>\n",
            "\t<key>ProgramArguments</key>\n\t<array>\n{program}\t</array>\n",
            "\t<key>RunAtLoad</key>\n\t<true/>\n",
            "\t<key>StandardErrorPath</key>\n\t<string>{log}</string>\n",
            "\t<key>StandardOutPath</key>\n\t<string>{log}</string>\n",
            "\t<key>ThrottleInterval</key>\n\t<integer>10</integer>\n",
            "\t<key>Umask</key>\n\t<integer>63</integer>\n",
            "</dict>\n",
            "</plist>\n"
        ),
        exit = config.stop_grace_secs + 10,
        label = xml(&config.label)?,
        path = xml(&config.search_path)?,
        program = program,
        log = log,
    ))
}

fn xml(value: &str) -> Result<String> {
    if value.chars().any(char::is_control) {
        return Err(Error::refused(
            "a service value contains a control character",
        ));
    }
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}

/// What a service manager command printed and how it ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// The exit code, or `None` when a signal ended the command or it did
    /// not finish.
    pub code: Option<i32>,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

impl Output {
    /// Whether the command exited zero.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs service manager commands.
pub trait Runner {
    /// Runs `program` with `args` and returns its output.
    fn run(&mut self, program: &str, args: &[String]) -> Result<Output>;
}

/// Runs commands on this machine through the `supervise` process-group
/// contract, each bounded to 30 seconds.
pub struct SystemRunner;

/// The bytes of output kept from one service manager command.
const OUTPUT_MAX: u64 = 256 * 1024;

impl Runner for SystemRunner {
    fn run(&mut self, program: &str, args: &[String]) -> Result<Output> {
        let dir = std::env::temp_dir().join(format!(
            "coder-service-{}-{}",
            std::process::id(),
            fsx::now_ms()
        ));
        fs::create_dir(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let result = (|| -> Result<Output> {
            let stdout = fs::File::create(dir.join("stdout"))?;
            let stderr = fs::File::create(dir.join("stderr"))?;
            let mut command = Command::new(program);
            command
                .args(args)
                .stdin(Stdio::null())
                .stdout(stdout)
                .stderr(stderr);
            supervise::blocking::own_group(&mut command);
            let mut child = command.spawn()?;
            let ending = supervise::blocking::wait(&mut child, Duration::from_secs(30));
            let read = |name: &str| -> Result<String> {
                let bytes = fsx::read_bounded(&dir.join(name), OUTPUT_MAX)?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            };
            Ok(Output {
                code: ending.code(),
                stdout: read("stdout")?,
                stderr: read("stderr")?,
            })
        })();
        let _ = fs::remove_dir_all(&dir);
        result
    }
}

fn run_checked(runner: &mut dyn Runner, program: &str, args: &[String]) -> Result<Output> {
    let output = runner.run(program, args)?;
    if output.success() {
        Ok(output)
    } else {
        Err(Error::refused(format!(
            "`{program} {}` failed: {}",
            args.join(" "),
            output.stderr.trim()
        )))
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn uid() -> u32 {
    // SAFETY: `getuid` takes no arguments and cannot fail.
    unsafe { libc::getuid() }
}

fn launchd_target(config: &Config) -> String {
    format!("gui/{}/{}", uid(), config.label)
}

/// Copies the launcher binary into `<root>/bin/<sha256>/coder-service` and
/// returns the copy's path, so the service never runs a build directory's
/// binary that a later build replaces.
pub fn install_launcher_binary(layout: &Layout, source: &Path) -> Result<PathBuf> {
    let digest = fsx::sha256_file(source)?;
    let directory = layout.bin_dir().join(&digest);
    let destination = directory.join("coder-service");
    if fs::symlink_metadata(&destination).is_ok() {
        if fsx::sha256_file(&destination)? != digest {
            return Err(Error::refused("an installed launcher binary changed"));
        }
        return Ok(destination);
    }
    fsx::private_dir(&layout.bin_dir())?;
    fsx::private_dir(&directory)?;
    let staging = directory.join(".coder-service.pending");
    fsx::remove_file_if_present(&staging)?;
    fs::copy(source, &staging)?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o500))?;
    fs::File::open(&staging)?.sync_all()?;
    if fsx::sha256_file(&staging)? != digest {
        fsx::remove_file_if_present(&staging)?;
        return Err(Error::refused(
            "the launcher binary changed while it was copied",
        ));
    }
    fs::rename(&staging, &destination)?;
    fsx::sync_dir(&directory)?;
    Ok(destination)
}

/// What install did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InstallReport {
    /// The canonical definition.
    pub definition: PathBuf,
    /// The registration link.
    pub registration: PathBuf,
    /// Whether install started or restarted the service.
    pub started: bool,
    /// Whether install enabled linger. Always `false` on macOS.
    pub linger_enabled: bool,
}

/// Writes the definition, registers it, enables it, and, when `start` is
/// set, starts it. `linger` enables systemd linger for this user; it is
/// refused on macOS.
pub fn install(
    layout: &Layout,
    config: &Config,
    runner: &mut dyn Runner,
    start: bool,
    linger: bool,
) -> Result<InstallReport> {
    if linger && config.platform == Platform::Macos {
        return Err(Error::refused(
            "linger is a systemd setting; a launchd agent runs while you are logged in",
        ));
    }
    let contents = render(layout, config)?;
    fsx::private_dir(&layout.service_dir())?;
    let canonical = canonical_path(layout, config);
    let registration = registration_path(config);
    match fs::symlink_metadata(&registration) {
        Ok(_) if fs::read_link(&registration).ok().as_deref() != Some(canonical.as_path()) => {
            return Err(Error::refused(format!(
                "{} exists and does not belong to this host; remove it or choose another label",
                registration.display()
            )));
        }
        _ => {}
    }
    fsx::atomic_write(&canonical, contents.as_bytes(), 0o600)?;
    fs::create_dir_all(&config.registration_dir)?;
    if fs::symlink_metadata(&registration).is_err() {
        std::os::unix::fs::symlink(&canonical, &registration)?;
    }
    let registration_arg = registration.display().to_string();
    match config.platform {
        Platform::Linux => {
            let unit = file_name(config);
            run_checked(runner, "systemctl", &strings(&["--user", "daemon-reload"]))?;
            run_checked(runner, "systemctl", &strings(&["--user", "enable", &unit]))?;
            if start {
                run_checked(runner, "systemctl", &strings(&["--user", "restart", &unit]))?;
            }
            if linger {
                run_checked(runner, "loginctl", &strings(&["enable-linger"]))?;
            }
        }
        Platform::Macos => {
            let target = launchd_target(config);
            if runner
                .run("launchctl", &strings(&["print", &target]))?
                .success()
            {
                run_checked(runner, "launchctl", &strings(&["bootout", &target]))?;
            }
            // `enable` writes a persistent override, so it runs only for a
            // label that is disabled now.
            let domain = format!("gui/{}", uid());
            let disabled = runner.run("launchctl", &strings(&["print-disabled", &domain]))?;
            if parse_launchd_disabled(&disabled.stdout, &config.label) {
                run_checked(runner, "launchctl", &strings(&["enable", &target]))?;
            }
            if start {
                run_checked(
                    runner,
                    "launchctl",
                    &strings(&["bootstrap", &domain, &registration_arg]),
                )?;
            }
        }
    }
    Ok(InstallReport {
        definition: canonical,
        registration,
        started: start,
        linger_enabled: linger,
    })
}

/// When the service starts on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartsAt {
    /// At boot, before anyone logs in.
    Boot,
    /// When you log in.
    Login,
    /// Only when started by hand.
    Never,
}

/// What status reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Status {
    /// The service platform.
    pub platform: Platform,
    /// The label.
    pub label: String,
    /// Whether the canonical definition exists and matches the current
    /// configuration's rendering.
    pub definition_current: bool,
    /// Whether the registration link points at this host root.
    pub registered: bool,
    /// Whether the service manager has the service loaded.
    pub loaded: bool,
    /// Whether the launcher process is running.
    pub running: bool,
    /// The launcher's process identifier, when running.
    pub pid: Option<u32>,
    /// Whether the service is enabled to start on its own.
    pub enabled: bool,
    /// When the service starts on its own.
    pub starts_at: StartsAt,
    /// Whether the service keeps running after you log out.
    pub survives_logout: bool,
    /// The systemd linger setting; `None` on macOS or when it cannot be
    /// read.
    pub linger: Option<bool>,
    /// Whether a restart is needed to apply a change.
    pub pending_restart: bool,
    /// Why a restart is pending.
    pub pending_reasons: Vec<String>,
    /// The committed version.
    pub committed: Option<String>,
    /// The descriptor the launcher last wrote.
    pub descriptor: Option<HostDescriptor>,
}

/// Parses `key=value` lines, as `systemctl show` and `loginctl show-user`
/// print them.
#[must_use]
pub fn parse_properties(text: &str) -> std::collections::BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

/// Reads `state = running` and `pid = N` from `launchctl print`.
#[must_use]
pub fn parse_launchd_print(text: &str) -> (bool, Option<u32>) {
    let mut running = false;
    let mut pid = None;
    for line in text.lines() {
        let line = line.trim();
        if line == "state = running" {
            running = true;
        } else if let Some(value) = line.strip_prefix("pid = ") {
            pid = value.trim().parse().ok();
        }
    }
    (running, pid)
}

/// Reads whether `label` is disabled from `launchctl print-disabled`. Both
/// the `=> disabled` and the older `=> true` spellings mean disabled.
#[must_use]
pub fn parse_launchd_disabled(text: &str, label: &str) -> bool {
    let needle = format!("\"{label}\" =>");
    text.lines().any(|line| {
        let line = line.trim();
        line.strip_prefix(&needle)
            .is_some_and(|rest| matches!(rest.trim(), "disabled" | "true"))
    })
}

/// Reports the service's state without changing it.
pub fn status(layout: &Layout, config: &Config, runner: &mut dyn Runner) -> Result<Status> {
    let canonical = canonical_path(layout, config);
    let registration = registration_path(config);
    let expected = render(layout, config)?;
    let definition_current =
        fsx::read_optional(&canonical)?.is_some_and(|bytes| bytes == expected.as_bytes());
    let registered = fs::read_link(&registration).ok().as_deref() == Some(canonical.as_path());
    let mut reasons = Vec::new();
    if !definition_current {
        reasons.push(
            "the service definition differs from the configuration; run install again".into(),
        );
    }
    let (loaded, running, pid, enabled, linger);
    match config.platform {
        Platform::Linux => {
            let unit = file_name(config);
            let show = runner.run(
                "systemctl",
                &strings(&[
                    "--user",
                    "show",
                    &unit,
                    "--property=LoadState,ActiveState,SubState,UnitFileState,NeedDaemonReload,MainPID",
                ]),
            )?;
            let properties = parse_properties(&show.stdout);
            let get = |key: &str| properties.get(key).map(String::as_str).unwrap_or("");
            loaded = show.success() && get("LoadState") == "loaded";
            running = get("ActiveState") == "active" && get("SubState") == "running";
            pid = get("MainPID").parse().ok().filter(|pid| *pid != 0);
            enabled = matches!(
                get("UnitFileState"),
                "enabled" | "enabled-runtime" | "linked"
            );
            if get("NeedDaemonReload") == "yes" {
                reasons.push("systemd needs a daemon reload for the changed unit".into());
            }
            let show_user = runner.run(
                "loginctl",
                &strings(&["show-user", &uid().to_string(), "--property=Linger"]),
            )?;
            linger = show_user
                .success()
                .then(|| {
                    parse_properties(&show_user.stdout)
                        .get("Linger")
                        .map(|value| value == "yes")
                })
                .flatten();
        }
        Platform::Macos => {
            let print = runner.run("launchctl", &strings(&["print", &launchd_target(config)]))?;
            loaded = print.success();
            (running, pid) = if loaded {
                parse_launchd_print(&print.stdout)
            } else {
                (false, None)
            };
            let domain = format!("gui/{}", uid());
            let disabled = runner.run("launchctl", &strings(&["print-disabled", &domain]))?;
            enabled = registered && !parse_launchd_disabled(&disabled.stdout, &config.label);
            linger = None;
        }
    }
    let starts_at = match (enabled, linger) {
        (false, _) => StartsAt::Never,
        (true, Some(true)) => StartsAt::Boot,
        (true, _) => StartsAt::Login,
    };
    let state: Option<LauncherState> = fsx::read_optional(&layout.state())?
        .map(|bytes| serde_json::from_slice(&bytes))
        .transpose()?;
    let descriptor = launcher::read_descriptor(layout).ok().flatten();
    if fs::symlink_metadata(layout.request()).is_ok() {
        reasons.push("an update request is waiting for the launcher".into());
    }
    if let (Some(state), Some(descriptor)) = (&state, &descriptor)
        && running
        && descriptor.version.as_deref() != Some(state.committed.as_str())
    {
        reasons.push("the running host is not the committed version".into());
    }
    Ok(Status {
        platform: config.platform,
        label: config.label.clone(),
        definition_current,
        registered,
        loaded,
        running,
        pid,
        enabled,
        starts_at,
        survives_logout: config.platform == Platform::Linux && linger == Some(true),
        linger,
        pending_restart: !reasons.is_empty(),
        pending_reasons: reasons,
        committed: state.map(|state| state.committed),
        descriptor,
    })
}

/// Restarts the service, loading it first when it is registered but not
/// loaded.
pub fn restart(config: &Config, runner: &mut dyn Runner) -> Result<()> {
    match config.platform {
        Platform::Linux => {
            run_checked(
                runner,
                "systemctl",
                &strings(&["--user", "restart", &file_name(config)]),
            )?;
        }
        Platform::Macos => {
            let target = launchd_target(config);
            if runner
                .run("launchctl", &strings(&["print", &target]))?
                .success()
            {
                run_checked(runner, "launchctl", &strings(&["kickstart", "-k", &target]))?;
            } else {
                let domain = format!("gui/{}", uid());
                let registration = registration_path(config).display().to_string();
                run_checked(
                    runner,
                    "launchctl",
                    &strings(&["bootstrap", &domain, &registration]),
                )?;
            }
        }
    }
    Ok(())
}

/// What uninstall did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UninstallReport {
    /// Whether the service manager reports the service stopped and
    /// unloaded when uninstall returns.
    pub stopped: bool,
    /// Whether the registration link was removed.
    pub registration_removed: bool,
    /// Whether the canonical definition was removed.
    pub definition_removed: bool,
    /// What uninstall leaves in place.
    pub preserved: Vec<String>,
}

/// Stops and unregisters the service. It keeps the host's state, bundles,
/// launcher record, descriptor, logs, and the linger setting.
pub fn uninstall(
    layout: &Layout,
    config: &Config,
    runner: &mut dyn Runner,
) -> Result<UninstallReport> {
    let canonical = canonical_path(layout, config);
    let registration = registration_path(config);
    // Read before stopping: `systemctl disable` removes a linked unit's
    // registration link itself.
    let ours = fs::read_link(&registration).ok().as_deref() == Some(canonical.as_path());
    let stopped = match config.platform {
        Platform::Linux => {
            let unit = file_name(config);
            // `disable --now` waits for the stop job. Disabling a unit that
            // was never enabled fails harmlessly, so the result is read
            // from the unit's state instead.
            let _ = runner.run(
                "systemctl",
                &strings(&["--user", "disable", "--now", &unit]),
            )?;
            let show = runner.run(
                "systemctl",
                &strings(&["--user", "show", &unit, "--property=ActiveState"]),
            )?;
            parse_properties(&show.stdout)
                .get("ActiveState")
                .is_none_or(|state| state == "inactive" || state == "failed")
        }
        Platform::Macos => {
            let target = launchd_target(config);
            let print = strings(&["print", &target]);
            if runner.run("launchctl", &print)?.success() {
                run_checked(runner, "launchctl", &strings(&["bootout", &target]))?;
            }
            // `bootout` can return while launchd still waits for the
            // launcher to exit, so wait for the agent to disappear.
            let deadline =
                std::time::Instant::now() + Duration::from_secs(config.stop_grace_secs + 10);
            loop {
                if !runner.run("launchctl", &print)?.success() {
                    break true;
                }
                if std::time::Instant::now() >= deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    if ours && fs::read_link(&registration).ok().as_deref() == Some(canonical.as_path()) {
        fs::remove_file(&registration)?;
    }
    if config.platform == Platform::Linux {
        run_checked(runner, "systemctl", &strings(&["--user", "daemon-reload"]))?;
    }
    let definition_removed = fs::symlink_metadata(&canonical).is_ok();
    fsx::remove_file_if_present(&canonical)?;
    let mut preserved = strings(&[
        "host state directories",
        "staged bundles",
        "launcher record and descriptor",
        "logs",
    ]);
    if config.platform == Platform::Linux {
        preserved.push("the linger setting".into());
    }
    Ok(UninstallReport {
        stopped,
        registration_removed: ours,
        definition_removed,
        preserved,
    })
}

#[cfg(test)]
mod tests;
