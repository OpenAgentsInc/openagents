//! A sealed session: model commands that can't reach GitHub or, when the
//! seal says so, the network.
//!
//! An evaluation that replays a past issue is worthless if the model can
//! read the closed issue and its merged fix. [`Seal`] closes the two ways
//! a command would do that. Every command a sealed [`crate::Workspace`]
//! runs gets:
//!
//! - no GitHub credential: every `GH_*` and `GITHUB_*` variable is
//!   withheld, `GH_CONFIG_DIR` names an empty directory the seal owns, so
//!   the operator's `gh` login is out of reach, and Git's credential
//!   helpers are cleared for the command;
//! - a `gh` that refuses: a stub earlier on `PATH` that prints
//!   [`GH_REFUSAL`] and exits 1;
//! - when the seal is offline, no network beyond loopback, enforced by
//!   `coder-boundary`, with `CARGO_NET_OFFLINE=true` so Cargo builds from
//!   what's already fetched instead of failing on the index.
//!
//! Git keeps working on the local checkout.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Files and tool state a sealed evaluation may reach. All paths name
/// host-approved inputs; the model cannot change this scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadScope {
    pub readable: Vec<PathBuf>,
    pub writable: Vec<PathBuf>,
    pub environment: Vec<(OsString, OsString)>,
}

/// What the stub `gh` prints before it exits 1.
pub const GH_REFUSAL: &str = "gh: GitHub access is off during an evaluation run.";

/// How a sealed session's commands are cut off; see the module docs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seal {
    /// The directory holding the stub `gh`, put first on `PATH`.
    stubs: PathBuf,
    /// The empty directory `GH_CONFIG_DIR` names.
    gh_config: PathBuf,
    /// Whether commands run with no network beyond loopback.
    offline: bool,
    reads: Option<ReadScope>,
}

impl Seal {
    /// Lays the seal out under `dir`, which it creates: `dir/bin/gh`, the
    /// stub, and `dir/gh-config`, empty. `offline` takes the network away
    /// from every command as well.
    ///
    /// # Errors
    ///
    /// The I/O error when a directory or the stub can't be written.
    pub fn create(dir: &Path, offline: bool) -> std::io::Result<Self> {
        let stubs = dir.join("bin");
        let gh_config = dir.join("gh-config");
        std::fs::create_dir_all(&stubs)?;
        std::fs::create_dir_all(&gh_config)?;
        let gh = stubs.join("gh");
        std::fs::write(&gh, format!("#!/bin/sh\necho '{GH_REFUSAL}' >&2\nexit 1\n"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(Seal {
            stubs: stubs.canonicalize()?,
            gh_config: gh_config.canonicalize()?,
            offline,
            reads: None,
        })
    }

    /// Confines commands to their workspace, system tools, and this scope.
    #[must_use]
    pub fn with_read_scope(mut self, scope: ReadScope) -> Self {
        self.reads = Some(scope);
        self
    }

    #[must_use]
    pub fn read_scope(&self) -> Option<&ReadScope> {
        self.reads.as_ref()
    }

    /// Applies the same read scope to a session command or a host test gate.
    #[must_use]
    pub fn constrain_reads(&self, mut spec: coder_boundary::Spec) -> coder_boundary::Spec {
        if let Some(scope) = &self.reads {
            spec = spec
                .confining_reads()
                .readable(&self.stubs)
                .readable(&self.gh_config);
            for path in &scope.readable {
                spec = spec.readable(path);
            }
            for path in &scope.writable {
                spec = spec.writable(path);
            }
        }
        spec
    }

    /// Whether commands run with no network beyond loopback.
    #[must_use]
    pub fn offline(&self) -> bool {
        self.offline
    }

    /// The directory the stub `gh` is in.
    #[must_use]
    pub fn stubs(&self) -> &Path {
        &self.stubs
    }

    /// The empty directory `GH_CONFIG_DIR` names.
    #[must_use]
    pub fn gh_config(&self) -> &Path {
        &self.gh_config
    }

    /// Sets `command`'s environment as the module docs say. The network
    /// half is the boundary's, not this: see [`crate::Workspace::sealed_by`].
    pub fn apply(&self, command: &mut Command) {
        for (name, _) in std::env::vars_os() {
            if name.to_str().is_some_and(is_github) {
                command.env_remove(&name);
            }
        }
        for name in ["SSH_AUTH_SOCK", "GIT_ASKPASS", "SSH_ASKPASS"] {
            command.env_remove(name);
        }
        command.env("GH_CONFIG_DIR", &self.gh_config);
        command.env("GH_PROMPT_DISABLED", "1");
        command.env("GIT_TERMINAL_PROMPT", "0");
        // An empty `credential.helper` clears every helper the operator's
        // Git configuration names, such as `gh auth git-credential`.
        command.env("GIT_CONFIG_COUNT", "1");
        command.env("GIT_CONFIG_KEY_0", "credential.helper");
        command.env("GIT_CONFIG_VALUE_0", "");
        let mut path = OsString::from(self.stubs.as_os_str());
        if let Some(rest) = std::env::var_os("PATH").filter(|rest| !rest.is_empty()) {
            path.push(":");
            path.push(rest);
        }
        command.env("PATH", path);
        if self.offline {
            command.env("CARGO_NET_OFFLINE", "true");
        }
        if let Some(scope) = &self.reads {
            command.envs(scope.environment.iter().map(|(name, value)| (name, value)));
        }
    }
}

/// A variable that configures or authenticates GitHub access: `GH_*` and
/// `GITHUB_*`, such as `GH_TOKEN`, `GH_ENTERPRISE_TOKEN`, `GH_HOST`, and
/// `GITHUB_TOKEN`.
pub fn is_github(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("GH_") || upper.starts_with("GITHUB_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_variables_are_withheld_and_others_kept() {
        for name in [
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "GH_ENTERPRISE_TOKEN",
            "GITHUB_ENTERPRISE_TOKEN",
            "GH_HOST",
            "GH_CONFIG_DIR",
            "gh_token",
        ] {
            assert!(is_github(name), "{name}");
        }
        for name in ["PATH", "HOME", "GIT_DIR", "HIGH_SCORE", "CARGO_HOME"] {
            assert!(!is_github(name), "{name}");
        }
    }

    /// The command's environment, as it will be spawned: set values, and
    /// `None` for a removed variable.
    fn envs(command: &Command) -> Vec<(String, Option<String>)> {
        command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    #[test]
    fn a_sealed_command_gets_an_empty_gh_config_a_stub_gh_and_no_credential_helper() {
        let dir = tempfile::tempdir().unwrap();
        let seal = Seal::create(dir.path(), true).unwrap();
        let mut command = Command::new("/bin/sh");
        seal.apply(&mut command);
        let set = envs(&command);
        let get = |name: &str| {
            set.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(
            get("GH_CONFIG_DIR"),
            Some(Some(seal.gh_config().display().to_string()))
        );
        assert!(
            std::fs::read_dir(seal.gh_config())
                .unwrap()
                .next()
                .is_none()
        );
        let path = get("PATH").flatten().unwrap();
        assert!(
            path.starts_with(&seal.stubs().display().to_string()),
            "{path}"
        );
        assert_eq!(
            get("GIT_CONFIG_KEY_0"),
            Some(Some("credential.helper".into()))
        );
        assert_eq!(get("GIT_CONFIG_VALUE_0"), Some(Some(String::new())));
        assert_eq!(get("CARGO_NET_OFFLINE"), Some(Some("true".into())));
        assert_eq!(get("SSH_AUTH_SOCK"), Some(None));
        // Every GitHub variable this process holds is removed.
        for (name, _) in std::env::vars() {
            if is_github(&name) && name != "GH_CONFIG_DIR" {
                assert_eq!(get(&name), Some(None), "{name}");
            }
        }
        let online = Seal::create(&dir.path().join("online"), false).unwrap();
        let mut command = Command::new("/bin/sh");
        online.apply(&mut command);
        assert!(
            !envs(&command)
                .iter()
                .any(|(key, _)| key == "CARGO_NET_OFFLINE")
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_stub_gh_refuses_plainly() {
        let dir = tempfile::tempdir().unwrap();
        let seal = Seal::create(dir.path(), false).unwrap();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "gh issue view 9450"]);
        seal.apply(&mut command);
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), GH_REFUSAL);
    }
}
