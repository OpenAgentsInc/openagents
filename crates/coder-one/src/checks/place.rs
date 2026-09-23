//! Where a check reads the task's files and runs its commands.
//!
//! In an episode, the check runs in the task's own container: a path the
//! instruction names is the path on disk, and a command runs as the
//! executor ran it. A replay of a retained trial has no container, so it
//! rebuilds the task's filesystem under a replay root, the task image's
//! public files with the trial's collected outputs on top, and reads each
//! absolute path the task names under that root. Commands then run in a
//! `bwrap` sandbox whose `/` is the replay root, with the host's Nix store
//! mounted read-only for tools, no network, no home directory, and an
//! environment that holds no credentials.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Variables a check never passes to a command it runs.
pub const CREDENTIALS: &[&str] = &[
    "OPENAGENTS_API_KEY",
    "TYPESAFE_API_KEY",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "OPENAI_API_KEY",
    "CODEX_AUTH_JSON_PATH",
    "CODER_ONE_POLICY",
];

/// Where a replay keeps its tool links, inside the replay root.
const TOOLS: &str = ".replay/bin";

/// The live task filesystem, or a replay root standing in for it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Place {
    root: Option<PathBuf>,
}

impl Place {
    /// The live task filesystem.
    #[must_use]
    pub fn live() -> Self {
        Place { root: None }
    }

    /// A replay root: the task's filesystem rebuilt under `root`.
    #[must_use]
    pub fn replay(root: impl Into<PathBuf>) -> Self {
        Place {
            root: Some(root.into()),
        }
    }

    /// The place a workspace's `root` names.
    #[must_use]
    pub fn of(root: Option<&str>) -> Self {
        root.filter(|r| !r.is_empty())
            .map_or_else(Place::live, Place::replay)
    }

    /// Whether this is a replay rather than the live task.
    #[must_use]
    pub fn is_replay(&self) -> bool {
        self.root.is_some()
    }

    /// The host path of `path`, a path in the task's filesystem.
    #[must_use]
    pub fn host(&self, path: &Path) -> PathBuf {
        match &self.root {
            None => path.to_path_buf(),
            Some(root) => root.join(path.strip_prefix("/").unwrap_or(path)),
        }
    }

    /// A command that runs `script` with `sh -c` in `dir`, a directory in
    /// the task's filesystem, without credentials, and without the
    /// executables named in `hidden` on its `PATH`. `scratch` is a host
    /// directory the place may use for the hidden-executable `PATH`.
    ///
    /// # Errors
    ///
    /// Returns why the command can't run here: no shell, or no `bwrap` for
    /// a replay.
    pub fn shell(
        &self,
        script: &str,
        dir: &str,
        hidden: &[String],
        scratch: &Path,
    ) -> Result<Command, String> {
        match &self.root {
            None => {
                let shell = crate::minitask::process::which("bash")
                    .or_else(|| crate::minitask::process::which("sh"))
                    .ok_or_else(|| "no shell on this host".to_string())?;
                let mut command = Command::new(shell);
                command.arg("-c").arg(script).current_dir(dir);
                for name in CREDENTIALS {
                    command.env_remove(name);
                }
                if !hidden.is_empty() {
                    command.env("PATH", hiding_path(hidden, scratch)?);
                }
                command.stdin(Stdio::null());
                Ok(command)
            }
            Some(root) => {
                let bwrap = crate::minitask::process::which("bwrap")
                    .ok_or_else(|| "a replay needs bwrap, and this host has none".to_string())?;
                prepare_root(root, hidden)?;
                let mut command = Command::new(bwrap);
                command
                    .arg("--bind")
                    .arg(root)
                    .arg("/")
                    .args(["--ro-bind", "/nix", "/nix"])
                    .args(["--dev", "/dev", "--proc", "/proc"])
                    .args(["--unshare-net", "--unshare-pid", "--die-with-parent"])
                    .args(["--clearenv"])
                    .args([
                        "--setenv",
                        "PATH",
                        &format!(
                            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/{TOOLS}"
                        ),
                    ])
                    .args(["--setenv", "HOME", "/root"])
                    .args(["--setenv", "LANG", "C.UTF-8"])
                    .args(["--setenv", "PYTHONDONTWRITEBYTECODE", "1"])
                    .args(["--chdir", dir])
                    .args(["/bin/sh", "-c", script])
                    .stdin(Stdio::null());
                Ok(command)
            }
        }
    }
}

/// A `PATH` of links to every executable on the current `PATH` except the
/// `hidden` names, under `scratch`.
fn hiding_path(hidden: &[String], scratch: &Path) -> Result<String, String> {
    let dir = scratch.join("path");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = std::env::var_os("PATH").unwrap_or_default();
    for entry in std::env::split_paths(&path) {
        for file in std::fs::read_dir(&entry).into_iter().flatten().flatten() {
            let name = file.file_name();
            let name = name.to_string_lossy();
            if hidden.iter().any(|h| h == name.as_ref()) {
                continue;
            }
            let link = dir.join(name.as_ref());
            if !link.exists() {
                let _ = std::os::unix::fs::symlink(file.path(), &link);
            }
        }
    }
    Ok(dir.to_string_lossy().into_owned())
}

/// The host executables a replay may use: every one on this process's
/// `PATH`, and on `CODER_ONE_REPLAY_PATH`, that resolves into the Nix
/// store, so nothing under a home directory is reachable.
fn host_tools() -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for var in ["CODER_ONE_REPLAY_PATH", "PATH"] {
        if let Some(value) = std::env::var_os(var) {
            dirs.extend(std::env::split_paths(&value));
        }
    }
    for dir in dirs {
        for file in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if found.iter().any(|(n, _)| *n == name) {
                continue;
            }
            if let Ok(real) = std::fs::canonicalize(file.path())
                && real.starts_with("/nix/store/")
                && real.is_file()
            {
                found.push((name, real));
            }
        }
    }
    found
}

/// Makes `root` runnable: tool links under `/.replay/bin`, and the
/// interpreters a script's first line usually names, where the task image
/// left none.
fn prepare_root(root: &Path, hidden: &[String]) -> Result<(), String> {
    let tools = root.join(TOOLS);
    let _ = std::fs::remove_dir_all(&tools);
    std::fs::create_dir_all(&tools)
        .map_err(|e| format!("cannot create {}: {e}", tools.display()))?;
    let found = host_tools();
    for (name, real) in &found {
        if hidden.iter().any(|h| h == name) {
            continue;
        }
        let _ = std::os::unix::fs::symlink(real, tools.join(name));
    }
    for dir in [
        "bin",
        "usr/bin",
        "usr/local/bin",
        "tmp",
        "root",
        "dev",
        "proc",
        "nix",
    ] {
        let _ = std::fs::create_dir_all(root.join(dir));
    }
    for (at, name) in [
        ("bin/sh", "sh"),
        ("bin/bash", "bash"),
        ("usr/bin/env", "env"),
        ("usr/bin/python3", "python3"),
        ("usr/bin/python", "python3"),
        ("usr/local/bin/python3", "python3"),
        ("usr/local/bin/python", "python3"),
    ] {
        let link = root.join(at);
        if link.symlink_metadata().is_err()
            && let Some((_, real)) = found.iter().find(|(n, _)| n == name)
        {
            let _ = std::os::unix::fs::symlink(real, &link);
        }
    }
    Ok(())
}

/// Whether a command's output says the replay host lacks something the
/// task's image provides: a module, a command, or an interpreter.
#[must_use]
pub fn missing_environment(output: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        let missing = line.contains("ModuleNotFoundError")
            || line.contains("No module named")
            || line.ends_with("command not found")
            || line.ends_with(": not found")
            || line.contains("Cannot find module")
            || (line.contains("No such file or directory") && line.contains("bad interpreter"));
        if missing {
            return Some(crate::judge::clip(line, 200));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replay_maps_absolute_paths_under_its_root() {
        let place = Place::replay("/tmp/r");
        assert_eq!(
            place.host(Path::new("/app/out.json")),
            PathBuf::from("/tmp/r/app/out.json")
        );
        assert_eq!(
            Place::live().host(Path::new("/app/out.json")),
            PathBuf::from("/app/out.json")
        );
        assert!(!Place::of(None).is_replay());
        assert!(Place::of(Some("/tmp/r")).is_replay());
    }

    #[test]
    fn a_missing_module_is_the_environment_not_the_candidate() {
        assert!(missing_environment("ModuleNotFoundError: No module named 'yaml'").is_some());
        assert!(missing_environment("sh: 1: rdflib-cli: not found").is_some());
        assert!(missing_environment("AssertionError: counts differ").is_none());
    }
}
