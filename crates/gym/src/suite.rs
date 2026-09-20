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
//! The ledger is an append-only file meant to be committed, and the read
//! itself is a transaction. [`LockedLedger::read_locked`] takes the ledger's
//! lock — a sibling `*.lock` file created with `create_new`, the same
//! discipline the result store keeps for its single writer — and holds it
//! across reading the file, deciding eligibility, appending the record, and
//! syncing the append to durable storage. Only then are the items handed
//! back, so a read that returned is a read that was recorded, and two
//! racing readers cannot both be first: the loser finds the winner's
//! committed record and is refused.
//!
//! The lock is taken on the ledger's canonical path, so two spellings of
//! one file — a symlink, a `..` segment, a relative path — still take one
//! lock. Two names for one inode cannot be told apart that way, so a ledger
//! with more than one name is refused rather than half-serialized, and so
//! is a symlink whose target does not exist.
//!
//! An interrupted append fails closed rather than opening a second spend.
//! A last line that parses is a committed record and counts; a last line
//! that does not parse means a writer died mid-record, and the ledger
//! reports [`SuiteError::Interrupted`] rather than read past it — the read
//! that line was writing may or may not have committed, and guessing wrong
//! is how a locked partition gets spent twice. A lock that outlives the
//! wait bound is reported rather than waited on forever; whether the
//! holder is a live read or a file a killed reader left is a person's
//! check, not the wait's. `docs/gym/ledger.md` covers the transaction, the
//! alias policy, and the recovery rules.
//!
//! # What the digest covers
//!
//! [`Suite::compute_digest`] hashes the items and nothing else, so it covers
//! every label and every partition assignment and excludes `name`,
//! `description`, `tier`, `gate`, and `questions`. A changed label or a moved
//! item is a different suite. Tightening a gate floor is not: the manifest
//! names its gate by id, the rule itself lives in `crates/gym/gates/` with
//! its own digest, and each result row pins the digest of the gate that
//! judged it, so retuning a floor produces a new rule rather than new
//! history.
//!
//! Rewording a question is not either, and that is newer. The question text
//! used to be a field of [`Item`] and so inside this digest, which made a
//! reworded question a different suite rather than a candidate against the
//! same items — see [`crate::questions`], which owns the text now. The
//! manifest names its question set by id for the same reason it names its
//! gate by id, and each row pins the question digest it was served.
//!
//! A digest mismatch is refused outright rather than reported as drift. The
//! reference implementation in `~/work/coder` reports drift, which is right
//! for a task manifest whose upstream pins legitimately move, and wrong
//! here: in a labelled suite a changed label is tampering.
//!
//! # When the locked partition is training data
//!
//! A held-out number means nothing for a door that trained on the items.
//! `support-v2-three-way` was partitioned over the same 196 items that the
//! two-way `support-v2` had already split, and `training/lev-adapter` had
//! trained every Lev adapter from that two-way calibration split before the
//! three-way file locked 20 of those 98 records (openagents#9399). The
//! manifest says so in a typed field, [`Suite::exposure`], and
//! [`LockedLedger::read_locked`] refuses to spend an exposed locked
//! partition for any door that serves an adapter. Base and hosted doors
//! never trained on anything, so their read still goes through. The clean
//! set the adapters can be confirmed on is `support-v2-unseen`, which the
//! exposure names as its `successor`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::row::LabelSource;
use crate::store::{StoreError, WriteLock};

/// The schema tag a three-way suite carries.
pub const SUITE_SCHEMA: &str = "openagents.gym.suite.v1";

/// The schema tag a recorded read of a locked partition carries.
pub const LOCKED_READ_SCHEMA: &str = "openagents.gym.locked_read.v1";

/// The suite this repository scores on, as committed.
pub const SUPPORT_V2_THREE_WAY: &str = include_str!("../suites/support-v2-three-way.json");

/// The 98 items of `support-v2` that no Lev adapter trained on, under the
/// partitions `support-v2-three-way` gave them. This is the suite an adapted
/// door's locked read is spent on, because the three-way suite's locked
/// partition is half training data for every adapter.
pub const SUPPORT_V2_UNSEEN: &str = include_str!("../suites/support-v2-unseen.json");

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
    /// Some items carry their question text and some do not.
    #[error(
        "{carried} of {total} items carry their own question text, and the rest take it \
         from a question set; a suite carries its text on every item or on none of them, \
         because a per-item override is the per-item question data a question set removes"
    )]
    MixedQuestions {
        /// How many items carry inline text.
        carried: usize,
        /// How many items there are.
        total: usize,
    },
    /// The suite has no question text anywhere: none inline, and none named.
    #[error(
        "the suite's items carry no question text and the manifest names no question set, \
         so there is nothing to ask; name a set in `questions`"
    )]
    NoQuestions,
    /// Someone asked [`Suite::partition`] for the locked items.
    #[error(
        "the locked partition is not read through `partition`: spend it through a `LockedLedger`, which records the read"
    )]
    LockedNotOpen,
    /// The manifest's exposure record does not describe the suite it is on.
    #[error("the suite's exposure record is not usable: {0}")]
    ExposureMalformed(String),
    /// The locked partition holds training data for adapted doors, and the
    /// caller is spending it for one. A number read off memorized items is
    /// not a confirmation, so the read is refused rather than recorded.
    #[error(
        "the locked partition of {suite} is training data for adapted doors: {items} of its \
         {locked} items were exposed through {through}, so it cannot confirm the adapter \
         `{adapter}`{successor}"
    )]
    Exposed {
        /// The suite whose locked partition is exposed.
        suite: String,
        /// How many locked items were exposed.
        items: usize,
        /// How many items the locked partition holds.
        locked: usize,
        /// What exposed them.
        through: String,
        /// The adapter the caller wanted to confirm.
        adapter: String,
        /// `; spend <successor> instead` when the manifest names a
        /// replacement, and empty otherwise.
        successor: String,
    },
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
    /// The ledger's lock stayed held past the wait bound. That may be a
    /// read still running or a file a killed reader left; the message
    /// names the lock and the pid it claims so a person can check before
    /// removing it.
    #[error(
        "the ledger lock {lock} stayed held{holder}. That may be a read still running or a file \
         a killed reader left; check the holder before removing it"
    )]
    Locked {
        /// The lock file's path.
        lock: String,
        /// Who holds it, when the file says.
        holder: String,
    },
    /// The ledger's last line is incomplete. An earlier append was
    /// interrupted, and whether the read it was writing committed cannot be
    /// told from what landed, so the ledger fails closed rather than guess
    /// and spend a partition twice.
    #[error(
        "the read ledger {path} ends mid-record: an earlier append was interrupted, and whether \
         its read committed cannot be told from what landed. Remove or complete the last line \
         before spending again"
    )]
    Interrupted {
        /// The ledger's path.
        path: String,
    },
}

impl SuiteError {
    /// Whether the ledger's lock stayed held past the wait bound. A caller
    /// that meets this can retry; every other refusal means the spend was
    /// wrong, not early.
    pub fn is_locked(&self) -> bool {
        matches!(self, SuiteError::Locked { .. })
    }
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
    ///
    /// Absent when the suite names a [`crate::questions::QuestionSet`]
    /// instead, which is what a suite written after openagents#9386 does:
    /// 196 items shared exactly one question text per family, so the text
    /// was stored 196 times and digested as if it were per-item data.
    ///
    /// Present on every item of `support-v2-three-way`, which predates that
    /// change. Its text stays inline and inside its digest, because moving
    /// it would reissue `54fbf4137c…` and invalidate every row that pins it.
    /// [`crate::questions::QuestionSet::authored`] reads that text back out
    /// under a name a row can pin.
    ///
    /// A suite carries its text inline on every item or on none of them.
    /// [`Suite::load`] refuses a mixture, because a per-item override of a
    /// set-provided question is the per-item question data the split removes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<Value>,
    /// The option key a knowledgeable person picks.
    pub truth: String,
    /// Which partition this item belongs to. Inside the digest.
    pub partition: Partition,
    /// What kind of evidence the label rests on. Inside the digest, so a
    /// label that changes from a reading to an outcome is a different suite.
    ///
    /// Absent means [`LabelSource::Author`], which is what every item
    /// written before `coder-turns-v1` is. It is absent rather than written
    /// out so that adding this field left the earlier suites' digests alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_source: Option<LabelSource>,
    /// The rule that produced the label, in one sentence.
    ///
    /// Per item rather than per family, because the digest covers items and
    /// a rule kept anywhere else can be rewritten without the suite
    /// noticing. The repetition is the point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_rule: Option<String>,
}

impl Item {
    /// What kind of evidence this item's label rests on.
    #[must_use]
    pub fn evidence(&self) -> LabelSource {
        self.label_source.clone().unwrap_or_default()
    }
}

/// A record that a partition's items were training data for a door.
///
/// A suite carries this when items it holds back were trained on before it
/// held them back, which is the ordering accident openagents#9399 found:
/// the two-way `support-v2` split trained every Lev adapter, and the
/// three-way partitioning of the same items came later and locked 20 of
/// the 98 training records. The record is typed rather than written in a
/// document so the tooling reads it: [`LockedLedger::read_locked`] refuses
/// to spend an exposed locked partition for a door that serves an adapter.
///
/// Outside the digest, like `gate` and `questions`: it describes what
/// happened to the items, not what they are, and adding it to a suite must
/// leave every row and record that pins that suite's digest valid.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Exposure {
    /// Which partition was exposed.
    pub partition: Partition,
    /// How many of that partition's items were exposed.
    pub items: usize,
    /// What exposed them: the script, the split it read, and when.
    pub through: String,
    /// The suite that replaces this one for the doors the exposure
    /// affects, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor: Option<String>,
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
    /// The acceptance rule a run of this suite is normally judged by, named
    /// by the id of a file in `crates/gym/gates/`.
    ///
    /// An id and not a block of thresholds. This field held an inline rule
    /// called `calibration-admission-v1` until 2026-09-19, with numbers that
    /// had already been superseded by `probability-v1`, and nothing said
    /// which of the two a run had used. A name can be resolved to one rule
    /// with one digest; a restatement is a second rule wearing the suite's
    /// name.
    ///
    /// Outside the digest, so retuning a floor does not make historical runs
    /// read as drifted; each row pins its own `gate_digest` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    /// The question text a run of this suite serves, named by the id of a
    /// file in `crates/gym/questions/`.
    ///
    /// Outside the digest, for the reason `gate` is: the digest covers what
    /// was asked about and what the answer is, and rewording the question is
    /// neither. A reword produces a new question set with its own digest,
    /// which each row pins beside the suite's, so a question-text variant is
    /// a candidate against unchanged items rather than a different suite.
    ///
    /// Absent when the items carry their own text, which is
    /// `support-v2-three-way`'s older shape; the committed manifest names
    /// `support-v2-three-way-v1`, the same text under an id, and a test
    /// asserts the two agree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub questions: Option<String>,
    /// What the suite's states were drawn from, when they were not taken
    /// whole.
    ///
    /// A suite harvested from a record is a sample of a population, and a
    /// sample drawn to put failures in front of a door does not carry the
    /// population's rates. This names the counts the sample came from, so a
    /// reader can put the per-class rates back on the population instead of
    /// reading the suite's own mix as the workload's.
    ///
    /// Outside the digest: it describes where the items came from, not what
    /// they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<Value>,
    /// Which of this suite's partitions was training data for a door,
    /// when one was. Read by the ledger, which refuses to spend an exposed
    /// locked partition for an adapted door. Outside the digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<Exposure>,
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
        let carried = suite
            .items
            .iter()
            .filter(|item| item.question.is_some())
            .count();
        if carried != 0 && carried != suite.items.len() {
            return Err(SuiteError::MixedQuestions {
                carried,
                total: suite.items.len(),
            });
        }
        if carried == 0 && suite.questions.is_none() {
            return Err(SuiteError::NoQuestions);
        }
        if let Some(exposure) = &suite.exposure {
            let held = suite.counts()[&exposure.partition];
            if exposure.items == 0 {
                return Err(SuiteError::ExposureMalformed(
                    "it exposes zero items, which is no exposure; remove the record".to_string(),
                ));
            }
            if exposure.items > held {
                return Err(SuiteError::ExposureMalformed(format!(
                    "it exposes {} items of a {} partition that holds {held}",
                    exposure.items, exposure.partition
                )));
            }
            if exposure.through.trim().is_empty() {
                return Err(SuiteError::ExposureMalformed(
                    "it does not say what exposed the items".to_string(),
                ));
            }
        }
        Ok(suite)
    }

    /// Whether the locked partition is training data for adapted doors.
    #[must_use]
    pub fn locked_is_exposed(&self) -> bool {
        self.exposure
            .as_ref()
            .is_some_and(|exposure| exposure.partition == Partition::Locked)
    }

    /// Refuses to spend the locked partition for a door whose adapter may
    /// have trained on it. A door that serves no adapter, which is what an
    /// empty `adapter` says, never trained on anything and is not refused.
    fn check_exposure(&self, adapter: &str) -> Result<(), SuiteError> {
        let Some(exposure) = &self.exposure else {
            return Ok(());
        };
        if exposure.partition != Partition::Locked || adapter.trim().is_empty() {
            return Ok(());
        }
        Err(SuiteError::Exposed {
            suite: self.name.clone(),
            items: exposure.items,
            locked: self.counts()[&Partition::Locked],
            through: exposure.through.clone(),
            adapter: adapter.to_owned(),
            successor: exposure
                .successor
                .as_deref()
                .map(|successor| format!("; spend `{successor}` instead"))
                .unwrap_or_default(),
        })
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

    /// How many items rest on each kind of evidence.
    ///
    /// A suite that mixes outcome labels and read labels is stronger than
    /// one that has only the second, and weaker than it looks if nobody
    /// prints the mix. Counting does not read the locked partition.
    #[must_use]
    pub fn evidence_counts(&self) -> BTreeMap<String, usize> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for item in &self.items {
            *counts
                .entry(item.evidence().label().to_string())
                .or_insert(0) += 1;
        }
        counts
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

/// Loads the suite an adapted door's locked read is spent on.
pub fn support_v2_unseen() -> Result<Suite, SuiteError> {
    Suite::load(SUPPORT_V2_UNSEEN)
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
    /// The adapter the door serves, as [`crate::row::DoorIdentity::adapter`]
    /// reports it, or empty for a base or hosted door that serves none.
    ///
    /// The ledger reads this against [`Suite::exposure`]: a locked
    /// partition that was training data for adapters is refused to any
    /// door that serves one. Naming the adapter is what makes the refusal
    /// possible, so a read that leaves it blank is a read for a door that
    /// never trained, and the caller is saying so.
    pub adapter: &'a str,
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
    /// The adapter the door served, or absent for a door that served none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub adapter: String,
    /// Set when this read overrides an earlier one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Override>,
}

/// How long a read waits for the process holding the ledger's lock before
/// reporting the hold. An expired wait does not establish whether the
/// holder is still running, wedged, or dead. Check it before removing a
/// lock file.
const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

/// How often a read that is waiting on the lock retries it.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// An append-only record of every locked-partition read.
///
/// Point it at a file that is committed. The ledger's value is that a
/// second read shows up in review, which it cannot do from a temporary
/// directory.
///
/// A read is a transaction, not a look followed by an append: a sibling
/// `*.lock` file is taken first and held until the record is synced to
/// durable storage, so two racing readers cannot both be first and a read
/// that returned is a read that survives a crash. The lock lives beside
/// the ledger under its canonical name, so spellings that resolve to the
/// same file take the same lock; a ledger with more than one name is
/// refused rather than half-serialized.
#[derive(Clone, Debug)]
pub struct LockedLedger {
    path: PathBuf,
    wait: Duration,
}

impl LockedLedger {
    /// A ledger kept at this path. The file is created on the first append.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            wait: DEFAULT_LOCK_WAIT,
        }
    }

    /// How long a read waits for the process holding the ledger's lock
    /// before reporting the hold. The default is ten seconds. A hold that
    /// outlasts the wait is reported with the lock's path and the pid it
    /// claims; whether that holder is alive is a check for the person
    /// removing it.
    #[must_use]
    pub fn lock_wait(mut self, wait: Duration) -> Self {
        self.wait = wait;
        self
    }

    /// Where the ledger is kept.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every read the ledger holds, oldest first.
    ///
    /// The file is read without the lock, so a `reads` that meets a writer
    /// mid-append can see the torn line that writer is still writing; the
    /// report is [`SuiteError::Interrupted`] either way, because an
    /// interrupted write and a write in flight cannot be told apart.
    pub fn reads(&self) -> Result<Vec<LockedRead>, SuiteError> {
        let Some(text) = self.read_text()? else {
            return Ok(Vec::new());
        };
        self.parse(&text)
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
    /// The read is counted against the suite's digest, and it is one
    /// transaction: the ledger's lock is taken, the file is read, the check
    /// runs against what is committed, the record is appended and synced,
    /// and only then are the items handed back. If that digest has been
    /// read before, this is refused: a second read is an override, and
    /// [`LockedLedger::read_locked_again`] is how you take one deliberately.
    ///
    /// A suite whose [`Suite::exposure`] marks the locked partition as
    /// training data is refused with [`SuiteError::Exposed`] when the spend
    /// names an adapter, before the ledger is touched. Nothing is recorded:
    /// a read that would have meant nothing did not happen.
    pub fn read_locked<'a>(
        &self,
        suite: &'a Suite,
        spend: &Spend<'_>,
    ) -> Result<Vec<&'a Item>, SuiteError> {
        spend.check()?;
        suite.check_exposure(spend.adapter)?;
        self.spend(suite, spend, |earlier| match earlier.first() {
            Some(first) => Err(SuiteError::AlreadyRead {
                suite: suite.name.clone(),
                at: first.at.clone(),
                subject: first.subject.clone(),
            }),
            None => Ok(None),
        })
    }

    /// Reads the locked partition again, against an explicit override that
    /// is recorded beside the read.
    ///
    /// This is refused when there is nothing to override. An override with
    /// no earlier read is a caller reaching for the loud door by habit.
    ///
    /// An exposed locked partition is refused here too, for the same door.
    /// Authority can spend a partition twice; it cannot make memorized
    /// items into held-out ones.
    pub fn read_locked_again<'a>(
        &self,
        suite: &'a Suite,
        spend: &Spend<'_>,
        authority: &str,
        because: &str,
    ) -> Result<Vec<&'a Item>, SuiteError> {
        spend.check()?;
        suite.check_exposure(spend.adapter)?;
        if authority.trim().is_empty() {
            return Err(SuiteError::Unrecorded { field: "authority" });
        }
        if because.trim().is_empty() {
            return Err(SuiteError::Unrecorded { field: "because" });
        }
        self.spend(suite, spend, |earlier| {
            if earlier.is_empty() {
                return Err(SuiteError::NothingToOverride {
                    suite: suite.name.clone(),
                });
            }
            Ok(Some(Override {
                read: earlier.len(),
                authority: authority.to_owned(),
                because: because.to_owned(),
            }))
        })
    }

    /// The transaction both read paths share. The lock goes on before the
    /// file is read and stays on until the record is durable, so `decide`
    /// runs against what is committed rather than what was committed when
    /// the caller last looked, and two callers cannot both pass it. A
    /// refusal drops the lock without touching the file, and the items are
    /// handed back only after the record is on disk and synced.
    fn spend<'a>(
        &self,
        suite: &'a Suite,
        spend: &Spend<'_>,
        decide: impl FnOnce(&[LockedRead]) -> Result<Option<Override>, SuiteError>,
    ) -> Result<Vec<&'a Item>, SuiteError> {
        // A symlink whose target does not exist would be created through
        // the link, under a lock covering only this spelling of it: two
        // dangling aliases of one target would not serialize.
        if let Ok(meta) = fs::symlink_metadata(&self.path)
            && meta.file_type().is_symlink()
            && !self.path.exists()
        {
            return Err(self.trouble(
                "the path is a symlink whose target does not exist; name the target, because two \
                 dangling aliases of it would not serialize",
            ));
        }
        // The directories a first write would create, captured before the
        // lock or the file can create any of them: the durable commit syncs
        // every directory that gained a name in this transaction.
        let new_dirs = self.new_dirs();
        let lock = self.lock()?;
        let text = self.read_text()?;
        let reads = self.parse(text.as_deref().unwrap_or_default())?;
        let earlier: Vec<LockedRead> = reads
            .into_iter()
            .filter(|read| read.digest == suite.digest)
            .collect();
        let overrides = decide(&earlier)?;
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
            adapter: spend.adapter.to_owned(),
            overrides,
        };
        self.commit(text.as_deref(), &record, &new_dirs)?;
        drop(lock);
        Ok(items)
    }

    /// The ledger's canonical path, for locking: the lock lives beside the
    /// ledger under the name the filesystem gives it, so callers that spell
    /// the same ledger differently — a symlink, a `..` segment, a relative
    /// path — still serialize. Components that do not exist yet are
    /// reattached to the deepest ancestor that does.
    fn resolved_path(&self) -> PathBuf {
        let absolute = std::path::absolute(&self.path).unwrap_or_else(|_| self.path.clone());
        let mut cursor = absolute.clone();
        let mut tail = Vec::new();
        while !cursor.exists() {
            match (cursor.file_name(), cursor.parent()) {
                (Some(name), Some(parent)) => {
                    tail.push(name.to_os_string());
                    cursor = parent.to_path_buf();
                }
                _ => break,
            }
        }
        let mut resolved = fs::canonicalize(&cursor).unwrap_or(cursor);
        for name in tail.iter().rev() {
            resolved.push(name);
        }
        resolved
    }

    /// The directories from the ledger's parent up to the first one that
    /// already exists, deepest first. Captured before anything is created,
    /// so a durable commit can sync every directory that gained a name in
    /// this transaction: the file's name lives in its parent, and each new
    /// directory's name lives in the next one up.
    fn new_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let mut dir = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        loop {
            let existed = dir.exists();
            dirs.push(dir.to_path_buf());
            if existed {
                break;
            }
            match dir.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => dir = parent,
                Some(_) => dir = Path::new("."),
                None => break,
            }
        }
        dirs
    }

    /// Takes the ledger's lock, waiting a holder out and reporting the hold
    /// once the wait runs out. The lock is the result store's: a sibling
    /// `*.lock` file created with `create_new` beside the ledger's
    /// canonical name, which holds across processes on one machine and not
    /// only across threads. The wait is measured as elapsed time, so an
    /// unbounded `lock_wait` is a promise to wait rather than an overflow.
    fn lock(&self) -> Result<WriteLock, SuiteError> {
        let ledger = self.resolved_path();
        let started = Instant::now();
        loop {
            match WriteLock::acquire(&ledger) {
                Ok(lock) => return Ok(lock),
                Err(StoreError::Locked { lock, holder }) => {
                    if started.elapsed() >= self.wait {
                        return Err(SuiteError::Locked { lock, holder });
                    }
                    std::thread::sleep(LOCK_POLL_INTERVAL);
                }
                Err(error) => return Err(self.trouble(&error.to_string())),
            }
        }
    }

    /// The ledger's raw text, when the file exists.
    fn read_text(&self) -> Result<Option<String>, SuiteError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(self.trouble(&error.to_string())),
        }
    }

    /// Every record in `text`, oldest first.
    ///
    /// A line that does not parse is a corrupted ledger — unless it is the
    /// last line of a file that does not end in a newline, which is what an
    /// interrupted append leaves. That line is reported as
    /// [`SuiteError::Interrupted`]: the read it was writing may or may not
    /// have committed, so it is neither counted nor written off.
    fn parse(&self, text: &str) -> Result<Vec<LockedRead>, SuiteError> {
        let complete = text.is_empty() || text.ends_with('\n');
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        let mut reads = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate() {
            match serde_json::from_str(line) {
                Ok(read) => reads.push(read),
                Err(_) if !complete && index == lines.len() - 1 => {
                    return Err(SuiteError::Interrupted {
                        path: self.path.display().to_string(),
                    });
                }
                Err(error) => return Err(self.trouble(&error.to_string())),
            }
        }
        Ok(reads)
    }

    /// Appends one record and makes it durable before returning: the record
    /// is written, the file is synced, and when this write is the ledger's
    /// first every directory that gained a name in this transaction is
    /// synced too, so the file's name and each new directory's name survive
    /// the same crash the contents do. The record is written first on
    /// purpose: a read that fails to record is not a read.
    ///
    /// `prior` is the file's text as read under the lock. A prior text that
    /// does not end in a newline holds a record that parsed but whose
    /// terminating newline never landed; completing its line before the new
    /// record repairs the file without changing what the line says.
    ///
    /// `new_dirs` is the [`LockedLedger::new_dirs`] snapshot taken before
    /// the lock, deepest first.
    fn commit(
        &self,
        prior: Option<&str>,
        record: &LockedRead,
        new_dirs: &[PathBuf],
    ) -> Result<(), SuiteError> {
        let mut line = serde_json::to_string(record)
            .map_err(|error| SuiteError::Malformed(error.to_string()))?;
        line.push('\n');
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
        self.refuse_hardlinked(&file)?;
        if prior.is_some_and(|text| !text.is_empty() && !text.ends_with('\n')) {
            file.write_all(b"\n")
                .map_err(|error| self.trouble(&error.to_string()))?;
        }
        file.write_all(line.as_bytes())
            .map_err(|error| self.trouble(&error.to_string()))?;
        file.sync_all()
            .map_err(|error| self.trouble(&error.to_string()))?;
        if prior.is_none_or(str::is_empty) {
            self.sync_dirs(new_dirs)?;
        }
        Ok(())
    }

    /// One ledger, one name. A hardlinked alias reaches the same bytes
    /// through a path the canonical lock cannot see — two names for one
    /// inode resolve to two locks — so a ledger that has more than one is
    /// refused rather than half-serialized.
    #[cfg(unix)]
    fn refuse_hardlinked(&self, file: &fs::File) -> Result<(), SuiteError> {
        use std::os::unix::fs::MetadataExt;

        let metadata = file
            .metadata()
            .map_err(|error| self.trouble(&error.to_string()))?;
        if metadata.nlink() > 1 {
            return Err(self.trouble(
                "the ledger has more than one name; a hardlinked alias is not serialized, so point \
                 every reader at one path",
            ));
        }
        Ok(())
    }

    /// The non-Unix stand-in: hardlink detection relies on the link count,
    /// which other platforms do not expose this way.
    #[cfg(not(unix))]
    fn refuse_hardlinked(&self, _file: &fs::File) -> Result<(), SuiteError> {
        Ok(())
    }

    /// Syncs each directory that gained a name in this transaction, deepest
    /// first: the ledger file's name lives in its parent, and each new
    /// directory's name lives in the next one up, so a ledger created under
    /// a new directory chain is durable end to end. A relative ledger path
    /// resolves its parent to the working directory, so `ledger.jsonl`
    /// syncs `.` rather than skipping the sync. A filesystem that cannot
    /// sync a directory gets the error, not silence.
    #[cfg(unix)]
    fn sync_dirs(&self, dirs: &[PathBuf]) -> Result<(), SuiteError> {
        for dir in dirs {
            fs::File::open(dir)
                .and_then(|dir| dir.sync_all())
                .map_err(|error| self.trouble(&error.to_string()))?;
        }
        Ok(())
    }

    /// The non-Unix stand-in: the record's own sync still holds; the
    /// directory-entry guarantee does not exist there.
    #[cfg(not(unix))]
    fn sync_dirs(&self, _dirs: &[PathBuf]) -> Result<(), SuiteError> {
        Ok(())
    }

    fn trouble(&self, message: &str) -> SuiteError {
        SuiteError::Ledger {
            path: self.path.display().to_string(),
            message: message.to_owned(),
        }
    }
}

/// Serializes with object keys sorted, which is what the digest is over.
pub(crate) fn canonicalize(value: &Value) -> String {
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
    use serde_json::json;

    fn suite() -> Suite {
        support_v2_three_way().expect("the committed suite loads")
    }

    /// The `coder` suite, loaded from the file rather than compiled in.
    ///
    /// It is 3.7 MB of real states. `support-v2-three-way` is small enough
    /// to embed and this is not, and a suite does not have to be embedded to
    /// be scored: `gym eval --suite` takes a path.
    fn coder_turns() -> Suite {
        Suite::load_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/suites/coder-turns-v1.json"
        ))
        .expect("the committed coder suite loads")
    }

    #[test]
    fn the_coder_suite_loads_and_its_digest_matches() {
        let suite = coder_turns();
        assert_eq!(suite.name, "coder-turns-v1");
        assert_eq!(suite.questions.as_deref(), Some("coder-turns-v1"));
        let counts = suite.counts();
        assert!(counts.values().all(|count| *count > 0));
        assert_eq!(counts.values().sum::<usize>(), suite.items.len());
    }

    #[test]
    fn the_coder_suite_carries_both_kinds_of_evidence_and_says_which() {
        let suite = coder_turns();
        let counts = suite.evidence_counts();
        assert!(counts["outcome"] > 0, "no outcome-labelled items");
        assert!(counts["author"] > 0, "no author-labelled items");
        for item in &suite.items {
            assert!(
                item.label_source.is_some(),
                "{} does not say what its label rests on",
                item.id
            );
            assert!(
                item.label_rule.is_some(),
                "{} does not name its label rule",
                item.id
            );
        }
    }

    #[test]
    fn one_state_never_straddles_two_partitions() {
        // The families share states: one turn is asked four questions. An
        // item of that turn in `calibration` and another in `development`
        // would put the same state on both sides of a fit.
        let suite = coder_turns();
        let mut seen: BTreeMap<String, Partition> = BTreeMap::new();
        for item in &suite.items {
            let state = item
                .id
                .split_once('/')
                .map_or("", |(_, rest)| rest)
                .to_string();
            match seen.get(&state) {
                Some(partition) => assert_eq!(
                    *partition, item.partition,
                    "the state behind {} is in two partitions",
                    item.id
                ),
                None => {
                    seen.insert(state, item.partition);
                }
            }
        }
    }

    fn ledger() -> (tempfile::TempDir, LockedLedger) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let ledger = LockedLedger::at(directory.path().join("locked-reads.jsonl"));
        (directory, ledger)
    }

    const SPEND: Spend<'static> = Spend {
        subject: "lev-base",
        reason: "the published candidate, scored once before the model card",
        at: "2026-09-19",
        adapter: "",
    };

    /// A spend for a door that serves an adapter trained from `support-v2`.
    const ADAPTED_SPEND: Spend<'static> = Spend {
        subject: "lev-band-adapter-e4",
        reason: "the published candidate, scored once before the model card",
        at: "2026-09-19",
        adapter: "band-v1",
    };

    fn unseen() -> Suite {
        support_v2_unseen().expect("the unseen suite loads")
    }

    #[test]
    fn the_three_way_suite_says_its_locked_partition_is_exposed() {
        let suite = suite();
        let exposure = suite.exposure.as_ref().expect("the manifest says it");
        assert_eq!(exposure.partition, Partition::Locked);
        assert_eq!(exposure.items, 20);
        assert_eq!(exposure.successor.as_deref(), Some("support-v2-unseen"));
        assert!(suite.locked_is_exposed());
        assert_eq!(
            suite.digest, "54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9",
            "saying so did not reissue the digest the week's rows pin"
        );
    }

    #[test]
    fn an_exposed_locked_partition_is_refused_to_an_adapted_door_and_not_recorded() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        let error = ledger.read_locked(&suite, &ADAPTED_SPEND).unwrap_err();
        assert!(matches!(error, SuiteError::Exposed { .. }), "{error}");
        let message = error.to_string();
        assert!(message.contains("band-v1"), "{message}");
        assert!(message.contains("support-v2-unseen"), "{message}");
        assert!(message.contains("20 of its 39"), "{message}");
        assert!(!ledger.path().exists(), "a refused read leaves no record");

        // Authority does not turn memorized items into held-out ones.
        let error = ledger
            .read_locked_again(&suite, &ADAPTED_SPEND, "chris", "the number is wanted")
            .unwrap_err();
        assert!(matches!(error, SuiteError::Exposed { .. }), "{error}");
        assert!(!ledger.path().exists());
    }

    #[test]
    fn an_exposed_locked_partition_still_serves_a_door_that_never_trained() {
        // Base and hosted doors trained on nothing, so for them the
        // partition is as held out as it ever was.
        let suite = suite();
        let (_directory, ledger) = ledger();
        let items = ledger
            .read_locked(&suite, &SPEND)
            .expect("a base door reads");
        assert_eq!(items.len(), 39);
        let reads = ledger.reads().expect("the ledger reads back");
        assert_eq!(reads[0].adapter, "");
    }

    #[test]
    fn the_unseen_suite_is_the_three_way_suite_minus_what_the_adapters_saw() {
        let three_way = suite();
        let unseen = unseen();
        assert_eq!(unseen.name, "support-v2-unseen");
        assert_eq!(unseen.items.len(), 98);
        assert!(unseen.exposure.is_none(), "nothing trained on these");
        assert_ne!(unseen.digest, three_way.digest);
        let counts = unseen.counts();
        assert_eq!(counts[&Partition::Calibration], 40);
        assert_eq!(counts[&Partition::Development], 39);
        assert_eq!(counts[&Partition::Locked], 19);

        // Every item is one of the three-way suite's, with the same label
        // and the same partition, so a development read of one is a
        // development read of the other.
        let two_way: Value =
            serde_json::from_str(include_str!("../suites/support-v2.json")).expect("it parses");
        let trained: Vec<&str> = two_way["items"]
            .as_array()
            .expect("items")
            .iter()
            .filter(|item| item["split"] == "calibration")
            .map(|item| item["id"].as_str().expect("an id"))
            .collect();
        assert_eq!(trained.len(), 98);
        for item in &unseen.items {
            assert!(
                !trained.contains(&item.id.as_str()),
                "{} was in the adapters' training split",
                item.id
            );
            let original = three_way
                .items
                .iter()
                .find(|candidate| candidate.id == item.id)
                .expect("the item is in the three-way suite");
            assert_eq!(original.truth, item.truth);
            assert_eq!(original.partition, item.partition);
            assert_eq!(original.question, item.question);
        }

        let (_directory, ledger) = ledger();
        let items = ledger
            .read_locked(&unseen, &ADAPTED_SPEND)
            .expect("the clean partition serves an adapted door");
        assert_eq!(items.len(), 19);
    }

    #[test]
    fn an_exposure_that_does_not_describe_its_suite_is_refused() {
        let mut value: Value = serde_json::from_str(SUPPORT_V2_THREE_WAY).expect("it parses");
        value["exposure"]["items"] = json!(40);
        let error = Suite::load(&value.to_string()).unwrap_err();
        assert!(matches!(error, SuiteError::ExposureMalformed(_)), "{error}");
        assert!(error.to_string().contains("holds 39"), "{error}");

        value["exposure"]["items"] = json!(0);
        assert!(matches!(
            Suite::load(&value.to_string()),
            Err(SuiteError::ExposureMalformed(_))
        ));

        value["exposure"]["items"] = json!(20);
        value["exposure"]["through"] = json!("  ");
        assert!(matches!(
            Suite::load(&value.to_string()),
            Err(SuiteError::ExposureMalformed(_))
        ));

        // An exposure of an open partition is recorded but does not refuse
        // a locked read; the locked items are not the ones that leaked.
        value["exposure"] = json!({
            "partition": "development",
            "items": 39,
            "through": "a test"
        });
        let suite = Suite::load(&value.to_string()).expect("it loads");
        assert!(!suite.locked_is_exposed());
        let (_directory, ledger) = ledger();
        ledger
            .read_locked(&suite, &ADAPTED_SPEND)
            .expect("the locked partition itself is clean");
    }

    #[test]
    fn the_committed_suite_loads_and_its_digest_matches() {
        let suite = suite();
        assert_eq!(suite.name, "support-v2-three-way");
        assert_eq!(suite.items.len(), 196);
        assert_eq!(suite.tier.as_deref(), Some("scored"));
        assert_eq!(suite.gate.as_deref(), Some("probability-v2"));
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
        // slipped locked items into calibration would not load. The
        // item's field is matched by its indentation; the manifest's
        // `exposure` record names the same partition one level up, and
        // that record is outside the digest.
        let moved = SUPPORT_V2_THREE_WAY.replacen(
            "   \"partition\": \"locked\"",
            "   \"partition\": \"calibration\"",
            1,
        );
        assert!(matches!(
            Suite::load(&moved),
            Err(SuiteError::Tampered { .. })
        ));
    }

    #[test]
    fn the_manifest_names_its_gate_and_does_not_restate_it() {
        let suite = suite();
        assert_eq!(suite.gate.as_deref(), Some("probability-v2"));
        crate::gate::load(suite.gate.as_deref().expect("the manifest names a gate"))
            .expect("the named gate is a committed rule");
        assert!(
            !SUPPORT_V2_THREE_WAY.contains("calibration-admission-v1"),
            "the superseded inline rule is gone from the manifest"
        );
        assert!(
            !SUPPORT_V2_THREE_WAY.contains("brier_rises_by_at_most"),
            "a threshold in the manifest is a second rule with the same job"
        );
    }

    #[test]
    fn pointing_the_gate_elsewhere_leaves_the_digest_alone() {
        // The digest covers the items, so which rule judges them is outside
        // it. That is what lets this change land without moving
        // 54fbf4137c…, and the assertion below is the promise.
        let repointed = SUPPORT_V2_THREE_WAY
            .replace("\"gate\": \"probability-v2\"", "\"gate\": \"decision-v1\"");
        assert_ne!(repointed, SUPPORT_V2_THREE_WAY, "the gate was in the file");
        let judged_by_another = Suite::load(&repointed).expect("a repointed gate is not drift");
        assert_eq!(judged_by_another.gate.as_deref(), Some("decision-v1"));
        assert_eq!(judged_by_another.digest, suite().digest);
    }

    #[test]
    fn the_committed_digest_did_not_move_when_the_gate_became_a_reference() {
        // Recorded literally, because a test that recomputes the digest from
        // the file it is checking cannot catch the file changing.
        assert_eq!(
            suite().digest,
            "54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9"
        );
    }

    #[test]
    fn naming_a_question_set_leaves_the_digest_alone() {
        // The promise this change is built on. `support-v2-three-way` gained
        // a `questions` field on 2026-09-19, and every row and calibration
        // record written that week pins 54fbf4137c…. If naming a question set
        // moved the digest, the migration would have invalidated the chain it
        // exists to protect.
        assert!(
            SUPPORT_V2_THREE_WAY.contains("\"questions\": \"support-v2-three-way-v1\""),
            "the manifest names its question set"
        );
        let unnamed =
            SUPPORT_V2_THREE_WAY.replace(" \"questions\": \"support-v2-three-way-v1\",\n", "");
        assert_ne!(unnamed, SUPPORT_V2_THREE_WAY, "the field was in the file");
        let without = Suite::load(&unnamed).expect("a suite may carry its text inline");
        assert_eq!(without.questions, None);
        assert_eq!(without.digest, suite().digest);
        assert_eq!(
            suite().digest,
            "54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9"
        );
    }

    #[test]
    fn rewording_a_question_in_the_set_does_not_touch_the_suite() {
        // The seam openagents#9386 closed, stated as the suite sees it. The
        // reword lives in a question set, the items do not move, and the
        // digest every row pins does not move either — so the variant is a
        // candidate against these items rather than a second suite.
        let pinned = suite();
        let authored = crate::questions::QuestionSet::authored(&pinned)
            .expect("the items carry their own text");
        let mut reworded = authored.clone();
        reworded.questions.get_mut("routing").expect("routing")["instructions"] =
            serde_json::json!("Which team handles this?");
        assert_ne!(
            reworded.digest(),
            authored.digest(),
            "the reword is a new set"
        );
        assert_eq!(pinned.compute_digest().expect("a digest"), pinned.digest);
        assert_eq!(
            pinned.digest, "54fbf4137c3de538f2dea07d47ca1ee835c09eb25aa26a320441679129f618f9",
            "the items did not move, so neither did what every row pins"
        );
    }

    #[test]
    fn a_suite_carries_its_question_text_on_every_item_or_on_none() {
        // A per-item override of a set-provided question is the per-item
        // question data the question set removes, so a mixture is refused
        // rather than resolved.
        let mut mixed = suite();
        mixed.items[0].question = None;
        let text = serde_json::to_string(&mixed).expect("a suite serializes");
        assert!(
            matches!(Suite::load(&text), Err(SuiteError::Tampered { .. })),
            "dropping one item's text moves the digest, before anything else looks at it"
        );

        let mut none = suite();
        for item in &mut none.items {
            item.question = None;
        }
        none.digest = none.compute_digest().expect("a digest");
        let text = serde_json::to_string(&none).expect("a suite serializes");
        Suite::load(&text).expect("a suite whose text lives in a set loads");

        none.questions = None;
        none.digest = none.compute_digest().expect("a digest");
        let text = serde_json::to_string(&none).expect("a suite serializes");
        assert!(
            matches!(Suite::load(&text), Err(SuiteError::NoQuestions)),
            "a suite with no text anywhere asks nothing"
        );

        let mut half = none.clone();
        half.questions = Some("support-v2-three-way-v1".to_string());
        half.items[0].question = Some(serde_json::json!({ "type": "choice" }));
        half.digest = half.compute_digest().expect("a digest");
        let text = serde_json::to_string(&half).expect("a suite serializes");
        assert!(matches!(
            Suite::load(&text),
            Err(SuiteError::MixedQuestions {
                carried: 1,
                total: 196
            })
        ));
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
            adapter: "",
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
    fn sixteen_callers_get_exactly_one_first_read() {
        // The A06 race. Before the lock covered the transaction, callers
        // released together each read an empty ledger and most of them
        // appended as the first reader. Now the eligibility check runs
        // under the lock, so exactly one wins and the rest meet the
        // committed record.
        let suite = suite();
        let (_directory, ledger) = ledger();
        let ledger = &ledger;
        let suite = &suite;
        let barrier = std::sync::Barrier::new(16);
        let outcomes: Vec<Result<usize, SuiteError>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        ledger.read_locked(suite, &SPEND).map(|items| items.len())
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a caller finished"))
                .collect()
        });
        let accepted = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
        let refused = outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Err(SuiteError::AlreadyRead { .. })))
            .count();
        assert_eq!(accepted, 1, "exactly one first read: {outcomes:?}");
        assert_eq!(refused, 15, "everyone else met the committed record");
        assert_eq!(ledger.reads().expect("the ledger reads back").len(), 1);
        assert_eq!(
            outcomes.iter().find_map(|outcome| outcome.as_ref().ok()),
            Some(&39),
            "the winner got the items"
        );
    }

    #[test]
    fn an_interrupted_append_fails_closed_until_the_line_is_repaired() {
        // A writer killed mid-append leaves a last line that does not
        // parse. The ledger cannot tell whether the read it was writing
        // committed, so it refuses to read past the tear rather than permit
        // a second spend.
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(ledger.path())
            .expect("the ledger opens");
        file.write_all(b"{\"schema\":\"openagents.gym.locked_read.v1\",\"suite\":\"sup")
            .expect("the torn write lands");
        file.sync_all().expect("the torn write is durable");
        drop(file);

        let error = ledger.read_locked(&suite, &SPEND).unwrap_err();
        assert!(
            matches!(error, SuiteError::Interrupted { .. }),
            "a torn tail is not AlreadyRead and not a second spend: {error}"
        );
        assert!(error.to_string().contains("interrupted"), "{error}");
        assert!(
            matches!(ledger.reads().unwrap_err(), SuiteError::Interrupted { .. }),
            "inspection fails closed too"
        );

        // Repair is a person's, not the ledger's: truncate the file to the
        // last committed line. The read that committed still counts.
        let bytes = fs::read(ledger.path()).expect("the ledger reads");
        let committed = bytes.iter().rposition(|byte| *byte == b'\n').unwrap() + 1;
        fs::write(ledger.path(), &bytes[..committed]).expect("the repair writes");
        assert!(
            matches!(
                ledger.read_locked(&suite, &SPEND),
                Err(SuiteError::AlreadyRead { .. })
            ),
            "the committed record survives the repair"
        );
    }

    #[test]
    fn a_record_that_lost_its_newline_still_counts_and_the_next_append_repairs_it() {
        // The other shape an interrupted write leaves: the record landed
        // whole and the newline did not. The record parses, so the spend
        // counts — reading past it would spend the partition twice.
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        let text = fs::read_to_string(ledger.path()).expect("the ledger reads");
        fs::write(ledger.path(), text.trim_end_matches('\n')).expect("the newline is removed");

        assert!(
            matches!(
                ledger.read_locked(&suite, &SPEND),
                Err(SuiteError::AlreadyRead { .. })
            ),
            "a committed record counts whether or not its newline landed"
        );

        ledger
            .read_locked_again(&suite, &SPEND, "chris", "the newline never landed")
            .expect("an override still works");
        let repaired = fs::read_to_string(ledger.path()).expect("the ledger reads");
        assert!(
            repaired.ends_with('\n'),
            "the append repaired the line it would have joined"
        );
        assert_eq!(ledger.reads().expect("the ledger reads back").len(), 2);
    }

    #[test]
    fn a_held_lock_is_reported_after_the_wait() {
        // The lock is a file, so a killed reader leaves it behind. A read
        // waits the hold out, then reports the file it is stuck on —
        // naming it, and naming the pid it claims, so a person can check
        // whether the holder is a live read before removing it. A zero
        // wait reports the hold at once.
        let suite = suite();
        let (_directory, ledger) = ledger();
        // The lock lives beside the ledger's canonical name, which a
        // tempdir under `/var` already aliases on macOS.
        let lock_path = PathBuf::from(format!("{}.lock", ledger.resolved_path().display()));
        fs::write(&lock_path, "999999\n").expect("a stale lock file");

        let ledger = ledger.lock_wait(Duration::from_millis(50));
        let error = ledger.read_locked(&suite, &SPEND).unwrap_err();
        assert!(matches!(error, SuiteError::Locked { .. }), "{error}");
        assert!(error.is_locked(), "a caller may retry on it");
        let message = error.to_string();
        assert!(message.contains(".lock"), "{message}");
        assert!(message.contains("999999"), "{message}");

        let zero = ledger.clone().lock_wait(Duration::ZERO);
        assert!(
            matches!(
                zero.read_locked(&suite, &SPEND),
                Err(SuiteError::Locked { .. })
            ),
            "a zero wait reports the hold without waiting"
        );

        fs::remove_file(&lock_path).expect("the repair");
        ledger
            .read_locked(&suite, &SPEND)
            .expect("a cleared lock reads");
    }

    #[test]
    fn an_extreme_lock_wait_neither_panics_nor_waits_on_a_free_ledger() {
        // The wait is measured as elapsed time rather than added to an
        // instant, so an unbounded value is a promise to wait, not an
        // overflow — and a free ledger answers at once.
        let suite = suite();
        let (_directory, ledger) = ledger();
        let ledger = ledger.lock_wait(Duration::MAX);
        ledger
            .read_locked(&suite, &SPEND)
            .expect("a free ledger reads at once");
    }

    #[test]
    fn a_refused_read_leaves_no_lock_and_no_record() {
        let suite = suite();
        let (_directory, ledger) = ledger();
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        assert!(matches!(
            ledger.read_locked(&suite, &SPEND),
            Err(SuiteError::AlreadyRead { .. })
        ));
        let lock_path = PathBuf::from(format!("{}.lock", ledger.resolved_path().display()));
        assert!(!lock_path.exists(), "a refusal released the lock");
        assert_eq!(ledger.reads().expect("the ledger reads back").len(), 1);
    }

    #[test]
    fn aliased_spellings_of_one_ledger_share_one_first_read() {
        // The lock lives beside the ledger under its canonical name, so
        // callers that spell the same file differently still serialize: a
        // canonical path against the lexical one, a `..` segment, and on
        // Unix a symlinked directory.
        let suite = suite();
        let (directory, _ledger) = ledger();
        let lexical = directory.path().join("ledger.jsonl");
        let canonical = fs::canonicalize(directory.path())
            .expect("the directory resolves")
            .join("ledger.jsonl");
        let dotdot = directory.path().join("subdir/../ledger.jsonl");
        let mut spellings = vec![lexical, canonical, dotdot];
        #[cfg(unix)]
        {
            let alias = directory.path().join("alias");
            std::os::unix::fs::symlink(directory.path(), &alias).expect("the symlink lands");
            spellings.push(alias.join("ledger.jsonl"));
        }
        let spellings = &spellings;
        let suite = &suite;
        let barrier = std::sync::Barrier::new(8);
        let outcomes: Vec<Result<usize, SuiteError>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|index| {
                    let path = &spellings[index % spellings.len()];
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        LockedLedger::at(path)
                            .read_locked(suite, &SPEND)
                            .map(|items| items.len())
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a caller finished"))
                .collect()
        });
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
            1,
            "one ledger, one first read, however it is spelled: {outcomes:?}"
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, Err(SuiteError::AlreadyRead { .. })))
                .count(),
            7,
            "every other spelling met the committed record"
        );
    }

    #[test]
    fn a_ledger_under_new_directories_is_created_and_committed() {
        // A ledger path whose parent chain does not exist is created end
        // to end, and the commit syncs every directory that gained a name:
        // the file's in its parent, each new directory's in the next one
        // up.
        let suite = suite();
        let (directory, _ledger) = ledger();
        let ledger = LockedLedger::at(directory.path().join("a/b/c/ledger.jsonl"));
        ledger.read_locked(&suite, &SPEND).expect("the first read");
        assert!(ledger.path().exists());
        assert_eq!(ledger.reads().expect("the ledger reads back").len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlinked_ledger_is_refused() {
        // Two dangling aliases of one target would take two locks and write
        // one file, so the write is refused and the operator names the
        // target.
        let suite = suite();
        let (directory, _ledger) = ledger();
        let alias = directory.path().join("alias.jsonl");
        std::os::unix::fs::symlink("missing/ledger.jsonl", &alias).expect("the symlink lands");
        let error = LockedLedger::at(&alias)
            .read_locked(&suite, &SPEND)
            .unwrap_err();
        assert!(error.to_string().contains("symlink"), "{error}");
        assert!(
            !directory.path().join("missing").exists(),
            "nothing was created through the link"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_hardlinked_ledger_is_refused() {
        // Two names for one inode resolve to two canonical paths and so to
        // two locks; rather than half-serialize, the ledger refuses to be
        // written under more than one name.
        let suite = suite();
        let (directory, ledger) = ledger();
        fs::write(ledger.path(), "").expect("the ledger file exists");
        let alias = directory.path().join("same-bytes.jsonl");
        fs::hard_link(ledger.path(), &alias).expect("the hard link lands");
        for path in [ledger.path().to_path_buf(), alias] {
            let error = LockedLedger::at(&path)
                .read_locked(&suite, &SPEND)
                .unwrap_err();
            assert!(error.to_string().contains("more than one name"), "{error}");
        }
        assert_eq!(
            fs::read_to_string(ledger.path()).expect("the ledger reads"),
            "",
            "nothing was written"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_commit_that_fails_returns_no_items_and_spends_nothing() {
        // Fault injection through the real write path: the ledger file
        // refuses the append, so no items come back and no spend is
        // recorded. The next read is a first read, not a repeated one —
        // a failed commit committed nothing.
        use std::os::unix::fs::PermissionsExt;

        let suite = suite();
        let (_directory, ledger) = ledger();
        fs::write(ledger.path(), "").expect("the ledger file exists");
        fs::set_permissions(ledger.path(), fs::Permissions::from_mode(0o444))
            .expect("the ledger is read-only");

        let error = ledger.read_locked(&suite, &SPEND).unwrap_err();
        assert!(matches!(error, SuiteError::Ledger { .. }), "{error}");

        fs::set_permissions(ledger.path(), fs::Permissions::from_mode(0o644))
            .expect("the ledger is writable again");
        ledger
            .read_locked(&suite, &SPEND)
            .expect("a clean first read");
        assert_eq!(ledger.reads().expect("the ledger reads back").len(), 1);
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
        // Its digest is pinned by rows and calibration records too, and it
        // is a file this change does not touch.
        assert!(
            two_way.contains("6877c24bf261d5bdcb0550824c20f017c7bb5095aac22dbf5f4c6ef47b789368"),
            "support-v2 still records the digest its rows pin"
        );
    }
}
