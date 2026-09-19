//! A frozen, labelled suite in three partitions, one of which is spent.
//!
//! The items are authored in this repository and carry a content digest, so
//! a result row or a calibration record can name exactly what it was scored
//! on. Partitions are fixed in the file rather than drawn at run time:
//! fitting a map and scoring it on the same items produces a number that
//! means nothing.
//!
//! # Why there are three
//!
//! The suite this crate inherited had two partitions, calibration and
//! evaluation. Every tuning decision made in this repository in the week to
//! 2026-09-19 was made by reading the one evaluation partition: four epochs
//! against two, the band objective against the choice objective,
//! order-shuffle augmentation, which families are admitted, and the Brier
//! tolerance. A partition read that many times has been fitted on. It is a
//! development set wearing an evaluation set's name, and the numbers read
//! off it are optimistic by an amount nobody can estimate.
//!
//! So there are three. [`Partition::Calibration`] fits maps.
//! [`Partition::Development`] chooses between them, as often as you like.
//! [`Partition::Locked`] is read once, through [`LockedLedger::read_locked`],
//! and the read is written down.
//!
//! # How the locked partition is protected
//!
//! [`Suite::partition`] refuses to hand back the locked items at all. The
//! only way to them is a [`LockedLedger`], which appends a [`LockedRead`]
//! naming what the read was spent on and why. A second read of the same
//! suite is refused; [`LockedLedger::read_locked_again`] spends it anyway
//! and records who authorized that and against what argument.
//!
//! This is stricter than the upstream kev rule, which allows one locked read
//! per published candidate. The locked partition here is 39 items, and this
//! repository's recent history is one of reading the held-out set whenever a
//! number was wanted. One read per suite, and an override that leaves a
//! trace, is the rule that fits the failure that happened.
//!
//! The ledger is an append-only file meant to be committed, not a lock. It
//! stops an accidental second read and makes a deliberate one visible in
//! review. It does not stop two processes racing, and it is not trying to.
//!
//! # What the digest covers
//!
//! [`Suite::compute_digest`] hashes the items and nothing else, so it covers
//! every label and every partition assignment and excludes `name`,
//! `description`, `tier`, and `gate`. A changed label or a moved item is a
//! different suite. Tightening a gate floor is not: the gate is carried in
//! the manifest for convenience, and each result row pins the digest of the
//! gate that judged it, so retuning a floor produces a new rule rather than
//! new history.
//!
//! A digest mismatch is refused outright rather than reported as drift. The
//! reference implementation in `~/work/coder` reports drift, which is right
//! for a task manifest whose upstream pins legitimately move, and wrong
//! here: in a labelled suite a changed label is tampering.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The schema tag a three-way suite carries.
pub const SUITE_SCHEMA: &str = "openagents.gym.suite.v1";

/// The schema tag a recorded read of a locked partition carries.
pub const LOCKED_READ_SCHEMA: &str = "openagents.gym.locked_read.v1";

/// The suite this repository scores on, as committed.
pub const SUPPORT_V2_THREE_WAY: &str = include_str!("../suites/support-v2-three-way.json");

/// What can go wrong with a suite or with a read of its locked partition.
#[derive(Debug, thiserror::Error)]
pub enum SuiteError {
    /// The text is not a suite.
    #[error("the suite is not readable: {0}")]
    Malformed(String),
    /// The suite carries a schema tag this code does not read.
    #[error("the suite is tagged {found}, which is not {SUITE_SCHEMA}")]
    Schema {
        /// The tag the file carries.
        found: String,
    },
    /// The recorded digest does not match the items. Someone changed a label.
    #[error(
        "the suite's digest does not match its items: recorded {recorded}, computed {computed}"
    )]
    Tampered {
        /// The digest the file claims.
        recorded: String,
        /// The digest its items actually produce.
        computed: String,
    },
    /// Two items share an id, so one of them would be counted twice.
    #[error("the suite names the item {id} more than once")]
    DuplicateItem {
        /// The repeated id.
        id: String,
    },
    /// A partition holds no items, which makes the partitioning decorative.
    #[error("the suite's {partition} partition is empty")]
    EmptyPartition {
        /// The partition with nothing in it.
        partition: Partition,
    },
    /// Someone asked [`Suite::partition`] for the locked items.
    #[error(
        "the locked partition is not read through `partition`: spend it through a `LockedLedger`, which records the read"
    )]
    LockedNotOpen,
    /// A read of the locked partition left one of its own fields blank.
    #[error("a read of the locked partition records why it was spent, and {field} is empty")]
    Unrecorded {
        /// The field that was left blank.
        field: &'static str,
    },
    /// The locked partition has already been read.
    #[error(
        "the locked partition of {suite} was read on {at}, spent on {subject}; reading it again is an override, not a read"
    )]
    AlreadyRead {
        /// The suite whose locked partition is already spent.
        suite: String,
        /// When the earlier read happened.
        at: String,
        /// What the earlier read was spent on.
        subject: String,
    },
    /// Someone overrode a read that never happened.
    #[error("the locked partition of {suite} has not been read, so there is nothing to override")]
    NothingToOverride {
        /// The suite in question.
        suite: String,
    },
    /// A suite file could not be read.
    #[error("the suite file {path} is not usable: {message}")]
    File {
        /// The file's path.
        path: String,
        /// What the filesystem said.
        message: String,
    },
    /// The ledger file could not be read or appended to.
    #[error("the read ledger {path} is not usable: {message}")]
    Ledger {
        /// The ledger's path.
        path: String,
        /// What the filesystem or the parser said.
        message: String,
    },
}

/// Which partition an item belongs to.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    /// Fits a calibration map, a threshold, or any other parameter.
    Calibration,
    /// Chooses between fitted things. Read as often as you like.
    Development,
    /// Read once, through a [`LockedLedger`], and the read is recorded.
    Locked,
}

impl Partition {
    /// Every partition, in the order a report lists them.
    pub const ALL: [Self; 3] = [Self::Calibration, Self::Development, Self::Locked];

    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Calibration => "calibration",
            Self::Development => "development",
            Self::Locked => "locked",
        }
    }
}

impl fmt::Display for Partition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One labelled item.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Item {
    /// A stable id, unique within the suite.
    pub id: String,
    /// The question family, which a calibration map covers.
    pub family: String,
    /// The question type.
    pub kind: String,
    /// The document to judge.
    pub state: Value,
    /// The question to ask about it, in the shape the door reads.
    pub question: Value,
    /// The option key a knowledgeable person picks.
    pub truth: String,
    /// Which partition this item belongs to. Inside the digest.
    pub partition: Partition,
}

/// A suite of labelled items in three partitions.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Suite {
    /// The schema tag.
    pub schema: String,
    /// The suite's name.
    pub name: String,
    /// What it covers and where the labels come from.
    pub description: String,
    /// When it was authored.
    pub created: String,
    /// The digest recorded in the file, over the items alone.
    pub digest: String,
    /// How heavy a run of this suite is. `smoke` means a result is never a
    /// published score, however completely it runs. Outside the digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// The acceptance rule a run of this suite is normally judged by,
    /// carried verbatim and not interpreted here. Outside the digest, so
    /// tightening a floor does not make historical runs read as drifted;
    /// each row pins its own `gate_digest` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<Value>,
    /// The items.
    pub items: Vec<Item>,
}

impl Suite {
    /// Loads a suite from JSON, checks its digest, and checks that its
    /// partitioning is usable.
    ///
    /// A digest mismatch is an error, not a warning. In a labelled suite a
    /// changed label is tampering, so there is nothing to reconcile.
    pub fn load(text: &str) -> Result<Self, SuiteError> {
        let suite: Self =
            serde_json::from_str(text).map_err(|error| SuiteError::Malformed(error.to_string()))?;
        if suite.schema != SUITE_SCHEMA {
            return Err(SuiteError::Schema {
                found: suite.schema,
            });
        }
        let computed = suite.compute_digest()?;
        if computed != suite.digest {
            return Err(SuiteError::Tampered {
                recorded: suite.digest,
                computed,
            });
        }
        let mut seen: Vec<&str> = Vec::with_capacity(suite.items.len());
        for item in &suite.items {
            if seen.contains(&item.id.as_str()) {
                return Err(SuiteError::DuplicateItem {
                    id: item.id.clone(),
                });
            }
            seen.push(&item.id);
        }
        for partition in Partition::ALL {
            if !suite.items.iter().any(|item| item.partition == partition) {
                return Err(SuiteError::EmptyPartition { partition });
            }
        }
        Ok(suite)
    }

    /// Loads a suite from a file.
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self, SuiteError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|error| SuiteError::File {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Self::load(&text)
    }

    /// The digest over the items, as the builder writes it.
    ///
    /// The items carry their partitions, so this covers the partitioning.
    /// It covers nothing else on the suite: renaming a suite, rewriting its
    /// description, or retuning its gate leaves the digest alone.
    pub fn compute_digest(&self) -> Result<String, SuiteError> {
        let value = serde_json::to_value(&self.items)
            .map_err(|error| SuiteError::Malformed(error.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// The items in an open partition.
    ///
    /// [`Partition::Locked`] is refused here. Spend it through a
    /// [`LockedLedger`], which records the read.
    pub fn partition(&self, partition: Partition) -> Result<Vec<&Item>, SuiteError> {
        if partition == Partition::Locked {
            return Err(SuiteError::LockedNotOpen);
        }
        Ok(self
            .items
            .iter()
            .filter(|item| item.partition == partition)
            .collect())
    }

    /// How many items each partition holds. Counting the locked items does
    /// not read them, so this needs no ledger.
    #[must_use]
    pub fn counts(&self) -> BTreeMap<Partition, usize> {
        let mut counts: BTreeMap<Partition, usize> =
            Partition::ALL.iter().map(|part| (*part, 0)).collect();
        for item in &self.items {
            *counts.entry(item.partition).or_insert(0) += 1;
        }
        counts
    }

    /// The families this suite covers, in first-seen order.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for item in &self.items {
            if !out.contains(&item.family) {
                out.push(item.family.clone());
            }
        }
        out
    }

    /// How many items of each family one partition holds. Like
    /// [`Suite::counts`], this counts without reading.
    #[must_use]
    pub fn family_counts(&self, partition: Partition) -> BTreeMap<String, usize> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for item in self.items.iter().filter(|item| item.partition == partition) {
            *counts.entry(item.family.clone()).or_insert(0) += 1;
        }
        counts
    }
}

/// Loads the suite committed with this crate.
pub fn support_v2_three_way() -> Result<Suite, SuiteError> {
    Suite::load(SUPPORT_V2_THREE_WAY)
}

/// What a read of a locked partition records about itself.
#[derive(Clone, Copy, Debug)]
pub struct Spend<'a> {
    /// What the read is spent on: a candidate, a door identity, a record id.
    pub subject: &'a str,
    /// Why it is being spent now rather than later.
    pub reason: &'a str,
    /// When, as the caller dates it.
    pub at: &'a str,
}

impl Spend<'_> {
    fn check(&self) -> Result<(), SuiteError> {
        for (field, value) in [
            ("subject", self.subject),
            ("reason", self.reason),
            ("at", self.at),
        ] {
            if value.trim().is_empty() {
                return Err(SuiteError::Unrecorded { field });
            }
        }
        Ok(())
    }
}

/// Why a locked partition was read a second time.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Override {
    /// Which read this overrides. The first read is 1.
    pub read: usize,
    /// Who authorized spending the locked partition again.
    pub authority: String,
    /// Why the earlier read does not answer the question.
    pub because: String,
}

/// One recorded read of a locked partition.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LockedRead {
    /// The schema tag.
    pub schema: String,
    /// The suite's name, for reading the ledger by eye.
    pub suite: String,
    /// The suite's digest, which is what a read is counted against.
    pub digest: String,
    /// What the read was spent on.
    pub subject: String,
    /// Why it was spent.
    pub reason: String,
    /// When, as the caller dated it.
    pub at: String,
    /// How many items the read covered.
    pub items: usize,
    /// Set when this read overrides an earlier one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Override>,
}

/// An append-only record of every locked-partition read.
///
/// Point it at a file that is committed. The ledger's value is that a
/// second read shows up in review, which it cannot do from a temporary
/// directory.
#[derive(Clone, Debug)]
pub struct LockedLedger {
    path: PathBuf,
}

impl LockedLedger {
    /// A ledger kept at this path. The file is created on the first append.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Where the ledger is kept.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every read the ledger holds, oldest first.
    pub fn reads(&self) -> Result<Vec<LockedRead>, SuiteError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(self.trouble(&error.to_string())),
        };
        let mut reads = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let read: LockedRead =
                serde_json::from_str(line).map_err(|error| self.trouble(&error.to_string()))?;
            reads.push(read);
        }
        Ok(reads)
    }

    /// Every read of one suite, by its digest, oldest first.
    pub fn reads_of(&self, digest: &str) -> Result<Vec<LockedRead>, SuiteError> {
        Ok(self
            .reads()?
            .into_iter()
            .filter(|read| read.digest == digest)
            .collect())
    }

    /// Reads the locked partition and records the read.
    ///
    /// The read is counted against the suite's digest. If that digest has
    /// been read before, this is refused: a second read is an override, and
    /// [`LockedLedger::read_locked_again`] is how you take one deliberately.
    pub fn read_locked<'a>(
        &self,
        suite: &'a Suite,
        spend: &Spend<'_>,
    ) -> Result<Vec<&'a Item>, SuiteError> {
        spend.check()?;
        let earlier = self.reads_of(&suite.digest)?;
        if let Some(first) = earlier.first() {
            return Err(SuiteError::AlreadyRead {
                suite: suite.name.clone(),
                at: first.at.clone(),
                subject: first.subject.clone(),
            });
        }
        self.spend(suite, spend, None)
    }

    /// Reads the locked partition again, against an explicit override that
    /// is recorded beside the read.
    ///
    /// This is refused when there is nothing to override. An override with
    /// no earlier read is a caller reaching for the loud door by habit.
    pub fn read_locked_again<'a>(
        &self,
        suite: &'a Suite,
        spend: &Spend<'_>,
        authority: &str,
        because: &str,
    ) -> Result<Vec<&'a Item>, SuiteError> {
        spend.check()?;
        if authority.trim().is_empty() {
            return Err(SuiteError::Unrecorded { field: "authority" });
        }
        if because.trim().is_empty() {
            return Err(SuiteError::Unrecorded { field: "because" });
        }
        let earlier = self.reads_of(&suite.digest)?;
        if earlier.is_empty() {
            return Err(SuiteError::NothingToOverride {
                suite: suite.name.clone(),
            });
        }
        self.spend(
            suite,
            spend,
            Some(Override {
                read: earlier.len(),
                authority: authority.to_owned(),
                because: because.to_owned(),
            }),
        )
    }

    /// Appends the record, then hands back the items. The record is written
    /// first on purpose: a read that fails to record is not a read.
    fn spend<'a>(
        &self,
        suite: &'a Suite,
        spend: &Spend<'_>,
        overrides: Option<Override>,
    ) -> Result<Vec<&'a Item>, SuiteError> {
        let items: Vec<&Item> = suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Locked)
            .collect();
        let record = LockedRead {
            schema: LOCKED_READ_SCHEMA.to_owned(),
            suite: suite.name.clone(),
            digest: suite.digest.clone(),
            subject: spend.subject.to_owned(),
            reason: spend.reason.to_owned(),
            at: spend.at.to_owned(),
            items: items.len(),
            overrides,
        };
        let line = serde_json::to_string(&record)
            .map_err(|error| SuiteError::Malformed(error.to_string()))?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| self.trouble(&error.to_string()))?;
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| self.trouble(&error.to_string()))?;
        writeln!(file, "{line}").map_err(|error| self.trouble(&error.to_string()))?;
        Ok(items)
    }

    fn trouble(&self, message: &str) -> SuiteError {
        SuiteError::Ledger {
            path: self.path.display().to_string(),
            message: message.to_owned(),
        }
    }
}

/// Serializes with object keys sorted, which is what the digest is over.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonicalize(&fields[*key])
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(canonicalize).collect();
            format!("[{}]", inner.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suite() -> Suite {
        support_v2_three_way().expect("the committed suite loads")
    }

    fn ledger() -> (tempfile::TempDir, LockedLedger) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let ledger = LockedLedger::at(directory.path().join("locked-reads.jsonl"));
        (directory, ledger)
    }

    const SPEND: Spend<'static> = Spend {
        subject: "lev-band-adapter-e4",
        reason: "the published candidate, scored once before the model card",
        at: "2026-09-19",
    };

    #[test]
    fn the_committed_suite_loads_and_its_digest_matches() {
        let suite = suite();
        assert_eq!(suite.name, "support-v2-three-way");
        assert_eq!(suite.items.len(), 196);
        assert_eq!(suite.tier.as_deref(), Some("scored"));
        assert!(suite.gate.is_some(), "the manifest carries its gate");
    }

    #[test]
    fn the_three_partitions_are_disjoint_and_cover_every_item() {
        let suite = suite();
        let counts = suite.counts();
        let total: usize = counts.values().sum();
        assert_eq!(total, suite.items.len(), "every item is in exactly one");
        assert_eq!(counts[&Partition::Calibration], 79);
        assert_eq!(counts[&Partition::Development], 78);
        assert_eq!(counts[&Partition::Locked], 39);

        // Calibration fits a five-bin table, so it needs the evidence.
        assert!(counts[&Partition::Calibration] >= 70);
        assert!(counts[&Partition::Development] >= 70);
        assert!(counts[&Partition::Locked] >= 30);
    }

    #[test]
    fn every_family_appears_in_every_partition_in_the_same_proportion() {
        let suite = suite();
        let total = suite.items.len() as f64;
        for family in suite.families() {
            let overall = suite
                .items
                .iter()
                .filter(|item| item.family == family)
                .count() as f64
                / total;
            for partition in Partition::ALL {
                let counts = suite.family_counts(partition);
                let held = *counts.get(&family).unwrap_or(&0);
                assert!(held > 0, "{partition} holds no {family} items");
                let share = held as f64 / suite.counts()[&partition] as f64;
                assert!(
                    (share - overall).abs() < 0.02,
                    "{family} is {:.1}% of {partition} and {:.1}% of the suite",
                    share * 100.0,
                    overall * 100.0
                );
            }
        }
    }

    #[test]
    fn every_label_of_every_family_appears_in_every_partition() {
        // A locked partition missing a label scores a model on a question it
        // was never asked, and does it quietly.
        let suite = suite();
        for family in suite.families() {
            let mut labels: Vec<&str> = suite
                .items
                .iter()
                .filter(|item| item.family == family)
                .map(|item| item.truth.as_str())
                .collect();
            labels.sort_unstable();
            labels.dedup();
            for partition in Partition::ALL {
                for label in &labels {
                    assert!(
                        suite.items.iter().any(|item| item.partition == partition
                            && item.family == family
                            && item.truth == *label),
                        "{partition} holds no {family} item labelled {label}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_tampered_suite_is_refused() {
        let tampered =
            SUPPORT_V2_THREE_WAY.replacen("\"truth\": \"billing\"", "\"truth\": \"sales\"", 1);
        assert!(matches!(
            Suite::load(&tampered),
            Err(SuiteError::Tampered { .. })
        ));
    }

    #[test]
    fn moving_one_item_between_partitions_is_refused() {
        // The partitions are inside the digest, so a quiet reshuffle that
        // slipped locked items into calibration would not load.
        let moved = SUPPORT_V2_THREE_WAY.replacen(
            "\"partition\": \"locked\"",
            "\"partition\": \"calibration\"",
            1,
        );
        assert!(matches!(
            Suite::load(&moved),
            Err(SuiteError::Tampered { .. })
        ));
    }

    #[test]
    fn tightening_the_gate_leaves_the_digest_alone() {
        let tightened = SUPPORT_V2_THREE_WAY.replace(
            "\"brier_rises_by_at_most\": 0.1",
            "\"brier_rises_by_at_most\": 0.05",
        );
        assert_ne!(tightened, SUPPORT_V2_THREE_WAY, "the gate was in the file");
        let retuned = Suite::load(&tightened).expect("a retuned gate is not drift");
        assert_eq!(retuned.digest, suite().digest);
    }

    #[test]
    fn renaming_the_suite_leaves_the_digest_alone() {
        let renamed = SUPPORT_V2_THREE_WAY.replace("support-v2-three-way", "support-v2-3");
        let under_another_name = Suite::load(&renamed).expect("a rename is not drift");
        assert_eq!(under_another_name.name, "support-v2-3");
        assert_eq!(under_another_name.digest, suite().digest);
    }

    #[test]
    fn the_locked_partition_is_not_reachable_through_partition() {
        let suite = suite();
        assert!(matches!(
            suite.partition(Partition::Locked),
            Err(SuiteError::LockedNotOpen)
        ));
        assert_eq!(suite.partition(Partition::Calibration).unwrap().len(), 79);
        assert_eq!(suite.partition(Partition::Development).unwrap().len(), 78);
    }

    #[test]
    fn a_locked_read_hands_back_the_items_and_records_itself() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        let items = ledger.read_locked(&suite, &SPEND).expect("the first read");
        assert_eq!(items.len(), 39);
        assert!(items.iter().all(|item| item.partition == Partition::Locked));

        let reads = ledger.reads().expect("the ledger reads back");
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].schema, LOCKED_READ_SCHEMA);
        assert_eq!(reads[0].digest, suite.digest);
        assert_eq!(reads[0].subject, SPEND.subject);
        assert_eq!(reads[0].items, 39);
        assert!(reads[0].overrides.is_none());
    }

    #[test]
    fn a_second_read_without_an_override_is_refused() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        let again = ledger.read_locked(&suite, &SPEND);
        assert!(matches!(again, Err(SuiteError::AlreadyRead { .. })));
        assert_eq!(
            ledger.reads().expect("the ledger reads back").len(),
            1,
            "a refused read is not recorded as a read"
        );
    }

    #[test]
    fn an_override_is_allowed_and_recorded_as_one() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        let items = ledger
            .read_locked_again(
                &suite,
                &SPEND,
                "chris",
                "the first read scored a door with a broken adapter",
            )
            .expect("an authorized second read");
        assert_eq!(items.len(), 39);

        let reads = ledger.reads().expect("the ledger reads back");
        assert_eq!(reads.len(), 2);
        let recorded = reads[1].overrides.as_ref().expect("the override is on it");
        assert_eq!(recorded.read, 1);
        assert_eq!(recorded.authority, "chris");
        assert!(recorded.because.contains("broken adapter"));
    }

    #[test]
    fn an_override_with_nothing_to_override_is_refused() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        let straight_to_the_override = ledger.read_locked_again(&suite, &SPEND, "chris", "no");
        assert!(matches!(
            straight_to_the_override,
            Err(SuiteError::NothingToOverride { .. })
        ));
        assert!(ledger.reads().expect("the ledger reads back").is_empty());
    }

    #[test]
    fn a_read_that_does_not_say_why_is_refused() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        let blank = Spend {
            subject: "lev-band-adapter-e4",
            reason: "   ",
            at: "2026-09-19",
        };
        assert!(matches!(
            ledger.read_locked(&suite, &blank),
            Err(SuiteError::Unrecorded { field: "reason" })
        ));
        assert!(ledger.reads().expect("the ledger reads back").is_empty());
    }

    #[test]
    fn a_different_suite_has_its_own_read_to_spend() {
        // Reads are counted against the digest, so a genuinely new suite is
        // not blocked by the last one's read.
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");

        let mut other = suite.clone();
        other.name = "support-v3".to_owned();
        other.items.truncate(190);
        other.digest = other.compute_digest().expect("a digest");
        ledger.read_locked(&other, &SPEND).expect("its own read");
        assert_eq!(ledger.reads_of(&suite.digest).unwrap().len(), 1);
        assert_eq!(ledger.reads_of(&other.digest).unwrap().len(), 1);
    }

    #[test]
    fn a_suite_tagged_with_another_schema_is_refused() {
        let mistagged = SUPPORT_V2_THREE_WAY.replace(SUITE_SCHEMA, "openagents.gym.suite.v2");
        assert!(matches!(
            Suite::load(&mistagged),
            Err(SuiteError::Schema { .. })
        ));
    }

    #[test]
    fn the_two_way_suite_does_not_load_as_a_three_way_one() {
        // `support-v2.json` is carried here unchanged so the rows and
        // records written against it stay interpretable. It is not this
        // schema, and it does not quietly become it.
        let two_way = include_str!("../suites/support-v2.json");
        assert!(matches!(
            Suite::load(two_way),
            Err(SuiteError::Malformed(_))
        ));
    }
}
