//! A report commitment: the anchor a caller holds apart from the store.
//!
//! A receipt chain proves a store is internally consistent — that no row
//! was edited or resequenced inside the file it walks. It cannot prove the
//! file is whole, because a shortened file is a valid prefix, and it cannot
//! prove this store is the one that was reported, because a writer holding
//! the file can recompute an entire chain over different rows. Both gaps
//! close the same way: a digest of what the record claimed — its chain
//! head, its row count, its declared selection, and the identities and
//! digests its rows pinned — retained somewhere the store's writer cannot
//! reach. This module is that object.
//!
//! The commitment is deliberately small and self-digested, so a caller can
//! hold it in a ticket, a gist, or a signed message and check a store
//! against it later with `gym verify --commitment`. A store that has grown
//! past the commitment verifies over the committed prefix; a store that
//! shrank, or that was recomputed over different rows, fails with the
//! divergence named.
//!
//! What a verified commitment proves: the store's first `rows` rows are the
//! rows this commitment was taken over, unedited, in order. What it does
//! not prove: that the commitment itself is authentic — that comes from
//! the channel that carried it — or that any remote weights executed,
//! which no document can attest.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coverage::{Expected, run_groups};
use crate::row::Row;
use crate::suite::canonicalize;

/// The schema tag a commitment carries.
pub const COMMITMENT_SCHEMA: &str = "openagents.gym.commitment.v1";

/// The selection a report was declared against: the partitions, the one
/// family when it narrowed, the explicit item subset when one was named,
/// and the doors the run was meant to ask. This is what was claimed, in
/// the terms the claim was made — the `expected` list is its expansion.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Selection {
    /// The partitions the run covered, as wire labels.
    pub partitions: Vec<String>,
    /// The one family the run narrowed to, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// The explicit item ids the run narrowed to, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    /// The doors the run was meant to ask, in declared order.
    pub doors: Vec<String>,
}

/// One `(partition, item)` pair the declared selection covered.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExpectedItem {
    /// The item's partition, as its wire label.
    pub partition: String,
    /// The item's id.
    pub item: String,
}

/// One run identity a door was recorded under at commitment time, with the
/// coverage it had then. A door recorded under two identities has two of
/// these; the commitment binds both rather than picking one.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RunCommitment {
    /// The door identity the group's rows claimed, as serialized JSON.
    pub identity: String,
    /// The question-set digest the group pinned, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_digest: Option<String>,
    /// The gate digest the group pinned, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_digest: Option<String>,
    /// The estimator the group's rows name.
    pub estimator: String,
    /// The draws each estimate rested on, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples: Option<u64>,
    /// The seed base the estimator started from, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_base: Option<u64>,
    /// How many expected items this group recorded a row for.
    pub recorded: usize,
    /// How many expected items this group left unrecorded.
    pub missing: usize,
}

/// What one door committed to: its run identities and their coverage.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DoorCommitment {
    /// The door's name as the run used it.
    pub door: String,
    /// The run identities its rows carried, in first-seen order.
    pub runs: Vec<RunCommitment>,
}

/// The commitment itself: everything a report claimed, digested.
///
/// `digest` covers every field but itself, canonicalized the way the suite
/// digests its items, so the file is self-verifying before it is ever
/// checked against a store.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Commitment {
    /// The schema tag.
    pub schema: String,
    /// When the commitment was taken, as an RFC 3339 timestamp in UTC.
    pub created: String,
    /// The suite the report ran, by name.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// The question set served, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_set: Option<String>,
    /// That set's content digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_digest: Option<String>,
    /// The gate that judged the rows, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_id: Option<String>,
    /// That gate's content digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_digest: Option<String>,
    /// The digest of the suite's `provenance` block — label rules and
    /// agreement ceilings — when the report read them from the suite file.
    /// Provenance sits outside the suite digest, so a claim that quotes it
    /// must bind it here or stand uncommitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// The selection the report was declared against.
    pub selection: Selection,
    /// The `(partition, item)` pairs the selection expanded to.
    pub expected: Vec<ExpectedItem>,
    /// Per door, the run identities its rows carried and their coverage.
    pub doors: Vec<DoorCommitment>,
    /// How many rows the store held when the commitment was taken.
    pub rows: usize,
    /// The chain's head receipt then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The digest over every field above.
    pub digest: String,
}

impl Commitment {
    /// Take the commitment over a verified store's rows, bound to the
    /// declared selection the report ran against.
    ///
    /// `rows` are the whole store — the head and row count cover everything
    /// the file holds — while the identities, digests, and coverage are
    /// computed over the rows that pin this suite's digest. Rows from other
    /// runs folded into the same store are bound by the head but not
    /// claimed by the selection.
    ///
    /// `provenance` is the suite file's `provenance` block when the report
    /// read it; it digests into `provenance_digest` so a quoted ceiling is
    /// committed rather than decorative.
    #[must_use]
    pub fn of(
        suite: &crate::suite::Suite,
        expected: &Expected,
        selection: Selection,
        rows: &[Row],
        head: Option<String>,
        provenance: Option<&Value>,
    ) -> Self {
        let inside: Vec<&Row> = rows
            .iter()
            .filter(|row| row.suite_digest == suite.digest)
            .collect();
        let doors = expected
            .doors
            .iter()
            .map(|door| {
                let asked: Vec<Row> = inside
                    .iter()
                    .filter(|row| row.door == *door && row.permutation.is_none())
                    .map(|row| (*row).clone())
                    .collect();
                let runs = run_groups(&asked)
                    .iter()
                    .map(|(key, group)| {
                        let coverage = crate::coverage::Coverage::of(group, expected.items());
                        RunCommitment {
                            identity: key.door_identity.clone(),
                            question_digest: key.question_digest.clone(),
                            gate_digest: key.gate_digest.clone(),
                            estimator: key.estimator.clone(),
                            samples: key.samples,
                            seed_base: key.seed_base,
                            recorded: coverage.recorded(),
                            missing: coverage.missing.len(),
                        }
                    })
                    .collect();
                DoorCommitment {
                    door: door.clone(),
                    runs,
                }
            })
            .collect();
        let provenance_digest = provenance.map(|value| {
            let mut hasher = Sha256::new();
            hasher.update(canonicalize(value).as_bytes());
            format!("{:x}", hasher.finalize())
        });
        let mut commitment = Self {
            schema: COMMITMENT_SCHEMA.to_string(),
            created: crate::eval::now_utc(),
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: inside.iter().find_map(|row| row.question_set.clone()),
            question_digest: inside.iter().find_map(|row| row.question_digest.clone()),
            gate_id: inside.iter().find_map(|row| row.gate_id.clone()),
            gate_digest: inside.iter().find_map(|row| row.gate_digest.clone()),
            provenance_digest,
            selection,
            expected: expected
                .items()
                .iter()
                .map(|(partition, item)| ExpectedItem {
                    partition: partition.clone(),
                    item: item.clone(),
                })
                .collect(),
            doors,
            rows: rows.len(),
            head,
            digest: String::new(),
        };
        commitment.digest = commitment.compute_digest();
        commitment
    }

    /// The digest over every field but `digest`, canonicalized the way the
    /// suite digests its items.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a commitment serializes");
        value
            .as_object_mut()
            .expect("a commitment is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Read and self-check a commitment file.
    ///
    /// A file whose digest does not recompute over its contents is refused:
    /// a commitment that cannot verify itself cannot anchor a store.
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
        let commitment: Self =
            serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
        if commitment.schema != COMMITMENT_SCHEMA {
            return Err(format!(
                "{path}: schema `{}` is not `{COMMITMENT_SCHEMA}`",
                commitment.schema
            ));
        }
        if commitment.digest != commitment.compute_digest() {
            return Err(format!(
                "{path}: the commitment's digest does not recompute over its contents"
            ));
        }
        Ok(commitment)
    }

    /// The `(partition, item)` pairs as the set [`crate::coverage::Coverage`]
    /// checks against.
    #[must_use]
    pub fn expected_set(&self) -> BTreeSet<(String, String)> {
        self.expected
            .iter()
            .map(|item| (item.partition.clone(), item.item.clone()))
            .collect()
    }
}

/// Check a store against a retained commitment.
///
/// `rows` are the store's typed rows after the chain verified — the caller
/// verifies the chain first, because a commitment checked against a broken
/// chain is answering a question nobody asked. The returned list is every
/// divergence found; empty means the committed prefix stands.
///
/// A store longer than the commitment is not a fault — appends after the
/// commitment are the chain doing its job — but the growth is named in the
/// returned notes so a reader knows the committed claim stops at `rows`.
/// Divergences and notes are distinguished by the caller: `check` returns
/// faults only, and `growth` reports the note.
pub fn check(commitment: &Commitment, rows: &[Row]) -> Vec<String> {
    let mut faults = Vec::new();
    if rows.len() < commitment.rows {
        faults.push(format!(
            "the store holds {} rows; the commitment covers {}",
            rows.len(),
            commitment.rows
        ));
        return faults;
    }
    let prefix = &rows[..commitment.rows];

    // The committed prefix ends at the committed head. A tail removal
    // shortens the file past this point; a recomputed chain over different
    // rows lands a different receipt here.
    match (
        prefix.last().and_then(|row| row.receipt.clone()),
        &commitment.head,
    ) {
        (Some(head), Some(committed)) if head == *committed => {}
        (None, None) => {}
        (found, committed) => faults.push(format!(
            "the committed prefix ends at receipt {}; the store's row {} carries {}",
            committed.as_deref().unwrap_or("none"),
            commitment.rows,
            found.as_deref().unwrap_or("none"),
        )),
    }

    // Every committed-suite row pins the digests the commitment claims.
    // Rows pinning another suite's digest belong to a different run folded
    // into the same store — the head binds them, the selection does not.
    for (index, row) in prefix.iter().enumerate() {
        if row.suite_digest != commitment.suite_digest {
            continue;
        }
        if row.question_digest != commitment.question_digest {
            faults.push(format!(
                "row {index} pins question digest `{:?}`, not the committed `{:?}`",
                row.question_digest, commitment.question_digest,
            ));
        }
        if row.gate_digest != commitment.gate_digest {
            faults.push(format!(
                "row {index} pins gate digest `{:?}`, not the committed `{:?}`",
                row.gate_digest, commitment.gate_digest,
            ));
        }
    }

    // Each committed door's run identities and coverage hold on the prefix.
    let expected = commitment.expected_set();
    for door in &commitment.doors {
        let asked: Vec<Row> = prefix
            .iter()
            .filter(|row| {
                row.suite_digest == commitment.suite_digest
                    && row.door == door.door
                    && row.permutation.is_none()
            })
            .cloned()
            .collect();
        let groups = run_groups(&asked);
        let held: Vec<(String, usize, usize)> = groups
            .iter()
            .map(|(key, group)| {
                let coverage = crate::coverage::Coverage::of(group, &expected);
                (
                    key.door_identity.clone(),
                    coverage.recorded(),
                    coverage.missing.len(),
                )
            })
            .collect();
        let committed: Vec<(String, usize, usize)> = door
            .runs
            .iter()
            .map(|run| (run.identity.clone(), run.recorded, run.missing))
            .collect();
        if held != committed {
            faults.push(format!(
                "door `{}` no longer shows the committed run identities and coverage",
                door.door
            ));
        }
    }
    faults
}

/// How many rows a store holds beyond the commitment's horizon.
#[must_use]
pub fn growth(commitment: &Commitment, rows: &[Row]) -> usize {
    rows.len().saturating_sub(commitment.rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::row::RefusalCode;
    use crate::store::Store;
    use crate::suite::{Partition, Suite};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gym-commit-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn suite() -> Suite {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/caller-v1/suite.json");
        Suite::load_file(path.to_str().unwrap()).unwrap()
    }

    fn row(suite: &Suite, item: &crate::suite::Item, door: &str) -> Row {
        Row {
            recorded_at: "2026-09-20T00:00:00Z".to_string(),
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: Some("caller-v1".to_string()),
            question_digest: Some("question-digest".to_string()),
            split: item.partition.as_str().to_string(),
            family: item.family.clone(),
            item_id: item.id.clone(),
            door: door.to_string(),
            estimator: "greedy".to_string(),
            answered: true,
            correct: Some(true),
            ..Row::default()
        }
    }

    fn committed() -> (Suite, Expected, Vec<Row>, String) {
        let suite = suite();
        let expected = Expected::of(
            &suite,
            &[Partition::Development],
            None,
            None,
            vec!["stub".to_string()],
        )
        .unwrap();
        let rows: Vec<Row> = suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .map(|item| row(&suite, item, "stub"))
            .collect();
        let store = Store::at(scratch("store").join("store.jsonl").to_str().unwrap());
        for row in &rows {
            store.append(row).unwrap();
        }
        let committed_rows: Vec<Row> = store
            .verified_rows()
            .unwrap()
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        let head = committed_rows
            .last()
            .and_then(|row| row.receipt.clone())
            .unwrap();
        (suite, expected, committed_rows, head)
    }

    #[test]
    fn a_store_verifies_against_its_commitment() {
        let (suite, expected, rows, head) = committed();
        let commitment = Commitment::of(
            &suite,
            &expected,
            Selection {
                partitions: vec!["development".to_string()],
                family: None,
                items: None,
                doors: vec!["stub".to_string()],
            },
            &rows,
            Some(head),
            None,
        );
        assert_eq!(commitment.digest, commitment.compute_digest());
        assert!(check(&commitment, &rows).is_empty());
    }

    #[test]
    fn a_dropped_tail_breaks_against_the_commitment() {
        let (suite, expected, rows, head) = committed();
        let commitment = Commitment::of(
            &suite,
            &expected,
            Selection {
                partitions: vec!["development".to_string()],
                family: None,
                items: None,
                doors: vec!["stub".to_string()],
            },
            &rows,
            Some(head),
            None,
        );
        let trimmed = &rows[..rows.len() - 1];
        let faults = check(&commitment, trimmed);
        assert!(
            faults
                .iter()
                .any(|fault| fault.contains("the commitment covers")),
            "{faults:?}"
        );
    }

    #[test]
    fn a_recomputed_chain_over_different_rows_breaks_against_the_commitment() {
        let (suite, expected, rows, head) = committed();
        let commitment = Commitment::of(
            &suite,
            &expected,
            Selection {
                partitions: vec!["development".to_string()],
                family: None,
                items: None,
                doors: vec!["stub".to_string()],
            },
            &rows,
            Some(head),
            None,
        );
        // A replacement store: same item ids, different answers, a fresh
        // chain. Internally consistent, and still not the committed history.
        let mut other = rows.clone();
        for row in &mut other {
            row.correct = Some(false);
            row.receipt = None;
            row.previous_receipt = None;
        }
        let store = Store::at(scratch("rewrite").join("store.jsonl").to_str().unwrap());
        for row in &other {
            store.append(row).unwrap();
        }
        let rewritten: Vec<Row> = store
            .verified_rows()
            .unwrap()
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        let faults = check(&commitment, &rewritten);
        assert!(
            faults.iter().any(|fault| fault.contains("receipt")),
            "{faults:?}"
        );
    }

    #[test]
    fn an_edited_commitment_does_not_self_verify() {
        let (suite, expected, rows, head) = committed();
        let mut commitment = Commitment::of(
            &suite,
            &expected,
            Selection {
                partitions: vec!["development".to_string()],
                family: None,
                items: None,
                doors: vec!["stub".to_string()],
            },
            &rows,
            Some(head),
            None,
        );
        commitment.rows += 1;
        let path = scratch("self").join("commitment.json");
        std::fs::write(&path, serde_json::to_string_pretty(&commitment).unwrap()).unwrap();
        let trouble = Commitment::load(path.to_str().unwrap()).unwrap_err();
        assert!(trouble.contains("does not recompute"), "{trouble}");
    }

    #[test]
    fn a_refusal_is_recorded_in_the_committed_coverage() {
        let suite = suite();
        let expected = Expected::of(
            &suite,
            &[Partition::Development],
            None,
            None,
            vec!["stub".to_string()],
        )
        .unwrap();
        let mut rows: Vec<Row> = suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .map(|item| row(&suite, item, "stub"))
            .collect();
        rows[0] = rows[0].clone().refused(RefusalCode::Busy);
        let store = Store::at(scratch("refused").join("store.jsonl").to_str().unwrap());
        for row in &rows {
            store.append(row).unwrap();
        }
        let stored: Vec<Row> = store
            .verified_rows()
            .unwrap()
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect();
        let head = stored.last().and_then(|row| row.receipt.clone());
        let commitment = Commitment::of(
            &suite,
            &expected,
            Selection {
                partitions: vec!["development".to_string()],
                family: None,
                items: None,
                doors: vec!["stub".to_string()],
            },
            &stored,
            head,
            None,
        );
        assert_eq!(commitment.doors[0].runs[0].recorded, 10);
        assert!(check(&commitment, &stored).is_empty());
    }
}
