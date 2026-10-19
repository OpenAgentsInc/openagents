//! Keep the background engine on the app's own build (#12179).
//!
//! The service keeps running whatever program it started with. When the app
//! finds its data folder's engine on another build, it stops that service,
//! points the service file at its own program, and starts it again. Agents
//! under keepers keep running through the restart, and the new engine picks
//! them up.

use std::time::{Duration, Instant};

use openagents_proto::EngineBuild;

use crate::{FileState, Outcome, ServiceManager, ServiceSpec};

/// How a running engine's build compares with the app's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// The same program: attach.
    Current,
    /// Another, older program, or one too old to say: replace it.
    Stale,
    /// A program newer than the app's: attach, and never downgrade it.
    Newer,
}

/// Compare the app's build with the one the engine reports. An app that
/// can't read its own build leaves the engine alone.
pub fn freshness(app: Option<&EngineBuild>, engine: Option<&EngineBuild>) -> Freshness {
    let Some(app) = app else {
        return Freshness::Current;
    };
    match engine {
        None => Freshness::Stale,
        Some(engine) if engine == app => Freshness::Current,
        Some(engine) if engine.modified_ms > app.modified_ms => Freshness::Newer,
        Some(_) => Freshness::Stale,
    }
}

/// Everything [`replace`] needs, passed in so tests use temporary folders
/// and a recording service manager.
pub struct Replace<'a> {
    pub spec: &'a ServiceSpec,
    pub manager: &'a dyn ServiceManager,
    /// Whether an engine answers on the IPC port right now.
    pub listening: &'a dyn Fn() -> bool,
    /// Whether a process holds the data folder's engine lock right now.
    pub locked: &'a dyn Fn() -> bool,
    /// How long to wait for the old engine to stop, and for the new one to
    /// answer.
    pub wait: Duration,
}

/// Restart the data folder's service on this app's program. Only a service
/// this app installed is replaced: an engine running any other way is left
/// alone, and the caller attaches to it.
pub fn replace(input: Replace<'_>) -> Outcome {
    if input.spec.file_state() == FileState::Missing {
        return Outcome::Failed("the engine doesn't run as this app's service".into());
    }
    if let Err(error) = input.manager.stop(input.spec) {
        return Outcome::Failed(error);
    }
    let stopped = wait_until(&|| !(input.listening)() && !(input.locked)(), input.wait);
    if !stopped {
        return Outcome::Failed(format!(
            "the old engine didn't stop within {} seconds",
            input.wait.as_secs()
        ));
    }
    if let Err(error) = input.spec.write() {
        return Outcome::Failed(format!(
            "couldn't write {}: {error}",
            input.spec.file_path().display()
        ));
    }
    if let Err(error) = input.manager.start(input.spec, true) {
        return Outcome::Failed(error);
    }
    if wait_until(input.listening, input.wait) {
        Outcome::Running {
            started: true,
            installed: true,
        }
    } else {
        Outcome::Failed(format!(
            "the new engine didn't answer within {} seconds",
            input.wait.as_secs()
        ))
    }
}

fn wait_until(done: &dyn Fn() -> bool, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
