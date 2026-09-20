//! The host's answer to "may this manifest's argv run": a store the
//! operator owns, records that name exact bytes, and a decision that
//! re-checks the adapter before every probe.
//!
//! A [`Record`] pins three identities, because a probe runs the manifest's
//! words through the operator's permissions:
//!
//! - the manifest body's digest — approve a file, and a changed file
//!   approves nothing;
//! - the adapter's canonical path and content digest — approve `devin`,
//!   and a replaced binary approves nothing;
//! - every argv word that names a file — an interpreted script is the
//!   adapter as surely as the binary is. The pin keeps the word's lexical
//!   text, its canonical path, and its digest, and the decision re-resolves
//!   the word in the directory the argv would actually run in, so a
//!   retargeted symlink approves nothing.
//!
//! The store lives outside any checkout. A store inside the workspace it
//! would approve — or inside the repository the manifest came from — is
//! checkout data, and a record written by the thing it approves is no
//! approval at all. [`Trust::decide`] fails closed: a store that cannot be
//! resolved or read right now approves nothing, whatever an earlier load
//! saw, and a workspace or manifest that cannot be resolved is not
//! probed.
//!
//! What a pin covers, stated honestly: the manifest's bytes (every inline
//! `-c` script included), the adapter's bytes, and the argv words that
//! name files. What it does not cover: dependencies an adapter loads
//! dynamically that no argv names — a record approves what the manifest
//! says, not everything the binary might reach.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::manifest::Manifest;
use crate::registry::{Entry, Registry, resolve, search, search_dirs};
use crate::{STORE_ENV, Source, is_slug};

/// The file the store serializes to — `{"v": 1, "records": [...]}`.
const STORE_VERSION: u32 = 1;

/// The store's default location, under the operator's home.
#[must_use]
pub fn store_path() -> PathBuf {
    if let Some(named) = std::env::var_os(STORE_ENV).filter(|named| !named.is_empty()) {
        return PathBuf::from(named);
    }
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(".openagents")
                .join("capability-trust.json")
        })
        .unwrap_or_else(|| PathBuf::from("capability-trust.json"))
}

/// The digest of exact bytes — of a manifest file, an adapter binary, a
/// script an argv names.
#[must_use]
pub fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(bytes))
}

/// The digest of a file's exact contents.
///
/// # Errors
///
/// Returns the read failure — a file that cannot be read cannot be
/// verified.
pub fn digest_file(path: &Path) -> Result<String, String> {
    std::fs::read(path)
        .map(|bytes| digest_bytes(&bytes))
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// One file the approval pins: the argv word that names it, the canonical
/// path that word resolved to, and the digest of what it held when the
/// operator approved it.
///
/// The word is kept because the path is not the identity: an argv runs the
/// word, resolved in whatever directory the probe runs in. The decision
/// re-resolves the word there and compares — a symlink retargeted under an
/// unchanged manifest approves nothing.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Pinned {
    /// The argv word as the manifest spells it.
    #[serde(default)]
    pub word: String,
    /// The canonical path the word resolved to at approval.
    pub path: PathBuf,
    /// The digest of its contents at approval.
    pub digest: String,
}

/// One approval: the operator's word that this exact manifest, driving
/// this exact adapter, may run probes.
///
/// A record names content, not location: the manifest by digest, the
/// adapter by canonical path plus content digest, and each
/// repo-controlled file an argv points at by path plus digest. Copied
/// into a repository it authorizes nothing, because the digest still has
/// to match the bytes that are actually there.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Record {
    /// The slug the approval names.
    pub slug: String,
    /// The digest of the manifest file's exact bytes.
    pub manifest: String,
    /// The canonical path the manifest's `detect.binary` resolved to.
    pub adapter: PathBuf,
    /// The digest of that file's contents at approval.
    pub adapter_digest: String,
    /// Repo-controlled files the manifest's argvs interpret — a probe
    /// script is part of the adapter, so it is pinned the same way.
    #[serde(default)]
    pub pinned: Vec<Pinned>,
    /// Adapter state the operator grants a later filesystem boundary —
    /// the directories the adapter legitimately writes. An approval
    /// without them grants none.
    #[serde(default)]
    pub writable: Vec<PathBuf>,
}

/// The proof a probe carries: which host decision let it run.
///
/// Approval is permission to *probe*, nothing more. A manifest's
/// `enforces` list stays a claim — the answer to whether a bound is held
/// comes from the host's own enforcement, not from the approved file.
#[derive(Clone, Debug)]
pub enum Proof {
    /// No host decision approves this manifest.
    None,
    /// The operator's store holds a record naming this manifest's
    /// digest, and the adapter it pins still matches what resolves.
    Approved {
        /// The digest of the manifest body the record names.
        digest: String,
        /// The adapter state the record grants.
        writable: Vec<PathBuf>,
        /// The store the record lives in — the proof's source.
        store: PathBuf,
    },
    /// A trust that approves anything — for a caller that built the
    /// manifest itself, and for tests. The trace says so plainly.
    Unconditional,
}

impl Proof {
    /// The word a trace records.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Proof::None => "none",
            Proof::Approved { .. } => "approved",
            Proof::Unconditional => "unconditional",
        }
    }

    /// The adapter state the approval grants, when a boundary asks.
    #[must_use]
    pub fn writable(&self) -> &[PathBuf] {
        match self {
            Proof::Approved { writable, .. } => writable,
            _ => &[],
        }
    }
}

/// The trust decision: a probe may run, or it may not and the reason is
/// the operator's to act on.
#[derive(Clone, Debug)]
pub enum Decision {
    /// The probe may run; the proof says on whose word.
    Approved(Proof),
    /// The probe may not run; the string says why, in words an operator
    /// can act on.
    Unapproved(String),
}

/// The same decision, carrying the record the checks ran against.
///
/// [`Trust::decide`] answers for a caller that records the proof;
/// [`Trust::decide_verified`] answers for a caller that acts on what was
/// checked — a filesystem boundary built from an approval grants the
/// writable paths and seals the pinned files of this record, taken from
/// the store as it was at this decision rather than a later load that
/// could name a different one.
#[derive(Clone, Debug)]
pub enum Verified {
    /// A stored record approved the manifest — the record whose adapter,
    /// pins, and grants the checks verified.
    Approved(Record),
    /// A trust that approves everything; no record exists to carry.
    Unconditional,
    /// The manifest may not run; the string says why, in words an
    /// operator can act on.
    Unapproved(String),
}

/// The operator's approvals, loaded from the store.
///
/// `Trust` answers one question — may this manifest's argv run — and the
/// answer is never taken from where the manifest sits. A manifest in an
/// operator-named directory is still probed only under a record; the
/// directory names search order, not permission.
#[derive(Clone, Debug)]
pub struct Trust {
    store: PathBuf,
    records: Vec<Record>,
    /// Whether `store` is a real file the decision re-reads. A stored
    /// trust's cached records are for listing; [`Trust::decide`] re-loads
    /// the file so a deleted, replaced, or revoked store approves nothing.
    stored: bool,
    unconditional: bool,
    error: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct Stored {
    v: u32,
    #[serde(default)]
    records: Vec<Record>,
}

impl Trust {
    /// The operator's trust: the store at [`store_path`], loaded. A store
    /// that exists but will not parse fails closed — every decision is
    /// unapproved and the reason says the store could not be read.
    #[must_use]
    pub fn operator() -> Self {
        let store = store_path();
        Self::load(&store).unwrap_or_else(|error| Trust {
            store,
            records: Vec::new(),
            stored: true,
            unconditional: false,
            error: Some(error),
        })
    }

    /// A trust over a named store file.
    ///
    /// # Errors
    ///
    /// Returns why the store could not be read. A missing store is an
    /// empty trust, not an error.
    pub fn load(store: &Path) -> Result<Self, String> {
        let records = match std::fs::read_to_string(store) {
            Ok(text) => {
                let stored: Stored = serde_json::from_str(&text)
                    .map_err(|error| format!("{}: {error}", store.display()))?;
                if stored.v != STORE_VERSION {
                    return Err(format!(
                        "{}: store version is {}, this version reads {STORE_VERSION}",
                        store.display(),
                        stored.v
                    ));
                }
                stored.records
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("{}: {error}", store.display())),
        };
        Ok(Trust {
            store: store.to_path_buf(),
            records,
            stored: true,
            unconditional: false,
            error: None,
        })
    }

    /// A trust that approves nothing — every manifest is unprobed.
    ///
    /// It is not a store: it names no file and reads none, so nothing an
    /// untrusted directory contains can become its policy.
    #[must_use]
    pub fn empty() -> Self {
        Trust {
            store: store_path(),
            records: Vec::new(),
            stored: false,
            unconditional: false,
            error: None,
        }
    }

    /// A trust that approves everything, for tests. A survey under it
    /// probes any manifest, and the proof says `unconditional` rather
    /// than pretending a decision happened. Nothing reads it from a
    /// store, so no file can become it.
    #[must_use]
    pub fn everything() -> Self {
        Trust {
            store: store_path(),
            records: Vec::new(),
            stored: false,
            unconditional: true,
            error: None,
        }
    }

    /// The store this trust reads and writes.
    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    /// The records, as loaded.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Why the store could not be read, when it could not.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether `entry`'s manifest may run its probe in `workspace`.
    ///
    /// This is [`Trust::decide_verified`] reduced to the [`Proof`] a
    /// caller records: the checks are the same, and the record they ran
    /// against becomes the proof's digest, grants, and store.
    #[must_use]
    pub fn decide(&self, entry: &Entry, workspace: &Path) -> Decision {
        match self.decide_verified(entry, workspace) {
            Verified::Approved(record) => Decision::Approved(Proof::Approved {
                digest: record.manifest.clone(),
                writable: record.writable.clone(),
                store: self.store.clone(),
            }),
            Verified::Unconditional => Decision::Approved(Proof::Unconditional),
            Verified::Unapproved(why) => Decision::Unapproved(why),
        }
    }

    /// Whether `entry`'s manifest may run its probe in `workspace`,
    /// carrying the record the checks ran against.
    ///
    /// [`Trust::decide`] answers the question for a caller that records
    /// the proof; this form is for a caller that *acts* on what was
    /// checked. A filesystem boundary built from an approval grants the
    /// writable paths and seals the pinned files of the record this
    /// decision verified — taken from the store as it was at this check,
    /// not a later load that could name a different record.
    ///
    /// The order is the contract, and every step fails closed:
    ///
    /// - a stored trust re-reads its file — a store deleted, replaced, or
    ///   unreadable since the records were cached approves nothing;
    /// - a store inside the workspace, or inside the repository the
    ///   manifest came from, is checkout data — a manifest cannot approve
    ///   itself, and a worktree elsewhere does not launder it;
    /// - an unrecorded manifest is unapproved, and the reason names the
    ///   approval path;
    /// - a recorded manifest whose adapter resolves to a different path,
    ///   whose bytes changed, or whose argv names a file that re-resolves
    ///   differently in `workspace` is unapproved again.
    #[must_use]
    pub fn decide_verified(&self, entry: &Entry, workspace: &Path) -> Verified {
        if self.unconditional {
            return Verified::Unconditional;
        }
        if let Some(error) = &self.error {
            return Verified::Unapproved(format!(
                "the trust store could not be read ({error}), so nothing it names may run"
            ));
        }
        // A stored trust's records are the file's, read now — a deleted
        // or replaced store revokes what an earlier load saw.
        let records = if self.stored {
            match Self::load(&self.store) {
                Ok(fresh) => fresh.records,
                Err(error) => {
                    return Verified::Unapproved(format!(
                        "the trust store could not be read ({error}), so nothing it names may run"
                    ));
                }
            }
        } else {
            self.records.clone()
        };
        let Some(record) = records
            .iter()
            .find(|record| record.manifest == entry.digest)
        else {
            return Verified::Unapproved(format!(
                "no approval names this exact manifest — \
                 `capability-trust approve {}` records one in {}",
                entry.manifest.slug,
                self.store.display()
            ));
        };
        // A record matches — now where the store and the workspace sit
        // decides whether it is the operator's word or the checkout's.
        // Both checks need real paths, and a path that cannot be resolved
        // cannot approve.
        let work = match workspace.canonicalize() {
            Ok(work) => work,
            Err(error) => {
                return Verified::Unapproved(format!(
                    "the workspace {} cannot be resolved ({error}), so nothing may run in it",
                    workspace.display()
                ));
            }
        };
        let store = match self.store.canonicalize() {
            Ok(store) => store,
            Err(error) => {
                return Verified::Unapproved(format!(
                    "the trust store at {} cannot be resolved ({error}), so the approval \
                     it names cannot be checked — nothing it names may run",
                    self.store.display()
                ));
            }
        };
        if store.starts_with(&work) {
            return Verified::Unapproved(format!(
                "the trust store at {} sits inside the workspace it would approve — \
                 set {STORE_ENV} to a path outside it",
                store.display()
            ));
        }
        // A repository manifest's store check is against its own
        // checkout: `<repo>/capabilities/<slug>.json` puts the store's
        // boundary at the repository root, and a manifest whose file
        // cannot be resolved cannot rule the store out of it.
        if entry.source == Source::Repository {
            // Resolve the source directory, not the manifest target: a
            // manifest symlink must not move the repository's boundary.
            let root = entry.path.canonicalize().ok().and_then(|_| {
                entry
                    .path
                    .parent()
                    .and_then(Path::parent)
                    .and_then(|root| root.canonicalize().ok())
            });
            match root {
                Some(root) if store.starts_with(&root) => {
                    return Verified::Unapproved(format!(
                        "the trust store at {} sits inside the repository the manifest came from — \
                         a manifest cannot approve itself",
                        store.display()
                    ));
                }
                None => {
                    return Verified::Unapproved(format!(
                        "the manifest at {} cannot be resolved, so the store's place \
                         against its repository cannot be checked — nothing it names may run",
                        entry.path.display()
                    ));
                }
                _ => {}
            }
        }
        if let Err(why) = self.verify_adapter(entry, record, &work) {
            return Verified::Unapproved(why);
        }
        Verified::Approved(record.clone())
    }

    /// The adapter half of the decision: the manifest's `detect.binary`
    /// must resolve to the pinned canonical path, the file must still
    /// hold the pinned bytes, and every argv word that names a file must
    /// still resolve — in `work`, the directory the argv runs in — to the
    /// pinned file holding the pinned bytes.
    fn verify_adapter(&self, entry: &Entry, record: &Record, work: &Path) -> Result<(), String> {
        let binary = &entry.manifest.detect.binary;
        let resolved = resolve(binary, &search_dirs()).ok_or_else(|| {
            format!(
                "the approved adapter {} is not findable — {binary} resolves to nothing now",
                record.adapter.display()
            )
        })?;
        let canonical = resolved
            .canonicalize()
            .map_err(|error| format!("{}: {error}", resolved.display()))?;
        if canonical != record.adapter {
            return Err(format!(
                "{binary} resolves to {}, which is not the approved {} — \
                 `capability-trust approve {}` renews it",
                canonical.display(),
                record.adapter.display(),
                entry.manifest.slug
            ));
        }
        let digest = digest_file(&canonical)?;
        if digest != record.adapter_digest {
            return Err(format!(
                "the approved adapter at {} changed on disk — \
                 `capability-trust approve {}` renews it",
                canonical.display(),
                entry.manifest.slug
            ));
        }
        for pinned in &record.pinned {
            // The word is what the argv runs; the path is what the word
            // meant at approval. Every lexical alias must remain pinned,
            // even when several aliases initially named the same file —
            // a record without its word cannot be re-resolved at all.
            if pinned.word.is_empty() {
                return Err(
                    "an argv approval lacks its original path; approve the manifest again"
                        .to_string(),
                );
            }
            let current = resolve_word(&pinned.word, work)?;
            if current != pinned.path {
                return Err(format!(
                    "argv {:?} resolves to {}, not the approved {} — \
                     `capability-trust approve {}` renews it",
                    pinned.word,
                    current.display(),
                    pinned.path.display(),
                    entry.manifest.slug
                ));
            }
            let digest = digest_file(&pinned.path).map_err(|error| {
                format!(
                    "the adapter file {} the approval pins could not be verified: {error}",
                    pinned.path.display()
                )
            })?;
            if digest != pinned.digest {
                return Err(format!(
                    "{} changed since the manifest was approved — \
                     `capability-trust approve {}` renews it",
                    pinned.path.display(),
                    entry.manifest.slug
                ));
            }
        }
        // Every argv word is re-resolved in `work`, the directory the argv
        // actually runs in. A word that resolves to a file the record does
        // not pin is a file nobody approved — whether it was created after
        // the approval, is relative and resolves differently here, or a
        // record was written before words were pinned.
        for word in argv_tails(&entry.manifest) {
            if word.is_empty() || word.starts_with('-') {
                continue;
            }
            let path = Path::new(word);
            let resolved = if path.is_absolute() {
                path.canonicalize().ok()
            } else {
                work.join(path).canonicalize().ok()
            };
            let Some(resolved) = resolved.filter(|path| path.is_file()) else {
                continue;
            };
            if !record.pinned.iter().any(|pinned| pinned.path == resolved) {
                return Err(format!(
                    "argv {word:?} resolves to {} in this workspace — a file the approval \
                     does not pin, so it cannot run under it; name it absolutely and \
                     `capability-trust approve {}` again",
                    resolved.display(),
                    entry.manifest.slug
                ));
            }
        }
        Ok(())
    }

    /// Adds or replaces a record — keyed on the manifest digest, so
    /// approving a changed manifest leaves the old approval behind.
    ///
    /// # Errors
    ///
    /// Returns why the store could not be written, or why a store that
    /// could not be read was not overwritten.
    pub fn record(&mut self, record: Record) -> Result<(), String> {
        if !is_slug(&record.slug) {
            return Err(format!("{:?} is not a capability slug", record.slug));
        }
        if let Some(error) = &self.error {
            return Err(format!(
                "the trust store could not be read ({error}), so it will not be overwritten"
            ));
        }
        // A stored trust re-reads before it writes, so records held here
        // do not clobber ones the store gained or lost since the load.
        if self.stored {
            self.records = Self::load(&self.store)?.records;
        }
        match self
            .records
            .iter()
            .position(|held| held.manifest == record.manifest)
        {
            Some(at) => self.records[at] = record,
            None => self.records.push(record),
        }
        self.save()
    }

    /// Removes every record naming `slug`.
    ///
    /// # Errors
    ///
    /// Returns why the store could not be written.
    pub fn revoke(&mut self, slug: &str) -> Result<usize, String> {
        if let Some(error) = &self.error {
            return Err(format!(
                "the trust store could not be read ({error}), so it will not be overwritten"
            ));
        }
        if self.stored {
            self.records = Self::load(&self.store)?.records;
        }
        let before = self.records.len();
        self.records.retain(|record| record.slug != slug);
        self.save()?;
        Ok(before - self.records.len())
    }

    /// Approves probing the capability `slug` names, at the manifest it
    /// resolves to right now, and writes the record to this store.
    ///
    /// The record pins three identities: the manifest body by digest, the
    /// adapter by canonical path and content digest, and every argv word
    /// that names a file by its lexical text, its canonical path, and its
    /// digest. A file that changes afterwards — or a link retargeted under
    /// an unchanged word — approves nothing; the approval names bytes,
    /// not the places they sit.
    ///
    /// `writable` lists adapter state a filesystem boundary may let the
    /// executor write — the operator's word that those paths belong to
    /// the adapter. Each must be absolute and must exist to be approved:
    /// the canonical path is what the record pins.
    ///
    /// # Errors
    ///
    /// Returns why the approval was not recorded: the slug is not a slug,
    /// no registry holds it, the adapter does not resolve, a `writable`
    /// path is relative or does not resolve, the trust store sits inside
    /// the repository it would approve, or the store could not be read
    /// or written.
    pub fn approve(
        &mut self,
        repository: Option<&Path>,
        slug: &str,
        writable: &[PathBuf],
    ) -> Result<Approval, String> {
        if self.unconditional {
            return Err("a trust that approves everything writes no records".to_string());
        }
        if !is_slug(slug) {
            return Err(format!("{slug:?} is not a capability slug"));
        }
        let dirs = search(repository);
        let registry = Registry::open(&dirs);
        let entry = registry.entry(slug).cloned().ok_or_else(|| {
            format!(
                "no {slug}.json under {}",
                dirs.iter()
                    .map(|dir| dir.path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        // The store names what may run; a store inside the repository is
        // the repository's own word for itself. The check needs real
        // paths — one that cannot be resolved cannot be checked, so it
        // cannot approve.
        if let Some(root) = repository {
            let store = canonical_or_parent(&self.store).map_err(|error| {
                format!(
                    "the trust store at {} cannot be resolved ({error}), so its place \
                     against the repository cannot be checked",
                    self.store.display()
                )
            })?;
            let root = root.canonicalize().map_err(|error| {
                format!(
                    "the repository {} cannot be resolved ({error}), so the store's place \
                     against it cannot be checked",
                    root.display()
                )
            })?;
            if store.starts_with(&root) {
                return Err(format!(
                    "the trust store at {} sits inside the repository it would approve — \
                     set {STORE_ENV} to a path outside it",
                    self.store.display()
                ));
            }
        }

        // The adapter is pinned by canonical path and content — what the
        // probe resolves is what the record names.
        let binary = &entry.manifest.detect.binary;
        let resolved = resolve(binary, &search_dirs())
            .ok_or_else(|| format!("no {binary} on this machine to approve"))?;
        let adapter = resolved
            .canonicalize()
            .map_err(|error| format!("{}: {error}", resolved.display()))?;
        let adapter_digest = digest_file(&adapter)?;

        let pinned = pinned_argvs(&entry.manifest, repository)?;

        let mut granted = Vec::with_capacity(writable.len());
        for dir in writable {
            if !dir.is_absolute() {
                return Err(format!(
                    "{} is not absolute — an approval names real paths",
                    dir.display()
                ));
            }
            granted.push(
                dir.canonicalize()
                    .map_err(|error| format!("{}: {error}", dir.display()))?,
            );
        }

        let record = Record {
            slug: slug.to_string(),
            manifest: entry.digest.clone(),
            adapter,
            adapter_digest,
            pinned,
            writable: granted,
        };
        self.record(record.clone())?;
        Ok(Approval {
            slug: slug.to_string(),
            digest: record.manifest,
            manifest: entry.path.clone(),
            invoke: entry.manifest.invoke.clone(),
            invoke_writing: entry.manifest.invoke_writing.clone(),
            adapter: record.adapter,
            pinned: record.pinned,
            writable: record.writable,
            store: self.store.clone(),
            source: entry.source,
        })
    }

    /// Writes the store: `{v, records}`, operator-readable only.
    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.store.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&Stored {
            v: STORE_VERSION,
            records: self.records.clone(),
        })
        .map_err(|error| error.to_string())?;
        std::fs::write(&self.store, text)
            .map_err(|error| format!("{}: {error}", self.store.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.store, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("{}: {error}", self.store.display()))?;
        }
        Ok(())
    }
}

/// Every argv tail word the manifest declares — across `detect.version`,
/// `detect.probe`, `workspace_probe.argv`, `invoke`, and `invoke_writing`
/// — so approval and verification look at exactly the same words.
fn argv_tails<'a>(manifest: &'a Manifest) -> impl Iterator<Item = &'a String> + 'a {
    [
        Some(&manifest.detect.version),
        manifest.detect.probe.as_ref(),
        manifest.workspace_probe.as_ref().map(|probe| &probe.argv),
        Some(&manifest.invoke).filter(|argv| !argv.is_empty()),
        Some(&manifest.invoke_writing).filter(|argv| !argv.is_empty()),
    ]
    .into_iter()
    .flatten()
    .flat_map(|argv| argv.iter().skip(1))
}

/// What an argv word resolves to when run in `cwd`: the word itself when
/// it is absolute, `cwd` joined under it when it is not.
fn resolve_word(word: &str, cwd: &Path) -> Result<PathBuf, String> {
    let path = Path::new(word);
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    candidate.canonicalize().map_err(|error| {
        format!(
            "{word:?} resolves to {}, which cannot be read: {error}",
            candidate.display()
        )
    })
}

/// The files an approval pins beyond the adapter itself: every argv word
/// — from `detect.version`, `detect.probe`, `workspace_probe.argv`, and
/// `invoke`, and `invoke_writing` — that names a file. An interpreted script is the adapter as
/// surely as the binary is; an inline `-c` script is pinned already, by
/// the manifest digest that covers it.
///
/// Two words refuse an approval outright rather than being silently
/// skipped: an absolute path that does not resolve, because a probe
/// would reach for a file nobody verified; and a relative word that
/// resolves under `repository`, because a relative path runs wherever the
/// argv is run — pinning it here would bless a different file there.
/// Name it absolutely, and the pin re-resolves it at run time.
fn pinned_argvs(manifest: &Manifest, repository: Option<&Path>) -> Result<Vec<Pinned>, String> {
    let base = match repository {
        Some(root) => root.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|error| format!("no working directory to resolve argvs in: {error}"))?,
    };
    let mut pinned = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for word in argv_tails(manifest) {
        if word.is_empty() || word.starts_with('-') {
            continue;
        }
        if Path::new(word).is_absolute() {
            let path = resolve_word(word, Path::new("/"))?;
            if !path.is_file() {
                return Err(format!(
                    "argv names {word:?}, which resolves to {} — a directory, not a file an approval can pin",
                    path.display()
                ));
            }
            if !seen.insert(word.clone()) {
                continue;
            }
            pinned.push(Pinned {
                word: word.clone(),
                digest: digest_file(&path)?,
                path,
            });
        } else if base.join(word).is_file() {
            return Err(format!(
                "argv {word:?} is a relative path that resolves to {} — it would run whatever \
                 the probe's directory holds, so approve it absolutely or not at all",
                base.join(word).display()
            ));
        }
    }
    Ok(pinned)
}

/// A store path canonicalized when it exists, or by its parent when it
/// does not — a store that has not been written still has to answer where
/// it will sit.
fn canonical_or_parent(store: &Path) -> Result<PathBuf, std::io::Error> {
    match store.canonicalize() {
        Ok(path) => Ok(path),
        Err(_) => {
            let parent = store.parent().unwrap_or(Path::new(".")).canonicalize()?;
            Ok(parent.join(store.file_name().unwrap_or_default()))
        }
    }
}

/// What an approval recorded: the manifest, the pinned adapter, and
/// where the record lives.
#[derive(Clone, Debug)]
pub struct Approval {
    /// The capability slug the approval names.
    pub slug: String,
    /// The digest of the manifest body the approval covers.
    pub digest: String,
    /// The manifest file the digest was read from.
    pub manifest: PathBuf,
    /// The argv the manifest says drives the executor, so the operator
    /// sees what was approved rather than what was asked for.
    pub invoke: Vec<String>,
    /// The argv a writing task runs instead, when the manifest declares
    /// one; empty when a writing task runs `invoke`.
    pub invoke_writing: Vec<String>,
    /// The canonical adapter path the record pins.
    pub adapter: PathBuf,
    /// The argv files the record pins — each word as the manifest spells
    /// it, re-resolved where the argv actually runs.
    pub pinned: Vec<Pinned>,
    /// The adapter state the approval grants, when a boundary asks.
    pub writable: Vec<PathBuf>,
    /// The store the record was written to.
    pub store: PathBuf,
    /// Where the manifest came from — the record, not the permission.
    pub source: Source,
}

impl std::fmt::Display for Approval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "approved {} — {} manifest ({})\n  manifest {}\n  adapter  {}\n  invoke   {}\n  store    {}",
            self.slug,
            self.source.word(),
            &self.digest[..12.min(self.digest.len())],
            self.manifest.display(),
            self.adapter.display(),
            self.invoke.join(" "),
            self.store.display(),
        )?;
        if !self.invoke_writing.is_empty() {
            write!(f, "\n  writing  {}", self.invoke_writing.join(" "))?;
        }
        for pinned in &self.pinned {
            if pinned.word.is_empty() {
                write!(f, "\n  pinned   {}", pinned.path.display())?;
            } else {
                write!(
                    f,
                    "\n  pinned   {} = {}",
                    pinned.word,
                    pinned.path.display()
                )?;
            }
        }
        for dir in &self.writable {
            write!(f, "\n  writable {}", dir.display())?;
        }
        Ok(())
    }
}
