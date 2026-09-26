//! Measuring another author's synced entry version: the runner's half of a
//! NIP-XP `kb-transfer` completion (`nips/openagents/NIP-XP.md`).
//!
//! `kb sync` caches each author's current version of each entry under
//! `~/.openagents/knowledge/remote/<author>/`. A run that searched with
//! other authors' entries records the ID and digest of every entry it
//! showed. This module picks one cached version by author and ID, and
//! measures it with [`evidence::measure`] over the runs that showed exactly
//! that version (by digest) and the runs that didn't show the ID at all.
//!
//! A run that showed the same ID with another digest showed another
//! version, or another author's entry with the same ID, so it's in neither
//! arm. So is a run whose summary records the ID without a digest. The
//! tasks the entry was written from never count, as with a local entry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nostr::domain::Event;
use nostr::kb;
use serde_json::{Value, json};

use crate::Entry;
use crate::evidence::{self, Artifacts, Evaluator, Measured, Run};
use crate::remote::{npub, read_cache};

/// One author's cached entry version.
#[derive(Clone, Debug)]
pub struct Synced {
    /// The author's hex public key.
    pub author: String,
    pub entry: Entry,
    /// The entry file's exact bytes, as the event carries them.
    pub document: String,
    /// The signed `3190`.
    pub event: Event,
}

impl Synced {
    /// The `3190` EventRef a report's subject names.
    #[must_use]
    pub fn event_ref(&self) -> Value {
        json!({"id": self.event.id, "pubkey": self.event.pubkey, "kind": kb::ENTRY_KIND})
    }

    /// The lowercase hex SHA-256 of the document, as the event's `x` tag
    /// carries it.
    #[must_use]
    pub fn hex_digest(&self) -> &str {
        self.entry.digest.trim_start_matches("sha256:")
    }
}

/// The cached entries by `author` (hex) in `remote`, or the ones `ids`
/// name. Each cache file is re-checked against its signature as it's
/// read.
///
/// # Errors
///
/// When an ID names no cached entry by that author.
pub fn synced(remote: &Path, author: &str, ids: &[String]) -> Result<Vec<Synced>, String> {
    let (cached, _) = read_cache(remote);
    let mut out = Vec::new();
    for (by, entry, event) in cached {
        if by != author || !(ids.is_empty() || ids.contains(&entry.id)) {
            continue;
        }
        let document = kb::parse_entry(&event).map_err(|e| e.to_string())?.document;
        out.push(Synced {
            author: by,
            entry,
            document,
            event,
        });
    }
    for id in ids {
        if !out.iter().any(|s| &s.entry.id == id) {
            return Err(format!(
                "no entry {id} by {} in {}; run microcoder kb sync --author {} first",
                npub(author),
                remote.display(),
                npub(author)
            ));
        }
    }
    if out.is_empty() {
        return Err(format!(
            "no entries by {} in {}; run microcoder kb sync --author {} first",
            npub(author),
            remote.display(),
            npub(author)
        ));
    }
    Ok(out)
}

/// The retained intake's entry pins. This never rereads a mutable summary.
#[must_use]
pub fn shown(_runs_dir: &Path, run: &Run) -> Option<BTreeMap<String, Option<String>>> {
    run.knowledge_complete.then(|| {
        run.knowledge
            .iter()
            .map(|pin| (pin.id.clone(), pin.digest.clone()))
            .collect()
    })
}

/// The runs that count for one entry version.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Every intake, including the records excluded from either arm.
    pub intake: Vec<Run>,
    /// Runs that showed exactly this version, and runs that didn't show its
    /// ID at all.
    pub runs: Vec<Run>,
    /// Runs that showed the ID with another digest: another version, or
    /// another author's entry with the same ID.
    pub other_versions: usize,
    /// Runs that showed the ID with no digest recorded, or whose summary
    /// can't be read again.
    pub unpinned: usize,
}

/// Picks the runs that count for `entry` from `runs`, all recorded under
/// `runs_dir`.
#[must_use]
pub fn select(entry: &Entry, runs: &[Run], runs_dir: &Path) -> Selection {
    let mut selection = Selection {
        intake: runs.to_vec(),
        ..Selection::default()
    };
    for run in runs {
        if !run.used.contains(&entry.id) {
            selection.runs.push(run.clone());
            continue;
        }
        match shown(runs_dir, run).and_then(|shown| shown.get(&entry.id).cloned().flatten()) {
            Some(digest) if digest == entry.digest => selection.runs.push(run.clone()),
            Some(_) => selection.other_versions += 1,
            None => selection.unpinned += 1,
        }
    }
    selection
}

/// Measures the synced version `synced` against `runs`, recorded under
/// `runs_dir`: the runs that showed its exact digest against the runs that
/// didn't show its ID, never on a task it was written from.
#[must_use]
pub fn measure(synced: &Synced, runs: &[Run], runs_dir: &Path) -> (Measured, Selection) {
    let selection = select(&synced.entry, runs, runs_dir);
    let mut measured = evidence::measure(&synced.entry, &selection.runs);
    let intake = evidence::measure(&synced.entry, runs);
    measured.intake_records = intake.intake_records;
    measured.intake_faults = intake.intake_faults;
    measured.unknown_membership = intake.unknown_membership;
    measured.changed_entry_runs = intake.changed_entry_runs;
    measured.unpinned_entry_runs = intake.unpinned_entry_runs;
    measured.noncomparable_runs = intake.noncomparable_runs;
    measured.prospective_unverified_runs = intake.prospective_unverified_runs;
    (measured, selection)
}

/// The NIP-EVAL report on `synced` by `evaluator`, citing the author's
/// exact `3190`. Its subject is the entry's qualified ID in the author's
/// namespace, with the document's exact bytes; `meta.kb` also names the
/// author, the digest, and the runs left out because they showed another
/// version or recorded no digest.
#[must_use]
pub fn report(
    synced: &Synced,
    measured: &Measured,
    selection: &Selection,
    evaluator: &Evaluator,
) -> (Value, Artifacts) {
    let (mut report, artifacts) = evidence::report_about(
        measured,
        &synced.document,
        &synced.author,
        &selection.intake,
        evaluator,
        Some(synced.event_ref()),
    );
    let kb = &mut report["meta"]["kb"];
    kb["author"] = json!(synced.author);
    kb["digest"] = json!(synced.entry.digest);
    kb["other_version_runs"] = json!(selection.other_versions);
    kb["unpinned_runs"] = json!(selection.unpinned);
    (report, artifacts)
}

/// `<dir>/remote/<author>/<id>.v<version>.json`, where a synced entry's
/// local report is kept.
#[must_use]
pub fn report_path(dir: &Path, synced: &Synced) -> PathBuf {
    evidence::report_path(
        &dir.join("remote").join(&synced.author),
        &synced.entry.id,
        synced.entry.version,
    )
}

/// What the measurement left out, for a person to read: `2 runs showed
/// another version; 1 recorded no digest`, or nothing.
#[must_use]
pub fn left_out(selection: &Selection) -> String {
    let mut parts = Vec::new();
    if selection.other_versions > 0 {
        parts.push(format!(
            "{} showed another version",
            evidence::count(selection.other_versions, "run")
        ));
    }
    if selection.unpinned > 0 {
        parts.push(format!(
            "{} recorded no digest",
            evidence::count(selection.unpinned, "run")
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("; not counted: {}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests;
