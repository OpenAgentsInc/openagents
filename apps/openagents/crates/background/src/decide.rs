//! The attach-or-start decision. Pure.

use std::path::{Component, Path};

use crate::service::{FileState, Platform};

/// Where the app's engine runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    /// In the background service; the app attaches over IPC.
    Background,
    /// Inside the app process, as before #11931.
    Embedded,
}

impl EngineMode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "background" => Some(Self::Background),
            "embedded" => Some(Self::Embedded),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Embedded => "embedded",
        }
    }
}

/// Pick the engine mode for this launch, with the reason.
///
/// An explicit `env` value wins, then the saved `preference`. Without either,
/// a supported platform runs the engine in the background, except for a
/// source build (`cargo run`) or a translocated copy, which would install a
/// service that points at a folder that won't last, and a development bearer
/// token, which the service doesn't get.
pub fn engine_mode(
    platform: Option<Platform>,
    env: Option<&str>,
    preference: Option<EngineMode>,
    source_build: bool,
    dev_token: bool,
) -> (EngineMode, &'static str) {
    if platform.is_none() {
        return (
            EngineMode::Embedded,
            "no background service on this platform",
        );
    }
    if let Some(mode) = env.and_then(EngineMode::parse) {
        return (mode, "set by OPENAGENTS_ENGINE_MODE");
    }
    if let Some(mode) = preference {
        return (mode, "saved choice");
    }
    if dev_token {
        return (EngineMode::Embedded, "development token");
    }
    if source_build {
        return (EngineMode::Embedded, "source build");
    }
    (EngineMode::Background, "default")
}

/// Whether a service shouldn't point at `exe`: a Cargo build output, or a
/// copy macOS runs from a temporary App Translocation folder.
pub fn is_source_build(exe: &Path) -> bool {
    exe.components().any(|component| {
        component == Component::Normal("target".as_ref())
            || component == Component::Normal("AppTranslocation".as_ref())
    })
}

/// What the app sees at launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    /// An engine answers on the IPC port.
    pub engine_listening: bool,
    /// Another process holds the data folder's engine lock.
    pub data_dir_locked: bool,
    /// The installed service file, compared with the one this app would write.
    pub service_file: FileState,
}

/// What to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Attach to the engine that answers.
    Attach,
    /// An engine is starting (it holds the lock but doesn't answer yet).
    WaitForEngine,
    /// Start the installed, current service.
    Start,
    /// Write the service file, then start it.
    InstallAndStart,
    /// Embed an engine in the app.
    Embed,
}

/// The attach-or-start decision.
///
/// A running engine always wins, whatever the mode: two engines must never
/// share a data folder. The service file is only rewritten while no engine
/// runs, so an install never restarts a live engine.
pub fn decide(mode: EngineMode, observed: Observed) -> Action {
    if observed.engine_listening {
        return Action::Attach;
    }
    if observed.data_dir_locked {
        return Action::WaitForEngine;
    }
    match (mode, observed.service_file) {
        (EngineMode::Embedded, _) => Action::Embed,
        (EngineMode::Background, FileState::Current) => Action::Start,
        (EngineMode::Background, FileState::Missing | FileState::Different) => {
            Action::InstallAndStart
        }
    }
}
