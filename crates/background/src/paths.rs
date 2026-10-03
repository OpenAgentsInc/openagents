//! Where everything lives, the built-in deny list, and the file-system
//! primitives every check shares: no symbolic links, no other volumes,
//! sizes as allocated blocks.

use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::rule::{Rule, expand};

/// Target slots per project that Coder keeps ([`crates/coder` targets]).
/// A slot numbered past this is left over and belongs to class 1.
pub const SLOTS: usize = 4;

/// Names another task store than `~/.openagents/tasks`; Coder reads the
/// same variable ([`crates/coder` `task::local::STORE_VAR`]).
pub const STORE_VAR: &str = "OPENAGENTS_TASKS";

/// The task store on this computer: `$OPENAGENTS_TASKS`, else
/// `HOME/.openagents/tasks`. Coder and the background runner both use it,
/// so they agree on where tasks, worktrees and target slots live.
#[must_use]
pub fn task_store(home: &Path) -> PathBuf {
    std::env::var_os(STORE_VAR)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| home.join(".openagents/tasks"), PathBuf::from)
}

/// The folder Coder makes task worktrees in: beside the task store.
#[must_use]
pub fn task_worktrees(store: &Path) -> PathBuf {
    store.parent().unwrap_or(store).join("worktrees")
}

/// The folder Coder keeps Cargo target slots in: beside the task store.
#[must_use]
pub fn task_targets(store: &Path) -> PathBuf {
    store.parent().unwrap_or(store).join("targets")
}

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
        let home = std::env::var_os("HOME")
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

    /// `~/work/NAME`, made: an agent target directory in tests.
    #[cfg(test)]
    pub(crate) fn agent_dir(&self, name: &str) -> PathBuf {
        let path = self.home.join("work").join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    /// Never deleted, whatever a rule says: the host's state, the task
    /// store, keys, the wallet, the background rules and log, other
    /// agents' homes, and documents folders; then the rule's own deny and
    /// report lists. An entry that holds the whole home (the operating
    /// system's temporary folders, when a test's home is inside them) is
    /// left out, since no candidate could be outside it.
    #[must_use]
    pub fn deny(&self, rule: &Rule) -> Vec<PathBuf> {
        let o = &self.openagents;
        let mut deny = vec![
            o.join("host"),
            o.join("dev-host"),
            o.join("bin"),
            o.join("coder-access"),
            o.join("coder-connect"),
            o.join("nostr"),
            o.join("nostr-secret"),
            o.join("decision.key"),
            o.join("delegate-key"),
            o.join("credentials.json"),
            o.join("bearer"),
            o.join("wallet"),
            o.join("pylon"),
            o.join("extensions"),
            self.store.clone(),
            self.rules(),
            self.runs(),
            self.state(),
            self.home.join(".claude"),
            self.home.join(".codex"),
            self.home.join("Documents"),
            self.home.join("Desktop"),
            self.home.join("Downloads"),
            self.home.join("Pictures"),
            self.home.join("Movies"),
            self.home.join("Music"),
            self.home.join("Library"),
        ];
        for entry in rule.safety.deny.iter().chain(&rule.safety.report) {
            deny.push(expand(entry, &self.home));
        }
        deny.retain(|path| !self.home.starts_with(path));
        deny
    }

    /// The rule's allow roots, expanded. A rule names Coder's folders as
    /// `~/.openagents/targets` and `~/.openagents/worktrees`; those roots
    /// mean wherever this layout's task store keeps them.
    #[must_use]
    pub fn allow(&self, rule: &Rule) -> Vec<PathBuf> {
        let targets = self.openagents.join("targets");
        let worktrees = self.openagents.join("worktrees");
        let mut roots = Vec::new();
        for root in rule
            .safety
            .allow
            .iter()
            .chain(rule.classes.judged.iter().map(|judged| &judged.path))
        {
            let root = expand(root, &self.home);
            let moved = if root == targets {
                Some(self.targets())
            } else if root == worktrees {
                Some(self.worktrees())
            } else {
                None
            };
            if let Some(moved) = moved.filter(|moved| *moved != root) {
                roots.push(moved);
            }
            roots.push(root);
        }
        roots
    }

    /// Why `path` may not be deleted under `rule`, or `None` when it may:
    /// it must be absolute and plain, under an allow root, neither under nor
    /// holding a denied path, and reached through no symbolic link.
    #[must_use]
    pub fn refuse(&self, rule: &Rule, path: &Path) -> Option<&'static str> {
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Some("not a plain absolute path");
        }
        if !self
            .allow(rule)
            .iter()
            .any(|root| path.starts_with(root) && path != self.home)
        {
            return Some("outside the allowed folders");
        }
        if self
            .deny(rule)
            .iter()
            .any(|denied| path.starts_with(denied) || denied.starts_with(path))
        {
            return Some("on the deny list");
        }
        // Checked before anything on disk is looked at: reading inside
        // one of these makes macOS ask the person about this program.
        if coder_boundary::privacy::protected(&self.home)
            .iter()
            .any(|protected| path.starts_with(protected) || protected.starts_with(path))
        {
            return Some("a macOS privacy-protected folder");
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => return Some("a symbolic link"),
            Ok(meta) if !meta.is_dir() => return Some("not a folder"),
            Ok(_) => {}
            Err(_) => return Some("gone"),
        }
        match path.canonicalize() {
            Ok(real) if real == path => {}
            _ => return Some("reached through a symbolic link"),
        }
        if mount_point(path) {
            return Some("another volume");
        }
        None
    }
}

/// A directory that is not a symbolic link.
#[must_use]
pub fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

/// Whether `path` is on a different device from its parent.
#[must_use]
pub fn mount_point(path: &Path) -> bool {
    let (Ok(meta), Some(parent)) = (std::fs::symlink_metadata(path), path.parent()) else {
        return false;
    };
    std::fs::symlink_metadata(parent).is_ok_and(|up| up.dev() != meta.dev())
}

/// Seconds since the epoch.
#[must_use]
pub fn unix(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The current time in seconds since the epoch.
#[must_use]
pub fn now() -> u64 {
    unix(SystemTime::now())
}

fn mtime(path: &Path) -> Option<u64> {
    std::fs::symlink_metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(unix)
}

/// When a Cargo target directory was last used: the newest modification
/// time of the directory, its slot lock, and each profile's directory,
/// `.cargo-lock`, `.fingerprint`, `deps`, and `build`.
#[must_use]
pub fn touched(path: &Path) -> u64 {
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let mut times = vec![mtime(path), mtime(Path::new(&lock))];
    for profile in ["debug", "release"] {
        let dir = path.join(profile);
        for part in [
            "",
            ".cargo-lock",
            ".fingerprint",
            "deps",
            "build",
            "incremental",
        ] {
            times.push(mtime(&if part.is_empty() {
                dir.clone()
            } else {
                dir.join(part)
            }));
        }
    }
    times.into_iter().flatten().max().unwrap_or(0)
}

/// When a worktree was last used: the newest of the directory, its `.git`
/// file, and its index.
#[must_use]
pub fn touched_worktree(path: &Path) -> u64 {
    [mtime(path), mtime(&path.join(".git"))]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0)
}

/// What a walk found under a folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Measure {
    /// Allocated bytes (`st_blocks × 512`).
    pub bytes: u64,
    /// A folder inside sits on another volume.
    pub foreign: bool,
}

/// Walk `path` without following symbolic links, adding allocated blocks
/// and noting any folder on another device. A folder macOS guards with a
/// privacy prompt for `home` ([`coder_boundary::privacy`]) is never
/// entered; measuring one at the top is refused.
///
/// # Errors
/// `path` cannot be read, or is privacy-protected.
pub fn measure(path: &Path, home: &Path) -> std::io::Result<Measure> {
    walk(path, &|inside| {
        coder_boundary::privacy::is_protected(inside, home)
    })
}

/// [`measure`] for a folder whose size is only reported, never cleaned
/// (`~/Library/Caches`, `/private/var/folders`): it also leaves out every
/// folder named for one of Apple's own programs
/// ([`coder_boundary::privacy::private_cache`]), whose caches may hold an
/// app's private data behind a privacy prompt.
///
/// # Errors
/// As [`measure`].
pub fn measure_report(path: &Path, home: &Path) -> std::io::Result<Measure> {
    walk(path, &|inside| {
        coder_boundary::privacy::private_cache(inside, home)
    })
}

fn walk(path: &Path, skip: &dyn Fn(&Path) -> bool) -> std::io::Result<Measure> {
    if skip(path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "a macOS privacy-protected folder",
        ));
    }
    let top = std::fs::symlink_metadata(path)?;
    let device = top.dev();
    let mut out = Measure {
        bytes: top.blocks() * 512,
        foreign: false,
    };
    if !top.is_dir() {
        return Ok(out);
    }
    // Cargo hard-links outputs (`debug/foo` and `debug/deps/foo-…`):
    // count each linked file once, as deleting the folder frees it once.
    let mut linked = std::collections::HashSet::new();
    let mut stack = vec![path.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if skip(&entry.path()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_dir() && meta.nlink() > 1 && !linked.insert((meta.dev(), meta.ino())) {
                continue;
            }
            out.bytes = out.bytes.saturating_add(meta.blocks() * 512);
            if meta.is_dir() {
                if meta.dev() == device {
                    stack.push(entry.path());
                } else {
                    out.foreign = true;
                }
            }
        }
    }
    Ok(out)
}

/// A path shown to a person: `~/…` under the home.
#[must_use]
pub fn show(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Bytes as a person reads them: whole gigabytes from 1 GB, megabytes
/// below.
#[must_use]
pub fn bytes(n: u64) -> String {
    const GB: u64 = crate::rule::GB;
    if n >= GB {
        format!("{} GB", (n + GB / 2) / GB)
    } else {
        format!("{} MB", (n + 500_000) / 1_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coders_folders_sit_beside_any_task_store_and_rules_follow_them() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("elsewhere/tasks");
        std::fs::create_dir_all(&store).unwrap();
        let layout = Layout::new(dir.path(), Some(store.clone())).unwrap();
        let beside = store.canonicalize().unwrap().parent().unwrap().to_owned();
        assert_eq!(layout.worktrees(), beside.join("worktrees"));
        assert_eq!(layout.targets(), beside.join("targets"));
        assert_eq!(layout.worktrees(), task_worktrees(&layout.store));
        assert_eq!(layout.targets(), task_targets(&layout.store));
        // The default rules name `~/.openagents/{targets,worktrees}`; they
        // mean the store's folders, which may now be cleaned.
        let rule = crate::rule::disk();
        let slot = layout.targets().join("p-slot-9");
        let tree = layout.worktrees().join("t");
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::create_dir_all(&tree).unwrap();
        assert_eq!(layout.refuse(&rule, &slot), None);
        assert_eq!(layout.refuse(&rule, &tree), None);
        // The task store itself stays denied wherever it is.
        assert_eq!(
            layout.refuse(&rule, &layout.store),
            Some("outside the allowed folders")
        );
        // The default layout keeps them under `~/.openagents`.
        let home = Layout::new(dir.path(), None).unwrap();
        assert_eq!(home.worktrees(), home.openagents.join("worktrees"));
        assert_eq!(home.targets(), home.openagents.join("targets"));
    }

    #[test]
    fn deny_wins_and_symlinks_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        let mut rule = crate::rule::disk();
        let target = layout.targets().join("x");
        std::fs::create_dir_all(&target).unwrap();
        assert_eq!(layout.refuse(&rule, &target), None);
        rule.safety.deny.push("~/.openagents/targets".into());
        assert_eq!(layout.refuse(&rule, &target), Some("on the deny list"));
        let rule = crate::rule::disk();
        // A folder holding a denied path is refused too.
        assert_eq!(
            layout.refuse(&rule, &layout.openagents),
            Some("outside the allowed folders")
        );
        let link = layout.targets().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(layout.refuse(&rule, &link), Some("a symbolic link"));
        assert_eq!(
            layout.refuse(&rule, &layout.store),
            Some("outside the allowed folders")
        );
    }

    #[test]
    fn privacy_protected_folders_are_never_walked_or_cleaned() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let music = home.join("Music");
        std::fs::create_dir_all(&music).unwrap();
        std::fs::write(music.join("song"), vec![1u8; 1 << 20]).unwrap();
        let caches = home.join("Library/Caches");
        for name in ["com.apple.Music", "com.apple.dt.Xcode", "Homebrew"] {
            std::fs::create_dir_all(caches.join(name)).unwrap();
            std::fs::write(caches.join(name).join("blob"), vec![1u8; 1 << 20]).unwrap();
        }
        let protected = cfg!(target_os = "macos");
        // A walk of the home leaves Music out; measuring Music is refused.
        let whole = measure(&home, &home).unwrap().bytes;
        assert_eq!(whole < 3 << 20, protected, "{whole}");
        assert_eq!(measure(&music, &home).is_err(), protected);
        // A report-only walk of the caches counts only Homebrew; a
        // cleaning walk skips just the protected Music cache, not Xcode's.
        let reported = measure_report(&caches, &home).unwrap().bytes;
        assert_eq!(reported < 2 << 20, protected, "{reported}");
        let cleaning = measure(&caches, &home).unwrap().bytes;
        assert!(cleaning >= 2 << 20, "{cleaning}");
        assert_eq!(cleaning < 3 << 20, protected, "{cleaning}");
        // No rule may clean inside one, or clean a folder that holds one.
        let layout = Layout::new(&home, None).unwrap();
        let mut rule = crate::rule::disk();
        rule.safety.allow.push("~".into());
        rule.safety.allow.push("/Volumes".into());
        assert!(layout.refuse(&rule, &music).is_some());
        assert!(layout.refuse(&rule, &home.join("Library")).is_some());
        if protected {
            // Beyond the built-in deny list: a rule that allows a removable
            // volume still never reaches it.
            assert_eq!(
                layout.refuse(&rule, Path::new("/Volumes/USB")),
                Some("a macOS privacy-protected folder")
            );
            assert!(crate::rule::glob("~/Music/*", &home).is_empty());
            assert!(crate::rule::glob("~/Music", &home).is_empty());
            assert!(!crate::rule::glob("~/*", &home).contains(&music));
        }
    }

    #[test]
    fn measure_counts_blocks_and_never_follows_links() {
        let dir = tempfile::tempdir().unwrap();
        let inside = dir.path().join("in");
        let outside = dir.path().join("out");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("big"), vec![1u8; 1 << 20]).unwrap();
        std::os::unix::fs::symlink(&outside, inside.join("link")).unwrap();
        let measured = measure(&inside, dir.path()).unwrap();
        assert!(measured.bytes < 1 << 20);
        let once = measure(&outside, dir.path()).unwrap().bytes;
        assert!(once >= 1 << 20);
        // A hard link to the same file counts once.
        std::fs::hard_link(outside.join("big"), outside.join("again")).unwrap();
        assert!(measure(&outside, dir.path()).unwrap().bytes < once + 4096 * 4);
    }
}
