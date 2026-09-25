//! The oracle a trial receives from the host.
//!
//! In a Harbor trial the lean loop runs inside the task's container, which
//! can't start the writer's own container, so the host writes the oracle
//! before the trial starts ([`super::host`]) and places `oracle.json` and
//! `spec.json` in the trial, read-only. The harness names their directory
//! in [`DIR_ENV`] and the oracle's digest from the host's record in
//! [`DIGEST_ENV`], or `unavailable` when the host couldn't write one.
//!
//! [`receive`] reads them, reseals the oracle and its spec, and uses the
//! oracle only when both digests match what they say and the oracle's
//! digest matches the host's record. A mismatch is refused and recorded,
//! and the loop runs without an oracle. The loop runs the oracle from the
//! files inside the verified `oracle.json`, never from other files beside
//! it. Inside a task container, the loop never writes an oracle itself.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Oracle, Source, Spec};

/// Names the directory the harness places the oracle in.
pub const DIR_ENV: &str = "CODER_ONE_ORACLE_DIR";

/// Names the oracle's digest from the host's record, or [`UNAVAILABLE`].
pub const DIGEST_ENV: &str = "CODER_ONE_ORACLE_DIGEST";

/// Where the harness places the oracle when [`DIR_ENV`] isn't set.
pub const DEFAULT_DIR: &str = "/opt/openagents/oracle";

/// What [`DIGEST_ENV`] says when the host couldn't write an oracle.
pub const UNAVAILABLE: &str = "unavailable";

/// Where the loop's oracle comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The harness placed an oracle, with this digest, in this directory.
    Delivered { dir: PathBuf, digest: String },
    /// No oracle can be used, for this reason. The loop runs without one.
    Unavailable(String),
    /// Outside a task container, with nothing delivered: the loop writes
    /// the oracle itself, in a boundary on the host.
    WriteHere,
}

/// Decides where the oracle comes from, reading the environment through
/// `get`.
#[must_use]
pub fn plan(get: impl Fn(&str) -> Option<String>, in_task_container: bool) -> Plan {
    let digest = get(DIGEST_ENV)
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());
    match digest {
        Some(d) if d == UNAVAILABLE => Plan::Unavailable(
            "the host couldn't write an oracle before the trial started".to_string(),
        ),
        Some(digest) => Plan::Delivered {
            dir: get(DIR_ENV)
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .map_or_else(|| PathBuf::from(DEFAULT_DIR), PathBuf::from),
            digest,
        },
        None if in_task_container => Plan::Unavailable(
            "no oracle was delivered, and the loop never writes one inside a task container"
                .to_string(),
        ),
        None => Plan::WriteHere,
    }
}

/// Why a delivered oracle can't be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A file isn't there or can't be read.
    Missing(String),
    /// The files don't match their digests or the host's record.
    Mismatch(String),
}

impl Refusal {
    fn status(&self) -> &'static str {
        match self {
            Refusal::Missing(_) => "missing",
            Refusal::Mismatch(_) => "refused",
        }
    }

    fn why(&self) -> &str {
        match self {
            Refusal::Missing(why) | Refusal::Mismatch(why) => why,
        }
    }
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Refusal> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| Refusal::Missing(format!("{} can't be read: {error}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|error| Refusal::Mismatch(format!("{} isn't valid: {error}", path.display())))
}

/// Reads and checks the oracle in `dir` against `digest`.
///
/// # Errors
///
/// [`Refusal::Missing`] when `oracle.json`, or a written oracle's
/// `spec.json`, isn't there. [`Refusal::Mismatch`] when a file doesn't
/// match its own digest, the oracle's digest isn't `digest`, or the spec
/// isn't the one the oracle was written from.
pub fn load(dir: &Path, digest: &str) -> Result<(Oracle, Option<Spec>), Refusal> {
    let oracle: Oracle = read(&dir.join("oracle.json"))?;
    let resealed = oracle.clone().sealed();
    if resealed.digest != oracle.digest {
        return Err(Refusal::Mismatch(format!(
            "oracle.json says its digest is {}, but its contents digest to {}",
            oracle.digest, resealed.digest
        )));
    }
    if oracle.digest != digest {
        return Err(Refusal::Mismatch(format!(
            "the delivered oracle's digest is {}, but the host recorded {digest}",
            oracle.digest
        )));
    }
    let spec = match (oracle.source, &oracle.spec) {
        (Source::Found, _) => None,
        (Source::Written, None) => {
            return Err(Refusal::Mismatch(
                "the written oracle names no spec".to_string(),
            ));
        }
        (Source::Written, Some(expected)) => {
            if !oracle.files.contains_key("oracle.py") {
                return Err(Refusal::Mismatch(
                    "the written oracle has no oracle.py".to_string(),
                ));
            }
            let spec: Spec = read(&dir.join("spec.json"))?;
            let resealed = spec.clone().sealed();
            if resealed.digest != spec.digest || &spec.digest != expected {
                return Err(Refusal::Mismatch(format!(
                    "spec.json digests to {}, but the oracle was written from {expected}",
                    resealed.digest
                )));
            }
            Some(spec)
        }
    };
    Ok((resealed, spec))
}

/// Receives the oracle `plan` names: the oracle and its spec when they can
/// be used, and the record the loop keeps either way. `None` with
/// [`Plan::WriteHere`] means the caller writes the oracle itself.
#[must_use]
pub fn receive(plan: &Plan) -> (Option<(Oracle, Option<Spec>)>, Value) {
    match plan {
        Plan::WriteHere => (None, json!({ "status": "write_here" })),
        Plan::Unavailable(why) => (None, json!({ "status": UNAVAILABLE, "why": why })),
        Plan::Delivered { dir, digest } => match load(dir, digest) {
            Ok(found) => (
                Some(found),
                json!({
                    "status": "delivered",
                    "dir": dir,
                    "digest": digest,
                    "verified": true,
                }),
            ),
            Err(refusal) => (
                None,
                json!({
                    "status": refusal.status(),
                    "dir": dir,
                    "digest": digest,
                    "verified": false,
                    "why": refusal.why(),
                }),
            ),
        },
    }
}

/// Writes a written oracle's files, from the verified oracle, into `dir`,
/// where the loop runs it.
///
/// # Errors
///
/// A message when a file can't be written.
pub fn stage(oracle: &Oracle, dir: &Path) -> Result<(), String> {
    if oracle.files.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (name, text) in &oracle.files {
        if name.contains('/') || name.contains("..") {
            return Err(format!("the oracle names an unsafe file: {name}"));
        }
        std::fs::write(dir.join(name), text).map_err(|e| e.to_string())?;
    }
    Ok(())
}
