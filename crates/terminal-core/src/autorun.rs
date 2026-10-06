//! The opt-in that lets an exact read-only proposal run without Enter
//! (#10695).
//!
//! Off by default. A person admits auto-run for one workspace root at a
//! time, and the admission is recorded with who made it and when; it is
//! never inherited from pairing, a share, or world membership. Even then a
//! proposal runs at once only when the shared effect boundary classed it
//! read-only, its command has no shell substitution, redirection, or
//! sequencing, its binding is the pane's current one, and it is still
//! pending. Anything else stays pending for Enter. Turning auto-run off
//! takes effect for the next proposal; nothing already pending runs.
//!
//! The setting is a file, `autorun.json`, written owner-only and replaced
//! atomically, so every surface on this computer that reads it sees the
//! same answer. Coder worktree policies and studio merge approval are
//! separate settings and never read it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The settings file's schema.
pub const SCHEMA: &str = "openagents.terminal-autorun.v1";
/// The most workspaces one file admits.
pub const WORKSPACES_MAX: usize = 64;

/// One workspace root where read-only proposals run without Enter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    /// An absolute directory; proposals bound to it or below it qualify.
    pub root: String,
    /// Who turned auto-run on, such as `local-user`.
    pub admitted_by: String,
    /// Unix milliseconds.
    pub admitted_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: String,
    /// Increases with every change, so a reader can tell a stale copy.
    revision: u64,
    workspaces: Vec<Admission>,
}

/// The auto-run setting, backed by its file when it has one.
#[derive(Clone, Debug, Default)]
pub struct AutoRun {
    path: Option<PathBuf>,
    file: File,
}

impl AutoRun {
    /// The setting stored at `path`. A missing, unreadable, or malformed
    /// file reads as off.
    #[must_use]
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let file = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<File>(&bytes).ok())
            .filter(|file| file.schema == SCHEMA && file.workspaces.len() <= WORKSPACES_MAX)
            .unwrap_or_default();
        Self {
            path: Some(path),
            file,
        }
    }

    /// Reads the file again, so a change another surface made applies to
    /// the next proposal.
    pub fn reload(&mut self) {
        if let Some(path) = self.path.clone() {
            *self = Self::load(path);
        }
    }

    /// The admitted workspaces.
    #[must_use]
    pub fn workspaces(&self) -> &[Admission] {
        &self.file.workspaces
    }

    /// The change counter.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.file.revision
    }

    /// The admission that covers `cwd`, if any.
    #[must_use]
    pub fn admitting(&self, cwd: &str) -> Option<&Admission> {
        let cwd = Path::new(cwd);
        self.file
            .workspaces
            .iter()
            .find(|admission| cwd.starts_with(&admission.root))
    }

    /// Turns auto-run on for `root`.
    ///
    /// # Errors
    /// A relative root, a full file, or a write that failed; the setting
    /// is unchanged then.
    pub fn admit(&mut self, root: &str, by: &str, at: u64) -> Result<(), String> {
        if !Path::new(root).is_absolute() || root.chars().any(char::is_control) || by.is_empty() {
            return Err("auto-run needs an absolute workspace root and who admits it".into());
        }
        let mut next = self.file.clone();
        next.workspaces.retain(|admission| admission.root != root);
        if next.workspaces.len() >= WORKSPACES_MAX {
            return Err("auto-run admits at most 64 workspaces".into());
        }
        next.workspaces.push(Admission {
            root: root.to_owned(),
            admitted_by: by.to_owned(),
            admitted_at: at,
        });
        self.commit(next)
    }

    /// Turns auto-run off for every admission that covers `cwd`. Returns
    /// whether one did.
    ///
    /// # Errors
    /// A write that failed; the setting is unchanged then.
    pub fn revoke(&mut self, cwd: &str) -> Result<bool, String> {
        let cwd = Path::new(cwd);
        let mut next = self.file.clone();
        next.workspaces
            .retain(|admission| !cwd.starts_with(&admission.root));
        if next.workspaces.len() == self.file.workspaces.len() {
            return Ok(false);
        }
        self.commit(next).map(|()| true)
    }

    fn commit(&mut self, mut next: File) -> Result<(), String> {
        next.schema = SCHEMA.into();
        next.revision = self.file.revision + 1;
        if let Some(path) = &self.path {
            write(path, &next)?;
        }
        self.file = next;
        Ok(())
    }
}

/// Writes the file owner-only through a temporary name and a rename.
fn write(path: &Path, file: &File) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(file).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut out = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        out.write_all(&bytes).map_err(|error| error.to_string())?;
        out.sync_all().map_err(|error| error.to_string())?;
    }
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())
}

/// Whether a command's text is one plain invocation: no substitution,
/// expansion of variables, redirection, pipes, background jobs, sequencing,
/// globbing, or line breaks. Only such a command can run without Enter.
#[must_use]
pub fn plain(command: &str) -> bool {
    !command.is_empty()
        && !command.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '$' | '`'
                        | '|'
                        | '&'
                        | ';'
                        | '<'
                        | '>'
                        | '('
                        | ')'
                        | '{'
                        | '}'
                        | '*'
                        | '?'
                        | '['
                        | ']'
                        | '~'
                        | '!'
                        | '\\'
                        | '#'
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default_persisted_and_revocable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("terminal/autorun.json");
        let mut setting = AutoRun::load(&path);
        assert!(setting.workspaces().is_empty());
        assert!(setting.admitting("/srv/app").is_none());
        assert!(setting.admit("relative", "local-user", 1).is_err());
        setting.admit("/srv/app", "local-user", 1).unwrap();
        assert!(setting.admitting("/srv/app/src").is_some());
        assert!(setting.admitting("/srv/application").is_none());
        // Another surface reading the same file sees it.
        let other = AutoRun::load(&path);
        assert_eq!(other.workspaces(), setting.workspaces());
        assert_eq!(other.revision(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(setting.revoke("/srv/app/src").unwrap());
        assert!(!setting.revoke("/srv/app").unwrap());
        assert!(AutoRun::load(&path).admitting("/srv/app").is_none());
        // A malformed file reads as off.
        std::fs::write(&path, b"{\"schema\":\"other\"}").unwrap();
        assert!(AutoRun::load(&path).workspaces().is_empty());
    }

    #[test]
    fn only_a_plain_invocation_qualifies() {
        for good in [
            "git status",
            "ls -la src",
            "cargo test -p terminal-core",
            "cat README.md",
        ] {
            assert!(plain(good), "{good}");
        }
        for bad in [
            "",
            "cat $(which sh)",
            "echo `id`",
            "ls > out",
            "ls | sh",
            "true && rm -rf x",
            "ls; rm x",
            "echo $HOME",
            "ls *",
            "cat ~/.ssh/id_ed25519",
            "ls\nrm x",
            "sleep 9 &",
        ] {
            assert!(!plain(bad), "{bad:?}");
        }
    }
}
