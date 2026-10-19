//! The service file for one data folder: a LaunchAgent property list on macOS,
//! a systemd user unit on Linux.

use std::path::{Path, PathBuf};

/// The platforms with a background service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Linux,
}

impl Platform {
    /// This build's platform, or `None` where there's no service manager.
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else {
            None
        }
    }
}

/// Where service files live for one user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub platform: Platform,
    pub home: PathBuf,
    /// `XDG_CONFIG_HOME`, or `~/.config`.
    pub config_home: PathBuf,
}

impl Host {
    /// The current user's folders. The only reader of `HOME` in this crate.
    pub fn from_env() -> Option<Self> {
        #[cfg(test)]
        panic!("a test reached the real home folder");
        #[cfg(not(test))]
        {
            let platform = Platform::current()?;
            let home = PathBuf::from(openagents_brand::env_var_os("HOME")?);
            let config_home = std::env::var_os("XDG_CONFIG_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            Some(Self {
                platform,
                home,
                config_home,
            })
        }
    }
}

/// Environment variables the service gets from the app that installs it.
/// `PATH` lets the engine find agent programs; the rest select the same
/// deployment and workspace. Secrets such as the development bearer token
/// are never written into a service file.
const CAPTURED_ENV: &[&str] = &[
    "PATH",
    "OPENAGENTS_EDGE_URL",
    "OPENAGENTS_ORG_ID",
    "OPENAGENTS_WORKOS_CLIENT_ID",
    "OPENAGENTS_WORKOS_API_BASE",
    "OPENAGENTS_CALLBACK_PORT",
    "OPENAGENTS_HARNESS",
    "OPENAGENTS_DEVICE_NAME",
    "OPENAGENTS_DEVICE_ORIGIN",
    "OPENAGENTS_CLOUD_HOST_COMPUTER_ID",
    "CODER_DESKTOP_API_URL",
    "CODER_DESKTOP_WEBSITE_URL",
    "RUST_LOG",
];

/// The captured variables that `get` has a value for.
pub fn captured_env(get: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    CAPTURED_ENV
        .iter()
        .filter_map(|key| get(key).map(|value| (key.to_string(), value)))
        .collect()
}

/// A short, stable name for a data folder (64-bit FNV-1a of its path).
pub fn folder_id(data_dir: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data_dir.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:012x}", hash >> 16)
}

/// Whether the installed service file matches the one this app would write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Missing,
    Current,
    Different,
}

/// One data folder's background engine service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSpec {
    pub host: Host,
    /// The program the service runs with `headless`.
    pub exe: PathBuf,
    pub data_dir: PathBuf,
    pub ipc_port: u16,
    /// From [`captured_env`].
    pub env: Vec<(String, String)>,
}

impl ServiceSpec {
    /// The launchd label.
    pub fn label(&self) -> String {
        format!(
            "{}.{}",
            openagents_brand::BACKGROUND_ENGINE_LABEL,
            folder_id(&self.data_dir)
        )
    }

    /// The systemd unit name.
    pub fn unit_name(&self) -> String {
        format!(
            "{}-{}.service",
            openagents_brand::BACKGROUND_ENGINE_UNIT,
            folder_id(&self.data_dir)
        )
    }

    /// Where the service file goes.
    pub fn file_path(&self) -> PathBuf {
        match self.host.platform {
            Platform::MacOs => self
                .host
                .home
                .join("Library/LaunchAgents")
                .join(format!("{}.plist", self.label())),
            Platform::Linux => self
                .host
                .config_home
                .join("systemd/user")
                .join(self.unit_name()),
        }
    }

    /// The engine's log on macOS (systemd keeps it in the journal).
    pub fn log_path(&self) -> PathBuf {
        self.data_dir.join("logs").join("background-engine.log")
    }

    /// The full environment the engine runs with.
    fn environment(&self) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = self
            .env
            .iter()
            .filter(|(key, _)| key != "OPENAGENTS_DATA_DIR" && key != "OPENAGENTS_IPC_PORT")
            .cloned()
            .collect();
        env.push((
            "OPENAGENTS_DATA_DIR".into(),
            self.data_dir.to_string_lossy().into_owned(),
        ));
        env.push(("OPENAGENTS_IPC_PORT".into(), self.ipc_port.to_string()));
        env
    }

    /// The service file's contents.
    pub fn render(&self) -> String {
        match self.host.platform {
            Platform::MacOs => self.render_plist(),
            Platform::Linux => self.render_unit(),
        }
    }

    fn render_plist(&self) -> String {
        let mut env = String::new();
        for (key, value) in self.environment() {
            env.push_str(&format!(
                "      <key>{}</key><string>{}</string>\n",
                xml_escape(&key),
                xml_escape(&value)
            ));
        }
        // AbandonProcessGroup: keepers start their own sessions, and launchd
        // must not end them when it stops or restarts the engine.
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key><string>{label}</string>
    <key>AssociatedBundleIdentifiers</key><string>{bundle}</string>
    <key>ProgramArguments</key>
    <array>
      <string>{exe}</string>
      <string>headless</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
{env}    </dict>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key>
    <dict>
      <key>SuccessfulExit</key><false/>
    </dict>
    <key>ThrottleInterval</key><integer>10</integer>
    <key>AbandonProcessGroup</key><true/>
    <key>StandardOutPath</key><string>{log}</string>
    <key>StandardErrorPath</key><string>{log}</string>
  </dict>
</plist>
"#,
            label = xml_escape(&self.label()),
            bundle = openagents_brand::BUNDLE_ID,
            exe = xml_escape(&self.exe.to_string_lossy()),
            log = xml_escape(&self.log_path().to_string_lossy()),
        )
    }

    fn render_unit(&self) -> String {
        let mut unit = format!(
            "[Unit]\nDescription=OpenAgents background engine for {}\nAfter=network-online.target\nStartLimitIntervalSec=60\nStartLimitBurst=5\n\n[Service]\n",
            systemd_escape(&self.data_dir.to_string_lossy())
        );
        for (key, value) in self.environment() {
            let value = systemd_escape(&value)
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            unit.push_str(&format!("Environment=\"{key}={value}\"\n"));
        }
        unit.push_str(&format!(
            "ExecStart={} headless\nRestart=on-failure\nRestartSec=5\n\
             # Agents run under keepers in sessions of their own; stopping or\n\
             # restarting the engine must not end them.\nKillMode=process\n\n\
             [Install]\nWantedBy=default.target\n",
            self.exec_start()
        ));
        unit
    }

    /// The ExecStart program. A binary from the installer under
    /// `~/.openagents/app/` runs through the `current` link, so an upgrade
    /// never leaves the unit on an old version.
    fn exec_start(&self) -> String {
        if self.exe.starts_with(self.host.home.join(".openagents/app")) {
            return "%h/.openagents/app/current/openagents".into();
        }
        let path = systemd_escape(&self.exe.to_string_lossy());
        if path.contains(char::is_whitespace) || path.contains('"') {
            format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
        } else {
            path
        }
    }

    /// Compare the installed file with [`Self::render`].
    pub fn file_state(&self) -> FileState {
        match std::fs::read_to_string(self.file_path()) {
            Ok(text) if text == self.render() => FileState::Current,
            Ok(_) => FileState::Different,
            Err(_) => FileState::Missing,
        }
    }

    /// Write the service file, and the log folder it names.
    pub fn write(&self) -> std::io::Result<()> {
        let path = self.file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if self.host.platform == Platform::MacOs
            && let Some(logs) = self.log_path().parent()
        {
            std::fs::create_dir_all(logs)?;
        }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, self.render())?;
        std::fs::rename(&temp, &path)
    }
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// systemd expands `%` specifiers in most settings; a literal `%` is `%%`.
fn systemd_escape(input: &str) -> String {
    input.replace('%', "%%")
}
