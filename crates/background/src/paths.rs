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
    /// `~/.openagents/tasks`).
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

    /// This user's layout, from `HOME`.
    ///
    /// # Errors
    /// `HOME` is unset, relative, or unreadable.
    pub fn from_env() -> std::io::Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
            .ok_or_else(|| std::io::Error::other("HOME must be an absolute path"))?;
        Self::new(&home, None)
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
    #[must_use]
    pub fn targets(&self) -> PathBuf {
        self.openagents.join("targets")
    }
    #[must_use]
    pub fn worktrees(&self) -> PathBuf {
        self.openagents.join("worktrees")
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

    /// The rule's allow roots, expanded.
    #[must_use]
    pub fn allow(&self, rule: &Rule) -> Vec<PathBuf> {
        rule.safety
            .allow
            .iter()
            .map(|root| expand(root, &self.home))
            .collect()
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
