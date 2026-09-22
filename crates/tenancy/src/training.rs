//! Tenant training: authorized caller data to a sealed candidate.
//!
//! A tenant corpus is a labelled dataset in four partitions —
//! `training`, `calibration`, `development`, and `locked` — where a Gym
//! suite's three exist to measure and the fourth exists to fit on. The
//! roles are fixed in the document: `calibration` fits maps,
//! `development` chooses between candidates, `locked` is read once by
//! the admission flow that already owns it, and `training` is the only
//! partition a trainer may read. Every item carries its provenance —
//! source, license, and the permission that authorizes training on it —
//! because an undocumented item is an unauthorized one.
//!
//! The flow a corpus goes through, each step leaving a sealed document
//! in the store beside the registry:
//!
//! - `check` validates and registers the corpus: a training partition
//!   must exist, every item needs provenance and a label, a `group` may
//!   not span partitions (a session's near-duplicates belong to one
//!   role), and the leakage check refuses exact and near duplicates
//!   across partition boundaries. Labels sourced from a model are not
//!   ground truth — they carry a `confirmed` annotation or the corpus
//!   is refused.
//! - `headroom` scores the frozen baseline's results on the development
//!   partition and buckets every failure: state shape, ambiguous
//!   question, label uncertainty, and exact logic that belongs in Rust
//!   are their own causes; what remains is model-addressable headroom.
//!   A corpus with none is refused before a recipe is frozen — a
//!   documented decision not to train is a report like any other.
//! - `freeze` digests a recipe: base model, adapter and head shape,
//!   seeds, trial cap, compute budget, the winning metric, rejection
//!   rules, and transfer controls. Editing a frozen recipe makes a new
//!   one; nothing rewrites what a trial was run under.
//! - `trial` appends one record per attempt to the trials ledger —
//!   kept, rejected, and failed runs alike, each pinned to the recipe
//!   digest.
//! - `seal` binds the winning trial's artifacts — adapter, pointer
//!   head, tokenizer — with the corpus, recipe, code, and base-model
//!   identities into a candidate whose `artifact_signature` is what
//!   [`crate::admission`] later replays evidence against. Sealing
//!   registers a candidate for inspection; it admits and serves
//!   nothing.
//! - `delete` tombstones a corpus under its retention policy: item
//!   content leaves the store, ids, partitions, and digests stay, so a
//!   candidate's `corpus_digest` still resolves to what it was trained
//!   on without retaining the text.
//!
//! Nothing here routes a request or serves a door. A candidate becomes
//! an admitted binding only through [`crate::registry`]'s admission
//! path, which replays Gym evidence spent under a locked read.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::manifest::canonicalize;

/// The schema tag a tenant corpus carries.
pub const CORPUS_SCHEMA: &str = "openagents.tenant_training.corpus.v1";
/// The schema tag a frozen recipe carries.
pub const RECIPE_SCHEMA: &str = "openagents.tenant_training.recipe.v1";
/// The schema tag one trial-ledger record carries.
pub const TRIAL_SCHEMA: &str = "openagents.tenant_training.trial.v1";
/// The schema tag a sealed candidate carries.
pub const CANDIDATE_SCHEMA: &str = "openagents.tenant_training.candidate.v1";
/// The schema tag a headroom report carries.
pub const HEADROOM_SCHEMA: &str = "openagents.tenant_training.headroom.v1";
/// The schema tag a baseline-results file carries.
pub const SCORES_SCHEMA: &str = "openagents.tenant_training.scores.v1";

/// Two partitions' items are near-duplicates at this token-set overlap.
const LEAK_JACCARD: f64 = 0.8;
/// A corpus may not train on more items than this without an explicit
/// budget lift in the recipe — a bound, not a recommendation.
const MAX_TRAIN_ITEMS: usize = 50_000;
/// A state document over this size is a state-shape cause in headroom
/// accounting, not a model-addressable failure.
const STATE_BOUND_BYTES: usize = 64 * 1024;

/// The role an item plays. A corpus adds `training` to the Gym suite's
/// three; the other names are deliberately identical so a corpus and a
/// suite can be compared without translation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The only partition a trainer reads.
    Training,
    /// Fits maps and thresholds.
    Calibration,
    /// Chooses between candidates; read freely.
    Development,
    /// Spent once by the admission flow; never read here.
    Locked,
}

impl Role {
    /// Every role, in the order a report lists them.
    pub const ALL: [Self; 4] = [
        Self::Training,
        Self::Calibration,
        Self::Development,
        Self::Locked,
    ];

    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Training => "training",
            Self::Calibration => "calibration",
            Self::Development => "development",
            Self::Locked => "locked",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Where an item's label and text came from, in the form a reviewer
/// audits.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Provenance {
    /// The system or document the item was drawn from.
    pub source: String,
    /// The license the item is used under.
    pub license: String,
    /// The permission that authorizes training on it — a grant
    /// reference, never the word "assumed".
    pub permission: String,
}

/// What produced an item's label.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    /// Written by the tenant's own reviewer or the corpus author.
    #[default]
    Author,
    /// Two annotators disagreed and a ruling settled it.
    Adjudicated,
    /// Read off a measurement — a counter, a log, a test outcome.
    Measurement,
    /// A hosted model's output, which is a draft and not ground truth:
    /// the item is usable only with `annotations.confirmed` set by a
    /// reviewer.
    Model,
}

/// Cause annotations a reviewer may attach to an item. The headroom
/// report reads them to keep model-addressable failures honest — a
/// failure an annotation already explains is not headroom.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Annotations {
    /// The state document is missing or malformed for the question.
    #[serde(default)]
    pub state_shape: bool,
    /// The question admits more than one defensible answer.
    #[serde(default)]
    pub ambiguous: bool,
    /// The label itself is uncertain — low agreement, weak evidence.
    #[serde(default)]
    pub label_uncertain: bool,
    /// The label is derivable by a rule; the fix belongs in Rust, not in
    /// weights.
    #[serde(default)]
    pub rule_derivable: bool,
    /// A reviewer confirmed a `model`-sourced label.
    #[serde(default)]
    pub confirmed: bool,
}

/// One labelled item in one partition.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CorpusItem {
    /// A stable id, unique within the corpus.
    pub id: String,
    /// The session or source group the item belongs to. A group may not
    /// span partitions — near-duplicates of one session leaking across
    /// roles is how held-out numbers turn into memorization.
    pub group: String,
    /// Which partition the item belongs to. Inside the digest.
    pub partition: Role,
    /// The document to judge, in the same shape a suite state carries.
    pub state: Value,
    /// The question to ask, when the corpus carries per-item text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<Value>,
    /// The option key a knowledgeable person picks.
    pub label: String,
    /// What kind of evidence the label rests on. Inside the digest.
    #[serde(default)]
    pub label_source: LabelSource,
    /// The rule that produced the label, in one sentence — per item, so
    /// the digest binds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_rule: Option<String>,
    /// Where the item came from and what authorizes training on it.
    /// Inside the digest: an item whose provenance is edited is a
    /// different item.
    pub provenance: Provenance,
    /// Reviewer annotations the headroom report reads.
    #[serde(default, skip_serializing_if = "Annotations::is_empty")]
    pub annotations: Annotations,
}

impl Annotations {
    fn is_empty(&self) -> bool {
        !self.state_shape
            && !self.ambiguous
            && !self.label_uncertain
            && !self.rule_derivable
            && !self.confirmed
    }
}

/// The retention and deletion terms the tenant authorized, recorded on
/// the corpus so a candidate can say what it may keep.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Retention {
    /// How long raw item content may be retained, in days; `0` means
    /// delete as soon as the corpus stops being active.
    pub days: u64,
    /// Who may read the corpus content — `owner` means the tenant's
    /// workspace only; `operator` adds the training operator.
    pub access: String,
    /// Whether derived artifacts may outlive the corpus. `digests-only`
    /// is the default a candidate inherits.
    pub artifacts: String,
}

/// A versioned, digested tenant corpus. `digest` covers the items alone,
/// canonicalized — provenance and partition assignments included, so an
/// item edited after sealing is a different corpus.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Corpus {
    /// The schema tag.
    pub v: String,
    /// The workspace the corpus belongs to.
    pub workspace: String,
    /// The corpus's name, unique within the workspace's store.
    pub name: String,
    /// When it was authored.
    pub created: String,
    /// The label rules the corpus applies, in prose, beside the per-item
    /// `label_rule` lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_rules: Option<String>,
    /// The retention and deletion terms.
    pub retention: Retention,
    /// `sha256:` digest over the items, canonicalized.
    pub digest: String,
    /// The labelled items.
    pub items: Vec<CorpusItem>,
    /// Tombstone record, set by [`Book::delete`]: the corpus's content
    /// was removed under its retention terms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone: Option<Tombstone>,
}

/// What a deletion left behind: content digests without content.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tombstone {
    /// When the deletion was applied.
    pub deleted_at: String,
    /// Why — the retention clause or request that applied.
    pub reason: String,
    /// `sha256:` digest of each item's content at deletion, by item id,
    /// so what was removed is still provable.
    pub item_digests: BTreeMap<String, String>,
}

/// A pair of items the leakage check caught across a partition
/// boundary.
#[derive(Clone, Debug)]
pub struct Leak {
    /// The item in the earlier partition.
    pub first: String,
    /// The item in the later partition.
    pub second: String,
    /// The partitions the pair spans.
    pub between: (Role, Role),
    /// `exact` for identical normalized text, `near` for a token-set
    /// overlap at or over [`LEAK_JACCARD`].
    pub kind: &'static str,
}

/// What [`Corpus::check`] can refuse on.
#[derive(Debug)]
pub enum CorpusFault {
    /// Not JSON, or not the corpus shape.
    Malformed(String),
    /// A document tagged for another schema.
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// The recorded digest does not recompute over the items.
    Tampered {
        /// The digest the document claims.
        recorded: String,
        /// The digest its items produce.
        computed: String,
    },
    /// The corpus has no training partition — the role this whole
    /// document exists for.
    NoTraining,
    /// A partition holds no items.
    EmptyPartition {
        /// The empty role.
        role: Role,
    },
    /// Two items share an id.
    DuplicateItem {
        /// The repeated id.
        id: String,
    },
    /// An item lacks provenance, a permission, or a label.
    Undocumented {
        /// The item id.
        id: String,
        /// What it lacks.
        missing: String,
    },
    /// A group appears in more than one partition.
    GroupSpills {
        /// The group.
        group: String,
        /// The partitions it appears in.
        roles: Vec<Role>,
    },
    /// A `model`-sourced label was never confirmed by a reviewer.
    UnconfirmedLabel {
        /// The item id.
        id: String,
    },
    /// Exact or near-duplicate text across a partition boundary.
    Leak(Leak),
    /// The training partition exceeds the corpus bound.
    Oversized {
        /// How many training items it holds.
        items: usize,
        /// The bound.
        bound: usize,
    },
    /// The corpus was tombstoned; its content is gone.
    Deleted,
}

impl fmt::Display for CorpusFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "the corpus is not readable: {detail}"),
            Self::Schema { found } => {
                write!(
                    f,
                    "the document is tagged {found}, which is not {CORPUS_SCHEMA}"
                )
            }
            Self::Tampered { recorded, computed } => write!(
                f,
                "the corpus digest does not match its items: recorded {recorded}, computed {computed}"
            ),
            Self::NoTraining => write!(
                f,
                "the corpus has no training partition; a measurement suite is not a training corpus"
            ),
            Self::EmptyPartition { role } => {
                write!(f, "the corpus's {role} partition is empty")
            }
            Self::DuplicateItem { id } => {
                write!(f, "the corpus names the item {id} more than once")
            }
            Self::Undocumented { id, missing } => {
                write!(f, "corpus item {id} has no {missing}")
            }
            Self::GroupSpills { group, roles } => write!(
                f,
                "group {group} appears in {} partitions ({}); a session's items take one role",
                roles.len(),
                roles
                    .iter()
                    .map(|role| role.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::UnconfirmedLabel { id } => write!(
                f,
                "corpus item {id}'s label is model output with no reviewer confirmation"
            ),
            Self::Leak(leak) => write!(
                f,
                "{} leakage across {} and {}: items {} and {}",
                leak.kind, leak.between.0, leak.between.1, leak.first, leak.second
            ),
            Self::Oversized { items, bound } => {
                write!(
                    f,
                    "the training partition holds {items} items; the bound is {bound}"
                )
            }
            Self::Deleted => write!(f, "the corpus was deleted under its retention terms"),
        }
    }
}

impl std::error::Error for CorpusFault {}

/// Normalize a state's text for the leakage check: lowercase, collapse
/// whitespace, drop punctuation — a paraphrase detector's substrate,
/// not a similarity claim.
fn normalized(value: &Value) -> String {
    let raw = match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    raw.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokens(text: &str) -> HashSet<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// The digest an item's content leaves at tombstone time.
fn content_digest(item: &CorpusItem) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(&serde_json::to_value(item).unwrap_or(Value::Null)).as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

impl Corpus {
    /// Parse and fully check a corpus document. An empty `digest` is
    /// filled on first author; any non-empty digest is recomputed and
    /// trusted only when it matches.
    pub fn check(text: &str) -> Result<Self, CorpusFault> {
        let mut corpus: Self =
            serde_json::from_str(text).map_err(|e| CorpusFault::Malformed(e.to_string()))?;
        if corpus.digest.trim().is_empty() {
            corpus.digest = Self::digest_of(&corpus.items);
        }
        corpus.validate()?;
        Ok(corpus)
    }

    /// Every structural, provenance, and leakage rule. `check` calls
    /// this after parsing; a stored corpus re-validates on open.
    pub fn validate(&self) -> Result<(), CorpusFault> {
        if self.v != CORPUS_SCHEMA {
            return Err(CorpusFault::Schema {
                found: self.v.clone(),
            });
        }
        if self.tombstone.is_some() {
            return Err(CorpusFault::Deleted);
        }
        let computed = Self::digest_of(&self.items);
        if computed != self.digest {
            return Err(CorpusFault::Tampered {
                recorded: self.digest.clone(),
                computed,
            });
        }
        let mut counts: BTreeMap<Role, usize> = BTreeMap::new();
        let mut ids = HashSet::new();
        let mut groups: HashMap<&str, Role> = HashMap::new();
        let mut spills: HashMap<&str, BTreeSet<Role>> = HashMap::new();
        for item in &self.items {
            *counts.entry(item.partition).or_default() += 1;
            if !ids.insert(item.id.as_str()) {
                return Err(CorpusFault::DuplicateItem {
                    id: item.id.clone(),
                });
            }
            for (field, value) in [
                ("source", item.provenance.source.trim()),
                ("license", item.provenance.license.trim()),
                ("permission", item.provenance.permission.trim()),
                ("label", item.label.trim()),
                ("group", item.group.trim()),
            ] {
                if value.is_empty() {
                    return Err(CorpusFault::Undocumented {
                        id: item.id.clone(),
                        missing: field.to_string(),
                    });
                }
            }
            if item.label_source == LabelSource::Model && !item.annotations.confirmed {
                return Err(CorpusFault::UnconfirmedLabel {
                    id: item.id.clone(),
                });
            }
            match groups.get(item.group.as_str()) {
                Some(&role) if role != item.partition => {
                    spills.entry(item.group.as_str()).or_default().insert(role);
                    spills
                        .entry(item.group.as_str())
                        .or_default()
                        .insert(item.partition);
                }
                Some(_) => {}
                None => {
                    groups.insert(item.group.as_str(), item.partition);
                }
            }
        }
        if let Some((group, roles)) = spills.into_iter().next() {
            return Err(CorpusFault::GroupSpills {
                group: group.to_string(),
                roles: roles.into_iter().collect(),
            });
        }
        if counts.get(&Role::Training).copied().unwrap_or(0) == 0 {
            return Err(CorpusFault::NoTraining);
        }
        for role in Role::ALL {
            if counts.contains_key(&role) && counts[&role] == 0 {
                return Err(CorpusFault::EmptyPartition { role });
            }
        }
        let training = counts[&Role::Training];
        if training > MAX_TRAIN_ITEMS {
            return Err(CorpusFault::Oversized {
                items: training,
                bound: MAX_TRAIN_ITEMS,
            });
        }
        self.leak_check()?;
        Ok(())
    }

    /// `sha256:` over the canonicalized item list.
    pub fn digest_of(items: &[CorpusItem]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&serde_json::to_value(items).unwrap_or(Value::Null)).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Exact and near-duplicate detection across partition boundaries.
    /// Same-partition duplicates are the group's business; cross-role
    /// duplication is leakage.
    fn leak_check(&self) -> Result<(), CorpusFault> {
        let normalized_items: Vec<(&CorpusItem, String)> = self
            .items
            .iter()
            .map(|item| (item, normalized(&item.state)))
            .collect();
        for (index, (first, first_text)) in normalized_items.iter().enumerate() {
            for (second, second_text) in normalized_items.iter().skip(index + 1) {
                if first.partition == second.partition {
                    continue;
                }
                if first_text == second_text {
                    return Err(CorpusFault::Leak(Leak {
                        first: first.id.clone(),
                        second: second.id.clone(),
                        between: (first.partition, second.partition),
                        kind: "exact",
                    }));
                }
                let a = tokens(first_text);
                let b = tokens(second_text);
                if a.is_empty() || b.is_empty() {
                    continue;
                }
                let overlap = a.intersection(&b).count() as f64;
                let union = a.union(&b).count() as f64;
                if overlap / union >= LEAK_JACCARD {
                    return Err(CorpusFault::Leak(Leak {
                        first: first.id.clone(),
                        second: second.id.clone(),
                        between: (first.partition, second.partition),
                        kind: "near",
                    }));
                }
            }
        }
        Ok(())
    }

    /// The items a role holds — every role but `locked`, which no path
    /// in this crate reads: a locked item's content stays in the
    /// document only so its digest binds the corpus it was withheld
    /// from.
    pub fn items_in(&self, role: Role) -> Vec<&CorpusItem> {
        self.items
            .iter()
            .filter(|item| item.partition == role && role != Role::Locked)
            .collect()
    }

    /// Per-role item counts.
    pub fn counts(&self) -> BTreeMap<Role, usize> {
        let mut counts = BTreeMap::new();
        for item in &self.items {
            *counts.entry(item.partition).or_default() += 1;
        }
        counts
    }
}

/// A frozen training recipe — everything a run needs that is not data,
/// digested so a trial pins exactly what it ran under.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recipe {
    /// The schema tag.
    pub v: String,
    /// The recipe's name.
    pub name: String,
    /// When it was frozen.
    pub created: String,
    /// The base model the adapter is trained against: a published id
    /// and its artifact signature.
    pub base_model: BaseModel,
    /// The adapter the run produces.
    pub adapter: AdapterSpec,
    /// The pointer head the run produces.
    pub head: HeadSpec,
    /// The seeds a trial may draw; a trial naming another seed is not
    /// this recipe's.
    pub seeds: Vec<u64>,
    /// The most trials the recipe permits, failures included.
    pub trials_max: u32,
    /// The compute budget a trial may not exceed.
    pub budget: Budget,
    /// The metric a winning trial is chosen by, and the margin it must
    /// clear over the baseline on the development partition.
    pub metric: Metric,
    /// Rules that reject a trial regardless of its metric.
    #[serde(default)]
    pub reject_rules: Vec<String>,
    /// What may be done with the artifacts: the candidate lane only,
    /// digests in public, the tenant's data never republished.
    pub transfer_controls: BTreeMap<String, String>,
    /// `sha256:` digest over the document's other fields, written by
    /// [`Recipe::freeze`].
    #[serde(default)]
    pub digest: String,
}

/// The base model identity a recipe trains against.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BaseModel {
    /// The published model id.
    pub id: String,
    /// The artifact signature of the weights.
    pub signature: String,
}

/// The adapter a run produces — a peft-shaped LoRA.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdapterSpec {
    /// Always `lora`; another adapter family is a different recipe.
    pub kind: String,
    /// LoRA rank.
    pub rank: u32,
    /// LoRA alpha; the merge scale is `alpha / rank`.
    pub alpha: u32,
    /// Projection names the adapter targets.
    pub targets: Vec<String>,
    /// Epochs per trial.
    pub epochs: u32,
    /// Learning rate, as a string so no float formatting decides a
    /// digest.
    pub learning_rate: String,
}

/// The pointer head a run produces.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HeadSpec {
    /// Always `pointer` today.
    pub kind: String,
    /// Pointer dimension; the score scale is `1/sqrt(dp)`.
    pub dp: u32,
}

/// The compute bound a trial may not exceed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Budget {
    /// The most training items a run may read.
    pub max_train_items: usize,
    /// The most optimizer steps across the whole recipe.
    pub max_steps: u64,
    /// Wall-clock seconds a run may not exceed.
    pub max_seconds: u64,
}

/// How a trial wins: a statistic on the development partition and the
/// margin it must clear over the measured baseline.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metric {
    /// The statistic — `accuracy`, `brier`, or another Gym measure by
    /// name.
    pub statistic: String,
    /// The baseline's measured value it must beat.
    pub baseline: f64,
    /// The minimum margin over the baseline.
    pub min_margin: f64,
}

/// What a recipe check refuses on.
#[derive(Debug)]
pub enum RecipeFault {
    /// Not JSON, or not the recipe shape.
    Malformed(String),
    /// Tagged for another schema.
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// The frozen digest does not recompute.
    Tampered {
        /// The digest it claims.
        recorded: String,
        /// The digest its fields produce.
        computed: String,
    },
    /// A required bound is absent or degenerate.
    Unbounded(String),
}

impl fmt::Display for RecipeFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "the recipe is not readable: {detail}"),
            Self::Schema { found } => {
                write!(
                    f,
                    "the document is tagged {found}, which is not {RECIPE_SCHEMA}"
                )
            }
            Self::Tampered { recorded, computed } => write!(
                f,
                "the recipe digest does not match its fields: recorded {recorded}, computed {computed}"
            ),
            Self::Unbounded(what) => write!(f, "the recipe leaves {what} unbounded"),
        }
    }
}

impl std::error::Error for RecipeFault {}

impl Recipe {
    /// Parse a recipe document. `digest` may be empty on first read;
    /// [`Recipe::freeze`] seals it.
    pub fn load(text: &str) -> Result<Self, RecipeFault> {
        serde_json::from_str(text).map_err(|e| RecipeFault::Malformed(e.to_string()))
    }

    /// Fill `digest` over every other field and check the bounds.
    pub fn freeze(&mut self) -> Result<(), RecipeFault> {
        self.check_shape()?;
        self.digest = self.compute_digest();
        Ok(())
    }

    /// Recompute the digest and the bounds.
    pub fn verify(&self) -> Result<(), RecipeFault> {
        if self.v != RECIPE_SCHEMA {
            return Err(RecipeFault::Schema {
                found: self.v.clone(),
            });
        }
        self.check_shape()?;
        let computed = self.compute_digest();
        if computed != self.digest {
            return Err(RecipeFault::Tampered {
                recorded: self.digest.clone(),
                computed,
            });
        }
        Ok(())
    }

    fn check_shape(&self) -> Result<(), RecipeFault> {
        if self.v != RECIPE_SCHEMA {
            return Err(RecipeFault::Schema {
                found: self.v.clone(),
            });
        }
        if self.seeds.is_empty() {
            return Err(RecipeFault::Unbounded("a seed policy".into()));
        }
        if self.trials_max == 0 {
            return Err(RecipeFault::Unbounded("a trial cap".into()));
        }
        if self.budget.max_train_items == 0
            || self.budget.max_steps == 0
            || self.budget.max_seconds == 0
        {
            return Err(RecipeFault::Unbounded("a compute budget".into()));
        }
        if self.metric.min_margin < 0.0 || self.metric.statistic.trim().is_empty() {
            return Err(RecipeFault::Unbounded("a winning metric".into()));
        }
        if !self.transfer_controls.contains_key("serving")
            || !self.transfer_controls.contains_key("publish")
        {
            return Err(RecipeFault::Unbounded("transfer controls".into()));
        }
        Ok(())
    }

    fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        value.as_object_mut().map(|map| map.remove("digest"));
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }
}

/// How a trial ended — kept in the ledger either way.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialOutcome {
    /// Won the metric; the candidate's evidence.
    Kept,
    /// Ran clean and lost, or broke a rejection rule.
    Rejected,
    /// The run itself failed — a crash, a budget breach.
    Failed,
}

/// One record in the trials ledger.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trial {
    /// The schema tag.
    pub v: String,
    /// The recipe the trial ran under, by digest — never by name.
    pub recipe_digest: String,
    /// The seed it drew from the recipe's policy.
    pub seed: u64,
    /// The hyperparameters actually run, verbatim.
    pub params: BTreeMap<String, Value>,
    /// The development-partition metrics the run measured.
    pub metrics: BTreeMap<String, f64>,
    /// The artifact identities the run produced, when it produced any.
    #[serde(default)]
    pub artifacts: BTreeMap<String, String>,
    /// How it ended.
    pub outcome: TrialOutcome,
    /// Why it was rejected or failed, in one line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// When the record was appended.
    pub recorded_at: String,
}

/// Why a trial record is refused.
#[derive(Debug)]
pub enum TrialFault {
    /// Not JSON, or not the trial shape.
    Malformed(String),
    /// Tagged for another schema.
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// The seed is outside the recipe's policy.
    ForeignSeed {
        /// The seed it ran.
        seed: u64,
    },
    /// The recipe is already at its trial cap.
    TrialCap {
        /// The cap.
        cap: u32,
    },
    /// A kept trial carries no artifacts to seal.
    NoArtifacts,
}

impl fmt::Display for TrialFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "the trial record is not readable: {detail}"),
            Self::Schema { found } => {
                write!(
                    f,
                    "the document is tagged {found}, which is not {TRIAL_SCHEMA}"
                )
            }
            Self::ForeignSeed { seed } => write!(
                f,
                "the trial ran seed {seed}, which its recipe's seed policy does not allow"
            ),
            Self::TrialCap { cap } => {
                write!(f, "the recipe is at its {cap}-trial cap")
            }
            Self::NoArtifacts => write!(f, "a kept trial carries no artifacts to seal"),
        }
    }
}

impl std::error::Error for TrialFault {}

/// The cause a headroom report assigns one development failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    /// Missing or oversized state — a pipeline bug, not a model gap.
    StateShape,
    /// The question admits more than one defensible answer.
    AmbiguousQuestion,
    /// The label itself is uncertain.
    LabelUncertainty,
    /// Exact logic that belongs in Rust.
    ExactLogic,
    /// A failure none of the above explains — the model's to fix.
    ModelAddressable,
}

impl Cause {
    /// Every cause, in the order a report lists them.
    pub const ALL: [Self; 5] = [
        Self::StateShape,
        Self::AmbiguousQuestion,
        Self::LabelUncertainty,
        Self::ExactLogic,
        Self::ModelAddressable,
    ];

    /// The wire label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StateShape => "state_shape",
            Self::AmbiguousQuestion => "ambiguous_question",
            Self::LabelUncertainty => "label_uncertainty",
            Self::ExactLogic => "exact_logic",
            Self::ModelAddressable => "model_addressable",
        }
    }
}

/// One baseline score on one development item.
#[derive(Clone, Debug, Deserialize)]
pub struct Score {
    /// The development item it scored.
    pub item_id: String,
    /// What the baseline answered.
    pub predicted: String,
    /// Whether the answer was correct.
    pub correct: bool,
}

/// A baseline results file: scores over a corpus's development
/// partition, one row per item.
#[derive(Clone, Debug, Deserialize)]
pub struct Scores {
    /// The schema tag.
    pub v: String,
    /// The door identity that produced the rows.
    pub door: String,
    /// When they were measured.
    pub measured: String,
    /// The rows.
    pub rows: Vec<Score>,
}

/// The headroom verdict.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Model-addressable headroom exists; a recipe may freeze.
    Train,
    /// A documented decision not to train — equally a result.
    NoTrain,
}

/// The sealed headroom report.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Headroom {
    /// The schema tag.
    pub v: String,
    /// The corpus assessed, by digest.
    pub corpus_digest: String,
    /// The baseline door the scores came from.
    pub baseline_door: String,
    /// When the assessment ran.
    pub assessed_at: String,
    /// Development items scored.
    pub scored: usize,
    /// Development items the baseline failed.
    pub failed: usize,
    /// Failure counts by cause.
    pub causes: BTreeMap<String, usize>,
    /// The verdict.
    pub verdict: Verdict,
    /// The item ids behind each cause — the report's evidence.
    pub items: BTreeMap<String, Vec<String>>,
    /// `sha256:` over the report's other fields.
    pub digest: String,
}

/// What a headroom assessment refuses on.
#[derive(Debug)]
pub enum HeadroomFault {
    /// The scores file is not the scores shape.
    Malformed(String),
    /// Tagged for another schema.
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// A scored item is not in the corpus's development partition.
    ForeignItem {
        /// The item id.
        id: String,
    },
    /// The scores do not cover the whole development partition — an
    /// assessment over a subset is not the baseline.
    Incomplete {
        /// How many development items went unscored.
        missing: usize,
    },
}

impl fmt::Display for HeadroomFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "the scores file is not readable: {detail}"),
            Self::Schema { found } => {
                write!(
                    f,
                    "the document is tagged {found}, which is not {SCORES_SCHEMA}"
                )
            }
            Self::ForeignItem { id } => write!(
                f,
                "the scores name item {id}, which is not in the corpus's development partition"
            ),
            Self::Incomplete { missing } => {
                write!(f, "the scores leave {missing} development items unmeasured")
            }
        }
    }
}

impl std::error::Error for HeadroomFault {}

/// The fewest model-addressable failures a corpus needs before a
/// recipe may freeze — below it, training spends compute to move noise.
const MIN_HEADROOM: usize = 3;

/// Bucket one failed development item by its annotations and state.
fn cause_of(item: &CorpusItem) -> Cause {
    if item.annotations.state_shape
        || serde_json::to_vec(&item.state)
            .map(|bytes| bytes.len() > STATE_BOUND_BYTES)
            .unwrap_or(false)
        || item.state.is_null()
    {
        Cause::StateShape
    } else if item.annotations.ambiguous {
        Cause::AmbiguousQuestion
    } else if item.annotations.label_uncertain {
        Cause::LabelUncertainty
    } else if item.annotations.rule_derivable {
        Cause::ExactLogic
    } else {
        Cause::ModelAddressable
    }
}

/// Score the baseline's results against a corpus's development
/// partition and seal the verdict.
pub fn assess_headroom(
    corpus: &Corpus,
    scores_text: &str,
    assessed_at: &str,
) -> Result<Headroom, HeadroomFault> {
    let scores: Scores =
        serde_json::from_str(scores_text).map_err(|e| HeadroomFault::Malformed(e.to_string()))?;
    if scores.v != SCORES_SCHEMA {
        return Err(HeadroomFault::Schema {
            found: scores.v.clone(),
        });
    }
    let development: HashMap<&str, &CorpusItem> = corpus
        .items
        .iter()
        .filter(|item| item.partition == Role::Development)
        .map(|item| (item.id.as_str(), item))
        .collect();
    let mut seen = HashSet::new();
    let mut causes: BTreeMap<String, usize> = BTreeMap::new();
    let mut items: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut failed = 0usize;
    for row in &scores.rows {
        let Some(item) = development.get(row.item_id.as_str()) else {
            return Err(HeadroomFault::ForeignItem {
                id: row.item_id.clone(),
            });
        };
        seen.insert(row.item_id.as_str());
        if row.correct {
            continue;
        }
        failed += 1;
        let cause = cause_of(item);
        *causes.entry(cause.as_str().to_string()).or_default() += 1;
        items
            .entry(cause.as_str().to_string())
            .or_default()
            .push(item.id.clone());
    }
    let missing = development.len() - seen.len();
    if missing > 0 {
        return Err(HeadroomFault::Incomplete { missing });
    }
    let addressable = causes
        .get(Cause::ModelAddressable.as_str())
        .copied()
        .unwrap_or(0);
    let verdict = if addressable >= MIN_HEADROOM {
        Verdict::Train
    } else {
        Verdict::NoTrain
    };
    let mut report = Headroom {
        v: HEADROOM_SCHEMA.to_string(),
        corpus_digest: corpus.digest.clone(),
        baseline_door: scores.door.clone(),
        assessed_at: assessed_at.to_string(),
        scored: scores.rows.len(),
        failed,
        causes,
        verdict,
        items,
        digest: String::new(),
    };
    report.digest = digest_field(&serde_json::to_value(&report).unwrap_or(Value::Null));
    Ok(report)
}

/// `sha256:` over a document with its `digest`/`signature` field
/// removed — the seal discipline every document here shares.
fn digest_field(value: &Value) -> String {
    let mut value = value.clone();
    if let Some(map) = value.as_object_mut() {
        map.remove("digest");
        map.remove("signature");
    }
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(&value).as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// A sealed candidate: the artifact identities a trial produced, bound
/// to the corpus, recipe, code, and base model that produced them.
/// Registration makes a candidate inspectable; it never makes it
/// servable — admission and activation belong to [`crate::registry`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CandidateDoc {
    /// The schema tag.
    pub v: String,
    /// The workspace that owns the candidate.
    pub workspace: String,
    /// The candidate's name.
    pub name: String,
    /// When it was sealed.
    pub created: String,
    /// The identities the seal binds.
    pub identities: Identities,
    /// The development evidence the seal records: the kept trial's
    /// digest and the metrics it measured.
    pub evidence: Evidence,
    /// The retention terms inherited from the corpus.
    pub retention: Retention,
    /// `sha256:` over every other field — the `artifact_signature` an
    /// admission record binds.
    #[serde(default)]
    pub signature: String,
}

/// The identity set a candidate binds.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Identities {
    /// The code that trained: crate or tool, and its version or commit.
    pub code: BTreeMap<String, String>,
    /// The recipe digest the trials ran under.
    pub recipe_digest: String,
    /// The corpus digest the training read.
    pub corpus_digest: String,
    /// The base model the adapter was trained against.
    pub base_model: BaseModel,
    /// `sha256:` of the adapter artifact.
    pub adapter: String,
    /// `sha256:` of the pointer-head artifact.
    pub head: String,
    /// `sha256:` of the tokenizer or rendering configuration.
    pub tokenizer: String,
}

/// The evidence a seal carries: which trial won and what it measured.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Evidence {
    /// The kept trial's ledger position — its record line number.
    pub trial: u64,
    /// The development metrics that trial measured.
    pub metrics: BTreeMap<String, f64>,
    /// The suite or corpus partition the confirmation will spend.
    #[serde(default)]
    pub confirmation: String,
}

/// What sealing a candidate refuses on.
#[derive(Debug)]
pub enum SealFault {
    /// Not JSON, or not the candidate shape.
    Malformed(String),
    /// Tagged for another schema.
    Schema {
        /// The tag it carried.
        found: String,
    },
    /// A referenced document is not in the store.
    Unknown {
        /// What is missing.
        what: String,
    },
    /// An artifact identity is absent or not a digest — a name is not
    /// an artifact.
    Unpinned(String),
    /// The evidence names no kept trial.
    NoKeptTrial,
}

impl fmt::Display for SealFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(detail) => write!(f, "the candidate is not readable: {detail}"),
            Self::Schema { found } => {
                write!(
                    f,
                    "the document is tagged {found}, which is not {CANDIDATE_SCHEMA}"
                )
            }
            Self::Unknown { what } => write!(f, "{what}"),
            Self::Unpinned(what) => write!(f, "{what} names no artifact digest"),
            Self::NoKeptTrial => write!(f, "the evidence names no kept trial"),
        }
    }
}

impl std::error::Error for SealFault {}

impl CandidateDoc {
    /// The `admission::Candidate` an admission record binds: the model
    /// id the door publishes, the adapter name, and the seal signature
    /// as `artifact_signature`.
    pub fn admission_candidate(&self) -> crate::admission::Candidate {
        crate::admission::Candidate {
            model: self.identities.base_model.id.clone(),
            adapter: Some(self.name.clone()),
            artifact_signature: self.signature.clone(),
            execution: BTreeMap::new(),
        }
    }
}

/// Everything the training store can fail with.
#[derive(Debug)]
pub enum Trouble {
    /// A corpus failed its check.
    Corpus(CorpusFault),
    /// A recipe failed its check.
    Recipe(RecipeFault),
    /// A trial record failed its check.
    Trial(TrialFault),
    /// A headroom assessment was refused.
    Headroom(HeadroomFault),
    /// A candidate seal was refused.
    Seal(SealFault),
    /// A named document is not in the store.
    NotFound(String),
    /// The store itself failed.
    Store(String),
}

impl fmt::Display for Trouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corpus(fault) => write!(f, "{fault}"),
            Self::Recipe(fault) => write!(f, "{fault}"),
            Self::Trial(fault) => write!(f, "{fault}"),
            Self::Headroom(fault) => write!(f, "{fault}"),
            Self::Seal(fault) => write!(f, "{fault}"),
            Self::NotFound(what) => write!(f, "{what} is not in the training store"),
            Self::Store(detail) => write!(f, "training store: {detail}"),
        }
    }
}

impl std::error::Error for Trouble {}

impl From<CorpusFault> for Trouble {
    fn from(fault: CorpusFault) -> Self {
        Self::Corpus(fault)
    }
}

impl From<RecipeFault> for Trouble {
    fn from(fault: RecipeFault) -> Self {
        Self::Recipe(fault)
    }
}

impl From<TrialFault> for Trouble {
    fn from(fault: TrialFault) -> Self {
        Self::Trial(fault)
    }
}

impl From<HeadroomFault> for Trouble {
    fn from(fault: HeadroomFault) -> Self {
        Self::Headroom(fault)
    }
}

impl From<SealFault> for Trouble {
    fn from(fault: SealFault) -> Self {
        Self::Seal(fault)
    }
}

/// The training store: one directory beside the registry holding the
/// corpus documents, frozen recipes, the append-only trials ledger,
/// headroom reports, and sealed candidates.
pub struct Book {
    dir: PathBuf,
}

impl Book {
    /// Open — and create if needed — the store under `dir`.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, Trouble> {
        let dir = dir.as_ref().join("training");
        fs::create_dir_all(dir.join("corpora"))
            .and_then(|()| fs::create_dir_all(dir.join("recipes")))
            .and_then(|()| fs::create_dir_all(dir.join("headroom")))
            .and_then(|()| fs::create_dir_all(dir.join("candidates")))
            .map_err(|e| Trouble::Store(format!("create {}: {e}", dir.display())))?;
        Ok(Self { dir })
    }

    /// The store's root.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Validate a corpus document and register it under its name.
    /// Re-registering the identical document is a no-op; a different
    /// document under the same name is refused — a corpus's identity
    /// is its digest.
    pub fn register_corpus(&self, text: &str) -> Result<Corpus, Trouble> {
        let corpus = Corpus::check(text)?;
        let path = self
            .dir
            .join("corpora")
            .join(format!("{}.json", corpus.name));
        if let Ok(existing) = fs::read_to_string(&path) {
            let existing = Corpus::check(&existing).map_err(Trouble::Corpus)?;
            if existing.digest != corpus.digest {
                return Err(Trouble::Store(format!(
                    "corpus {} is already registered under a different digest",
                    corpus.name
                )));
            }
            return Ok(existing);
        }
        let sealed =
            serde_json::to_string_pretty(&corpus).map_err(|e| Trouble::Store(e.to_string()))?;
        write_synced(&path, &sealed)?;
        Ok(corpus)
    }

    /// Read a registered corpus back, re-validated.
    pub fn corpus(&self, name: &str) -> Result<Corpus, Trouble> {
        let path = self.dir.join("corpora").join(format!("{name}.json"));
        let text =
            fs::read_to_string(&path).map_err(|_| Trouble::NotFound(format!("corpus {name}")))?;
        Ok(Corpus::check(&text)?)
    }

    /// Freeze a recipe document and register it under its name.
    pub fn freeze_recipe(&self, text: &str) -> Result<Recipe, Trouble> {
        let mut recipe = Recipe::load(text)?;
        if recipe.digest.is_empty() {
            recipe.freeze()?;
        } else {
            recipe.verify()?;
        }
        let path = self
            .dir
            .join("recipes")
            .join(format!("{}.json", recipe.name));
        let frozen =
            serde_json::to_string_pretty(&recipe).map_err(|e| Trouble::Store(e.to_string()))?;
        write_synced(&path, &frozen)?;
        Ok(recipe)
    }

    /// Read a frozen recipe back, re-verified.
    pub fn recipe(&self, name: &str) -> Result<Recipe, Trouble> {
        let path = self.dir.join("recipes").join(format!("{name}.json"));
        let text =
            fs::read_to_string(&path).map_err(|_| Trouble::NotFound(format!("recipe {name}")))?;
        let recipe = Recipe::load(&text)?;
        recipe.verify()?;
        Ok(recipe)
    }

    /// Append a trial to the ledger — kept, rejected, and failed runs
    /// alike — under the recipe it names. The record is refused when its
    /// seed is outside the recipe's policy, when the recipe is at its
    /// cap, or when a kept trial carries no artifacts.
    pub fn record_trial(&self, text: &str) -> Result<usize, Trouble> {
        let trial: Trial =
            serde_json::from_str(text).map_err(|e| TrialFault::Malformed(e.to_string()))?;
        if trial.v != TRIAL_SCHEMA {
            return Err(TrialFault::Schema {
                found: trial.v.clone(),
            }
            .into());
        }
        let recipe = self
            .recipes()
            .into_iter()
            .find(|recipe| recipe.digest == trial.recipe_digest)
            .ok_or_else(|| Trouble::NotFound(format!("recipe {}", trial.recipe_digest)))?;
        if !recipe.seeds.contains(&trial.seed) {
            return Err(TrialFault::ForeignSeed { seed: trial.seed }.into());
        }
        let existing = self.trials_for(&recipe.digest).len();
        if existing >= recipe.trials_max as usize {
            return Err(TrialFault::TrialCap {
                cap: recipe.trials_max,
            }
            .into());
        }
        if trial.outcome == TrialOutcome::Kept && trial.artifacts.is_empty() {
            return Err(TrialFault::NoArtifacts.into());
        }
        let path = self.dir.join("trials.jsonl");
        let line = serde_json::to_string(&trial).map_err(|e| Trouble::Store(e.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| Trouble::Store(format!("open {}: {e}", path.display())))?;
        file.write_all(line.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(|e| Trouble::Store(format!("append {}: {e}", path.display())))?;
        Ok(existing + 1)
    }

    /// Every trial recorded against a recipe digest, in ledger order.
    pub fn trials_for(&self, recipe_digest: &str) -> Vec<Trial> {
        self.trials()
            .into_iter()
            .filter(|trial| trial.recipe_digest == recipe_digest)
            .collect()
    }

    /// Every trial in the ledger.
    pub fn trials(&self) -> Vec<Trial> {
        let path = self.dir.join("trials.jsonl");
        let Ok(text) = fs::read_to_string(&path) else {
            return Vec::new();
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    /// Run a headroom assessment over a registered corpus and seal the
    /// report — `train` and `no_train` verdicts are both recorded.
    pub fn assess(
        &self,
        corpus_name: &str,
        scores_text: &str,
        at: &str,
    ) -> Result<Headroom, Trouble> {
        let corpus = self.corpus(corpus_name)?;
        let report = assess_headroom(&corpus, scores_text, at)?;
        let path = self
            .dir
            .join("headroom")
            .join(format!("{corpus_name}.json"));
        let text =
            serde_json::to_string_pretty(&report).map_err(|e| Trouble::Store(e.to_string()))?;
        write_synced(&path, &text)?;
        Ok(report)
    }

    /// Seal a candidate document: every referenced identity must exist
    /// in the store — the corpus, the recipe, and a kept trial whose
    /// artifacts the seal pins — before the signature is computed and
    /// the candidate registered for inspection.
    pub fn seal_candidate(&self, text: &str) -> Result<CandidateDoc, Trouble> {
        let mut candidate: CandidateDoc =
            serde_json::from_str(text).map_err(|e| SealFault::Malformed(e.to_string()))?;
        if candidate.v != CANDIDATE_SCHEMA {
            return Err(SealFault::Schema {
                found: candidate.v.clone(),
            }
            .into());
        }
        // The corpus must be registered — tombstoned is fine, since a
        // candidate keeps its digest either way — but never absent.
        let corpus_found = read_dir_jsons(&self.dir.join("corpora"))
            .any(|text| corpus_digest_of(&text) == candidate.identities.corpus_digest);
        if !corpus_found {
            return Err(SealFault::Unknown {
                what: format!(
                    "no registered corpus matches digest {}",
                    candidate.identities.corpus_digest
                ),
            }
            .into());
        }
        let recipe = self
            .recipes()
            .into_iter()
            .find(|recipe| recipe.digest == candidate.identities.recipe_digest)
            .ok_or_else(|| SealFault::Unknown {
                what: format!(
                    "no frozen recipe matches digest {}",
                    candidate.identities.recipe_digest
                ),
            })?;
        let ledger = self.trials_for(&recipe.digest);
        let Some(kept_trial) = ledger
            .get(candidate.evidence.trial.saturating_sub(1) as usize)
            .filter(|trial| trial.outcome == TrialOutcome::Kept)
        else {
            return Err(SealFault::NoKeptTrial.into());
        };
        for (what, sig) in [
            ("adapter", &candidate.identities.adapter),
            ("head", &candidate.identities.head),
            ("tokenizer", &candidate.identities.tokenizer),
        ] {
            if kept_trial.artifacts.get(what) != Some(sig) {
                return Err(SealFault::Unknown {
                    what: format!("the kept trial's {what} is not the artifact the seal names"),
                }
                .into());
            }
        }
        for (what, sig) in [
            ("adapter", &candidate.identities.adapter),
            ("head", &candidate.identities.head),
            ("tokenizer", &candidate.identities.tokenizer),
            ("base model", &candidate.identities.base_model.signature),
        ] {
            if !sig.starts_with("sha256:") || sig.len() != "sha256:".len() + 64 {
                return Err(SealFault::Unpinned(what.to_string()).into());
            }
        }
        candidate.signature =
            digest_field(&serde_json::to_value(&candidate).unwrap_or(Value::Null));
        let path = self
            .dir
            .join("candidates")
            .join(format!("{}.json", candidate.name));
        let text =
            serde_json::to_string_pretty(&candidate).map_err(|e| Trouble::Store(e.to_string()))?;
        write_synced(&path, &text)?;
        Ok(candidate)
    }

    /// Read a sealed candidate back — its signature recomputed.
    pub fn candidate(&self, name: &str) -> Result<CandidateDoc, Trouble> {
        let path = self.dir.join("candidates").join(format!("{name}.json"));
        let text = fs::read_to_string(&path)
            .map_err(|_| Trouble::NotFound(format!("candidate {name}")))?;
        let candidate: CandidateDoc = serde_json::from_str(&text)
            .map_err(|e| Trouble::Store(format!("candidate {name}: {e}")))?;
        Ok(candidate)
    }

    /// Tombstone a corpus: item content leaves the store and each
    /// item's content digest is kept, so the corpus's own digest — and
    /// any candidate trained on it — still resolves.
    pub fn delete_corpus(&self, name: &str, at: &str, reason: &str) -> Result<Tombstone, Trouble> {
        let path = self.dir.join("corpora").join(format!("{name}.json"));
        let text =
            fs::read_to_string(&path).map_err(|_| Trouble::NotFound(format!("corpus {name}")))?;
        let mut corpus: Corpus = serde_json::from_str(&text)
            .map_err(|e| Trouble::Store(format!("corpus {name}: {e}")))?;
        if let Some(tombstone) = corpus.tombstone {
            return Ok(tombstone);
        }
        let item_digests = corpus
            .items
            .iter()
            .map(|item| (item.id.clone(), content_digest(item)))
            .collect();
        corpus.items = corpus
            .items
            .into_iter()
            .map(|mut item| {
                item.state = Value::Null;
                item.question = None;
                item.label = String::new();
                item.label_rule = None;
                item.annotations = Annotations::default();
                item
            })
            .collect();
        let tombstone = Tombstone {
            deleted_at: at.to_string(),
            reason: reason.to_string(),
            item_digests,
        };
        corpus.tombstone = Some(tombstone.clone());
        let text =
            serde_json::to_string_pretty(&corpus).map_err(|e| Trouble::Store(e.to_string()))?;
        write_synced(&path, &text)?;
        Ok(tombstone)
    }

    /// The frozen recipes in the store.
    pub fn recipes(&self) -> Vec<Recipe> {
        read_dir_jsons(&self.dir.join("recipes"))
            .filter_map(|text| Recipe::load(&text).ok())
            .filter(|recipe| recipe.verify().is_ok())
            .collect()
    }

    /// The corpora in the store, tombstoned included.
    pub fn corpora(&self) -> Vec<String> {
        file_names(&self.dir.join("corpora"))
    }

    /// The sealed candidates in the store.
    pub fn candidates(&self) -> Vec<String> {
        file_names(&self.dir.join("candidates"))
    }
}

/// A stored corpus's digest, without re-validating: a tombstoned
/// corpus's original digest is what candidates pin, so read the field
/// the document recorded.
fn corpus_digest_of(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value["digest"].as_str().map(str::to_string))
        .unwrap_or_default()
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .and_then(|name| name.strip_suffix(".json").map(str::to_string))
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn read_dir_jsons(dir: &Path) -> impl Iterator<Item = String> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|n| n.ends_with(".json"))
                })
                .filter_map(|entry| fs::read_to_string(entry.path()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into_iter()
}

/// Write a file and sync it — the store's durability floor.
fn write_synced(path: &Path, text: &str) -> Result<(), Trouble> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|e| Trouble::Store(format!("open {}: {e}", path.display())))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| Trouble::Store(format!("write {}: {e}", path.display())))
}
