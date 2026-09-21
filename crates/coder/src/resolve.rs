//! Resolution: the host's answer to what a lock asks for.
//!
//! A [`Lock`] is a claim about what a run wants: the program, question
//! sets, sources, policies, and capability manifests it names, each with
//! the digest it was pinned at or the version range it accepts.
//! Resolution is the host answering that claim against what it already
//! holds — local first, pinned by digest, and honest about what it could
//! not find.
//!
//! # Local first
//!
//! [`Resolve::plan`] reads local directories and nothing else. There is
//! no socket, no fetch, and no remote catalog: a name this machine does
//! not hold is [`Refusal::Unresolved`], and it stays unresolved rather
//! than implying retrieval. Remote discovery is a separate layer that
//! produces candidates for a new lock; it never mutates an answer this
//! module already gave.
//!
//! # Pinned by digest
//!
//! A stated digest is binding: the file whose bytes produce it wins, and
//! when nothing does, the reference is refused — [`Refusal::Digest`]
//! when files claim the name but disagree, [`Refusal::Unresolved`] when
//! nothing claims it. Without a pin the highest local version that
//! satisfies the stated range serves. Either way the answer says how it
//! matched — [`Match::Exact`] or [`Match::Compatible`] — so nothing
//! resolves silently.
//!
//! # Dependencies are checked, not assumed
//!
//! Every resolved reference records the dependency edges declared for
//! it, and the plan walks them transitively. A walk that comes back to
//! itself is [`Refusal::Cyclic`]; a transitive dependency nothing local
//! answers, or whose answer fails the edge that required it, is refused
//! with the parent named. Two edges that reach one reference resolve to
//! one node — a diamond whose sides agree on the digest is not a
//! conflict.
//!
//! # A plan grants nothing
//!
//! Installing code is not authorizing its effects. [`Plan::grants`] is
//! always empty: probe approval, credentials, disclosure, and access are
//! the host's separate decision, made by other modules and other
//! authorities.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::capability::is_slug;
use crate::package::digest;
use crate::questions::is_question_id;

/// The lock record version this reads.
pub const LOCK_VERSION: u32 = 1;

/// A lock: the references a run wants resolved, with what each must
/// satisfy.
///
/// A lock is a claim, not an answer. Each entry names one reference in
/// `dir/name` form — `programs/burn-down`,
/// `questions/openagents.independence.v2` — and may pin the digest its
/// bytes must produce, state the version range it accepts, and declare
/// the other references it needs resolved first.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Lock {
    pub v: u32,
    /// What the run wants resolved. Order is the author's; the answer
    /// does not depend on it.
    #[serde(default)]
    pub entries: Vec<Entry>,
}

impl Lock {
    /// Whether this lock is one this host resolves.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not: a `v` it does not know, a
    /// reference outside `dir/name` form or its component's grammar, a
    /// reference named twice, a malformed stated digest, or an empty
    /// version requirement.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.v != LOCK_VERSION {
            return Err(Refusal::Record {
                source: "the lock".to_string(),
                reason: format!(
                    "body version is {}, this version reads {LOCK_VERSION}",
                    self.v
                ),
            });
        }
        let mut seen = Vec::new();
        for entry in &self.entries {
            well_formed(&entry.reference, &entry.reference)?;
            if seen.contains(&entry.reference) {
                return Err(Refusal::Record {
                    source: entry.reference.clone(),
                    reason: "is named twice".to_string(),
                });
            }
            seen.push(entry.reference.clone());
            stated(&entry.reference, &entry.digest, &entry.version)?;
            for edge in &entry.requires {
                edge_shape(&entry.reference, edge)?;
            }
        }
        Ok(())
    }
}

/// One reference a lock wants resolved.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Entry {
    /// The reference, in `dir/name` form: `programs/`, `questions/`,
    /// `sources/`, `policies/`, or `capabilities/` plus the name that
    /// component's own registry answers to.
    pub reference: String,
    /// The digest the file's bytes must produce, when the entry pins
    /// one. Stated, it is binding: bytes that do not produce it are
    /// refused, not used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// The version requirement the resolved record's `version` label
    /// must satisfy — `>=1.2`, `=1.2.0`, and their kin. A label is a
    /// human version, never an identity: the pin underneath is the
    /// digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The other references this one needs resolved first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Edge>,
}

/// One declared dependency edge: a reference, and what its resolution
/// must satisfy.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Edge {
    /// The required reference, in the same `dir/name` form.
    pub reference: String,
    /// The digest the dependency's bytes must produce, when the edge
    /// pins one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// The version requirement the dependency's record must satisfy,
    /// when the edge states one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The resolved answer to a lock: every reference reduced to the file,
/// the digest, and the match kind it actually got.
///
/// A plan is what a caller pins for a run's lifetime. Nothing in it is
/// guessed: each entry records how it matched, and anything the machine
/// could not resolve refused the whole plan rather than leaving a hole
/// in it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Plan {
    /// Every resolved reference, keyed by its `dir/name` form. The map's
    /// order is canonical, so two plans of the same world compare equal.
    pub resolved: BTreeMap<String, Resolved>,
    /// Always empty: a plan grants nothing. Installing code is not
    /// authorizing its effects — probe approval, credentials,
    /// disclosure, and access are the host's separate decision, made
    /// elsewhere.
    pub grants: Vec<Grant>,
}

impl Plan {
    /// This plan's digest: the whole resolved answer, canonically.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!(self))
    }
}

/// What a resolved package could hand a run — if resolution granted
/// anything, which it does not.
///
/// The type has no values because a plan carries no grants: installing
/// code is not authorizing its effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Grant {}

/// One reference, resolved: where it was found, what its bytes
/// produced, and how it matched.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Resolved {
    /// The local file the bytes came from.
    pub path: PathBuf,
    /// The digest the bytes actually produced — recomputed, never the
    /// record's word for them.
    pub digest: String,
    /// The version label the resolved record claims, or empty when it
    /// claims none. A label is not an identity; the digest above is.
    pub version: String,
    /// How the match was made: the pinned digest itself, or the highest
    /// compatible local version.
    pub matched: Match,
    /// The dependency edges walked from this reference — the ones the
    /// lock declared for it, plus any the resolved record declares for
    /// itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Edge>,
}

/// How a resolved reference matched what the lock asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Match {
    /// The file's bytes produced the digest the lock pinned.
    Exact,
    /// No pin governed, and this is the highest local version that
    /// satisfies the stated range.
    Compatible,
}

/// Why a lock did not resolve to a plan.
///
/// Every variant names what it refused, and a failure reached through a
/// dependency edge names the parent that required it: a resolution that
/// fails partway is a wrong plan, not a partial one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The lock itself is not a record this host reads.
    Record { source: String, reason: String },
    /// Nothing under the local directories answers the reference. It
    /// stays unresolved: no fetch is implied and nothing is guessed.
    Unresolved {
        reference: String,
        /// The reference whose edge asked for this one, when it was
        /// reached through a parent rather than the lock itself.
        required_by: Option<String>,
    },
    /// The bytes on disk do not produce the digest that was stated.
    /// Refused rather than repaired, under the same rule
    /// [`crate::package`] holds: the record is the authority on what it
    /// meant, the disk is the authority on what is here, and the two
    /// disagreeing is the finding.
    Digest {
        reference: String,
        stated: String,
        found: String,
        required_by: Option<String>,
    },
    /// No local record satisfies the version requirement that was
    /// stated.
    Incompatible {
        reference: String,
        requirement: String,
        required_by: Option<String>,
    },
    /// The dependency walk came back to a reference already being
    /// resolved. The path is the cycle, joined by ` -> `.
    Cyclic { path: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Record { source, reason } => write!(f, "{source}: {reason}"),
            Refusal::Unresolved {
                reference,
                required_by,
            } => {
                write!(f, "{reference}: nothing on this machine answers it")?;
                if let Some(parent) = required_by {
                    write!(f, ", which {parent} requires")?;
                }
                Ok(())
            }
            Refusal::Digest {
                reference,
                stated,
                found,
                required_by,
            } => {
                write!(
                    f,
                    "{reference}: states {stated}, and the bytes here digest to {found}"
                )?;
                if let Some(parent) = required_by {
                    write!(f, ", which {parent} requires")?;
                }
                Ok(())
            }
            Refusal::Incompatible {
                reference,
                requirement,
                required_by,
            } => {
                write!(
                    f,
                    "{reference}: requires {requirement}, which nothing local satisfies"
                )?;
                if let Some(parent) = required_by {
                    write!(f, ", which {parent} requires")?;
                }
                Ok(())
            }
            Refusal::Cyclic { path } => write!(f, "dependency cycle: {path}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// One entry's movement between two locks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The newer lock names a reference the older did not.
    Added {
        reference: String,
        digest: Option<String>,
        version: Option<String>,
    },
    /// The older lock named a reference the newer dropped.
    Removed {
        reference: String,
        digest: Option<String>,
        version: Option<String>,
    },
    /// Both name the reference; the pinned digest moved.
    Digest {
        reference: String,
        was: Option<String>,
        now: Option<String>,
    },
    /// Both name the reference; the version requirement moved.
    Version {
        reference: String,
        was: Option<String>,
        now: Option<String>,
    },
}

/// The host that answers a lock's claim.
///
/// `Resolve` holds no state because resolution needs none: the lock is
/// the claim, the directories are the world, and the answer falls out
/// of the two.
pub struct Resolve;

impl Resolve {
    /// Resolves every entry in `lock` against the local directories in
    /// `dirs`, then walks the declared dependency edges.
    ///
    /// `dirs` are searched in order, each holding the component
    /// directories the references name — `programs/`, `questions/`,
    /// `sources/`, `policies/`, `capabilities/` — the same discovery
    /// [`crate::program::Registry`] uses, applied to every component
    /// kind a lock names.
    ///
    /// # Errors
    ///
    /// Returns a typed [`Refusal`]: the lock is malformed, a name
    /// nothing local answers, bytes that do not produce a stated
    /// digest, a version requirement nothing local satisfies, a
    /// dependency cycle, or a transitive dependency a parent required
    /// that fails the same checks.
    pub fn plan(lock: &Lock, dirs: &[PathBuf]) -> Result<Plan, Refusal> {
        lock.validate()?;
        let mut walk = Walk {
            entries: lock
                .entries
                .iter()
                .map(|entry| (entry.reference.as_str(), entry))
                .collect(),
            dirs,
            resolved: BTreeMap::new(),
            visiting: Vec::new(),
            done: BTreeSet::new(),
        };
        for entry in &lock.entries {
            walk.visit(&entry.reference, None)?;
        }
        Ok(Plan {
            resolved: walk.resolved,
            grants: Vec::new(),
        })
    }
}

/// What an update from `older` to `newer` would change, in reference
/// order: every entry the update adds, drops, repins, or re-ranges.
///
/// Pure over the two locks — nothing is resolved and nothing is read.
/// Rollback is `diff` with the locks swapped: every [`Change`] answers
/// in the other direction.
#[must_use]
pub fn diff(older: &Lock, newer: &Lock) -> Vec<Change> {
    let was: BTreeMap<&str, &Entry> = older
        .entries
        .iter()
        .map(|entry| (entry.reference.as_str(), entry))
        .collect();
    let now: BTreeMap<&str, &Entry> = newer
        .entries
        .iter()
        .map(|entry| (entry.reference.as_str(), entry))
        .collect();
    let mut changes = Vec::new();
    for (reference, entry) in &now {
        match was.get(reference) {
            None => changes.push(Change::Added {
                reference: (*reference).to_string(),
                digest: entry.digest.clone(),
                version: entry.version.clone(),
            }),
            Some(old) => {
                if old.digest != entry.digest {
                    changes.push(Change::Digest {
                        reference: (*reference).to_string(),
                        was: old.digest.clone(),
                        now: entry.digest.clone(),
                    });
                }
                if old.version != entry.version {
                    changes.push(Change::Version {
                        reference: (*reference).to_string(),
                        was: old.version.clone(),
                        now: entry.version.clone(),
                    });
                }
            }
        }
    }
    for (reference, entry) in &was {
        if !now.contains_key(reference) {
            changes.push(Change::Removed {
                reference: (*reference).to_string(),
                digest: entry.digest.clone(),
                version: entry.version.clone(),
            });
        }
    }
    changes
}

/// One local file claiming the name: the digest its bytes produce, the
/// version label its record claims, and the dependency edges it
/// declares.
struct Candidate {
    path: PathBuf,
    digest: String,
    version: String,
    requires: Option<Value>,
}

/// The state one plan's dependency walk carries: the lock's entries by
/// reference, the directories being resolved against, what has
/// resolved, the chain already being walked, and what finished.
struct Walk<'a> {
    entries: BTreeMap<&'a str, &'a Entry>,
    dirs: &'a [PathBuf],
    resolved: BTreeMap<String, Resolved>,
    visiting: Vec<String>,
    done: BTreeSet<String>,
}

impl Walk<'_> {
    /// Walks one reference: resolve it, check the edge that reached it,
    /// then walk the edges it declares.
    ///
    /// `visiting` holds the chain already being walked — the only state
    /// the recursion needs, and the only place a cycle can hide. `done`
    /// holds the references already checked, so a diamond walks its
    /// shared side once while a second parent's edge is still checked.
    fn visit(&mut self, reference: &str, incoming: Option<(&str, &Edge)>) -> Result<(), Refusal> {
        if let Some(at) = self.visiting.iter().position(|seen| seen == reference) {
            let path = self.visiting[at..]
                .iter()
                .cloned()
                .chain([reference.to_string()])
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Refusal::Cyclic { path });
        }
        let required_by = incoming.map(|(parent, _)| parent.to_string());
        if !self.resolved.contains_key(reference) {
            let (pin, range) = match self.entries.get(reference) {
                Some(entry) => (entry.digest.as_deref(), entry.version.as_deref()),
                None => incoming
                    .map(|(_, edge)| (edge.digest.as_deref(), edge.version.as_deref()))
                    .unwrap_or((None, None)),
            };
            let mut node = on_disk(reference, pin, range, self.dirs, required_by.as_deref())?;
            if let Some(entry) = self.entries.get(reference) {
                for edge in &entry.requires {
                    if !node.requires.contains(edge) {
                        node.requires.push(edge.clone());
                    }
                }
            }
            self.resolved.insert(reference.to_string(), node);
        }
        let node = &self.resolved[reference];
        if let Some((parent, edge)) = incoming {
            check_edge(reference, parent, edge, node)?;
        }
        if self.done.contains(reference) {
            return Ok(());
        }
        let edges = node.requires.clone();
        self.visiting.push(reference.to_string());
        for edge in &edges {
            self.visit(&edge.reference, Some((reference, edge)))?;
        }
        self.visiting.pop();
        self.done.insert(reference.to_string());
        Ok(())
    }
}

/// Whether the resolved node satisfies the edge that required it.
fn check_edge(reference: &str, parent: &str, edge: &Edge, node: &Resolved) -> Result<(), Refusal> {
    if let Some(stated) = &edge.digest
        && stated != &node.digest
    {
        return Err(Refusal::Digest {
            reference: reference.to_string(),
            stated: stated.clone(),
            found: node.digest.clone(),
            required_by: Some(parent.to_string()),
        });
    }
    if let Some(requirement) = &edge.version
        && !satisfies(&node.version, requirement)
    {
        return Err(Refusal::Incompatible {
            reference: reference.to_string(),
            requirement: requirement.clone(),
            required_by: Some(parent.to_string()),
        });
    }
    Ok(())
}

/// The resolution one reference gets against the local directories.
///
/// The file whose bytes produce the stated digest wins outright, and a
/// stated range still has to hold on the pinned record. Without a pin
/// the highest local version satisfying the stated range serves.
/// Anything else is refused by name: nothing is guessed, and nothing is
/// fetched.
fn on_disk(
    reference: &str,
    pin: Option<&str>,
    range: Option<&str>,
    dirs: &[PathBuf],
    required_by: Option<&str>,
) -> Result<Resolved, Refusal> {
    let Some(component) = component(reference) else {
        return Err(Refusal::Record {
            source: reference.to_string(),
            reason: "is not a `dir/name` reference this host resolves".to_string(),
        });
    };
    let candidates = scan(dirs, component.dir, component.field, component.name);
    let required_by = required_by.map(str::to_string);
    if let Some(stated) = pin {
        let Some(candidate) = candidates.iter().find(|c| c.digest == stated) else {
            return Err(if candidates.is_empty() {
                Refusal::Unresolved {
                    reference: reference.to_string(),
                    required_by,
                }
            } else {
                Refusal::Digest {
                    reference: reference.to_string(),
                    stated: stated.to_string(),
                    found: candidates[0].digest.clone(),
                    required_by,
                }
            });
        };
        if let Some(requirement) = range
            && !satisfies(&candidate.version, requirement)
        {
            return Err(Refusal::Incompatible {
                reference: reference.to_string(),
                requirement: requirement.to_string(),
                required_by,
            });
        }
        return chosen(candidate, Match::Exact);
    }
    let mut pool: Vec<&Candidate> = candidates.iter().collect();
    if let Some(requirement) = range {
        pool.retain(|c| satisfies(&c.version, requirement));
        if pool.is_empty() {
            return Err(if candidates.is_empty() {
                Refusal::Unresolved {
                    reference: reference.to_string(),
                    required_by,
                }
            } else {
                Refusal::Incompatible {
                    reference: reference.to_string(),
                    requirement: requirement.to_string(),
                    required_by,
                }
            });
        }
    } else if pool.is_empty() {
        return Err(Refusal::Unresolved {
            reference: reference.to_string(),
            required_by,
        });
    }
    let mut best = pool[0];
    for candidate in &pool[1..] {
        if version_key(&candidate.version) > version_key(&best.version) {
            best = candidate;
        }
    }
    chosen(best, Match::Compatible)
}

/// The [`Resolved`] a picked candidate becomes: its declared edges read
/// out of its record now that it is the answer.
fn chosen(candidate: &Candidate, matched: Match) -> Result<Resolved, Refusal> {
    let mut requires = Vec::new();
    if let Some(raw) = &candidate.requires {
        requires = serde_json::from_value(raw.clone()).map_err(|e| Refusal::Record {
            source: candidate.path.display().to_string(),
            reason: format!("requires is not a list of dependency edges: {e}"),
        })?;
    }
    Ok(Resolved {
        path: candidate.path.clone(),
        digest: candidate.digest.clone(),
        version: candidate.version.clone(),
        matched,
        requires,
    })
}

/// Every local file whose record claims `name` under `field`, in
/// directory order then path order — the order an earlier definition
/// wins in, same as the registries.
fn scan(dirs: &[PathBuf], dir_name: &str, field: &str, name: &str) -> Vec<Candidate> {
    let mut found = Vec::new();
    for root in dirs {
        let mut paths: Vec<PathBuf> = match std::fs::read_dir(root.join(dir_name)) {
            Ok(read) => read
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .collect(),
            Err(_) => continue,
        };
        paths.sort();
        for path in paths {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if value.get(field).and_then(Value::as_str) != Some(name) {
                continue;
            }
            found.push(Candidate {
                path,
                digest: digest(&text),
                version: value
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                requires: value.get("requires").filter(|raw| !raw.is_null()).cloned(),
            });
        }
    }
    found
}

/// The parts of a `dir/name` reference: the directory under each search
/// root that holds it, the name its record claims, the field the name
/// is claimed under, and the grammar the name itself must pass.
struct Component<'a> {
    dir: &'a str,
    name: &'a str,
    field: &'static str,
    grammar: fn(&str) -> bool,
}

/// The [`Component`] a reference names, when it is in `dir/name` form
/// and `dir` is a component this host resolves.
fn component(reference: &str) -> Option<Component<'_>> {
    let (dir, name) = reference.split_once('/')?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let (field, grammar): (&str, fn(&str) -> bool) = match dir {
        "programs" => ("slug", is_slug),
        "questions" => ("id", is_question_id),
        "sources" | "policies" | "capabilities" => ("slug", is_slug),
        _ => return None,
    };
    Some(Component {
        dir,
        name,
        field,
        grammar,
    })
}

/// Whether `reference` reads as one this host resolves, refusing
/// against `source` when it does not.
fn well_formed(reference: &str, source: &str) -> Result<(), Refusal> {
    let Some(component) = component(reference) else {
        return Err(Refusal::Record {
            source: source.to_string(),
            reason: format!("{reference:?} is not a `dir/name` reference this host resolves"),
        });
    };
    if !(component.grammar)(component.name) {
        return Err(Refusal::Record {
            source: source.to_string(),
            reason: format!(
                "{:?} is not an identifier its registry resolves",
                component.name
            ),
        });
    }
    Ok(())
}

/// Whether a stated digest and version are well-formed where stated.
fn stated(source: &str, digest: &Option<String>, version: &Option<String>) -> Result<(), Refusal> {
    if let Some(stated) = digest
        && !is_digest(stated)
    {
        return Err(Refusal::Record {
            source: source.to_string(),
            reason: format!("states {stated:?}, which is not a digest"),
        });
    }
    if version.as_deref().is_some_and(str::is_empty) {
        return Err(Refusal::Record {
            source: source.to_string(),
            reason: "states an empty version requirement".to_string(),
        });
    }
    Ok(())
}

/// Whether one declared edge is well-formed, refusing against the
/// parent that declared it.
fn edge_shape(parent: &str, edge: &Edge) -> Result<(), Refusal> {
    well_formed(&edge.reference, parent)?;
    stated(parent, &edge.digest, &edge.version)
}

/// Whether `stated` reads as a digest this module writes: 64 lowercase
/// hexadecimal characters.
fn is_digest(stated: &str) -> bool {
    stated.len() == 64
        && stated
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Whether a dotted version label meets a requirement like `>=1.2`.
///
/// A bare label means exactly it: `1.2` is `1.2.0` and no other, and an
/// empty or unparseable label satisfies nothing — the same rule
/// [`crate::package`] checks compatibility by, kept identical so a lock
/// and a package read a range the same way.
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

/// The ordering key a version label sorts by; an unparseable or absent
/// label sorts below everything, because a record that cannot say its
/// version is never the highest.
fn version_key(label: &str) -> Vec<u64> {
    dots(label).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Writes `body` to `root/rel` and returns the digest it now has.
    fn stage(root: &Path, rel: &str, body: &str) -> String {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, body).unwrap();
        digest(body)
    }

    /// A program record claiming `slug` at `version`.
    fn program(slug: &str, version: &str) -> String {
        format!(
            r#"{{"v":1,"slug":"{slug}","version":"{version}","steps":[{{"name":"one","kind":"query","bounds":{{}}}}]}}"#
        )
    }

    /// A source record claiming `slug` at `version`.
    fn source(slug: &str, version: &str) -> String {
        format!(
            r#"{{"v":1,"slug":"{slug}","version":"{version}","from":{{"file":{{"path":"{slug}.json"}}}},"order":"id"}}"#
        )
    }

    /// An entry naming `reference`, asking for nothing more.
    fn entry(reference: &str) -> Entry {
        Entry {
            reference: reference.to_string(),
            digest: None,
            version: None,
            requires: Vec::new(),
        }
    }

    /// An edge naming `reference`, asking for nothing more.
    fn edge(reference: &str) -> Edge {
        Edge {
            reference: reference.to_string(),
            digest: None,
            version: None,
        }
    }

    /// A lock holding `entries`.
    fn lock(entries: Vec<Entry>) -> Lock {
        Lock {
            v: LOCK_VERSION,
            entries,
        }
    }

    /// One root holding the given program versions of `slug`, one file
    /// each, returning their digests in the same order.
    fn stage_programs(root: &Path, slug: &str, versions: &[&str]) -> Vec<String> {
        versions
            .iter()
            .map(|version| {
                stage(
                    root,
                    &format!("programs/{slug}-{version}.json"),
                    &program(slug, version),
                )
            })
            .collect()
    }

    #[test]
    fn a_pinned_digest_resolves_exactly_even_beside_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let digests = stage_programs(root, "tool", &["1.0.0", "2.0.0"]);

        let mut want = entry("programs/tool");
        want.digest = Some(digests[0].clone());
        let plan = Resolve::plan(&lock(vec![want]), &[root.to_path_buf()]).unwrap();

        let resolved = &plan.resolved["programs/tool"];
        assert_eq!(resolved.matched, Match::Exact);
        assert_eq!(resolved.digest, digests[0]);
        assert_eq!(resolved.version, "1.0.0");
        assert!(resolved.path.ends_with("tool-1.0.0.json"));
        assert!(plan.grants.is_empty(), "a plan grants nothing");
    }

    #[test]
    fn an_unpinned_reference_resolves_to_the_highest_compatible_local_version() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        stage_programs(first.path(), "tool", &["1.0.0", "2.0.0"]);
        let newest = stage(
            second.path(),
            "programs/tool-3.1.0.json",
            &program("tool", "3.1.0"),
        );

        let mut want = entry("programs/tool");
        want.version = Some(">=2.0".to_string());
        let plan = Resolve::plan(
            &lock(vec![want]),
            &[first.path().to_path_buf(), second.path().to_path_buf()],
        )
        .unwrap();

        let resolved = &plan.resolved["programs/tool"];
        assert_eq!(resolved.matched, Match::Compatible);
        assert_eq!(resolved.digest, newest);
        assert_eq!(resolved.version, "3.1.0");
        assert!(resolved.path.ends_with("tool-3.1.0.json"));
    }

    #[test]
    fn a_reference_nothing_local_answers_is_refused_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let refusal = Resolve::plan(
            &lock(vec![entry("sources/ghost")]),
            &[dir.path().to_path_buf()],
        )
        .unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Unresolved {
                reference: "sources/ghost".to_string(),
                required_by: None,
            }
        );
    }

    #[test]
    fn a_dependency_cycle_is_refused_with_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        stage(root, "programs/a.json", &program("a", "1.0.0"));
        stage(root, "programs/b.json", &program("b", "1.0.0"));

        let mut a = entry("programs/a");
        a.requires = vec![edge("programs/b")];
        let mut b = entry("programs/b");
        b.requires = vec![edge("programs/a")];

        let refusal = Resolve::plan(&lock(vec![a, b]), &[root.to_path_buf()]).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Cyclic {
                path: "programs/a -> programs/b -> programs/a".to_string(),
            }
        );
    }

    #[test]
    fn an_incompatible_transitive_dependency_names_the_parent_that_required_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        stage(root, "programs/p.json", &program("p", "1.0.0"));
        stage(root, "sources/q.json", &source("q", "1.0.0"));

        let mut p = entry("programs/p");
        p.requires = vec![Edge {
            version: Some(">=2.0".to_string()),
            ..edge("sources/q")
        }];

        let refusal = Resolve::plan(&lock(vec![p]), &[root.to_path_buf()]).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Incompatible {
                reference: "sources/q".to_string(),
                requirement: ">=2.0".to_string(),
                required_by: Some("programs/p".to_string()),
            }
        );
    }

    #[test]
    fn an_unresolved_transitive_dependency_names_the_parent_that_required_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        stage(root, "programs/p.json", &program("p", "1.0.0"));

        let mut p = entry("programs/p");
        p.requires = vec![edge("sources/ghost")];

        let refusal = Resolve::plan(&lock(vec![p]), &[root.to_path_buf()]).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Unresolved {
                reference: "sources/ghost".to_string(),
                required_by: Some("programs/p".to_string()),
            }
        );
    }

    #[test]
    fn a_diamond_that_resolves_both_sides_to_the_same_digest_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for slug in ["a", "b", "c"] {
            stage(
                root,
                &format!("programs/{slug}.json"),
                &program(slug, "1.0.0"),
            );
        }
        let older = stage(root, "sources/d-1.0.0.json", &source("d", "1.0.0"));
        let shared = stage(root, "sources/d-2.0.0.json", &source("d", "2.0.0"));

        let mut b = entry("programs/b");
        b.requires = vec![Edge {
            version: Some(">=1".to_string()),
            ..edge("sources/d")
        }];
        let mut c = entry("programs/c");
        c.requires = vec![Edge {
            version: Some(">=1".to_string()),
            ..edge("sources/d")
        }];
        let mut a = entry("programs/a");
        a.requires = vec![edge("programs/b"), edge("programs/c")];

        let plan = Resolve::plan(&lock(vec![a, b, c]), &[root.to_path_buf()]).unwrap();
        assert_eq!(plan.resolved.len(), 4);
        let d = &plan.resolved["sources/d"];
        assert_eq!(d.digest, shared, "both sides landed on the same file");
        assert_eq!(d.version, "2.0.0");
        assert_ne!(d.digest, older);
    }

    #[test]
    fn a_records_own_declared_dependencies_are_walked_too() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        stage(root, "programs/top.json", &program("top", "1.0.0"));
        stage(
            root,
            "programs/helper.json",
            r#"{"v":1,"slug":"helper","version":"1.0.0","requires":[{"reference":"sources/data","version":">=1"}],"steps":[{"name":"one","kind":"query","bounds":{}}]}"#,
        );
        stage(root, "sources/data.json", &source("data", "1.5.0"));

        let mut top = entry("programs/top");
        top.requires = vec![edge("programs/helper")];

        let plan = Resolve::plan(&lock(vec![top]), &[root.to_path_buf()]).unwrap();
        assert_eq!(
            plan.resolved.keys().collect::<Vec<_>>(),
            ["programs/helper", "programs/top", "sources/data"]
        );
        let data = &plan.resolved["sources/data"];
        assert_eq!(data.matched, Match::Compatible);
        assert_eq!(data.version, "1.5.0");
        let helper = &plan.resolved["programs/helper"];
        assert_eq!(
            helper.requires,
            vec![Edge {
                reference: "sources/data".to_string(),
                digest: None,
                version: Some(">=1".to_string()),
            }],
            "the edges the record declared are the edges the plan records"
        );
    }

    #[test]
    fn a_tampered_local_file_is_refused_not_used() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let original = stage(root, "programs/tool.json", &program("tool", "1.0.0"));
        let tampered = stage(
            root,
            "programs/tool.json",
            &program("tool", "1.0.0").replace("query", "decide"),
        );

        let mut want = entry("programs/tool");
        want.digest = Some(original.clone());
        let refusal = Resolve::plan(&lock(vec![want]), &[root.to_path_buf()]).unwrap_err();
        assert_eq!(
            refusal,
            Refusal::Digest {
                reference: "programs/tool".to_string(),
                stated: original,
                found: tampered,
                required_by: None,
            }
        );
    }

    #[test]
    fn diff_names_what_an_update_would_change() {
        let mut tool = entry("capabilities/tool");
        tool.digest = Some("d1".to_string());
        let mut x_before = entry("programs/x");
        x_before.digest = Some("d2".to_string());
        x_before.version = Some(">=1".to_string());
        let mut s_before = entry("sources/s");
        s_before.version = Some(">=1".to_string());
        let older = lock(vec![tool, x_before, s_before]);

        let mut policy = entry("policies/local");
        policy.version = Some("=1".to_string());
        let mut x_after = entry("programs/x");
        x_after.digest = Some("d3".to_string());
        x_after.version = Some(">=1".to_string());
        let mut s_after = entry("sources/s");
        s_after.version = Some(">=2".to_string());
        let newer = lock(vec![policy, x_after, s_after]);

        assert_eq!(
            diff(&older, &newer),
            vec![
                Change::Added {
                    reference: "policies/local".to_string(),
                    digest: None,
                    version: Some("=1".to_string()),
                },
                Change::Digest {
                    reference: "programs/x".to_string(),
                    was: Some("d2".to_string()),
                    now: Some("d3".to_string()),
                },
                Change::Version {
                    reference: "sources/s".to_string(),
                    was: Some(">=1".to_string()),
                    now: Some(">=2".to_string()),
                },
                Change::Removed {
                    reference: "capabilities/tool".to_string(),
                    digest: Some("d1".to_string()),
                    version: None,
                },
            ]
        );

        let reversed = diff(&newer, &older);
        assert_eq!(
            reversed,
            vec![
                Change::Added {
                    reference: "capabilities/tool".to_string(),
                    digest: Some("d1".to_string()),
                    version: None,
                },
                Change::Digest {
                    reference: "programs/x".to_string(),
                    was: Some("d3".to_string()),
                    now: Some("d2".to_string()),
                },
                Change::Version {
                    reference: "sources/s".to_string(),
                    was: Some(">=2".to_string()),
                    now: Some(">=1".to_string()),
                },
                Change::Removed {
                    reference: "policies/local".to_string(),
                    digest: None,
                    version: Some("=1".to_string()),
                },
            ],
            "rollback is diff in the other direction"
        );
        assert!(
            diff(&older, &older).is_empty(),
            "a lock does not differ from itself"
        );
    }

    #[test]
    fn a_plan_does_not_depend_on_the_order_the_lock_states() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        stage(root, "programs/a.json", &program("a", "1.0.0"));
        stage(root, "programs/b.json", &program("b", "1.0.0"));
        stage(root, "sources/s.json", &source("s", "2.0.0"));

        let forward = lock(vec![
            entry("programs/a"),
            entry("programs/b"),
            entry("sources/s"),
        ]);
        let backward = lock(vec![
            entry("sources/s"),
            entry("programs/b"),
            entry("programs/a"),
        ]);
        let first = Resolve::plan(&forward, &[root.to_path_buf()]).unwrap();
        let second = Resolve::plan(&backward, &[root.to_path_buf()]).unwrap();
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        assert_eq!(first.digest(), second.digest());
        assert_eq!(
            first.resolved.keys().collect::<Vec<_>>(),
            ["programs/a", "programs/b", "sources/s"]
        );
    }
}
