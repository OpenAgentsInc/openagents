//! The background engine for one data folder (#11931,
//! `docs/desktop/move-running-agents.md`).
//!
//! On launch the desktop app makes sure a service runs `openagents headless`
//! for its data folder, and attaches to it over the engine IPC instead of
//! embedding an engine. The service is a LaunchAgent on macOS and a systemd
//! user unit on Linux, named after the data folder, so two data folders never
//! share one. Agents then keep running when the last window closes or the app
//! quits.
//!
//! - [`ServiceSpec`] renders the LaunchAgent property list or the systemd
//!   unit, and says whether the installed file is current.
//! - [`decide`] is the attach-or-start decision; [`ensure`] carries it out
//!   through a [`ServiceManager`].
//! - [`preflight`] lists the sign-ins the service can't read, before an app
//!   hands its runs to it.
//! - [`cli`] is the hidden `openagents background` command.
//!
//! Nothing here touches the real home folder unless the caller passes it:
//! [`Host::from_env`] is the only place that reads `HOME`, and it panics in
//! this crate's tests.

pub mod cli;
mod decide;
mod manager;
pub mod preflight;
mod service;

pub use decide::{Action, EngineMode, Observed, decide, engine_mode, is_source_build};
pub use manager::{ServiceManager, SystemManager};
pub use service::{FileState, Host, Platform, ServiceSpec, captured_env, folder_id};

use std::path::Path;
use std::time::{Duration, Instant};

/// The file in the data folder that remembers the person's choice: `background`
/// or `embedded`. "Move to background" writes it.
pub const PREFERENCE_FILE: &str = "engine-mode";

/// The environment variable that overrides the preference for one launch.
pub const MODE_ENV: &str = "OPENAGENTS_ENGINE_MODE";

/// How long [`ensure`] waits for a started service to answer on its port.
pub const START_WAIT: Duration = Duration::from_secs(20);

/// The saved choice in `data_dir`, if any.
pub fn read_preference(data_dir: &Path) -> Option<EngineMode> {
    let text = std::fs::read_to_string(data_dir.join(PREFERENCE_FILE)).ok()?;
    EngineMode::parse(text.trim())
}

/// Save the choice in `data_dir`.
pub fn write_preference(data_dir: &Path, mode: EngineMode) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(
        data_dir.join(PREFERENCE_FILE),
        format!("{}\n", mode.as_str()),
    )
}

/// What [`ensure`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// An engine answers on the port: attach to it.
    Running {
        /// The service was started by this call.
        started: bool,
        /// The service file was written by this call.
        installed: bool,
    },
    /// Embed an engine in the app, for the reason given.
    Embed(String),
    /// The service couldn't be started; the app embeds instead.
    Failed(String),
}

impl Outcome {
    /// Whether the app should attach over IPC.
    pub fn attach(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// Everything [`ensure`] needs, passed in so tests use temporary folders and
/// a recording service manager.
pub struct Ensure<'a> {
    pub spec: &'a ServiceSpec,
    pub mode: EngineMode,
    /// Another process holds the data folder's engine lock.
    pub data_dir_locked: bool,
    pub manager: &'a dyn ServiceManager,
    /// Whether an engine answers on the IPC port right now.
    pub listening: &'a dyn Fn() -> bool,
    /// How long to wait for the engine to answer after a start.
    pub wait: Duration,
}

/// Attach to this data folder's background engine, starting or installing its
/// service first when needed.
pub fn ensure(input: Ensure<'_>) -> Outcome {
    let observed = Observed {
        engine_listening: (input.listening)(),
        data_dir_locked: input.data_dir_locked,
        service_file: input.spec.file_state(),
    };
    match decide(input.mode, observed) {
        Action::Attach => Outcome::Running {
            started: false,
            installed: false,
        },
        Action::Embed => Outcome::Embed("the engine runs inside the app".into()),
        Action::WaitForEngine => {
            if wait_for(input.listening, input.wait) {
                Outcome::Running {
                    started: false,
                    installed: false,
                }
            } else {
                Outcome::Failed("another engine holds the data folder but doesn't answer".into())
            }
        }
        action @ (Action::Start | Action::InstallAndStart) => {
            let installed = action == Action::InstallAndStart;
            if installed && let Err(error) = input.spec.write() {
                return Outcome::Failed(format!(
                    "couldn't write {}: {error}",
                    input.spec.file_path().display()
                ));
            }
            if let Err(error) = input.manager.start(input.spec, installed) {
                return Outcome::Failed(error);
            }
            if wait_for(input.listening, input.wait) {
                Outcome::Running {
                    started: true,
                    installed,
                }
            } else {
                Outcome::Failed(format!(
                    "the background engine didn't answer within {} seconds",
                    input.wait.as_secs()
                ))
            }
        }
    }
}

fn wait_for(listening: &dyn Fn() -> bool, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if listening() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Whether something accepts connections on `127.0.0.1:port`.
pub fn port_listening(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

#[cfg(test)]
mod tests;
