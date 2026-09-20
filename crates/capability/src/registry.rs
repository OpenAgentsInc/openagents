//! The directories manifests come from, and the registry that reads them
//! without running them.
//!
//! A [`SourceDir`] records provenance — which kind of directory a manifest
//! was found in — so a trace can say where a manifest came from.
//! Provenance is a label, not a permission: no directory confers the
//! right to run a probe. That decision is [`Trust`](crate::Trust)'s alone.

use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::probe::{Found, PROBE_WALL, Presence};
use crate::trust::{Decision, Proof, Trust};
use crate::{DIR_ENV, Manifest, PATH_ENV};

/// A manifest's provenance — which kind of directory it came from.
///
/// The label is for the record: a trace says whether a manifest was
/// read from the operator's directory or out of a checkout. It grants
/// nothing — a manifest in an operator-owned directory still probes only
/// under an approval record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A directory the operator named: `CODER_CAPABILITY_DIR` or
    /// `~/.openagents/capabilities`.
    Operator,
    /// A repository checkout — untrusted input the host reads.
    Repository,
}

impl Source {
    /// The word a trace or a check records.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Source::Operator => "operator",
            Source::Repository => "repository",
        }
    }
}

/// A directory of manifests, with its provenance attached.
#[derive(Clone, Debug)]
pub struct SourceDir {
    /// The directory itself.
    pub path: PathBuf,
    /// Who owns it — provenance, not permission.
    pub source: Source,
}

impl SourceDir {
    /// A directory the operator named.
    #[must_use]
    pub fn operator(path: impl Into<PathBuf>) -> Self {
        SourceDir {
            path: path.into(),
            source: Source::Operator,
        }
    }

    /// A directory inside a repository checkout.
    #[must_use]
    pub fn repository(path: impl Into<PathBuf>) -> Self {
        SourceDir {
            path: path.into(),
            source: Source::Repository,
        }
    }
}

/// The directories a survey or a preflight searches, in precedence order:
/// the operator's named directory first, then the repository's own
/// `capabilities/`, then the operator's home registry. The first
/// definition of a slug wins, so an operator can stand a manifest in
/// front of a repository's without editing it. Order decides which
/// manifest a slug names; it never decides whether it may run.
#[must_use]
pub fn search(repository: Option<&Path>) -> Vec<SourceDir> {
    let mut dirs = Vec::new();
    if let Some(dir) = env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        dirs.push(SourceDir::operator(dir));
    }
    if let Some(root) = repository {
        dirs.push(SourceDir::repository(root.join("capabilities")));
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(SourceDir::operator(
            PathBuf::from(home).join(".openagents").join("capabilities"),
        ));
    }
    dirs
}

/// A manifest as read: the body, the file it came from, the digest that
/// identifies it, and the provenance a record reports.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The manifest.
    pub manifest: Manifest,
    /// The file it was read from.
    pub path: PathBuf,
    /// The digest of the file's exact bytes — what an approval names, so
    /// a file that changes approves nothing.
    pub digest: String,
    /// Which kind of directory it came from.
    pub source: Source,
}

impl Entry {
    /// Reads a manifest file and computes its digest from the exact
    /// bytes — the digest names what was approved, not a re-serialized
    /// form of it.
    ///
    /// # Errors
    ///
    /// Returns the read or validation failure, with the path in the
    /// message.
    pub fn load(path: &Path, source: Source) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        manifest
            .validate()
            .map_err(|why| format!("{}: {why}", path.display()))?;
        Ok(Entry {
            manifest,
            path: path.to_path_buf(),
            digest: crate::trust::digest_bytes(&bytes),
            source,
        })
    }

    /// Probes this entry in `workspace` under `trust`: version argv
    /// first, workspace probe when the manifest declares one.
    ///
    /// The trust decision runs first. A manifest no approval names is
    /// `unprobed` — declared, never run. An approved one runs through
    /// the bounded supervisor under [`PROBE_WALL`], and the returned
    /// [`Found`] carries the [`Proof`] that let it run.
    #[must_use]
    pub fn probe(&self, workspace: &Path, trust: &Trust) -> Found {
        self.found(workspace, trust, PROBE_WALL, true)
    }

    /// The version check alone — what preflight asks, when the question
    /// is "is it installed" rather than "does it accept this workspace".
    #[must_use]
    pub fn detect(&self, workspace: &Path, trust: &Trust) -> Found {
        self.found(workspace, trust, PROBE_WALL, false)
    }

    /// Probes under a shorter wall clock — a test that wants a timeout
    /// without the wait.
    #[must_use]
    pub fn probe_within(&self, workspace: &Path, trust: &Trust, wall: Duration) -> Found {
        self.found(workspace, trust, wall, true)
    }

    fn found(&self, workspace: &Path, trust: &Trust, wall: Duration, ask: bool) -> Found {
        let started = Instant::now();
        // A relay manifest runs nothing here, so there is nothing for this
        // host to approve: the worker holds the approval, and the host's
        // relay door is what probes it. The registry records that it was
        // declared and leaves the answer to the host.
        if self.manifest.transport == crate::manifest::RELAY {
            return Found {
                manifest: self.manifest.clone(),
                presence: Presence::Unprobed {
                    reason: "a relay capability is probed by the host's relay door, not an argv"
                        .to_string(),
                },
                workspace: workspace.to_path_buf(),
                milliseconds: started.elapsed().as_millis() as u64,
                proof: Proof::None,
                source: self.source,
                digest: self.digest.clone(),
                path: self.path.clone(),
            };
        }
        let (proof, presence) = match trust.decide(self, workspace) {
            Decision::Unapproved(reason) => (Proof::None, Presence::Unprobed { reason }),
            Decision::Approved(proof) if self.manifest.transport != crate::manifest::SUBPROCESS => {
                (
                    proof,
                    Presence::Unprobed {
                        reason: format!(
                            "transport {} is not an argv this host runs",
                            self.manifest.transport
                        ),
                    },
                )
            }
            Decision::Approved(proof) => {
                let presence = self.manifest.presence(workspace, wall, ask);
                let presence =
                    crate::probe::check_executor_state(&self.manifest.slug, &proof, presence);
                (proof, presence)
            }
        };
        Found {
            manifest: self.manifest.clone(),
            presence,
            workspace: workspace.to_path_buf(),
            milliseconds: started.elapsed().as_millis() as u64,
            proof,
            source: self.source,
            digest: self.digest.clone(),
            path: self.path.clone(),
        }
    }
}

/// A directory of manifests, read.
///
/// Reading is inert: the registry parses and validates every manifest it
/// can read and refuses the ones it cannot, and nothing in it has run.
/// Probing is [`Entry::probe`] and [`Registry::probe_all`], which a
/// [`Trust`] gates.
#[derive(Debug)]
pub struct Registry {
    entries: Vec<Entry>,
    refused: Vec<(String, String)>,
}

impl Registry {
    /// Reads every manifest in every directory. A directory that does not
    /// exist is simply empty; a malformed manifest is recorded as refused
    /// with its reason, and the first definition of a slug wins.
    #[must_use]
    pub fn open(dirs: &[SourceDir]) -> Self {
        let mut entries = Vec::new();
        let mut refused = Vec::new();
        let mut seen = BTreeSet::new();
        for dir in dirs {
            let Some(listing) = read_dir(&dir.path) else {
                continue;
            };
            for path in listing {
                match Entry::load(&path, dir.source) {
                    Ok(entry) => {
                        if seen.insert(entry.manifest.slug.clone()) {
                            entries.push(entry);
                        }
                    }
                    Err(why) => refused.push((path.display().to_string(), why)),
                }
            }
        }
        Registry { entries, refused }
    }

    /// Reads one directory — a test that wants to refuse or load one
    /// registry at a time.
    ///
    /// # Errors
    ///
    /// Returns why the directory could not be read.
    pub fn read(dir: &SourceDir) -> Result<Self, String> {
        let mut registry = Registry {
            entries: Vec::new(),
            refused: Vec::new(),
        };
        for path in
            read_dir(&dir.path).ok_or_else(|| format!("{}: not a directory", dir.path.display()))?
        {
            match Entry::load(&path, dir.source) {
                Ok(entry) => registry.entries.push(entry),
                Err(why) => registry.refused.push((path.display().to_string(), why)),
            }
        }
        Ok(registry)
    }

    /// The manifests, as loaded.
    pub fn manifests(&self) -> impl Iterator<Item = &Manifest> {
        self.entries.iter().map(|entry| &entry.manifest)
    }

    /// The entries, with their provenance.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// A manifest by slug.
    #[must_use]
    pub fn get(&self, slug: &str) -> Option<&Manifest> {
        self.entries
            .iter()
            .find(|entry| entry.manifest.slug == slug)
            .map(|entry| &entry.manifest)
    }

    /// An entry by slug — the manifest with its provenance and digest.
    #[must_use]
    pub fn entry(&self, slug: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.manifest.slug == slug)
    }

    /// The manifests that failed to load, with why.
    #[must_use]
    pub fn refused(&self) -> &[(String, String)] {
        &self.refused
    }

    /// Probes each entry under `trust`, in registry order.
    #[must_use]
    pub fn probe_all(&self, workspace: &Path, trust: &Trust) -> Vec<Found> {
        self.entries
            .iter()
            .map(|entry| entry.probe(workspace, trust))
            .collect()
    }
}

/// The `.json` files in `dir`, sorted. `None` when `dir` is not there.
fn read_dir(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(std::ffi::OsStr::new("json")))
        .collect();
    paths.sort();
    Some(paths)
}

/// The directories an executable is searched for in: `CODER_CAPABILITY_PATH`
/// first, then `PATH`, then the usual places a developer's tool lives.
#[must_use]
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(extra) = env::var_os(PATH_ENV).filter(|extra| !extra.is_empty()) {
        dirs.extend(env::split_paths(&extra));
    }
    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path));
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        for dir in [".local/bin", "bin", ".cargo/bin", ".bun/bin"] {
            dirs.push(PathBuf::from(&home).join(dir));
        }
    }
    for dir in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ] {
        dirs.push(PathBuf::from(dir));
    }
    dirs
}

/// Resolves `binary` against `dirs`, or takes it as a path when it
/// carries a separator.
///
/// The resolved path is what a probe — and later a delegation — runs;
/// nothing re-searches `PATH` afterwards.
#[must_use]
pub fn resolve(binary: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if binary.contains('/') || binary.contains('\\') {
        let path = PathBuf::from(binary);
        return executable(&path).then(|| absolute(&path));
    }
    for dir in dirs {
        let path = dir.join(binary);
        if executable(&path) {
            return Some(absolute(&path));
        }
    }
    None
}

/// A path that exists, is a file, and is executable by this user.
fn executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// A path made absolute without resolving symlinks — the probe records
/// what it resolved, not what it guesses.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
