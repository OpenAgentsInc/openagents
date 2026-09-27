//! The host service: install it, move it to the staged bundle, or restart it
//! so recorded settings apply, through the `coder-service` binary.
//!
//! `scripts/coder-host.py` stages and selects bundles; `coder-service` owns
//! installation, trial updates, and rollback. This module only decides which
//! of its commands to run.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::{Error, Result};

/// The fields of `coder-service service status` this module reads.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Status {
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub committed: Option<String>,
    #[serde(default)]
    pub pending_restart: bool,
    #[serde(default)]
    pub linger: Option<bool>,
    #[serde(default)]
    pub starts_at: Option<String>,
}

/// What to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// No service yet: install it with the selected bundle.
    Install,
    /// The service runs another bundle: a trial update to this one.
    Update(String),
    /// The service runs the selected bundle but is stopped, or settings
    /// changed: restart it.
    Restart,
    /// Nothing to do.
    Nothing,
}

/// Decide the step from the current status (`None` when no service is
/// installed), the staged bundle the installation helper selected, and
/// whether this run changed the host's recorded settings.
///
/// # Errors
/// Refuses when no bundle is staged.
pub fn decide(status: Option<&Status>, selected: Option<&str>, changed: bool) -> Result<Step> {
    let selected = selected.ok_or_else(|| {
        Error::new(
            "no host bundle is staged; run scripts/link-device.sh, which builds and stages one",
        )
    })?;
    Ok(match status {
        None => Step::Install,
        Some(status) if status.committed.as_deref() != Some(selected) => {
            Step::Update(selected.to_owned())
        }
        Some(status) if !status.loaded || !status.running || status.pending_restart || changed => {
            Step::Restart
        }
        Some(_) => Step::Nothing,
    })
}

/// The `coder-service` command: `given`, else the one beside this
/// executable, else `~/.openagents/bin/coder-service`, else `PATH`'s.
#[must_use]
pub fn program(given: Option<&Path>) -> PathBuf {
    if let Some(path) = given {
        return path.to_path_buf();
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
        .and_then(|exe| exe.parent().map(|dir| dir.join("coder-service")))
        .filter(|path| path.is_file());
    let installed = crate::home(".openagents/bin/coder-service")
        .ok()
        .filter(|path| path.is_file());
    beside
        .or(installed)
        .unwrap_or_else(|| PathBuf::from("coder-service"))
}

/// Read the service status, or `None` when no service is installed.
///
/// # Errors
/// Reports a `coder-service` that cannot run or prints something else.
pub fn status(program: &Path) -> Result<Option<Status>> {
    let output = Command::new(program)
        .args(["service", "status"])
        .output()
        .map_err(|_| {
            Error::new(format!(
                "cannot run {}; build it with `cargo build --release -p coder-service` or pass --coder-service",
                program.display()
            ))
        })?;
    if !output.status.success() {
        // Status refuses when there is no service configuration yet.
        return Ok(None);
    }
    serde_json::from_slice(&output.stdout)
        .map(Some)
        .map_err(|_| Error::new("coder-service status printed something unexpected"))
}

/// Run the step. `host_key` is the host's public key, which the descriptor
/// names.
///
/// # Errors
/// Reports a failed `coder-service` command with its first error line.
pub fn apply(program: &Path, step: &Step, host_key: &str, linger: bool) -> Result<()> {
    let mut command = Command::new(program);
    match step {
        Step::Nothing => return Ok(()),
        Step::Install => {
            command.args(["service", "install", "--host-key", host_key]);
            if linger {
                command.arg("--linger");
            }
        }
        Step::Update(to) => {
            command.args(["update", "--to", to, "--wait", "180"]);
        }
        Step::Restart => {
            command.args(["service", "restart"]);
        }
    }
    let output = command
        .output()
        .map_err(|_| Error::new("cannot run coder-service"))?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stderr);
    let why = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    Err(Error::new(format!(
        "coder-service {} failed: {why}",
        match step {
            Step::Install => "service install",
            Step::Update(_) => "update",
            Step::Restart => "service restart",
            Step::Nothing => "",
        }
    )))
}

/// The bundle the installation helper selected under `root`
/// (`~/.openagents/host-bundle` by default).
///
/// # Errors
/// Reports an unreadable selection.
pub fn selected(root: &Path) -> Result<Option<String>> {
    coder_service::bundle::selected(root)
        .map_err(|error| Error::new(format!("cannot read the staged bundle: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(committed: &str) -> Status {
        Status {
            loaded: true,
            running: true,
            committed: Some(committed.into()),
            ..Status::default()
        }
    }

    #[test]
    fn install_update_restart_or_nothing() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        assert!(decide(None, None, false).is_err());
        assert_eq!(decide(None, Some(&a), false).unwrap(), Step::Install);
        assert_eq!(
            decide(Some(&running(&a)), Some(&b), false).unwrap(),
            Step::Update(b.clone())
        );
        assert_eq!(
            decide(Some(&running(&a)), Some(&a), false).unwrap(),
            Step::Nothing
        );
        assert_eq!(
            decide(Some(&running(&a)), Some(&a), true).unwrap(),
            Step::Restart
        );
        let stopped = Status {
            running: false,
            ..running(&a)
        };
        assert_eq!(
            decide(Some(&stopped), Some(&a), false).unwrap(),
            Step::Restart
        );
        let pending = Status {
            pending_restart: true,
            ..running(&a)
        };
        assert_eq!(
            decide(Some(&pending), Some(&a), false).unwrap(),
            Step::Restart
        );
    }

    #[test]
    fn status_reads_the_service_json() {
        let json = br#"{"platform":"linux","label":"org.openagents.coder-host","definition_current":true,
            "registered":true,"loaded":true,"running":true,"pid":7,"enabled":true,"starts_at":"boot",
            "survives_logout":true,"linger":true,"pending_restart":false,"pending_reasons":[],
            "committed":"aa","descriptor":null}"#;
        let status: Status = serde_json::from_slice(json).unwrap();
        assert!(status.running && status.loaded);
        assert_eq!(status.linger, Some(true));
        assert_eq!(status.starts_at.as_deref(), Some("boot"));
    }
}
