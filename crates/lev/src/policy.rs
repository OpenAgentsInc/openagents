//! Revocation: the half of the check a running door cannot do for itself.
//!
//! A release is pinned at load time. `lev-serve` reads the manifest, checks
//! the artifact's digest, checks the base signature against the device, and
//! exits when either disagrees. That protects a door that restarts. It does
//! nothing for a door already running, and nothing at all for a door on a
//! machine nobody can reach.
//!
//! The standing failure is dated rather than hypothetical. The base model
//! ships with the operating system, an update replaces it, and every adapter
//! and every calibration map fitted against the old signature becomes
//! invalid. [`crate::manifest::Base::signature`] is the field that event
//! names, and until now nothing could act on it.
//!
//! # The mechanism
//!
//! A [`Snapshot`] is the policy document a canonical service publishes. It
//! carries when it was issued, how long a client may keep serving on it, and
//! what it stops. A door caches one and reads it on every request, so a
//! change to the cache reaches a running door without a restart.
//!
//! The part that makes this a revocation mechanism rather than a check is the
//! freshness window. A door whose cached snapshot is older than its window
//! **stops serving its managed release entirely**, whether or not it has ever
//! heard of a revocation. So a revocation reaches a door that never contacts
//! the service again:
//!
//! > The maximum enforcement delay is the freshness window.
//!
//! The reasoning is one line. A door holds a snapshot issued at `T`. A
//! revocation is published at some `P >= T`, because the service cannot
//! revoke something before it issued the snapshot the door is holding. The
//! door stops at `T + window`, which is at or before `P + window`. It either
//! sees the revocation or stops.
//!
//! # The complement
//!
//! Deleting the cache must not turn a managed artifact into an unmanaged
//! local one, so an absent snapshot is an expired snapshot. A door whose
//! cache is missing, unreadable, or not a snapshot refuses exactly as a door
//! holding a stale one does. The manifest is what makes a release managed,
//! and nothing a client deletes can make it unmanaged.
//!
//! # What is deliberately absent
//!
//! Signatures and key material. The chain is transport to a canonical service
//! plus a digest checked before anything is written where a door can load it,
//! which is [`store`]. There is one owner and one machine, and a signing key
//! would be the second-hardest part of a system whose easiest part — a
//! document saying what may serve — did not exist a week ago.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Refusal, RefusalCode};

/// The schema tag every policy snapshot carries.
pub const POLICY_SCHEMA: &str = "openagents.lev.policy_snapshot.v1";

/// The window this repository publishes, in seconds.
///
/// A day. It is the number `coder`'s catalog uses, and the argument for it is
/// that the enforcement delay a person will accept for a bad artifact is
/// about a day and the fetch traffic it implies is one small document per
/// door per fetch interval.
pub const DEFAULT_WINDOW_SECONDS: u64 = 24 * 60 * 60;

/// Why a snapshot may not be used.
///
/// Each value names one field, for the reason [`crate::manifest::Fault`]
/// does: "the policy does not check out" is not an answer anybody can act on.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Trouble {
    /// The document is tagged as something other than a snapshot.
    #[error("schema: the document is tagged {found}, not {POLICY_SCHEMA}")]
    Schema {
        /// The tag the file carried.
        found: String,
    },
    /// The file did not read, or did not parse.
    #[error("{path}: {reason}")]
    Unreadable {
        /// What was being read.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// A required field is blank.
    #[error("{field} is blank")]
    Blank {
        /// The field.
        field: &'static str,
    },
    /// A timestamp is not one.
    ///
    /// Refused rather than ignored. A snapshot nobody can date is a snapshot
    /// whose freshness nobody can decide, and this mechanism fails closed.
    #[error("{field}: `{value}` is not an HTTP date such as `Sat, 19 Sep 2026 17:00:00 GMT`")]
    Timestamp {
        /// The field.
        field: &'static str,
        /// What it held.
        value: String,
    },
    /// The bytes fetched are not the bytes expected.
    ///
    /// The field is `origin` rather than `source` because `thiserror` reads a
    /// field of that name as a nested error, and this one is a path.
    #[error("sha256: {origin} digests to {found}, and {expected} was expected")]
    Digest {
        /// Where the bytes came from.
        origin: String,
        /// What the caller asked for.
        expected: String,
        /// What arrived.
        found: String,
    },
    /// A revocation names neither a release nor a base signature.
    ///
    /// Such an entry would stop every door that read it, which is a thing a
    /// publisher might mean and must not be able to write by leaving two
    /// fields blank.
    #[error(
        "revoked[{at}]: a revocation names neither a release nor a base signature, so it would \
         stop every door that read it"
    )]
    Unaimed {
        /// Which entry.
        at: usize,
    },
}

/// The clock a [`Policy`] reads, in seconds since the Unix epoch.
///
/// A settable clock is the seam a freshness window needs: the guarantee is
/// about a day passing, and a test that waited a day would not be run. A
/// fixed clock is shared, so a test can advance time under a door that is
/// already running and serving.
#[derive(Clone, Debug, Default)]
pub struct Clock(Option<Arc<AtomicI64>>);

impl Clock {
    /// The machine's clock.
    #[must_use]
    pub fn system() -> Self {
        Self(None)
    }

    /// A clock that reads `seconds` until something advances it.
    #[must_use]
    pub fn fixed(seconds: i64) -> Self {
        Self(Some(Arc::new(AtomicI64::new(seconds))))
    }

    /// Moves a fixed clock forward. The machine's clock ignores this.
    pub fn advance(&self, seconds: i64) {
        if let Some(held) = &self.0 {
            held.fetch_add(seconds, Ordering::Relaxed);
        }
    }

    /// The time this clock reads.
    #[must_use]
    pub fn now(&self) -> i64 {
        match &self.0 {
            Some(held) => held.load(Ordering::Relaxed),
            None => unix(SystemTime::now()),
        }
    }
}

/// One thing a snapshot stops.
///
/// A revocation is aimed by release, by base model signature, or by both. The
/// base signature is the aim the treadmill needs: an operating system update
/// replaces the base, and one entry naming the old signature stops every
/// release fitted against it without anybody listing them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revocation {
    /// The release it stops, as `name@version`. Blank matches every release.
    #[serde(default)]
    pub release: String,
    /// The base model signature it stops. Blank matches every base.
    ///
    /// Matched as a prefix in either direction, because the runtime publishes
    /// a prefix and a manifest pins the whole signature.
    #[serde(default)]
    pub base_signature: String,
    /// The question families it stops. Empty stops the release outright.
    #[serde(default)]
    pub families: Vec<String>,
    /// When it takes effect, as an HTTP date.
    pub effective: String,
    /// Why, in words a caller reading the refusal can act on.
    pub reason: String,
}

impl Revocation {
    /// A revocation of one release, effective now.
    #[must_use]
    pub fn of(release: impl Into<String>, reason: impl Into<String>, effective: i64) -> Self {
        Self {
            release: release.into(),
            base_signature: String::new(),
            families: Vec::new(),
            effective: stamp(effective),
            reason: reason.into(),
        }
    }

    /// The same, aimed at a base model signature rather than a release.
    #[must_use]
    pub fn of_base(
        signature: impl Into<String>,
        reason: impl Into<String>,
        effective: i64,
    ) -> Self {
        Self {
            release: String::new(),
            base_signature: signature.into(),
            families: Vec::new(),
            effective: stamp(effective),
            reason: reason.into(),
        }
    }

    /// Narrows a revocation to named families.
    #[must_use]
    pub fn for_families(mut self, families: Vec<String>) -> Self {
        self.families = families;
        self
    }

    /// Whether this entry is aimed at the door running `release` on `base`.
    ///
    /// The family is asked separately by [`Revocation::covers`], because a
    /// door has one release and many families.
    #[must_use]
    pub fn aims_at(&self, release: &str, base: &str) -> bool {
        let by_release = self.release.is_empty() || self.release == release;
        let by_base = self.base_signature.is_empty() || prefix_match(&self.base_signature, base);
        by_release && by_base
    }

    /// Whether this entry covers `family`.
    ///
    /// An empty `families` stops the release outright, so it covers every
    /// family and the release itself, which is the empty string.
    #[must_use]
    pub fn covers(&self, family: &str) -> bool {
        self.families.is_empty() || self.families.iter().any(|named| named == family)
    }

    /// The families it stops, in one line.
    #[must_use]
    pub fn scope(&self) -> String {
        if self.families.is_empty() {
            "every family".to_string()
        } else {
            self.families.join(", ")
        }
    }
}

/// What the service says, as of one moment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// What this document is. Always [`POLICY_SCHEMA`].
    #[serde(default = "policy_schema")]
    pub schema: String,
    /// When the service issued it, as an HTTP date.
    pub issued: String,
    /// How long a client may keep serving on it.
    ///
    /// The service's guarantee rather than the client's preference. A release
    /// may hold the window lower through
    /// [`SnapshotRef::freshness_window_seconds`]; it cannot raise it.
    pub freshness_window_seconds: u64,
    /// What it stops.
    #[serde(default)]
    pub revoked: Vec<Revocation>,
}

fn policy_schema() -> String {
    POLICY_SCHEMA.to_string()
}

impl Snapshot {
    /// An empty snapshot issued at `issued`, revoking nothing.
    #[must_use]
    pub fn new(issued: i64, window_seconds: u64) -> Self {
        Self {
            schema: policy_schema(),
            issued: stamp(issued),
            freshness_window_seconds: window_seconds,
            revoked: Vec::new(),
        }
    }

    /// Adds a revocation.
    #[must_use]
    pub fn revoking(mut self, revocation: Revocation) -> Self {
        self.revoked.push(revocation);
        self
    }

    /// Reads a snapshot from JSON and checks it.
    ///
    /// # Errors
    ///
    /// Returns the [`Trouble`] that names the field.
    pub fn from_json(source: &str) -> Result<Self, Trouble> {
        let snapshot: Self = serde_json::from_str(source).map_err(|error| Trouble::Unreadable {
            path: "the snapshot".to_string(),
            reason: error.to_string(),
        })?;
        snapshot.check()?;
        Ok(snapshot)
    }

    /// Writes it as the published document is written.
    ///
    /// # Errors
    ///
    /// Returns what `serde_json` reports.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Checks the schema tag, the timestamps, and the aim of every entry.
    ///
    /// # Errors
    ///
    /// Returns the [`Trouble`] that names the field.
    pub fn check(&self) -> Result<(), Trouble> {
        if self.schema != POLICY_SCHEMA {
            return Err(Trouble::Schema { found: self.schema.clone() });
        }
        if self.issued.trim().is_empty() {
            return Err(Trouble::Blank { field: "issued" });
        }
        parse(&self.issued, "issued")?;
        for (at, revocation) in self.revoked.iter().enumerate() {
            if revocation.release.trim().is_empty() && revocation.base_signature.trim().is_empty() {
                return Err(Trouble::Unaimed { at });
            }
            if revocation.reason.trim().is_empty() {
                return Err(Trouble::Blank { field: "revoked[].reason" });
            }
            parse(&revocation.effective, "revoked[].effective")?;
        }
        Ok(())
    }

    /// When it was issued, in seconds since the Unix epoch.
    ///
    /// # Errors
    ///
    /// Returns [`Trouble::Timestamp`] when the field is not an HTTP date.
    pub fn issued_at(&self) -> Result<i64, Trouble> {
        parse(&self.issued, "issued")
    }

    /// The entries aimed at a door running `release` on `base`, in effect at
    /// `now`.
    #[must_use]
    pub fn biting(&self, release: &str, base: &str, now: i64) -> Vec<&Revocation> {
        self.revoked
            .iter()
            .filter(|revocation| revocation.aims_at(release, base))
            .filter(|revocation| parse(&revocation.effective, "effective").is_ok_and(|at| at <= now))
            .collect()
    }
}

/// Where a release's policy snapshot comes from, and where its door keeps a
/// copy.
///
/// Carried by [`crate::manifest::Manifest`] as `policySnapshot`, and required
/// there. A release that could decline to name a policy source would be a
/// release that escapes revocation by omission, which is the same hole as a
/// deleted cache with a tidier name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRef {
    /// The canonical service, resolved against the manifest's own directory
    /// when it is a relative path.
    pub source: String,
    /// Where the fetched copy lives on this machine. A leading `~` is the
    /// running user's home.
    ///
    /// Machine-local on purpose, and never inside the repository: a cache is
    /// a client's copy of somebody else's document, and a copy that is
    /// committed is a copy that stops being fetched.
    pub cache: String,
    /// The longest window this release accepts, in seconds.
    ///
    /// The effective window is the smaller of this and the snapshot's, so a
    /// snapshot cannot make a release immortal by claiming a century.
    pub freshness_window_seconds: u64,
}

impl SnapshotRef {
    /// The reference this repository's releases carry.
    #[must_use]
    pub fn published() -> Self {
        Self {
            source: "../policy/current.json".to_string(),
            cache: "~/.lev/policy/current.json".to_string(),
            freshness_window_seconds: DEFAULT_WINDOW_SECONDS,
        }
    }
}

/// What a door's cached snapshot says about one question family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
    /// The snapshot is current and stops nothing this door was asked for.
    Current {
        /// How long the cached snapshot has left, in seconds.
        expires_in_seconds: i64,
    },
    /// The snapshot is current and stops this.
    Revoked {
        /// Why.
        reason: String,
        /// Since when.
        effective: String,
        /// What it stops, in one line.
        scope: String,
    },
    /// The cached snapshot is older than the window it was issued under.
    Stale {
        /// How old it is, in seconds.
        age_seconds: i64,
        /// The window it was judged against.
        window_seconds: u64,
    },
    /// Nothing is cached, which is an expired snapshot by another name.
    Absent {
        /// Where the door looked.
        cache: String,
    },
    /// Something is cached and it is not a snapshot.
    Unreadable {
        /// Where the door looked.
        cache: String,
        /// What is wrong with it.
        reason: String,
    },
}

impl Standing {
    /// The word `GET /v1/models` publishes.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Current { .. } => "current",
            Self::Revoked { .. } => "revoked",
            Self::Stale { .. } => "stale",
            Self::Absent { .. } => "absent",
            Self::Unreadable { .. } => "unreadable",
        }
    }

    /// Whether a door in this standing may answer.
    #[must_use]
    pub const fn serves(&self) -> bool {
        matches!(self, Self::Current { .. })
    }
}

/// What a door publishes about its policy.
#[derive(Clone, Debug)]
pub struct Report {
    /// The canonical service this door fetches from.
    pub source: String,
    /// Where its copy lives.
    pub cache: String,
    /// The release-wide standing.
    pub standing: Standing,
    /// When the cached snapshot was issued, when there is one.
    pub issued: String,
    /// The window in force, which is the smaller of the two.
    pub window_seconds: u64,
    /// How long the cached snapshot has left. Negative once it is stale.
    pub expires_in_seconds: i64,
    /// The cached snapshot's digest, when there is one.
    pub sha256: String,
    /// Every entry aimed at this release and in effect.
    pub revoked: Vec<Revocation>,
}

/// A door's standing with the service.
///
/// The cache is read on every question rather than at startup. That is the
/// whole point: a door that read its policy once would be a door that has to
/// restart to learn it may not serve, and restarting is the half we already
/// had. The document is one small file on local disk, next to a question that
/// costs seconds of on-device inference.
#[derive(Clone, Debug)]
pub struct Policy {
    source: PathBuf,
    cache: PathBuf,
    ceiling: u64,
    release: String,
    base_signature: String,
    clock: Clock,
}

/// The cached snapshot, read once per decision.
enum Cached {
    Held { snapshot: Snapshot, digest: String, issued: i64 },
    Missing,
    Broken(Trouble),
}

impl Policy {
    /// The policy governing a release.
    ///
    /// `beside` is the manifest's own directory, which a relative `source`
    /// resolves against, for the reason an `evalRef` path does: a document
    /// that named its own directory would be wrong the moment it moved.
    #[must_use]
    pub fn for_release(
        reference: &SnapshotRef,
        beside: &Path,
        release: impl Into<String>,
        base_signature: impl Into<String>,
    ) -> Self {
        Self {
            source: resolve(&reference.source, beside),
            cache: expand(&reference.cache),
            ceiling: reference.freshness_window_seconds,
            release: release.into(),
            base_signature: base_signature.into(),
            clock: Clock::system(),
        }
    }

    /// Reads the clock this policy judges freshness against.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// The canonical service.
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Where this door keeps its copy.
    #[must_use]
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    /// The release this policy governs.
    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }

    /// The clock it reads.
    #[must_use]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Fetches the canonical snapshot and stores it, if it checks out.
    ///
    /// # Errors
    ///
    /// Returns the [`Trouble`] that names the field. A fetch that fails
    /// leaves the cache alone, so a door keeps whatever it last confirmed —
    /// and goes stale on schedule if it never confirms another.
    pub fn fetch(&self) -> Result<String, Trouble> {
        let bytes = std::fs::read(&self.source).map_err(|error| Trouble::Unreadable {
            path: self.source.display().to_string(),
            reason: error.to_string(),
        })?;
        store(&self.cache, &bytes, None)
    }

    /// Whether this door may answer for `family` right now.
    ///
    /// The empty family asks about the release itself, which is what a
    /// request naming no family is asking for.
    ///
    /// # Errors
    ///
    /// Returns [`RefusalCode::Revoked`] when the service stopped this, and
    /// [`RefusalCode::PolicyStale`] when the door cannot say that it did not.
    pub fn admits(&self, family: &str) -> crate::error::Result<()> {
        match self.standing_for(family) {
            Standing::Current { .. } => Ok(()),
            Standing::Revoked { reason, effective, scope } => Err(Refusal::new(
                RefusalCode::Revoked,
                format!(
                    "{} is revoked as of {effective}, for {scope}: {reason}. A revoked release \
                     does not come back; serve another release.",
                    self.release
                ),
            )),
            Standing::Stale { age_seconds, window_seconds } => Err(Refusal::new(
                RefusalCode::PolicyStale,
                format!(
                    "{} is a managed release and its policy snapshot is {age_seconds} seconds \
                     old against a window of {window_seconds}. A door that cannot confirm its \
                     policy stops serving its managed release, so that a revocation reaches a \
                     door that never contacts the service again within the window. Fetch a \
                     current snapshot from {}.",
                    self.release,
                    self.source.display()
                ),
            )),
            Standing::Absent { cache } => Err(Refusal::new(
                RefusalCode::PolicyStale,
                format!(
                    "{} is a managed release and no policy snapshot is cached at {cache}. An \
                     absent snapshot is an expired one: deleting the cache does not make a \
                     managed release a local one. Fetch one from {}.",
                    self.release,
                    self.source.display()
                ),
            )),
            Standing::Unreadable { cache, reason } => Err(Refusal::new(
                RefusalCode::PolicyStale,
                format!(
                    "{} is a managed release and the snapshot cached at {cache} is not one: \
                     {reason}. A snapshot that cannot be read is one whose freshness cannot be \
                     decided, and this door refuses rather than assumes.",
                    self.release
                ),
            )),
        }
    }

    /// The release-wide standing, which is what `admits("")` decides on.
    #[must_use]
    pub fn standing(&self) -> Standing {
        self.standing_for("")
    }

    /// What the cached snapshot says about one family.
    #[must_use]
    pub fn standing_for(&self, family: &str) -> Standing {
        let now = self.clock.now();
        match self.cached() {
            Cached::Missing => Standing::Absent { cache: self.cache.display().to_string() },
            Cached::Broken(trouble) => Standing::Unreadable {
                cache: self.cache.display().to_string(),
                reason: trouble.to_string(),
            },
            Cached::Held { snapshot, issued, .. } => {
                let window = self.window(&snapshot);
                let expires_in = issued.saturating_add(as_i64(window)).saturating_sub(now);
                if expires_in < 0 {
                    return Standing::Stale {
                        age_seconds: now.saturating_sub(issued),
                        window_seconds: window,
                    };
                }
                let biting = snapshot.biting(&self.release, &self.base_signature, now);
                match biting.into_iter().find(|revocation| revocation.covers(family)) {
                    Some(revocation) => Standing::Revoked {
                        reason: revocation.reason.clone(),
                        effective: revocation.effective.clone(),
                        scope: revocation.scope(),
                    },
                    None => Standing::Current { expires_in_seconds: expires_in },
                }
            }
        }
    }

    /// Everything this door publishes about its policy, read in one pass.
    #[must_use]
    pub fn report(&self) -> Report {
        let now = self.clock.now();
        let standing = self.standing();
        let mut report = Report {
            source: self.source.display().to_string(),
            cache: self.cache.display().to_string(),
            standing,
            issued: String::new(),
            window_seconds: self.ceiling,
            expires_in_seconds: 0,
            sha256: String::new(),
            revoked: Vec::new(),
        };
        if let Cached::Held { snapshot, digest, issued } = self.cached() {
            let window = self.window(&snapshot);
            report.issued = snapshot.issued.clone();
            report.window_seconds = window;
            report.expires_in_seconds =
                issued.saturating_add(as_i64(window)).saturating_sub(now);
            report.sha256 = digest;
            report.revoked = snapshot
                .biting(&self.release, &self.base_signature, now)
                .into_iter()
                .cloned()
                .collect();
        }
        report
    }

    /// The families in `held` this door may still answer for.
    ///
    /// Empty when the release itself does not serve, because a stale snapshot
    /// stops the managed release outright rather than family by family.
    #[must_use]
    pub fn serving<'a>(&self, held: &[&'a str]) -> Vec<&'a str> {
        if !self.standing().serves() {
            return Vec::new();
        }
        held.iter().copied().filter(|family| self.standing_for(family).serves()).collect()
    }

    /// The window in force: the smaller of the release's and the snapshot's.
    fn window(&self, snapshot: &Snapshot) -> u64 {
        self.ceiling.min(snapshot.freshness_window_seconds)
    }

    fn cached(&self) -> Cached {
        let text = match std::fs::read_to_string(&self.cache) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Cached::Missing,
            Err(error) => {
                return Cached::Broken(Trouble::Unreadable {
                    path: self.cache.display().to_string(),
                    reason: error.to_string(),
                });
            }
        };
        let snapshot = match Snapshot::from_json(&text) {
            Ok(snapshot) => snapshot,
            Err(trouble) => return Cached::Broken(trouble),
        };
        let issued = match snapshot.issued_at() {
            Ok(issued) => issued,
            Err(trouble) => return Cached::Broken(trouble),
        };
        Cached::Held { snapshot, digest: digest_of(text.as_bytes()), issued }
    }
}

/// Writes a fetched snapshot into the cache, once it checks out.
///
/// The order is the whole of the trust machinery here: digest the bytes,
/// compare them against what was asked for, parse them, check them, and only
/// then put them somewhere a door will load them. A cache written first and
/// checked afterwards is a cache that was briefly authoritative and wrong.
///
/// The write is a rename over a temporary file beside the cache, so a door
/// reading the cache on another thread sees the old snapshot or the new one
/// and never half of either.
///
/// # Errors
///
/// Returns the [`Trouble`] that names the field. Nothing is written unless
/// every check passed.
pub fn store(cache: &Path, bytes: &[u8], expected: Option<&str>) -> Result<String, Trouble> {
    let digest = digest_of(bytes);
    if let Some(expected) = expected
        && expected != digest
    {
        return Err(Trouble::Digest {
            origin: cache.display().to_string(),
            expected: expected.to_string(),
            found: digest,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|error| Trouble::Unreadable {
        path: cache.display().to_string(),
        reason: error.to_string(),
    })?;
    Snapshot::from_json(text)?;
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent).map_err(|error| Trouble::Unreadable {
            path: parent.display().to_string(),
            reason: error.to_string(),
        })?;
    }
    let staged = cache.with_extension("fetching");
    std::fs::write(&staged, bytes).map_err(|error| Trouble::Unreadable {
        path: staged.display().to_string(),
        reason: error.to_string(),
    })?;
    std::fs::rename(&staged, cache).map_err(|error| Trouble::Unreadable {
        path: cache.display().to_string(),
        reason: error.to_string(),
    })?;
    Ok(digest)
}

/// Reads a snapshot from the canonical service, with its digest.
///
/// # Errors
///
/// Returns the [`Trouble`] that names the field.
pub fn read(source: &Path) -> Result<(Snapshot, String), Trouble> {
    let text = std::fs::read_to_string(source).map_err(|error| Trouble::Unreadable {
        path: source.display().to_string(),
        reason: error.to_string(),
    })?;
    Ok((Snapshot::from_json(&text)?, digest_of(text.as_bytes())))
}

/// A count of seconds, written as `86400`, `30m`, `24h`, or `7d`.
///
/// Returns `None` for anything else, so a caller can say what it wanted
/// rather than silently taking a default.
#[must_use]
pub fn duration(value: &str) -> Option<u64> {
    let value = value.trim();
    let (count, scale) = match value.chars().last()? {
        's' => (&value[..value.len() - 1], 1),
        'm' => (&value[..value.len() - 1], 60),
        'h' => (&value[..value.len() - 1], 60 * 60),
        'd' => (&value[..value.len() - 1], 24 * 60 * 60),
        _ => (value, 1),
    };
    count.parse::<u64>().ok().map(|count| count.saturating_mul(scale))
}

/// An HTTP date, as a snapshot writes one.
#[must_use]
pub fn stamp(seconds: i64) -> String {
    let at = if seconds >= 0 {
        UNIX_EPOCH + std::time::Duration::from_secs(seconds.unsigned_abs())
    } else {
        UNIX_EPOCH - std::time::Duration::from_secs(seconds.unsigned_abs())
    };
    httpdate::fmt_http_date(at)
}

/// The sha256 of some bytes, lowercase hex.
#[must_use]
pub fn digest_of(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn parse(value: &str, field: &'static str) -> Result<i64, Trouble> {
    httpdate::parse_http_date(value.trim())
        .map(unix)
        .map_err(|_| Trouble::Timestamp { field, value: value.to_string() })
}

fn unix(at: SystemTime) -> i64 {
    match at.duration_since(UNIX_EPOCH) {
        Ok(since) => as_i64(since.as_secs()),
        Err(before) => -as_i64(before.duration().as_secs()),
    }
}

/// Saturating, because a second count that overflows an `i64` is not a time
/// anybody meant.
fn as_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn prefix_match(left: &str, right: &str) -> bool {
    !left.is_empty() && !right.is_empty() && (left.starts_with(right) || right.starts_with(left))
}

/// A relative source resolves against the manifest's directory.
fn resolve(source: &str, beside: &Path) -> PathBuf {
    let path = expand(source);
    if path.is_absolute() { path } else { beside.join(path) }
}

/// A leading `~` is the running user's home. Any other path is untouched.
fn expand(path: &str) -> PathBuf {
    let Some(rest) = path.strip_prefix("~/") else { return PathBuf::from(path) };
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_789_000_000;
    const RELEASE: &str = "lev-base@1";
    const BASE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lev-policy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    /// A policy over a cache in `dir`, reading a clock a test can advance.
    fn policy(dir: &Path) -> Policy {
        let reference = SnapshotRef {
            source: "service.json".to_string(),
            cache: dir.join("cache.json").display().to_string(),
            freshness_window_seconds: DEFAULT_WINDOW_SECONDS,
        };
        Policy::for_release(&reference, dir, RELEASE, BASE).with_clock(Clock::fixed(NOW))
    }

    fn publish(dir: &Path, snapshot: &Snapshot) {
        std::fs::write(dir.join("service.json"), snapshot.to_json().expect("it encodes"))
            .expect("the service publishes");
    }

    #[test]
    fn a_window_is_written_the_way_an_operator_says_it() {
        assert_eq!(duration("86400"), Some(DEFAULT_WINDOW_SECONDS));
        assert_eq!(duration("24h"), Some(DEFAULT_WINDOW_SECONDS));
        assert_eq!(duration("1d"), Some(DEFAULT_WINDOW_SECONDS));
        assert_eq!(duration("30m"), Some(1_800));
        assert_eq!(duration("half a day"), None);
    }

    #[test]
    fn a_snapshot_round_trips_through_its_own_writer() {
        let written = Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS)
            .revoking(Revocation::of(RELEASE, "the base moved", NOW));
        let read = Snapshot::from_json(&written.to_json().expect("it encodes")).expect("it decodes");
        assert_eq!(read, written);
        assert_eq!(read.issued_at().expect("a date"), NOW);
    }

    #[test]
    fn a_revocation_aimed_at_nothing_is_refused() {
        // Two blank fields would otherwise stop every door that read the
        // document, which is a thing a publisher has to write on purpose.
        let text = serde_json::json!({
            "schema": POLICY_SCHEMA,
            "issued": stamp(NOW),
            "freshnessWindowSeconds": DEFAULT_WINDOW_SECONDS,
            "revoked": [{"effective": stamp(NOW), "reason": "everything"}],
        })
        .to_string();
        let trouble = Snapshot::from_json(&text).expect_err("an unaimed entry is refused");
        assert!(matches!(trouble, Trouble::Unaimed { at: 0 }), "{trouble}");
    }

    #[test]
    fn a_current_snapshot_serves_and_a_revoked_release_does_not() {
        let dir = scratch("revoked");
        let policy = policy(&dir);
        publish(&dir, &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS));
        policy.fetch().expect("the snapshot fetches");
        policy.admits("routing").expect("a current snapshot serves");

        publish(
            &dir,
            &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).revoking(Revocation::of(
                RELEASE,
                "the operating system replaced the base",
                NOW,
            )),
        );
        policy.fetch().expect("the revocation fetches");
        let refusal = policy.admits("routing").expect_err("a revoked release is refused");
        assert_eq!(refusal.code, RefusalCode::Revoked);
        assert!(refusal.message.contains("replaced the base"), "{}", refusal.message);
        assert!(!policy.standing().serves());
    }

    #[test]
    fn a_revocation_aimed_at_a_base_signature_stops_every_release_on_it() {
        // The treadmill, aimed the way the treadmill arrives: nobody lists
        // the releases, because the operating system did not either.
        let dir = scratch("base");
        let policy = policy(&dir);
        publish(
            &dir,
            &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).revoking(Revocation::of_base(
                "9799725",
                "25E246 shipped a new base",
                NOW,
            )),
        );
        policy.fetch().expect("the snapshot fetches");
        let refusal = policy.admits("").expect_err("every release on that base is refused");
        assert_eq!(refusal.code, RefusalCode::Revoked);
    }

    #[test]
    fn a_family_scoped_revocation_stops_that_family_and_no_other() {
        let dir = scratch("family");
        let policy = policy(&dir);
        publish(
            &dir,
            &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).revoking(
                Revocation::of(RELEASE, "the map was refitted and lost", NOW)
                    .for_families(vec!["routing".to_string()]),
            ),
        );
        policy.fetch().expect("the snapshot fetches");
        assert!(policy.admits("routing").is_err());
        policy.admits("severity").expect("another family still serves");
        assert_eq!(policy.serving(&["routing", "severity"]), vec!["severity"]);
    }

    #[test]
    fn a_revocation_that_is_not_yet_in_effect_does_not_bite() {
        let dir = scratch("effective");
        let policy = policy(&dir);
        publish(
            &dir,
            &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).revoking(Revocation::of(
                RELEASE,
                "the next build",
                NOW + 600,
            )),
        );
        policy.fetch().expect("the snapshot fetches");
        policy.admits("routing").expect("a future revocation does not bite yet");
        policy.clock().advance(600);
        assert!(policy.admits("routing").is_err(), "it bites once it is in effect");
    }

    #[test]
    fn the_window_is_the_maximum_enforcement_delay() {
        // The guarantee, stated as an assertion: a door holding a snapshot
        // that was current when it was fetched stops at the window, having
        // heard nothing since.
        let dir = scratch("window");
        let policy = policy(&dir);
        publish(&dir, &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS));
        policy.fetch().expect("the snapshot fetches");

        policy.clock().advance(as_i64(DEFAULT_WINDOW_SECONDS));
        policy.admits("routing").expect("the last second inside the window serves");
        policy.clock().advance(1);
        let refusal = policy.admits("routing").expect_err("one second past the window does not");
        assert_eq!(refusal.code, RefusalCode::PolicyStale);
        assert!(matches!(policy.standing(), Standing::Stale { .. }));
    }

    #[test]
    fn a_release_holds_the_window_lower_and_cannot_raise_it() {
        let dir = scratch("ceiling");
        let reference = SnapshotRef {
            source: "service.json".to_string(),
            cache: dir.join("cache.json").display().to_string(),
            freshness_window_seconds: 3_600,
        };
        let policy = Policy::for_release(&reference, &dir, RELEASE, BASE)
            .with_clock(Clock::fixed(NOW));
        publish(&dir, &Snapshot::new(NOW, 100 * DEFAULT_WINDOW_SECONDS));
        policy.fetch().expect("the snapshot fetches");
        assert_eq!(policy.report().window_seconds, 3_600);
        policy.clock().advance(3_601);
        assert!(policy.admits("routing").is_err(), "a generous snapshot did not raise the window");
    }

    #[test]
    fn deleting_the_cache_does_not_unmanage_the_release() {
        let dir = scratch("deleted");
        let policy = policy(&dir);
        publish(&dir, &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS));
        policy.fetch().expect("the snapshot fetches");
        policy.admits("routing").expect("a current snapshot serves");

        std::fs::remove_file(dir.join("cache.json")).expect("the cache deletes");
        let refusal = policy.admits("routing").expect_err("an absent snapshot is an expired one");
        assert_eq!(refusal.code, RefusalCode::PolicyStale);
        assert!(refusal.message.contains("deleting the cache"), "{}", refusal.message);
    }

    #[test]
    fn a_cache_that_is_not_a_snapshot_refuses_rather_than_being_ignored() {
        let dir = scratch("garbage");
        let policy = policy(&dir);
        std::fs::write(dir.join("cache.json"), "{\"schema\":\"something.else\"}")
            .expect("the cache writes");
        let refusal = policy.admits("routing").expect_err("a document that is not a snapshot");
        assert_eq!(refusal.code, RefusalCode::PolicyStale);
        assert!(matches!(policy.standing(), Standing::Unreadable { .. }));
    }

    #[test]
    fn a_fetch_that_does_not_digest_leaves_the_cache_alone() {
        // The order that matters: checked before it is written where a door
        // will load it, so a cache is never briefly authoritative and wrong.
        let dir = scratch("digest");
        let cache = dir.join("cache.json");
        let good = Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS).to_json().expect("it encodes");
        let held = store(&cache, good.as_bytes(), None).expect("the first snapshot stores");

        let trouble = store(&cache, b"{\"schema\":\"openagents.lev.policy_snapshot.v1\"}", Some(&held))
            .expect_err("bytes that do not digest are refused");
        assert!(matches!(trouble, Trouble::Digest { .. }), "{trouble}");
        assert_eq!(std::fs::read_to_string(&cache).expect("the cache reads"), good);

        let trouble = store(&cache, b"not a document", None).expect_err("and neither is garbage");
        assert!(matches!(trouble, Trouble::Unreadable { .. }), "{trouble}");
        assert_eq!(std::fs::read_to_string(&cache).expect("the cache reads"), good);
    }

    #[test]
    fn a_failed_fetch_leaves_the_door_on_its_own_clock() {
        // Nothing about an unreachable service makes a door serve longer.
        let dir = scratch("unreachable");
        let policy = policy(&dir);
        publish(&dir, &Snapshot::new(NOW, DEFAULT_WINDOW_SECONDS));
        policy.fetch().expect("the snapshot fetches");
        std::fs::remove_file(dir.join("service.json")).expect("the service goes away");

        policy.clock().advance(as_i64(DEFAULT_WINDOW_SECONDS) + 1);
        assert!(policy.fetch().is_err(), "the service is unreachable");
        assert_eq!(policy.admits("routing").expect_err("and the door stopped").code,
            RefusalCode::PolicyStale);
    }
}
