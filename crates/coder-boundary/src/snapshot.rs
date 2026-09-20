//! Independent observation of what a run left behind.
//!
//! `git status` answers a different question than the one a boundary
//! needs answered. It compares the worktree to HEAD, so a file that was
//! already dirty stays dirty in its output while its contents change; it
//! sees nothing a delegate wrote outside the repository; and it is the
//! same tool the observation exists to check. So this module walks the
//! filesystem itself, before and after, and compares what it saw.
//!
//! A [`Snapshot`] records every entry under one root — files by length,
//! content digest, modification time, and mode; symlinks by their target
//! and never followed; directories and anything else by kind. The root
//! itself is recorded at the empty path, so a write that touches only
//! the root's metadata is still seen. On Unix the walk is
//! descriptor-relative: every entry is opened with `openat` relative to
//! its parent's descriptor under `O_NOFOLLOW`, directories are opened
//! with `O_DIRECTORY` before they are listed, and a file is stat'd again
//! after it is hashed — a link swapped into place mid-walk is a fault,
//! not a followed link. On a platform where that cannot be promised,
//! every observation is refused rather than taken unsafely.
//!
//! [`compare`] then reports a [`Verdict`]: [`Verdict::Changed`] with a
//! deterministic list, [`Verdict::Clean`] only when two complete
//! observations agree, and [`Verdict::Unverifiable`] whenever either
//! observation is partial. An unreadable or over-bounded snapshot cannot
//! establish that nothing was written, so it never reads as clean.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How much one snapshot may hold: entries walked, and file bytes hashed.
///
/// Either bound reached is a [`Fault`], and a faulted snapshot cannot
/// establish that nothing changed — [`compare`] reports
/// [`Verdict::Unverifiable`] rather than letting a partial observation
/// read as clean.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The most entries the walk records.
    pub entries: usize,
    /// The most file bytes the walk hashes.
    pub bytes: u64,
}

impl Limits {
    /// Both bounds, stated.
    #[must_use]
    pub const fn bounded(entries: usize, bytes: u64) -> Self {
        Limits { entries, bytes }
    }
}

impl Default for Limits {
    /// A checkout-sized tree: two hundred thousand entries and four
    /// gibibytes of hashed content.
    fn default() -> Self {
        Limits::bounded(200_000, 4 * 1024 * 1024 * 1024)
    }
}

/// The identity that survives a rename: device and inode, on a platform
/// that has them.
type Id = (u64, u64);

/// One observed path: what it is, and enough of it to tell a change from
/// a rewrite.
#[derive(Clone, Debug)]
enum Entry {
    Directory {
        id: Option<Id>,
        mode: Option<u32>,
    },
    File {
        id: Option<Id>,
        length: u64,
        digest: [u8; 32],
        modified: Option<SystemTime>,
        mode: Option<u32>,
    },
    /// A symlink is recorded by its target and never followed — a link
    /// pointing outside the root is an observation of the link, not of
    /// whatever it names.
    Link {
        id: Option<Id>,
        target: OsString,
        mode: Option<u32>,
    },
    Other {
        id: Option<Id>,
        kind: &'static str,
        mode: Option<u32>,
    },
}

impl Entry {
    /// The kind word a retype reports.
    fn kind(&self) -> &'static str {
        match self {
            Entry::Directory { .. } => "directory",
            Entry::File { .. } => "file",
            Entry::Link { .. } => "symlink",
            Entry::Other { kind, .. } => kind,
        }
    }

    /// The identity a rename keeps, where the platform provides one.
    fn id(&self) -> Option<Id> {
        match self {
            Entry::Directory { id, .. }
            | Entry::File { id, .. }
            | Entry::Link { id, .. }
            | Entry::Other { id, .. } => *id,
        }
    }

    /// Whether two observations agree in identity, contents, target,
    /// modification time, and mode.
    fn same(&self, other: &Entry) -> bool {
        if self.id() != other.id() {
            return false;
        }
        match (self, other) {
            (Entry::Directory { mode: a, .. }, Entry::Directory { mode: b, .. }) => a == b,
            (
                Entry::File {
                    length: al,
                    digest: ad,
                    modified: am,
                    mode: amode,
                    ..
                },
                Entry::File {
                    length: bl,
                    digest: bd,
                    modified: bm,
                    mode: bmode,
                    ..
                },
            ) => al == bl && ad == bd && am == bm && amode == bmode,
            (
                Entry::Link {
                    target: a,
                    mode: am,
                    ..
                },
                Entry::Link {
                    target: b,
                    mode: bm,
                    ..
                },
            ) => a == b && am == bm,
            (
                Entry::Other {
                    kind: a, mode: am, ..
                },
                Entry::Other {
                    kind: b, mode: bm, ..
                },
            ) => a == b && am == bm,
            _ => false,
        }
    }

    /// Whether a removed entry and a created one are the same thing under
    /// a new path. The inode says so where the platform has one; without
    /// it, identical content is the weaker evidence that remains, and a
    /// directory — which carries no content — never pairs that way.
    fn pairs(&self, other: &Entry) -> bool {
        match (self.id(), other.id()) {
            (Some(a), Some(b)) => a == b,
            _ => {
                !matches!(self, Entry::Directory { .. })
                    && self.kind() == other.kind()
                    && self.same(other)
            }
        }
    }
}

/// What stopped a snapshot from being complete.
#[derive(Clone, Debug)]
pub enum Fault {
    /// The root could not be resolved, or is not a directory.
    Root { path: PathBuf, error: String },
    /// A path inside the root could not be read.
    Read { path: PathBuf, error: String },
    /// The entry bound was reached.
    Entries { limit: usize },
    /// The hashed-bytes bound was reached.
    Bytes { limit: u64 },
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Root { path, error } => {
                write!(f, "cannot observe {}: {error}", path.display())
            }
            Fault::Read { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            Fault::Entries { limit } => {
                write!(f, "the {limit}-entry bound was reached")
            }
            Fault::Bytes { limit } => {
                write!(f, "the {limit}-byte hashing bound was reached")
            }
        }
    }
}

/// One directory tree as it was observed: every entry under one
/// canonical root, or the faults that kept the observation partial.
#[derive(Debug)]
pub struct Snapshot {
    root: PathBuf,
    entries: BTreeMap<PathBuf, Entry>,
    faults: Vec<Fault>,
}

impl Snapshot {
    /// Observes a tree, bounded by [`Limits::default`].
    pub fn observe(root: &Path) -> Snapshot {
        Self::observe_within(root, Limits::default())
    }

    /// Observes a tree under stated bounds. The result is complete only
    /// if the whole tree fit inside them.
    pub fn observe_within(root: &Path, limits: Limits) -> Snapshot {
        walk(root, limits)
    }

    /// The canonical root the snapshot was taken of.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many entries the walk recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the root held no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether the observation is whole. Only a complete snapshot may
    /// ground a [`Verdict::Clean`].
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.faults.is_empty()
    }

    /// The faults the walk hit, in the order it hit them.
    #[must_use]
    pub fn faults(&self) -> &[Fault] {
        &self.faults
    }

    /// A digest over every recorded entry and every fault, for a record
    /// that wants to name the observation rather than repeat it.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut sha = Sha256::new();
        for (path, entry) in &self.entries {
            sha.update(path.as_os_str().as_encoded_bytes());
            sha.update([0]);
            match entry {
                Entry::Directory { mode, .. } => {
                    sha.update(b"directory");
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
                Entry::File {
                    length,
                    digest,
                    mode,
                    ..
                } => {
                    sha.update(b"file");
                    sha.update(length.to_le_bytes());
                    sha.update(digest);
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
                Entry::Link { target, mode, .. } => {
                    sha.update(b"symlink");
                    sha.update(target.as_encoded_bytes());
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
                Entry::Other { kind, mode, .. } => {
                    sha.update(kind.as_bytes());
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
            }
            sha.update([0xff]);
        }
        for fault in &self.faults {
            sha.update(fault.to_string().as_bytes());
            sha.update([0xff]);
        }
        format!("{:x}", sha.finalize())
    }
}

/// What changed between two observations of the same root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum Change {
    /// The path did not exist before.
    Created { path: PathBuf },
    /// The path does not exist after.
    Removed { path: PathBuf },
    /// The path exists in both, changed: contents, target, modification
    /// time, or mode.
    Modified { path: PathBuf },
    /// The same entry at a new path — an inode match where the platform
    /// has inodes — with `altered` when the move also changed it.
    Renamed {
        from: PathBuf,
        to: PathBuf,
        altered: bool,
    },
    /// The path exists in both as a different kind of thing.
    Retyped { path: PathBuf },
}

impl Change {
    /// A deterministic ordering key: path first, then kind of change.
    fn sort_key(&self) -> (Vec<u8>, u8) {
        let (path, rank) = match self {
            Change::Created { path } => (path, 0),
            Change::Removed { path } => (path, 1),
            Change::Modified { path } => (path, 2),
            Change::Retyped { path } => (path, 3),
            Change::Renamed { from, .. } => (from, 4),
        };
        (path.as_os_str().as_encoded_bytes().to_vec(), rank)
    }
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Change::Created { path } => write!(f, "created {}", path.display()),
            Change::Removed { path } => write!(f, "removed {}", path.display()),
            Change::Modified { path } => write!(f, "modified {}", path.display()),
            Change::Retyped { path } => write!(f, "retyped {}", path.display()),
            Change::Renamed { from, to, altered } => match altered {
                true => write!(f, "renamed {} to {}, altered", from.display(), to.display()),
                false => write!(f, "renamed {} to {}", from.display(), to.display()),
            },
        }
    }
}

/// What the comparison of two snapshots establishes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Two complete observations agree: nothing was created, removed,
    /// renamed, retyped, or modified.
    Clean,
    /// What changed, deterministically ordered.
    Changed(Vec<Change>),
    /// The comparison cannot establish what changed: a root mismatch, or
    /// a snapshot that was unreadable or over-bounded. An incomplete
    /// observation never reads as clean.
    Unverifiable(String),
}

impl Verdict {
    /// Whether nothing changed.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        matches!(self, Verdict::Clean)
    }

    /// Whether the comparison could not establish an answer.
    #[must_use]
    pub fn is_unverifiable(&self) -> bool {
        matches!(self, Verdict::Unverifiable(_))
    }

    /// The observed changes, empty for a clean or unverifiable verdict.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        match self {
            Verdict::Changed(changes) => changes,
            _ => &[],
        }
    }
}

/// What two observations of the same root establish together.
pub fn compare(before: &Snapshot, after: &Snapshot) -> Verdict {
    if before.root != after.root {
        return Verdict::Unverifiable("the two snapshots are not of the same root".to_string());
    }
    for (which, snapshot) in [("before", before), ("after", after)] {
        if let Some(fault) = snapshot.faults.first() {
            return Verdict::Unverifiable(format!("the {which} snapshot is incomplete: {fault}"));
        }
    }

    let mut changes = Vec::new();
    for (path, was) in &before.entries {
        if let Some(now) = after.entries.get(path) {
            if was.kind() != now.kind() {
                changes.push(Change::Retyped { path: path.clone() });
            } else if !was.same(now) {
                changes.push(Change::Modified { path: path.clone() });
            }
        }
    }

    // A removal and a creation of the same entry is a rename. Pair them
    // in sorted order so the answer does not depend on how the walk
    // happened to find things.
    let mut created: Vec<&PathBuf> = after
        .entries
        .keys()
        .filter(|path| !before.entries.contains_key(*path))
        .collect();
    for from in before
        .entries
        .keys()
        .filter(|path| !after.entries.contains_key(*path))
    {
        let was = &before.entries[from];
        let matched = created.iter().position(|to| was.pairs(&after.entries[*to]));
        match matched {
            Some(index) => {
                let to = created.swap_remove(index);
                changes.push(Change::Renamed {
                    from: from.clone(),
                    to: to.clone(),
                    altered: !was.same(&after.entries[to]),
                });
            }
            None => changes.push(Change::Removed { path: from.clone() }),
        }
    }
    changes.extend(
        created
            .into_iter()
            .map(|path| Change::Created { path: path.clone() }),
    );
    changes.sort_by_key(Change::sort_key);

    match changes.is_empty() {
        true => Verdict::Clean,
        false => Verdict::Changed(changes),
    }
}

/// The one walk behind [`Snapshot::observe_within`], on the platforms
/// where it can be done without ever following a link.
#[cfg(unix)]
mod observe;

#[cfg(unix)]
use observe::walk;

/// A platform without descriptor-relative no-follow opens cannot promise
/// that a link raced into place is never followed, so it observes
/// nothing rather than observe unsafely: every snapshot on one is
/// refused, and no observation there can read as clean.
#[cfg(not(unix))]
fn walk(root: &Path, _limits: Limits) -> Snapshot {
    Snapshot {
        root: root.to_path_buf(),
        entries: BTreeMap::new(),
        faults: vec![Fault::Root {
            path: root.to_path_buf(),
            error: "filesystem observation needs Unix no-follow semantics".to_string(),
        }],
    }
}
