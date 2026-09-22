//! Packages and locks: the portable form of a program, and the pinned
//! record a run holds.
//!
//! A program file is a description — named steps with bounds, and no
//! code, no commands, and no prompts. Nothing in the description says
//! which wording its `decide` steps asked from, which source its `query`
//! steps read, or which capability manifests a `delegate` step ran
//! beside, because those live in files of their own on whichever machine
//! resolved them. Two runs of "the same" program are comparable only when
//! all of that was the same too.
//!
//! A [`Package`] is the description plus its world, stated as one record:
//! the program and every question set, source, policy, and capability
//! manifest it names, each pinned by the digest of its bytes, with the
//! publisher and provenance the record claims, the other packages it
//! depends on, and the compatibility it requires. A [`Lock`] is the
//! resolved form — every reference reduced to the digest the bytes on
//! this machine actually have. The lock is what makes two runs
//! comparable: a run holds one for its whole lifetime, and nothing inside
//! it can be relabeled while the run is alive.
//!
//! # Verify, or refuse
//!
//! [`Package::resolve`] checks every pin against the bytes on disk and
//! produces a lock only when each one holds. A package that cannot verify
//! everything it names is refused, not partially trusted: a stated digest
//! the bytes do not produce is [`Refusal::Digest`], a name nothing on
//! this machine answers is [`Refusal::Unresolved`], a dependency chain
//! that comes back to itself is [`Refusal::Cyclic`], a compatibility
//! requirement the host does not satisfy is [`Refusal::Incompatible`],
//! and a publisher whose approval was withdrawn is [`Refusal::Revoked`].
//! [`HeldLock`] keeps one lock for the life of a run; a different lock
//! proposed while that run is alive is [`Refusal::Pinned`].
//!
//! # Inspection is not installation
//!
//! This module resolves and verifies records, and that is all it does.
//! Nothing here executes a step, approves a capability probe, grants a
//! credential, or writes an installation — those belong to other modules
//! and other authorities, and a lock is one of the things they inspect
//! before deciding. Resolving a package does not make it runnable; it
//! makes it knowable.
//!
//! # Trust is marked, never assumed
//!
//! A dependency carries a publisher claim, and the resolving package
//! names the publishers it extends trust to. A resolved dependency under
//! any other publisher lands in the lock as [`Trust::Untrusted`] — its
//! bytes pinned and verified like everything else, with the lock saying
//! plainly that the pin is all that was checked.
//!
//! # Local only
//!
//! Resolution reads local files and nothing else. There is no fetch, no
//! remote catalog, and no network: a name this machine does not hold is
//! unresolved rather than retrieved. Remote discovery is a later
//! acceptance item, and it arrives as a fetcher that produces records for
//! this module to verify — the record shapes here do not change.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::capability::is_slug;
use crate::questions::is_question_id;

/// The package record version this reads.
pub const PACKAGE_VERSION: u32 = 1;

/// The lock record version this writes.
pub const LOCK_VERSION: u32 = 1;

/// The digest a package states for a file's bytes.
///
/// The canonical form [`atif::digest`] already defines, pointed at the
/// file's bytes as one JSON string — the same rule [`crate::questions`]
/// digests wording by, run over bytes instead of fields. A package and
/// the lock it resolves to agree because both run this one function.
#[must_use]
pub fn digest(bytes: &str) -> String {
    atif::digest(&json!(bytes))
}

/// One component a package pins: a name and the digest its bytes must have.
///
/// `name` is the identifier the component's own registry answers to — the
/// slug for a program, source, policy, capability, or package, and the
/// question-set identifier (`openagents.independence.v2`) for wording.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub name: String,
    /// What [`digest`] returns for the file's bytes. Stated, then
    /// verified: a reference whose bytes do not produce it is refused,
    /// not repaired.
    pub digest: String,
}

/// One package another package depends on.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    /// The slug the dependency's record resolves under in `packages/`.
    pub package: String,
    /// The publisher the record must claim, when the edge names one. A
    /// record claiming another publisher is not the dependency this edge
    /// meant, and resolves as missing rather than substituted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// The requirement the resolved record's `version` label must satisfy
    /// — `>=1.2`, `=1.2.0`, and their kin. The label is a human version,
    /// never an identity: the pin underneath is the digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The digest the dependency's own package record must have, when the
    /// edge pins one. Unstated, the dependency is pinned by whatever its
    /// resolved lock digests to; stated, the record itself is verified
    /// before it is read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// One portable package: a program and everything it names, pinned.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub v: u32,
    /// The slug the package resolves under in `packages/`.
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    /// The human-readable release label. Carried, displayed, and checked
    /// against a dependency's `version` requirement — and never trusted
    /// as an identity, because a label can be rebound to different bytes
    /// and a digest cannot.
    #[serde(default)]
    pub version: String,
    /// Who the record claims published it. A claim, not a proof: the
    /// resolving package decides which publishers it extends trust to,
    /// and every other claim stays [`Trust::Untrusted`].
    #[serde(default)]
    pub publisher: String,
    /// Where the record came from — `local file`, and one day a relay or
    /// a catalog. Recorded so an earlier answer can be explained; it is
    /// not a verification of anything.
    #[serde(default)]
    pub provenance: String,
    /// The publishers this package extends dependency trust to. A
    /// dependency resolved under any other publisher is marked
    /// [`Trust::Untrusted`], never assumed.
    #[serde(default)]
    pub trusted_publishers: Vec<String>,
    /// The program this package exists to carry, pinned.
    pub program: Reference,
    /// The question sets the program's `decide` steps name, pinned.
    #[serde(default)]
    pub questions: Vec<Reference>,
    /// The sources the program's `query` steps name, pinned.
    #[serde(default)]
    pub sources: Vec<Reference>,
    /// The operator policies the package is read under, pinned.
    #[serde(default)]
    pub policies: Vec<Reference>,
    /// The capability manifests the package names, pinned. Naming a
    /// manifest pins its bytes; it does not approve the probe the
    /// manifest describes — that approval lives outside any checkout.
    #[serde(default)]
    pub capabilities: Vec<Reference>,
    /// The other packages this one needs resolved first.
    #[serde(default)]
    pub requires: Vec<Dependency>,
    /// What the host must satisfy. `coder` — this binary's own version —
    /// is the only requirement this host can check; any other name is a
    /// requirement the host has no answer for, which refuses rather than
    /// passing silently.
    #[serde(default)]
    pub compatibility: BTreeMap<String, String>,
}

impl Package {
    /// Reads a package record from a local file.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a package this host
    /// resolves: unreadable, unparseable, a `v` it does not know, a name
    /// outside its component's grammar, a malformed stated digest, or a
    /// component or dependency named twice.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let package: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        package
            .validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(package)
    }

    /// Whether this record is one this host resolves.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != PACKAGE_VERSION {
            return Err(format!(
                "body version is {}, this version reads {PACKAGE_VERSION}",
                self.v
            ));
        }
        if !is_slug(&self.slug) {
            return Err(format!("slug {:?} is not a package slug", self.slug));
        }
        check(&self.program, "the program", is_slug)?;
        for (what, references, grammar) in [
            (
                "questions",
                &self.questions,
                is_question_id as fn(&str) -> bool,
            ),
            ("sources", &self.sources, is_slug),
            ("policies", &self.policies, is_slug),
            ("capabilities", &self.capabilities, is_slug),
        ] {
            let mut seen = Vec::new();
            for reference in references {
                check(reference, what, grammar)?;
                if seen.contains(&reference.name) {
                    return Err(format!("{what} names {:?} twice", reference.name));
                }
                seen.push(reference.name.clone());
            }
        }
        let mut seen = Vec::new();
        for (field, value) in [
            ("name", self.name.as_str()),
            ("summary", self.summary.as_str()),
            ("version", self.version.as_str()),
            ("publisher", self.publisher.as_str()),
            ("provenance", self.provenance.as_str()),
        ] {
            if leaks_host(value) {
                return Err(format!(
                    "{field} carries a host path or a secret, which a portable package cannot"
                ));
            }
        }
        for publisher in &self.trusted_publishers {
            if leaks_host(publisher) {
                return Err(
                    "trusted_publishers carries a host path or a secret, which a portable package cannot"
                        .to_string(),
                );
            }
        }
        for (name, requirement) in &self.compatibility {
            if leaks_host(name) || leaks_host(requirement) {
                return Err(
                    "compatibility carries a host path or a secret, which a portable package cannot"
                        .to_string(),
                );
            }
        }
        for dep in &self.requires {
            if !is_slug(&dep.package) {
                return Err(format!("{:?} is not a package slug", dep.package));
            }
            if seen.contains(&dep.package) {
                return Err(format!("requires {:?} twice", dep.package));
            }
            seen.push(dep.package.clone());
            if dep.publisher.as_deref().is_some_and(leaks_host) {
                return Err(format!(
                    "dependency {:?} carries a host path or a secret, which a portable package cannot",
                    dep.package
                ));
            }
            if dep.version.as_deref().is_some_and(str::is_empty) {
                return Err(format!(
                    "dependency {:?} states an empty version requirement",
                    dep.package
                ));
            }
            if let Some(stated) = &dep.digest
                && !is_digest(stated)
            {
                return Err(format!(
                    "dependency {:?} states {stated:?}, which is not a digest",
                    dep.package
                ));
            }
        }
        Ok(())
    }

    /// Resolves a package against the local files under `root`.
    ///
    /// `root` is the directory the component registries live under —
    /// `programs/`, `questions/`, `sources/`, `policies/`,
    /// `capabilities/`, and `packages/` for dependencies. Every reference
    /// resolves to the file whose own record claims the name, and the
    /// file's bytes must produce the digest the package stated. The lock
    /// that comes back is the whole resolved record; nothing in it is
    /// trusted that was not verified.
    ///
    /// # Errors
    ///
    /// Returns a typed [`Refusal`]: the record itself is malformed, a
    /// name nothing on this machine answers, bytes that do not produce
    /// the stated digest, a dependency cycle, or a compatibility
    /// requirement the host does not satisfy.
    pub fn resolve(root: &Path, package: &Package) -> Result<Lock, Refusal> {
        resolve_tree(root, package, &mut Vec::new(), &BTreeSet::new())
    }

    /// Resolves `package` and refuses any publisher in `revoked`.
    ///
    /// Revocation wins over `trusted_publishers`. A revoked publisher is
    /// not marked untrusted and then accepted; the lock is not produced.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Revoked`] when the package or a dependency
    /// claims a revoked publisher, and every refusal [`Self::resolve`]
    /// returns otherwise.
    pub fn resolve_against(
        root: &Path,
        package: &Package,
        revoked: &BTreeSet<String>,
    ) -> Result<Lock, Refusal> {
        resolve_tree(root, package, &mut Vec::new(), revoked)
    }
}

/// A verified reference: the digest the bytes have and where they were
/// found.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Pin {
    /// The verified digest — the bytes', recomputed, never the record's
    /// word for them.
    pub digest: String,
    /// The path under the resolution root the bytes came from.
    pub found: String,
}

/// What the resolving package said about a dependency's publisher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Trust {
    /// The publisher is one the resolving package extends trust to.
    Trusted,
    /// Any other publisher. Marked, never assumed: an untrusted
    /// dependency's bytes are still pinned and verified, and the lock
    /// says plainly that the pin is all that was checked.
    Untrusted,
}

/// A resolved dependency and the trust it carries.
///
/// `record` pins the package file the dependency resolved from — its
/// digest and where it was found — so a lock holds not only what the
/// dependency resolved to but which record said so, and an offline
/// rebuild can put that record back beside everything else.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Locked {
    pub trust: Trust,
    /// The package record this dependency resolved from, pinned.
    pub record: Pin,
    pub lock: Box<Lock>,
}

/// The resolved form of a [`Package`]: every reference it named, reduced
/// to the digest the bytes on this machine actually have.
///
/// A lock is what a run holds for its whole lifetime. Nothing in one is
/// mutable and nothing is assumed — every pin was verified against disk
/// when the lock resolved, and every dependency carries the trust it was
/// given rather than the trust it asked for.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Lock {
    pub v: u32,
    /// The package's slug, publisher, and provenance, carried through.
    pub slug: String,
    pub publisher: String,
    pub provenance: String,
    /// The program, pinned.
    pub program: Pin,
    /// The named components, pinned, keyed by the name each resolved
    /// under.
    #[serde(default)]
    pub questions: BTreeMap<String, Pin>,
    #[serde(default)]
    pub sources: BTreeMap<String, Pin>,
    #[serde(default)]
    pub policies: BTreeMap<String, Pin>,
    #[serde(default)]
    pub capabilities: BTreeMap<String, Pin>,
    /// The packages this one needed, each resolved by the same rules.
    #[serde(default)]
    pub dependencies: BTreeMap<String, Locked>,
}

impl Lock {
    /// This lock's digest: the whole resolved record, canonically.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!(self))
    }

    /// Every pinned reference, flattened to `component -> digest`.
    ///
    /// A dependency appears once, as `packages/<slug>` pinned to the
    /// digest of its resolved entry — trust included — so any movement
    /// inside it, or in what the resolving package said about it, moves
    /// the pin a diff reports.
    #[must_use]
    pub fn pins(&self) -> BTreeMap<String, String> {
        let mut pins = BTreeMap::new();
        pins.insert(
            format!("programs/{}", self.slug),
            self.program.digest.clone(),
        );
        for (dir, list) in [
            ("questions", &self.questions),
            ("sources", &self.sources),
            ("policies", &self.policies),
            ("capabilities", &self.capabilities),
        ] {
            for (name, pin) in list {
                pins.insert(format!("{dir}/{name}"), pin.digest.clone());
            }
        }
        for (slug, locked) in &self.dependencies {
            pins.insert(format!("packages/{slug}"), atif::digest(&json!(locked)));
        }
        pins
    }

    /// What moved between `older` and this lock, in component order.
    ///
    /// The report an operator reads before accepting an update: every
    /// reference the two locks pin to different bytes, every one the
    /// newer lock added, and every one it dropped.
    #[must_use]
    pub fn diff(&self, older: &Lock) -> Vec<Change> {
        let (now, was) = (self.pins(), older.pins());
        let mut changes = Vec::new();
        for (component, digest) in &now {
            match was.get(component) {
                None => changes.push(Change::Added {
                    component: component.clone(),
                    digest: digest.clone(),
                }),
                Some(old) if old != digest => changes.push(Change::Repinned {
                    component: component.clone(),
                    was: old.clone(),
                    now: digest.clone(),
                }),
                _ => {}
            }
        }
        for (component, digest) in &was {
            if !now.contains_key(component) {
                changes.push(Change::Removed {
                    component: component.clone(),
                    digest: digest.clone(),
                });
            }
        }
        changes
    }
}

/// One pinned reference's movement between two locks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The newer lock pins a reference the older did not hold.
    Added { component: String, digest: String },
    /// The older lock pinned a reference the newer dropped.
    Removed { component: String, digest: String },
    /// Both locks pin the reference, to different bytes.
    Repinned {
        component: String,
        was: String,
        now: String,
    },
}

/// Why a package did not resolve to a lock.
///
/// Every variant names what it refused, because a resolution that
/// failed partway is a wrong lock, not a partial one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The record itself is not a package this host reads.
    Record { source: String, reason: String },
    /// A name the package stated is answered by nothing on this machine.
    Unresolved { component: String },
    /// The bytes on disk do not produce the digest the package stated.
    /// Refused rather than repaired: the record is the authority on what
    /// it meant, the disk is the authority on what is here, and the two
    /// disagreeing is the finding.
    Digest {
        component: String,
        stated: String,
        found: String,
    },
    /// The dependencies reach a package that is already being resolved.
    Cyclic { path: String },
    /// A compatibility requirement the host does not satisfy — including
    /// one naming something this host cannot check at all.
    Incompatible {
        component: String,
        requirement: String,
    },
    /// A publisher whose approval was withdrawn. The bytes are not pinned.
    Revoked { publisher: String },
    /// A run already holds a lock. A different lock waits until the run ends.
    Pinned { held: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Record { source, reason } => write!(f, "{source}: {reason}"),
            Refusal::Unresolved { component } => {
                write!(f, "{component}: nothing on this machine answers it")
            }
            Refusal::Digest {
                component,
                stated,
                found,
            } => write!(
                f,
                "{component}: states {stated}, and the bytes here digest to {found}"
            ),
            Refusal::Cyclic { path } => write!(f, "dependency cycle: {path}"),
            Refusal::Incompatible {
                component,
                requirement,
            } => write!(
                f,
                "{component}: requires {requirement}, which this host does not satisfy"
            ),
            Refusal::Revoked { publisher } => {
                write!(f, "publisher {publisher} is revoked")
            }
            Refusal::Pinned { held } => {
                write!(f, "a run holds lock {held}")
            }
        }
    }
}

/// The lock one run holds from admission until it finishes.
///
/// An update proposed while the run is alive does not replace it.
/// Rollback names an earlier lock, and only after the run has finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldLock {
    digest: String,
    finished: bool,
}

impl HeldLock {
    /// Hold `lock` for a run that has just been admitted.
    #[must_use]
    pub fn open(lock: &Lock) -> Self {
        Self {
            digest: lock.digest(),
            finished: false,
        }
    }

    /// Whether `proposed` may replace the held lock.
    ///
    /// The same digest is the lock the run already has. A different
    /// digest is refused until [`Self::finish`].
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Pinned`] while the run is alive and `proposed`
    /// is a different lock.
    pub fn consider(&self, proposed: &Lock) -> Result<(), Refusal> {
        if self.finished || proposed.digest() == self.digest {
            return Ok(());
        }
        Err(Refusal::Pinned {
            held: self.digest.clone(),
        })
    }

    /// The run has ended. A later rollback may name another lock.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// Return `previous` as the lock to restore.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Pinned`] while the run is alive, and
    /// [`Refusal::Incompatible`] when `previous` is the lock already held.
    pub fn rollback<'a>(&self, previous: &'a Lock) -> Result<&'a Lock, Refusal> {
        if !self.finished {
            return Err(Refusal::Pinned {
                held: self.digest.clone(),
            });
        }
        if previous.digest() == self.digest {
            return Err(Refusal::Incompatible {
                component: "lock".to_string(),
                requirement: "rollback must name an earlier lock".to_string(),
            });
        }
        Ok(previous)
    }
}

fn leaks_host(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("nsec1")
        || lower.contains("oak_")
        || lower.contains("api_key")
        || lower.contains("begin openssh")
        || lower.contains("begin private key")
        || value
            .split_whitespace()
            .any(|word| word.starts_with('/') || word.starts_with("~/"))
}

impl std::error::Error for Refusal {}

/// Whether `stated` reads as a digest this module writes: 64 lowercase
/// hexadecimal characters.
fn is_digest(stated: &str) -> bool {
    stated.len() == 64
        && stated
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Whether one stated reference is well-formed: a name in the
/// component's own grammar and a stated digest in this module's.
fn check(reference: &Reference, what: &str, grammar: fn(&str) -> bool) -> Result<(), String> {
    if !grammar(&reference.name) {
        return Err(format!(
            "{what} names {:?}, which is not an identifier it resolves",
            reference.name
        ));
    }
    if !is_digest(&reference.digest) {
        return Err(format!(
            "{what} {:?} states {:?}, which is not a digest",
            reference.name, reference.digest
        ));
    }
    Ok(())
}

/// The file under `dir` whose record claims `name` under `field`, if one
/// parses to it.
///
/// Identity comes from inside the file rather than the file's name: a
/// question set is `openagents.independence.v2` whichever file holds it,
/// and a file that does not parse or claims another name is simply not
/// the answer.
/// The name a registry file claims.
///
/// A NIP-CAP file names itself by the component of `definition.id`. A root
/// `slug` on that file is the earlier draft and is not the identity.
fn claimed_name(value: &Value, field: &str) -> Option<String> {
    if field == "slug"
        && let Some(id) = value
            .get("definition")
            .and_then(|definition| definition.get("id"))
            .and_then(Value::as_str)
    {
        return id.rsplit('/').next().map(str::to_string);
    }
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn find(dir: &Path, field: &str, name: &str) -> Option<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths.into_iter().find(|path| {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| claimed_name(&value, field))
            .is_some_and(|claimed| claimed == name)
    })
}

/// The pin one named component resolves to, or the refusal it earns.
fn pin(root: &Path, dir: &str, field: &str, reference: &Reference) -> Result<Pin, Refusal> {
    let component = format!("{dir}/{}", reference.name);
    let Some(path) = find(&root.join(dir), field, &reference.name) else {
        return Err(Refusal::Unresolved { component });
    };
    let bytes = std::fs::read_to_string(&path).map_err(|_| Refusal::Unresolved {
        component: component.clone(),
    })?;
    let found = digest(&bytes);
    if found != reference.digest {
        return Err(Refusal::Digest {
            component,
            stated: reference.digest.clone(),
            found,
        });
    }
    Ok(Pin {
        digest: found,
        found: path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string(),
    })
}

/// The pins a list of references resolves to, keyed by name.
fn pins(
    root: &Path,
    dir: &str,
    field: &str,
    references: &[Reference],
) -> Result<BTreeMap<String, Pin>, Refusal> {
    references
        .iter()
        .map(|reference| Ok((reference.name.clone(), pin(root, dir, field, reference)?)))
        .collect()
}

/// Whether the host satisfies every compatibility requirement the
/// package states.
fn compatible(package: &Package) -> Result<(), Refusal> {
    for (name, requirement) in &package.compatibility {
        let holds = match name.as_str() {
            "coder" => satisfies(env!("CARGO_PKG_VERSION"), requirement),
            _ => false,
        };
        if !holds {
            return Err(Refusal::Incompatible {
                component: package.slug.clone(),
                requirement: format!("{name} {requirement}"),
            });
        }
    }
    Ok(())
}

/// Whether a dotted version label meets a requirement like `>=1.2`.
///
/// A bare label means exactly it: `1.2` is `1.2.0` and no other, and an
/// empty or unparseable label satisfies nothing — a requirement the
/// record cannot state clearly is one the host cannot satisfy either.
fn satisfies(version: &str, requirement: &str) -> bool {
    let (op, want) = ["<=", ">=", "=", "<", ">"]
        .iter()
        .find_map(|op| requirement.strip_prefix(op).map(|rest| (*op, rest)))
        .unwrap_or(("=", requirement));
    let (Some(have), Some(need)) = (dots(version), dots(want)) else {
        return false;
    };
    match op {
        ">=" => have >= need,
        "<=" => have <= need,
        ">" => have > need,
        "<" => have < need,
        _ => have == need,
    }
}

/// A dotted label as numbers, padded: `1.2` reads as `1.2.0`.
fn dots(label: &str) -> Option<Vec<u64>> {
    let mut parts: Vec<u64> = label
        .trim()
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}

/// One dependency edge resolved: the package record loaded, its stated
/// publisher and version checked, then resolved like the root.
fn depend(
    root: &Path,
    package: &Package,
    dep: &Dependency,
    visiting: &mut Vec<String>,
    revoked: &BTreeSet<String>,
) -> Result<Locked, Refusal> {
    let component = format!("packages/{}", dep.package);
    let Some(path) = find(&root.join("packages"), "slug", &dep.package) else {
        return Err(Refusal::Unresolved { component });
    };
    let bytes = std::fs::read_to_string(&path).map_err(|_| Refusal::Unresolved {
        component: component.clone(),
    })?;
    let record_digest = digest(&bytes);
    if let Some(stated) = &dep.digest
        && record_digest != *stated
    {
        return Err(Refusal::Digest {
            component,
            stated: stated.clone(),
            found: record_digest.clone(),
        });
    }
    let needed: Package = serde_json::from_str(&bytes).map_err(|reason| Refusal::Record {
        source: component.clone(),
        reason: reason.to_string(),
    })?;
    needed.validate().map_err(|reason| Refusal::Record {
        source: component.clone(),
        reason,
    })?;
    if let Some(publisher) = &dep.publisher
        && needed.publisher != *publisher
    {
        return Err(Refusal::Unresolved {
            component: format!("{component} published by {publisher}"),
        });
    }
    if let Some(requirement) = &dep.version
        && !satisfies(&needed.version, requirement)
    {
        return Err(Refusal::Incompatible {
            component,
            requirement: requirement.clone(),
        });
    }
    if !needed.publisher.is_empty() && revoked.contains(&needed.publisher) {
        return Err(Refusal::Revoked {
            publisher: needed.publisher.clone(),
        });
    }
    let lock = resolve_tree(root, &needed, visiting, revoked)?;
    let trust = if package
        .trusted_publishers
        .iter()
        .any(|trusted| trusted == &needed.publisher)
    {
        Trust::Trusted
    } else {
        Trust::Untrusted
    };
    Ok(Locked {
        trust,
        record: Pin {
            digest: record_digest,
            found: path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string(),
        },
        lock: Box::new(lock),
    })
}

/// The resolution [`Package::resolve`] runs, with `visiting` holding the
/// dependency chain already being resolved — the only state the
/// recursion needs, and the only place a cycle can hide.
fn resolve_tree(
    root: &Path,
    package: &Package,
    visiting: &mut Vec<String>,
    revoked: &BTreeSet<String>,
) -> Result<Lock, Refusal> {
    package.validate().map_err(|reason| Refusal::Record {
        source: package.slug.clone(),
        reason,
    })?;
    if !package.publisher.is_empty() && revoked.contains(&package.publisher) {
        return Err(Refusal::Revoked {
            publisher: package.publisher.clone(),
        });
    }
    compatible(package)?;
    if visiting.contains(&package.slug) {
        let path = visiting
            .iter()
            .cloned()
            .chain([package.slug.clone()])
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(Refusal::Cyclic { path });
    }
    visiting.push(package.slug.clone());
    let attempted = (|| {
        let program = pin(root, "programs", "slug", &package.program)?;
        let questions = pins(root, "questions", "id", &package.questions)?;
        let sources = pins(root, "sources", "slug", &package.sources)?;
        let policies = pins(root, "policies", "slug", &package.policies)?;
        let capabilities = pins(root, "capabilities", "slug", &package.capabilities)?;
        let mut dependencies = BTreeMap::new();
        for dep in &package.requires {
            let locked = depend(root, package, dep, visiting, revoked)?;
            dependencies.insert(dep.package.clone(), locked);
        }
        Ok(Lock {
            v: LOCK_VERSION,
            slug: package.slug.clone(),
            publisher: package.publisher.clone(),
            provenance: package.provenance.clone(),
            program,
            questions,
            sources,
            policies,
            capabilities,
            dependencies,
        })
    })();
    visiting.pop();
    attempted
}

/// Apply one extension install transition.
///
/// The resolver does not run a build, a probe, or inference. A cleanup
/// that fails stays a tombstone, and a tombstone cannot become an
/// installation again.
///
/// # Errors
///
/// Returns the protocol refusal for an illegal transition.
pub fn extension_transition(
    state: nostr::ext::Install,
    step: nostr::ext::InstallStep,
) -> Result<nostr::ext::Install, nostr::contracts::ContractError> {
    nostr::ext::transition(&state, step)
}

/// The lock an in-flight run keeps. A newer release does not replace it.
#[must_use]
pub fn extension_active_pin<'a>(active: &'a str, proposed: &'a str) -> &'a str {
    nostr::ext::preserve_active_pin(active, proposed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A reference to a real file, stating the digest its bytes have.
    fn reference(root: &Path, rel: &str, name: &str) -> Reference {
        let bytes = std::fs::read_to_string(root.join(rel)).unwrap();
        Reference {
            name: name.to_string(),
            digest: digest(&bytes),
        }
    }

    /// A minimal package under `slug`, pinning the program `main`.
    fn package(slug: &str, program: &Reference) -> Package {
        Package {
            v: PACKAGE_VERSION,
            slug: slug.to_string(),
            name: String::new(),
            summary: String::new(),
            version: "1.0.0".to_string(),
            publisher: "openagents".to_string(),
            provenance: "local file".to_string(),
            trusted_publishers: Vec::new(),
            program: program.clone(),
            questions: Vec::new(),
            sources: Vec::new(),
            policies: Vec::new(),
            capabilities: Vec::new(),
            requires: Vec::new(),
            compatibility: BTreeMap::new(),
        }
    }

    /// Writes `body` to `root/rel` and returns the digest it now has.
    fn stage(root: &Path, rel: &str, body: &str) -> String {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, body).unwrap();
        digest(body)
    }

    /// A minimal root: one program file, and `packages/` for whatever
    /// the test resolves beside it.
    fn staged() -> (tempfile::TempDir, Reference) {
        let dir = tempfile::tempdir().unwrap();
        let pinned = stage(
            dir.path(),
            "programs/main.json",
            r#"{"v":1,"slug":"main","steps":[{"name":"one","kind":"query","bounds":{}}]}"#,
        );
        let program = Reference {
            name: "main".to_string(),
            digest: pinned,
        };
        (dir, program)
    }

    /// Writes a package record under `root/packages/` and returns it.
    fn stage_package(root: &Path, package: &Package) {
        stage(
            root,
            &format!("packages/{}.json", package.slug),
            &serde_json::to_string(package).unwrap(),
        );
    }

    #[test]
    fn a_package_naming_real_files_resolves_to_a_lock_with_verified_digests() {
        let root = repository();
        let mut package = package(
            "burn-down",
            &reference(&root, "programs/burn-down.json", "burn-down"),
        );
        package.provenance = "repository".to_string();
        package.questions = vec![
            reference(
                &root,
                "questions/independence-v2.json",
                "openagents.independence.v2",
            ),
            reference(
                &root,
                "questions/completion.json",
                "openagents.completion.v1",
            ),
        ];
        package.sources = vec![reference(&root, "sources/work-list.json", "work-list")];
        package.capabilities = vec![reference(
            &root,
            "capabilities/devin-local.json",
            "devin-local",
        )];

        let lock = Package::resolve(&root, &package).unwrap();
        assert_eq!(lock.v, LOCK_VERSION);
        assert_eq!(lock.slug, "burn-down");
        assert_eq!(lock.publisher, "openagents");
        assert_eq!(lock.provenance, "repository");
        assert_eq!(lock.program.found, "programs/burn-down.json");
        assert_eq!(
            lock.program.digest, package.program.digest,
            "the pin is the digest the file's bytes verified to"
        );
        assert_eq!(
            lock.questions.keys().collect::<Vec<_>>(),
            ["openagents.completion.v1", "openagents.independence.v2"]
        );
        assert_eq!(lock.sources["work-list"].found, "sources/work-list.json");
        assert_eq!(
            lock.capabilities["devin-local"].found,
            "capabilities/devin-local.json"
        );
        assert!(lock.policies.is_empty());
        assert!(lock.dependencies.is_empty());
        assert!(!lock.digest().is_empty());
    }

    #[test]
    fn a_stated_digest_the_bytes_do_not_produce_is_refused_not_repaired() {
        let root = repository();
        let mut package = package(
            "burn-down",
            &reference(&root, "programs/burn-down.json", "burn-down"),
        );
        package.program.digest = "0".repeat(64);

        let refusal = Package::resolve(&root, &package).unwrap_err();
        let Refusal::Digest {
            component,
            stated,
            found,
        } = refusal
        else {
            panic!("expected a digest refusal, got {refusal:?}");
        };
        assert_eq!(component, "programs/burn-down");
        assert_eq!(stated, "0".repeat(64));
        assert_eq!(
            found,
            digest(&std::fs::read_to_string(root.join("programs/burn-down.json")).unwrap())
        );
    }

    #[test]
    fn a_dependency_nothing_answers_is_unresolved() {
        let (dir, program) = staged();
        let mut package = package("root-pkg", &program);
        package.requires = vec![Dependency {
            package: "ghost".to_string(),
            publisher: None,
            version: None,
            digest: None,
        }];

        let refusal = Package::resolve(dir.path(), &package).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Unresolved {
                component: "packages/ghost".to_string()
            }
        );
    }

    #[test]
    fn a_dependency_chain_that_returns_to_itself_is_cyclic() {
        let (dir, program) = staged();
        let mut a = package("a", &program);
        a.requires = vec![Dependency {
            package: "b".to_string(),
            publisher: None,
            version: None,
            digest: None,
        }];
        let mut b = package("b", &program);
        b.requires = vec![Dependency {
            package: "a".to_string(),
            publisher: None,
            version: None,
            digest: None,
        }];
        stage_package(dir.path(), &a);
        stage_package(dir.path(), &b);

        let refusal = Package::resolve(dir.path(), &a).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Cyclic {
                path: "a -> b -> a".to_string()
            }
        );
    }

    #[test]
    fn a_dependency_under_an_unknown_publisher_is_marked_not_assumed() {
        let (dir, program) = staged();
        let mut friend = package("friend-pkg", &program);
        friend.publisher = "friend".to_string();
        let mut stranger = package("stranger-pkg", &program);
        stranger.publisher = "stranger".to_string();
        stage_package(dir.path(), &friend);
        stage_package(dir.path(), &stranger);

        let mut package = package("root-pkg", &program);
        package.trusted_publishers = vec!["friend".to_string()];
        package.requires = vec![
            Dependency {
                package: "friend-pkg".to_string(),
                publisher: None,
                version: None,
                digest: None,
            },
            Dependency {
                package: "stranger-pkg".to_string(),
                publisher: None,
                version: None,
                digest: None,
            },
        ];

        let lock = Package::resolve(dir.path(), &package).unwrap();
        assert_eq!(lock.dependencies["friend-pkg"].trust, Trust::Trusted);
        assert_eq!(lock.dependencies["stranger-pkg"].trust, Trust::Untrusted);
        assert_eq!(
            lock.pins().keys().collect::<Vec<_>>(),
            [
                "packages/friend-pkg",
                "packages/stranger-pkg",
                "programs/root-pkg"
            ]
        );
    }

    #[test]
    fn a_dependency_whose_version_does_not_satisfy_is_incompatible() {
        let (dir, program) = staged();
        let child = package("child", &program);
        stage_package(dir.path(), &child);

        let mut package = package("root-pkg", &program);
        package.requires = vec![Dependency {
            package: "child".to_string(),
            publisher: None,
            version: Some(">=2.0.0".to_string()),
            digest: None,
        }];

        let refusal = Package::resolve(dir.path(), &package).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Incompatible {
                component: "packages/child".to_string(),
                requirement: ">=2.0.0".to_string()
            }
        );
    }

    #[test]
    fn a_lock_diff_names_every_pin_that_moved() {
        let (dir, program) = staged();
        let root = dir.path();
        let question = stage(
            root,
            "questions/openagents.test.v1.json",
            r#"{"v":1,"id":"openagents.test.v1","questions":{"q":{"type":"noul","instructions":"First wording."}}}"#,
        );
        let capability = stage(
            root,
            "capabilities/tool.json",
            r#"{"v":1,"slug":"tool","transport":"subprocess"}"#,
        );
        let capability_ref = Reference {
            name: "tool".to_string(),
            digest: capability,
        };

        let mut before = package("main-pkg", &program);
        before.questions = vec![Reference {
            name: "openagents.test.v1".to_string(),
            digest: question.clone(),
        }];
        before.capabilities = vec![capability_ref.clone()];
        let older = Package::resolve(root, &before).unwrap();

        let reworded = stage(
            root,
            "questions/openagents.test.v1.json",
            r#"{"v":1,"id":"openagents.test.v1","questions":{"q":{"type":"noul","instructions":"Second wording."}}}"#,
        );
        let source = stage(
            root,
            "sources/list.json",
            r#"{"v":1,"slug":"list","from":{"file":{"path":"list.json"}},"order":"id"}"#,
        );
        let mut after = before.clone();
        after.questions = vec![Reference {
            name: "openagents.test.v1".to_string(),
            digest: reworded.clone(),
        }];
        after.sources = vec![Reference {
            name: "list".to_string(),
            digest: source.clone(),
        }];
        after.capabilities = Vec::new();
        let newer = Package::resolve(root, &after).unwrap();

        let changes = newer.diff(&older);
        assert_eq!(
            changes,
            vec![
                Change::Repinned {
                    component: "questions/openagents.test.v1".to_string(),
                    was: question,
                    now: reworded,
                },
                Change::Added {
                    component: "sources/list".to_string(),
                    digest: source,
                },
                Change::Removed {
                    component: "capabilities/tool".to_string(),
                    digest: capability_ref.digest,
                },
            ]
        );
        assert!(
            older.diff(&older).is_empty(),
            "a lock does not differ from itself"
        );
    }

    #[test]
    fn resolution_is_deterministic() {
        let root = repository();
        let mut package = package(
            "burn-down",
            &reference(&root, "programs/burn-down.json", "burn-down"),
        );
        package.questions = vec![
            reference(
                &root,
                "questions/independence-v2.json",
                "openagents.independence.v2",
            ),
            reference(
                &root,
                "questions/completion.json",
                "openagents.completion.v1",
            ),
        ];
        package.sources = vec![reference(&root, "sources/work-list.json", "work-list")];

        let first = Package::resolve(&root, &package).unwrap();
        let second = Package::resolve(&root, &package).unwrap();
        assert_eq!(first.digest(), second.digest());
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
    }

    #[test]
    fn an_extension_cleanup_failure_stays_a_tombstone_and_the_active_pin_stays() {
        use nostr::ext::{Install, InstallStep};

        let installed = extension_transition(
            Install::Absent,
            InstallStep::Stage {
                lock: "lock-a".into(),
                verified: true,
            },
        )
        .unwrap();
        let installed = extension_transition(installed, InstallStep::Commit).unwrap();
        assert_eq!(
            installed,
            Install::Installed {
                lock: "lock-a".into()
            }
        );
        let disabled = extension_transition(installed, InstallStep::BeginUninstall).unwrap();
        let tombstone =
            extension_transition(disabled, InstallStep::Cleanup { succeeded: false }).unwrap();
        assert_eq!(
            tombstone,
            Install::Tombstone {
                lock: "lock-a".into()
            }
        );
        assert!(extension_transition(tombstone, InstallStep::Reactivate).is_err());
        assert_eq!(extension_active_pin("lock-a", "lock-b"), "lock-a");
    }

    #[test]
    fn a_revoked_publisher_is_refused_even_when_the_package_trusted_it() {
        let (dir, program) = staged();
        let mut friend = package("friend-pkg", &program);
        friend.publisher = "friend".to_string();
        stage_package(dir.path(), &friend);

        let mut package = package("root-pkg", &program);
        package.trusted_publishers = vec!["friend".to_string()];
        package.requires = vec![Dependency {
            package: "friend-pkg".to_string(),
            publisher: Some("friend".to_string()),
            version: None,
            digest: None,
        }];
        let mut revoked = BTreeSet::new();
        revoked.insert("friend".to_string());

        let refusal = Package::resolve_against(dir.path(), &package, &revoked).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Revoked {
                publisher: "friend".to_string()
            }
        );
        assert!(Package::resolve(dir.path(), &package).is_ok());

        let mut root_revoked = BTreeSet::new();
        root_revoked.insert(package.publisher.clone());
        let root = Package::resolve_against(dir.path(), &package, &root_revoked).unwrap_err();
        assert_eq!(
            root,
            Refusal::Revoked {
                publisher: package.publisher.clone()
            }
        );
    }

    #[test]
    fn a_replaced_file_is_refused_as_a_stale_advertisement() {
        let (dir, program) = staged();
        let resolved = Package::resolve(dir.path(), &package("root-pkg", &program)).unwrap();
        stage(
            dir.path(),
            "programs/main.json",
            r#"{"v":1,"slug":"main","steps":[{"name":"one","kind":"query","bounds":{"limit":1}}]}"#,
        );
        let mut stale = package("root-pkg", &program);
        stale.program.digest = resolved.program.digest.clone();
        let refusal = Package::resolve(dir.path(), &stale).unwrap_err();
        assert!(matches!(refusal, Refusal::Digest { .. }), "{refusal}");
    }

    #[test]
    fn an_absent_remote_name_stays_unresolved_without_a_fetch() {
        let (dir, program) = staged();
        let mut package = package("root-pkg", &program);
        package.provenance = "wss://relay.example/packages/ghost".to_string();
        package.requires = vec![Dependency {
            package: "ghost".to_string(),
            publisher: None,
            version: None,
            digest: None,
        }];
        let refusal = Package::resolve(dir.path(), &package).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Unresolved {
                component: "packages/ghost".to_string()
            }
        );
    }

    #[test]
    fn a_run_holds_its_lock_until_it_finishes_and_then_can_roll_back() {
        let (dir, program) = staged();
        let held_lock = Package::resolve(dir.path(), &package("root-pkg", &program)).unwrap();
        let mut other = package("other-pkg", &program);
        other.slug = "other-pkg".to_string();
        let proposed = Package::resolve(dir.path(), &other).unwrap();
        let mut held = HeldLock::open(&held_lock);
        assert!(held.consider(&held_lock).is_ok());
        assert_eq!(
            held.consider(&proposed).unwrap_err(),
            Refusal::Pinned {
                held: held_lock.digest()
            }
        );
        assert!(held.rollback(&proposed).is_err());
        held.finish();
        assert_eq!(
            held.rollback(&proposed).unwrap().digest(),
            proposed.digest()
        );
        assert!(held.rollback(&held_lock).is_err());
    }

    #[test]
    fn a_package_cannot_carry_a_secret_or_an_approval() {
        let (dir, program) = staged();
        let mut record = serde_json::to_value(package("root-pkg", &program)).unwrap();
        record["secret"] = json!("oak_live_key");
        record["probe"] = json!("approved");
        let path = dir.path().join("packages/smuggled.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, record.to_string()).unwrap();
        let error = Package::load(&path).unwrap_err();
        assert!(error.contains("unknown field"), "{error}");

        let mut host = package("root-pkg", &program);
        host.summary = "key at /Users/me/.ssh/id_rsa".to_string();
        let error = host.validate().unwrap_err();
        assert!(error.contains("host path or a secret"), "{error}");

        host.summary.clear();
        host.publisher = "nsec1notakey".to_string();
        let error = host.validate().unwrap_err();
        assert!(error.contains("publisher"), "{error}");

        host.publisher = "openagents".to_string();
        host.trusted_publishers = vec!["~/secrets/token".to_string()];
        let error = host.validate().unwrap_err();
        assert!(error.contains("trusted_publishers"), "{error}");
    }
}
