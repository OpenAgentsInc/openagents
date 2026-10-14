//! Where everything lives: the layout every platform shares (the deny
//! list and the file-system checks stay in [`crate::paths`], Unix-only).

use std::path::{Path, PathBuf};

use crate::task_paths::{task_store, task_targets, task_worktrees};

/// The places one run reads and writes. Built from a canonical home, so a
/// path built from it has no symbolic link in it unless one is on disk.
#[derive(Clone, Debug)]
pub struct Layout {
    pub home: PathBuf,
    /// `~/.openagents`
    pub openagents: PathBuf,
    /// The Coder task store.
    pub store: PathBuf,
}

impl Layout {
    /// The layout under `home`, with the task store at `store` (default
    /// `~/.openagents/tasks`; [`Layout::from_env`] reads [`task_store`]).
    ///
    /// # Errors
    /// The home cannot be resolved.
    pub fn new(home: &Path, store: Option<PathBuf>) -> std::io::Result<Self> {
        let home = home.canonicalize()?;
        let openagents = home.join(".openagents");
        let store = match store {
            Some(store) => store.canonicalize().unwrap_or(store),
            None => openagents.join("tasks"),
        };
        Ok(Self {
            home,
            openagents,
            store,
        })
    }

    /// This user's layout, from `HOME`, with the task store Coder uses
    /// ([`task_store`]: `$OPENAGENTS_TASKS`, else `~/.openagents/tasks`).
    ///
    /// # Errors
    /// `HOME` is unset, relative, or unreadable.
    pub fn from_env() -> std::io::Result<Self> {
        let home = user_home::home_os()
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
            .ok_or_else(|| std::io::Error::other("HOME must be an absolute path"))?;
        let store = task_store(&home);
        Self::new(&home, Some(store))
    }

    #[must_use]
    pub fn background(&self) -> PathBuf {
        self.openagents.join("background")
    }
    #[must_use]
    pub fn rules(&self) -> PathBuf {
        self.background().join("rules")
    }
    #[must_use]
    pub fn runs(&self) -> PathBuf {
        self.background().join("runs.jsonl")
    }
    #[must_use]
    pub fn state(&self) -> PathBuf {
        self.background().join("state.json")
    }
    #[must_use]
    pub fn sizes(&self) -> PathBuf {
        self.background().join("sizes.json")
    }
    #[must_use]
    pub fn trash(&self) -> PathBuf {
        self.background().join("trash")
    }
    /// Rules compiled in conversation and not yet confirmed, one per
    /// thread (or per `add`), `drafts/<id>.json`.
    #[must_use]
    pub fn drafts(&self) -> PathBuf {
        self.background().join("drafts")
    }
    /// Folders Jev judged, and what the person said (phase 3).
    #[must_use]
    pub fn proposals(&self) -> PathBuf {
        self.background().join("proposals.json")
    }
    /// What the flake watch remembers.
    #[must_use]
    pub fn flakes(&self) -> PathBuf {
        self.background().join("flakes.json")
    }
    /// What the file trigger last saw of its watched paths.
    #[must_use]
    pub fn watched(&self) -> PathBuf {
        self.background().join("watched.json")
    }
    /// Held by the one runner on this computer for its lifetime.
    #[must_use]
    pub fn runner_lock(&self) -> PathBuf {
        self.background().join("runner.lock")
    }
    /// What the runner holding that lock last said about itself
    /// (#10349).
    #[must_use]
    pub fn runner_presence(&self) -> PathBuf {
        self.background().join("runner.json")
    }
    /// Held by whoever is deleting, so two runs never overlap.
    #[must_use]
    pub fn run_lock(&self) -> PathBuf {
        self.background().join("run.lock")
    }
    /// Plugins installed on this computer.
    #[must_use]
    pub fn extensions(&self) -> PathBuf {
        self.openagents.join("extensions")
    }
    /// Which installed plugins are on.
    #[must_use]
    pub fn enabled_plugins(&self) -> PathBuf {
        self.extensions().join("enabled.json")
    }
    /// Coder's target slots, beside the task store ([`task_targets`]).
    #[must_use]
    pub fn targets(&self) -> PathBuf {
        task_targets(&self.store)
    }
    /// Coder's task worktrees, beside the task store ([`task_worktrees`]).
    #[must_use]
    pub fn worktrees(&self) -> PathBuf {
        task_worktrees(&self.store)
    }
    #[must_use]
    pub fn coder_one_target(&self) -> PathBuf {
        self.openagents.join("coder-one/target")
    }
    #[must_use]
    pub fn gate(&self) -> PathBuf {
        self.openagents.join("gate")
    }
    /// Agent scratch, one directory a session (`coder_lease::scratch`).
    #[must_use]
    pub fn scratch(&self) -> PathBuf {
        self.openagents.join("scratch")
    }
    /// The machine's lease table (`coder_lease`).
    #[must_use]
    pub fn leases(&self) -> PathBuf {
        self.openagents.join("leases")
    }

    /// `~/work/NAME`, made: an agent target directory in tests.
    #[cfg(test)]
    pub(crate) fn agent_dir(&self, name: &str) -> PathBuf {
        let path = self.home.join("work").join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}

/// Writes `bytes` to `path` through a temporary file beside it, so a reader
/// never sees half a file.
///
/// # Errors
/// The folder or file cannot be written.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp-{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}
