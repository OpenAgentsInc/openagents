//! A published benchmark and status snapshot: the public view over a
//! verifiable store.
//!
//! `report` writes the measured record — the whole evidence, missing item
//! ids and all. This module renders the page a stranger reads: what was
//! measured, under which identities and digests, with which coverage, and
//! what the numbers may claim. Its discipline is the one the rest of this
//! crate already keeps, applied to a public audience:
//!
//! - A benchmark reads **complete** only against a declared selection the
//!   rows cover exactly — answered, refused, and missing each named, never
//!   a denominator that quietly shrank. Partial coverage renders as
//!   partial, and an undeclared selection renders as unverifiable.
//! - Rows that disagree on what ran — artifact identity, execution
//!   settings, question or gate digest, estimator configuration — are
//!   different measurements. Each gets its own workload section, and the
//!   page says a ranking across them is not supported rather than pooling
//!   them into one number.
//! - Accuracy is stated over its real numerator and denominator together:
//!   correct over answered over asked. An answered-only figure never
//!   wears the whole workload's name.
//! - Latency is the measured p50/p95 with the call count, the recording
//!   window, and the execution settings that produced it. An untimed run
//!   says untimed; a percentile over three calls is a percentile over
//!   three calls, not a floor.
//! - Cost follows [`Cost`]: a metered price only with its source and
//!   unit, an unmetered lane named unmetered, and no measurement named
//!   as no measurement. There is no zero.
//! - Live status is a separate section with its own source and freshness.
//!   Absent means the page publishes historical evidence and says so, not
//!   that the service is healthy or not.
//! - Nothing here carries tenant request text, label text, secrets, or
//!   door data a row did not record. The view is built only of
//!   aggregates, suite structure, and the facts a door published about
//!   itself.
//!
//! Every rendered claim is also written to a manifest
//! ([`SNAPSHOT_SCHEMA`]), and [`check`] recomputes the manifest's claims
//! over a store's rows and names each divergence — the page is
//! reproducible from the evidence it cites rather than trusted.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coverage::{Coverage, Expected, run_groups};
use crate::gate::Cost;
use crate::gate::Profile;
use crate::row::{DoorIdentity, Row};
use crate::suite::Suite;

/// What this document is, written into the manifest so a reader that never
/// loads this crate still knows what it holds.
pub const SNAPSHOT_SCHEMA: &str = "openagents.gym.snapshot.v1";

/// The file the rendered page is written to inside a snapshot directory.
pub const DOCUMENT_FILE: &str = "snapshot.md";

/// The file the manifest is written to inside a snapshot directory.
pub const MANIFEST_FILE: &str = "manifest.json";

/// The index a snapshot directory preserves across publications.
pub const INDEX_FILE: &str = "index.json";

/// Live service status the publisher declares, kept apart from the
/// historical measurement the rest of the page renders.
///
/// A snapshot is evidence over a store, never a probe: this note is the
/// only place a claim about *now* may appear, and it carries the source
/// that made it and the time it was checked so a reader can judge
/// freshness rather than take the page's word for it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StatusNote {
    /// Who or what reported the state — a probe, an operator, a monitor.
    pub source: String,
    /// When the source checked, as an RFC 3339 timestamp.
    pub checked_at: String,
    /// The state the source reported: `operational`, `degraded`, `down`,
    /// or `unknown`. This crate does not enumerate states a publisher may
    /// define; it prints what it is given.
    pub state: String,
    /// One line of context, or empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// The source itself says the note is stale. A publisher that knows
    /// its status feed is old marks it rather than letting a timestamp
    /// carry the whole burden of freshness.
    #[serde(default)]
    pub stale: bool,
}

/// A price the publisher declares for one door's decisions.
///
/// This is the only path by which a number may appear in a cost column,
/// and it is deliberately not a row field: what a decision costs is a
/// publisher's claim about a lane, sourced and versioned like any other
/// evidence rather than smuggled in as a measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeclaredCost {
    /// The door the declaration prices, by the name the rows use.
    pub door: String,
    /// Whether the lane is metered or unmetered — never free.
    pub cost: Cost,
    /// Where the price comes from: a price list, an invoice class, a
    /// version of the provider's terms. Required for a metered lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// What the number counts: `usd_per_decision` today. Required for a
    /// metered lane, so a reader does not guess at the denominator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

impl DeclaredCost {
    /// A declaration that does not meet the publication rule is an error,
    /// not a quiet omission: a metered price without a source or a unit is
    /// a number without provenance.
    pub fn check(&self) -> Result<(), String> {
        match self.cost {
            Cost::Metered { .. } => {
                if self.source.as_deref().unwrap_or("").is_empty() {
                    return Err(format!(
                        "a metered cost for `{}` needs its source",
                        self.door
                    ));
                }
                if self.unit.as_deref().unwrap_or("").is_empty() {
                    return Err(format!("a metered cost for `{}` needs its unit", self.door));
                }
            }
            Cost::UnmeteredLocalLane => {}
        }
        Ok(())
    }
}

/// Where the evidence lives, in the manifest.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoreRef {
    /// The SHA-256 of the store file as published. A store that grew past
    /// the snapshot's horizon digests differently without being different
    /// evidence; the head is the binding check.
    pub sha256: String,
    /// The receipt the chain reached at publication: the binding that a
    /// grown store must still reproduce over its first `rows` rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// How many rows the snapshot was taken over.
    pub rows: usize,
}

/// The report commitment published beside the snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CommitmentRef {
    /// The commitment's own self-verifying digest.
    pub digest: String,
    /// The file inside the snapshot directory that carries it.
    pub file: String,
}

/// One `(partition, item)` pair of the declared selection, with the family
/// it belongs to so per-family denominators reproduce without the suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedEntry {
    /// The partition the item sits in.
    pub partition: String,
    /// The item id.
    pub item: String,
    /// The family the suite places the item in.
    pub family: String,
}

/// What the run was meant to ask, as the snapshot records it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SelectionView {
    /// False when no `--suite` declared the selection. Everything
    /// downstream of this flag reads accordingly.
    pub declared: bool,
    /// The suite the selection was declared over, when declared.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub suite: String,
    /// Its content digest.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub suite_digest: String,
    /// The narrowing the run declared, in the words it was declared in.
    #[serde(default)]
    pub partitions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The family the run was narrowed to, when it was.
    pub family: Option<String>,
    /// The expected `(partition, item, family)` triples, expanded.
    #[serde(default)]
    pub items: Vec<ExpectedEntry>,
    /// Every door the run was meant to ask, including one that left no
    /// rows.
    #[serde(default)]
    pub doors: Vec<String>,
}

impl SelectionView {
    /// The `(partition, item)` pairs coverage is counted against.
    #[must_use]
    pub fn items_set(&self) -> BTreeSet<(String, String)> {
        self.items
            .iter()
            .map(|entry| (entry.partition.clone(), entry.item.clone()))
            .collect()
    }
}

/// The recording window a workload's rows span.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// The earliest `recorded_at` in the group.
    pub first: String,
    /// The latest `recorded_at` in the group.
    pub last: String,
}

/// Coverage as the manifest carries it: the counts and the named gaps.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverageView {
    /// How many `(partition, item)` pairs the selection asked for.
    pub expected: usize,
    /// Expected items with a recorded answer.
    pub answered: usize,
    /// Expected items the door refused.
    pub refused: usize,
    /// Expected items with no row.
    pub missing: Vec<String>,
    /// Expected items with more than one row.
    pub duplicates: Vec<String>,
    /// Recorded rows outside the selection.
    pub unexpected: Vec<String>,
}

impl CoverageView {
    /// The view of one [`Coverage`], with `(partition, item)` pairs
    /// flattened to the `partition/item` spelling the page uses.
    #[must_use]
    pub fn of(coverage: &Coverage) -> Self {
        let flatten = |pairs: &[(String, String)]| {
            pairs
                .iter()
                .map(|(partition, item)| format!("{partition}/{item}"))
                .collect()
        };
        Self {
            expected: coverage.expected,
            answered: coverage.answered,
            refused: coverage.refused,
            missing: flatten(&coverage.missing),
            duplicates: flatten(&coverage.duplicates),
            unexpected: flatten(&coverage.unexpected),
        }
    }

    /// Whether the rows recorded exactly the selection, once each.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.duplicates.is_empty() && self.unexpected.is_empty()
    }
}

/// One family's share of a workload: its own denominator, never pooled.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FamilyView {
    /// The family, as the suite names it.
    pub family: String,
    /// How many items of this family the selection asked, or how many
    /// recorded when no selection was declared.
    pub asked: usize,
    /// How many of those recorded an answer.
    pub answered: usize,
    /// How many the door refused.
    pub refused: usize,
    /// How many asked items left no row. `0` when the selection is not
    /// declared — unknown stays out of the count rather than becoming a
    /// false zero.
    pub missing: usize,
    /// How many answers matched their label.
    pub correct: usize,
}

/// One measurement in the snapshot: one door under one run configuration
/// over one suite digest. Rows that disagree on any of those are a
/// different workload and render as one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Workload {
    /// The door the rows name.
    pub door: String,
    /// The suite the rows were drawn from.
    pub suite: String,
    /// Its content digest.
    pub suite_digest: String,
    /// What the door says it was running, verbatim from the rows.
    pub identity: DoorIdentity,
    /// The question-set digest the rows pin, when they pin one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_digest: Option<String>,
    /// The gate digest the rows were judged under, when judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_digest: Option<String>,
    /// The estimator that produced the raw signal.
    pub estimator: String,
    /// The draws each estimate rests on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples: Option<u64>,
    /// The seed base the estimator started from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_base: Option<u64>,
    /// The window the rows were recorded across.
    pub window: Window,
    /// How many rows the group holds, perturbed re-asks included.
    pub rows: usize,
    /// Coverage against the declared selection, or `null` when no
    /// selection was declared for this suite digest. Null renders as
    /// "not declared" — never as complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<CoverageView>,
    /// Per-family counts, in first-seen order.
    #[serde(default)]
    pub families: Vec<FamilyView>,
    /// The measured latency profile over this group's timed calls.
    pub latency: Profile,
    /// The publisher-declared cost for this door, or `null` for
    /// unmeasured. An unmetered lane is a declaration too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<DeclaredCost>,
    /// Every label source the group's rows carry, deduplicated.
    #[serde(default)]
    pub label_sources: Vec<String>,
    /// The label rule the suite states, when the suite states one and its
    /// digest matches these rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_rule: Option<String>,
    /// The agreement ceiling each family's labels rest on, where the
    /// suite's provenance states one.
    #[serde(default)]
    pub agreement: BTreeMap<String, String>,
}

/// The manifest: the snapshot's every claim, in a shape [`check`] can
/// recompute and a reader can diff.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Always [`SNAPSHOT_SCHEMA`].
    pub schema: String,
    /// The snapshot directory's own name.
    pub id: String,
    /// When the snapshot was generated, RFC 3339.
    pub generated_at: String,
    /// The store this snapshot was taken over.
    pub store: StoreRef,
    /// The report commitment published beside it, when one was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment: Option<CommitmentRef>,
    /// The declared selection, or `declared: false`.
    pub selection: SelectionView,
    /// One entry per distinct measurement, first-seen order.
    #[serde(default)]
    pub workloads: Vec<Workload>,
    /// The declared live status, or `null` — which the page says out
    /// loud rather than implying a health nobody reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusNote>,
    /// The rendered document's own digest, filled in once the page is
    /// written so an edited page is detectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<DocumentRef>,
}

/// The rendered page's binding to its bytes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentRef {
    /// The file inside the snapshot directory.
    pub file: String,
    /// Its SHA-256 at publication.
    pub sha256: String,
}

/// One line of the preserved snapshot index.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// The snapshot's id.
    pub id: String,
    /// When it was generated.
    pub generated_at: String,
    /// The store head it was taken over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The commitment it published, when it published one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment_digest: Option<String>,
    /// `complete`, `partial`, or `undeclared`.
    pub coverage: String,
}

impl Snapshot {
    /// The index line this snapshot adds.
    #[must_use]
    pub fn index_entry(&self) -> IndexEntry {
        IndexEntry {
            id: self.id.clone(),
            generated_at: self.generated_at.clone(),
            head: self.store.head.clone(),
            commitment_digest: self.commitment.as_ref().map(|held| held.digest.clone()),
            coverage: self.coverage_word().to_string(),
        }
    }

    /// The one-word coverage verdict the index and the page share.
    #[must_use]
    pub fn coverage_word(&self) -> &'static str {
        if !self.selection.declared {
            return "undeclared";
        }
        if self.workloads.iter().all(|workload| {
            workload
                .coverage
                .as_ref()
                .is_some_and(CoverageView::complete)
        }) {
            "complete"
        } else {
            "partial"
        }
    }
}

/// Build the snapshot over `rows`.
///
/// `expected` is the declared selection when `--suite` named one; it
/// applies to the suite digest it was declared against, and rows pinning
/// another digest report their own coverage state rather than borrow
/// this one's — the same rule `report` and `commitment` keep.
/// `provenance` supplies the suite's label rules and agreement ceilings,
/// bound to these rows only when the digests match. `costs` are the
/// publisher's declarations; a door with no declaration renders
/// unmeasured.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build(
    id: &str,
    generated_at: &str,
    rows: &[Row],
    expected: Option<&Expected>,
    declared_suite: Option<&Suite>,
    provenance: Option<&Value>,
    store: StoreRef,
    commitment: Option<CommitmentRef>,
    status: Option<StatusNote>,
    costs: &[DeclaredCost],
) -> Snapshot {
    let selection = selection_view(expected, declared_suite);
    let mut workloads = Vec::new();
    for (suite, digest) in suite_groups(rows) {
        let inside: Vec<Row> = rows
            .iter()
            .filter(|row| row.suite == suite && row.suite_digest == digest)
            .cloned()
            .collect();
        // The declaration belongs to the suite digest it was declared
        // against, exactly as in `report`: a second digest in the same
        // store does not inherit it.
        let applicable =
            expected.filter(|_| declared_suite.is_some_and(|suite| suite.digest == digest));
        let provenance_for =
            provenance.filter(|_| declared_suite.is_some_and(|suite| suite.digest == digest));
        for door in doors_of(&inside) {
            let asked: Vec<Row> = inside
                .iter()
                .filter(|row| row.door == *door)
                .cloned()
                .collect();
            for (_key, group) in run_groups(&asked) {
                workloads.push(workload(
                    &door,
                    &suite,
                    &digest,
                    &group,
                    applicable,
                    declared_suite,
                    provenance_for,
                    costs,
                ));
            }
        }
    }
    Snapshot {
        schema: SNAPSHOT_SCHEMA.to_string(),
        id: id.to_string(),
        generated_at: generated_at.to_string(),
        store,
        commitment,
        selection,
        workloads,
        status,
        document: None,
    }
}

/// The `(suite, suite_digest)` groups `rows` hold, first-seen.
fn suite_groups(rows: &[Row]) -> Vec<(String, String)> {
    let mut groups: Vec<(String, String)> = Vec::new();
    for row in rows {
        let key = (row.suite.clone(), row.suite_digest.clone());
        if !groups.contains(&key) {
            groups.push(key);
        }
    }
    groups
}

/// The doors a group of rows name, first-seen.
fn doors_of(rows: &[Row]) -> Vec<String> {
    let mut doors: Vec<String> = Vec::new();
    for row in rows {
        if !doors.contains(&row.door) {
            doors.push(row.door.clone());
        }
    }
    doors
}

/// The manifest's record of the declared selection.
fn selection_view(expected: Option<&Expected>, suite: Option<&Suite>) -> SelectionView {
    let (Some(expected), Some(suite)) = (expected, suite) else {
        return SelectionView::default();
    };
    let mut partitions: Vec<String> = Vec::new();
    let items = expected
        .items()
        .iter()
        .map(|(partition, item)| {
            if !partitions.contains(partition) {
                partitions.push(partition.clone());
            }
            ExpectedEntry {
                partition: partition.clone(),
                item: item.clone(),
                family: suite
                    .items
                    .iter()
                    .find(|held| held.id == *item)
                    .map(|held| held.family.clone())
                    .unwrap_or_default(),
            }
        })
        .collect();
    SelectionView {
        declared: true,
        suite: suite.name.clone(),
        suite_digest: suite.digest.clone(),
        partitions,
        family: None,
        items,
        doors: expected.doors.clone(),
    }
}

/// One workload's claims over its rows.
#[allow(clippy::too_many_arguments)]
fn workload(
    door: &str,
    suite: &str,
    suite_digest: &str,
    group: &[Row],
    expected: Option<&Expected>,
    declared_suite: Option<&Suite>,
    provenance: Option<&Value>,
    costs: &[DeclaredCost],
) -> Workload {
    let canonical: Vec<Row> = group
        .iter()
        .filter(|row| row.permutation.is_none())
        .cloned()
        .collect();
    let first = group.iter().map(|row| &row.recorded_at).min().cloned();
    let last = group.iter().map(|row| &row.recorded_at).max().cloned();
    let coverage =
        expected.map(|expected| CoverageView::of(&Coverage::of(&canonical, expected.items())));
    let latencies: Vec<f64> = group.iter().filter_map(|row| row.latency_ms).collect();
    let mut latency = Profile::timed(&latencies);
    latency.refusals = Some(group.iter().filter(|row| row.is_refused()).count());
    Workload {
        door: door.to_string(),
        suite: suite.to_string(),
        suite_digest: suite_digest.to_string(),
        identity: group
            .first()
            .map(|row| row.door_identity.clone())
            .unwrap_or_default(),
        question_digest: group.first().and_then(|row| row.question_digest.clone()),
        gate_digest: group.first().and_then(|row| row.gate_digest.clone()),
        estimator: group
            .first()
            .map(|row| row.estimator.clone())
            .unwrap_or_default(),
        samples: group.first().and_then(|row| row.samples),
        seed_base: group.first().and_then(|row| row.seed_base),
        window: Window {
            first: first.unwrap_or_default(),
            last: last.unwrap_or_default(),
        },
        rows: group.len(),
        coverage,
        families: families(&canonical, expected),
        latency,
        cost: costs.iter().find(|held| held.door == door).cloned(),
        label_sources: label_sources(group),
        label_rule: label_rule(declared_suite, suite_digest),
        agreement: agreement(declared_suite, suite_digest, provenance),
    }
}

/// Per-family counts over the canonical rows of a group.
///
/// `asked` is the selection's count when one is declared; the recorded
/// count otherwise, so an undeclared run does not manufacture a
/// denominator.
fn families(canonical: &[Row], expected: Option<&Expected>) -> Vec<FamilyView> {
    let mut order: Vec<String> = Vec::new();
    for row in canonical {
        if !order.contains(&row.family) {
            order.push(row.family.clone());
        }
    }
    if let Some(expected) = expected {
        for family in expected.families() {
            if !order.contains(family) {
                order.push(family.clone());
            }
        }
    }
    order
        .iter()
        .map(|family| {
            let rows: Vec<&Row> = canonical
                .iter()
                .filter(|row| row.family == *family)
                .collect();
            let asked = expected
                .map(|expected| expected.for_family(family).len())
                .unwrap_or(rows.len());
            let missing = expected
                .map(|expected| {
                    let held: BTreeSet<(String, String)> = rows
                        .iter()
                        .map(|row| (row.split.clone(), row.item_id.clone()))
                        .collect();
                    expected.for_family(family).difference(&held).count()
                })
                .unwrap_or(0);
            FamilyView {
                family: family.clone(),
                asked,
                answered: rows.iter().filter(|row| row.is_scored()).count(),
                refused: rows.iter().filter(|row| row.is_refused()).count(),
                missing,
                correct: rows.iter().filter(|row| row.correct == Some(true)).count(),
            }
        })
        .collect()
}

/// The label sources a group carries, deduplicated, first-seen.
fn label_sources(group: &[Row]) -> Vec<String> {
    let mut sources: Vec<String> = Vec::new();
    for row in group {
        let source = row.label_source.label().to_string();
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    sources
}

/// The label rule the suite states, only when its digest is the one these
/// rows pin — a different suite's rule is not these rows' rule.
fn label_rule(suite: Option<&Suite>, digest: &str) -> Option<String> {
    let suite = suite.filter(|suite| suite.digest == digest)?;
    suite.items.iter().find_map(|item| item.label_rule.clone())
}

/// The agreement ceilings from the suite's `provenance.agreement` map,
/// bound to these rows by the suite digest.
fn agreement(
    suite: Option<&Suite>,
    digest: &str,
    provenance: Option<&Value>,
) -> BTreeMap<String, String> {
    let mut ceilings = BTreeMap::new();
    let (Some(suite), Some(provenance)) = (suite, provenance) else {
        return ceilings;
    };
    if suite.digest != digest {
        return ceilings;
    }
    if let Some(agreement) = provenance.get("agreement").and_then(Value::as_object) {
        for (family, ceiling) in agreement {
            let text = ceiling
                .as_str()
                .map(str::to_string)
                .or_else(|| ceiling.as_f64().map(|n| format!("{n:.3}")));
            if let Some(text) = text {
                ceilings.insert(family.clone(), text);
            }
        }
    }
    ceilings
}

/// Recompute the manifest's row-derived claims over `rows` and name each
/// divergence.
///
/// This is the reproduction path: a reader holding the published store
/// and the manifest does not have to trust the page. Claims that come
/// from the suite file rather than the rows — label rules, agreement
/// ceilings — are anchored by the commitment's provenance digest rather
/// than recomputed here. `rows` is the snapshot's horizon: callers pass
/// the store's first `store.rows` rows.
#[must_use]
pub fn check(snapshot: &Snapshot, rows: &[Row]) -> Vec<String> {
    let mut divergences = Vec::new();
    if rows.len() != snapshot.store.rows {
        divergences.push(format!(
            "store rows: manifest says {}, the supplied rows are {}",
            snapshot.store.rows,
            rows.len()
        ));
    }
    let expected_items = snapshot.selection.items_set();
    for workload in &snapshot.workloads {
        let prefix = format!("workload `{}` over `{}`", workload.door, workload.suite);
        let group: Vec<Row> = rows
            .iter()
            .filter(|row| {
                row.suite == workload.suite
                    && row.suite_digest == workload.suite_digest
                    && row.door == workload.door
            })
            .cloned()
            .collect();
        let matching: Vec<Row> = run_groups(&group)
            .into_iter()
            .find(|(key, _)| {
                key.door_identity == serde_json::to_string(&workload.identity).unwrap_or_default()
                    && key.question_digest == workload.question_digest
                    && key.gate_digest == workload.gate_digest
                    && key.estimator == workload.estimator
                    && key.samples == workload.samples
                    && key.seed_base == workload.seed_base
            })
            .map(|(_, group)| group)
            .unwrap_or_default();
        if matching.is_empty() {
            divergences.push(format!("{prefix}: no rows reproduce its run key"));
            continue;
        }
        if matching.len() != workload.rows {
            divergences.push(format!(
                "{prefix}: {} rows published, {} reproduce",
                workload.rows,
                matching.len()
            ));
        }
        let canonical: Vec<Row> = matching
            .iter()
            .filter(|row| row.permutation.is_none())
            .cloned()
            .collect();
        if snapshot.selection.declared {
            let coverage = CoverageView::of(&Coverage::of(&canonical, &expected_items));
            match &workload.coverage {
                Some(published) if *published != coverage => divergences.push(format!(
                    "{prefix}: coverage {:?} published, {:?} reproduces",
                    published, coverage
                )),
                None => divergences.push(format!(
                    "{prefix}: coverage published as undeclared, selection is declared"
                )),
                _ => {}
            }
        }
        for family in &workload.families {
            let rows_in: Vec<&Row> = canonical
                .iter()
                .filter(|row| row.family == family.family)
                .collect();
            let answered = rows_in.iter().filter(|row| row.is_scored()).count();
            let refused = rows_in.iter().filter(|row| row.is_refused()).count();
            let correct = rows_in
                .iter()
                .filter(|row| row.correct == Some(true))
                .count();
            if answered != family.answered || refused != family.refused || correct != family.correct
            {
                divergences.push(format!(
                    "{prefix} family `{}`: published {}/{} answered, {} refused, {} correct; \
                     reproduces {}/{} answered, {} refused, {} correct",
                    family.family,
                    family.answered,
                    family.asked,
                    family.refused,
                    family.correct,
                    answered,
                    rows_in.len(),
                    refused,
                    correct
                ));
            }
        }
        let latencies: Vec<f64> = matching.iter().filter_map(|row| row.latency_ms).collect();
        let mut profile = Profile::timed(&latencies);
        profile.refusals = Some(matching.iter().filter(|row| row.is_refused()).count());
        if profile != workload.latency {
            divergences.push(format!(
                "{prefix}: latency {profile:?} reproduces, {:?} published",
                workload.latency
            ));
        }
        let sources = label_sources(&matching);
        if sources != workload.label_sources {
            divergences.push(format!(
                "{prefix}: label sources {sources:?} reproduce, {:?} published",
                workload.label_sources
            ));
        }
    }
    divergences
}

/// The SHA-256 of `bytes`, hex — the store and document binding.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// The snapshot's own name: the generation time made filename-safe, plus
/// enough of the chain head to keep two snapshots of one store distinct.
#[must_use]
pub fn snapshot_id(generated_at: &str, head: Option<&str>) -> String {
    let stamp: String = generated_at
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    match head {
        Some(head) => {
            let tail: String = head
                .strip_prefix("receipt:")
                .unwrap_or(head)
                .chars()
                .filter(|ch| ch.is_ascii_hexdigit())
                .take(8)
                .collect();
            if tail.is_empty() {
                stamp
            } else {
                format!("{stamp}-{tail}")
            }
        }
        None => stamp,
    }
}

/// Render the snapshot as the public page.
///
/// `prior` is the preserved index entries before this snapshot, so the
/// page carries the history it joins rather than reading as the only
/// measurement ever taken.
#[must_use]
pub fn render(snapshot: &Snapshot, prior: &[IndexEntry]) -> String {
    let mut page = String::new();
    page.push_str("# OpenAgents decision-door benchmark and status\n\n");
    page.push_str(&format!(
        "Snapshot `{}`, generated {}. This page is a view over committed evidence, \
         not a live probe: every number below reproduces from the receipt-chained \
         store it cites.\n\n",
        snapshot.id, snapshot.generated_at
    ));
    page.push_str("## Evidence\n\n");
    page.push_str(&format!(
        "- Store: {} rows, receipt chain head `{}` (sha256 `{}`).\n",
        snapshot.store.rows,
        snapshot.store.head.as_deref().unwrap_or("unknown"),
        snapshot.store.sha256
    ));
    if let Some(commitment) = &snapshot.commitment {
        page.push_str(&format!(
            "- Report commitment: `{}` (`{}`). Verify the store against it with \
             `gym verify --store <store> --commitment {}`.\n",
            commitment.digest, commitment.file, commitment.file
        ));
    } else {
        page.push_str(
            "- No report commitment is published with this snapshot; the store's \
             chain verifies but no retained anchor binds it to this record.\n",
        );
    }
    page.push_str(
        "- Reproduce the claims: `gym verify --store <store> --snapshot <dir>` recomputes \
         every claim in `manifest.json` over the store and names any divergence.\n\n",
    );
    render_selection(snapshot, &mut page);
    render_workloads(snapshot, &mut page);
    render_status(snapshot, &mut page);
    render_history(snapshot, prior, &mut page);
    page.push_str(
        "\n_See `docs/gym/published-snapshots.md` for what each number may claim \
         and `manifest.json` in this directory for the checkable form of every \
         claim on this page._\n",
    );
    page
}

fn render_selection(snapshot: &Snapshot, page: &mut String) {
    page.push_str("## Coverage\n\n");
    if !snapshot.selection.declared {
        page.push_str(
            "**Coverage is not declared.** No suite selection was supplied at \
             publication, so what this run was meant to ask is unknown. The \
             figures below are a partial record and cannot read as a completed \
             evaluation.\n\n",
        );
        return;
    }
    let selection = &snapshot.selection;
    page.push_str(&format!(
        "Suite `{}` (`{}`) declared {} items over partition(s) {} across {} door(s).\n\n",
        selection.suite,
        selection.suite_digest,
        selection.items.len(),
        selection.partitions.join(", "),
        selection.doors.len()
    ));
    page.push_str(&format!(
        "Overall coverage: **{}**.\n\n",
        snapshot.coverage_word()
    ));
    for workload in &snapshot.workloads {
        match &workload.coverage {
            Some(coverage) if coverage.complete() => page.push_str(&format!(
                "- `{}`: **complete** — {} of {} expected items recorded once each.\n",
                workload.door, coverage.expected, coverage.expected
            )),
            Some(coverage) => page.push_str(&format!(
                "- `{}`: **partial** — {} answered, {} refused, {} missing, {} duplicate, \
                 {} unexpected of {} expected.\n",
                workload.door,
                coverage.answered,
                coverage.refused,
                coverage.missing.len(),
                coverage.duplicates.len(),
                coverage.unexpected.len(),
                coverage.expected
            )),
            None => page.push_str(&format!(
                "- `{}`: coverage not declared for suite digest `{}` — these rows \
                 report their own state and borrow nothing from the declaration.\n",
                workload.door, workload.suite_digest
            )),
        }
    }
    page.push('\n');
}

fn render_workloads(snapshot: &Snapshot, page: &mut String) {
    page.push_str("## Workloads\n\n");
    if snapshot.workloads.is_empty() {
        page.push_str("No rows in the store.\n\n");
        return;
    }
    let mut suites: BTreeSet<&str> = BTreeSet::new();
    for workload in &snapshot.workloads {
        suites.insert(&workload.suite);
    }
    for suite in suites {
        let mixed = snapshot
            .workloads
            .iter()
            .filter(|workload| workload.suite == *suite)
            .count()
            > 1;
        if mixed {
            page.push_str(&format!(
                "Suite `{suite}` has more than one measurement in this snapshot. They ran \
                 under different identities or configurations and are shown separately — \
                 a controlled ranking across them is not supported.\n\n"
            ));
        }
        for workload in snapshot
            .workloads
            .iter()
            .filter(|workload| workload.suite == *suite)
        {
            render_workload(workload, snapshot.selection.declared, page);
        }
    }
}

fn render_workload(workload: &Workload, declared: bool, page: &mut String) {
    page.push_str(&format!(
        "### `{}` on `{}`\n\n",
        workload.door, workload.suite
    ));
    let identity = &workload.identity;
    let model = if identity.model.is_empty() {
        "unreported"
    } else {
        identity.model.as_str()
    };
    let verified = if identity.verified {
        "verifiable"
    } else {
        "unverifiable — the door publishes a label, not a proof"
    };
    page.push_str(&format!(
        "- Artifact identity: model `{model}`, artifact signature `{}`, \
         {verified}.\n",
        if identity.artifact_signature.is_empty() {
            "unreported"
        } else {
            identity.artifact_signature.as_str()
        }
    ));
    if !identity.execution.is_empty() {
        let settings: Vec<String> = identity
            .execution
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        page.push_str(&format!("- Execution: {}.\n", settings.join(", ")));
    }
    page.push_str(&format!(
        "- Digests: suite `{}`, question set `{}`, gate `{}`.\n",
        workload.suite_digest,
        workload.question_digest.as_deref().unwrap_or("unpinned"),
        workload.gate_digest.as_deref().unwrap_or("unjudged")
    ));
    let trial = match (workload.samples, workload.seed_base) {
        (Some(samples), Some(seed)) => format!("{samples} draw(s) from seed base {seed}"),
        (Some(samples), None) => format!("{samples} draw(s), no seed reported"),
        (None, Some(seed)) => format!("draw count unreported, seed base {seed}"),
        (None, None) => "trial configuration unreported".to_string(),
    };
    page.push_str(&format!(
        "- Estimator `{}`, {trial}. Window {} through {} over {} row(s).\n\n",
        workload.estimator, workload.window.first, workload.window.last, workload.rows
    ));
    render_family_table(workload, declared, page);
    render_latency(workload, page);
    render_cost(workload, page);
    render_labels(workload, page);
    page.push('\n');
}

fn render_family_table(workload: &Workload, declared: bool, page: &mut String) {
    if workload.families.is_empty() {
        return;
    }
    page.push_str(
        "| Family | Asked | Answered | Refused | Missing | Correct | Accuracy |\n\
         |---|---|---|---|---|---|---|\n",
    );
    for family in &workload.families {
        let asked = if declared {
            family.asked.to_string()
        } else {
            format!("{} recorded", family.asked)
        };
        let accuracy = if family.answered == 0 {
            "—".to_string()
        } else {
            format!("{:.3}", family.correct as f64 / family.answered as f64)
        };
        let missing = if declared {
            family.missing.to_string()
        } else {
            "unknown".to_string()
        };
        page.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            family.family,
            asked,
            family.answered,
            family.refused,
            missing,
            family.correct,
            accuracy
        ));
    }
    page.push_str(
        "\nAccuracy is correct over answered only; refused and missing items are \
         workload the door did not complete, stated in their own columns rather \
         than folded into either side.\n\n",
    );
}

fn render_latency(workload: &Workload, page: &mut String) {
    let profile = &workload.latency;
    if profile.calls == 0 {
        page.push_str(
            "- Latency: **not timed** — no row in this group recorded a call duration.\n",
        );
        return;
    }
    let p50 = profile
        .latency_p50_ms
        .map(|ms| format!("{ms:.0} ms"))
        .unwrap_or_else(|| "unmeasured".to_string());
    let p95 = profile
        .latency_p95_ms
        .map(|ms| format!("{ms:.0} ms"))
        .unwrap_or_else(|| "unmeasured".to_string());
    page.push_str(&format!(
        "- Latency: p50 {p50}, p95 {p95} over {} timed call(s) in the window above. \
         Percentiles are nearest rank — p95 of a small run is that run's \
         nth-largest call, not a floor. Wall clock on the recording host; a \
         comparison across hosts is not controlled.\n",
        profile.calls
    ));
}

fn render_cost(workload: &Workload, page: &mut String) {
    match &workload.cost {
        Some(DeclaredCost {
            cost: Cost::Metered { usd_per_decision },
            source,
            unit,
            ..
        }) => page.push_str(&format!(
            "- Cost: ${usd_per_decision} per decision ({}, source: {}).\n",
            unit.as_deref().unwrap_or("unit unstated"),
            source.as_deref().unwrap_or("source unstated")
        )),
        Some(DeclaredCost {
            cost: Cost::UnmeteredLocalLane,
            ..
        }) => page.push_str(
            "- Cost: not metered: the door runs on local hardware, and nobody prices \
             its device time or power. Not metered does not mean free.\n",
        ),
        None => page.push_str("- Cost: not measured.\n"),
    }
}

fn render_labels(workload: &Workload, page: &mut String) {
    let sources = if workload.label_sources.is_empty() {
        "unrecorded".to_string()
    } else {
        workload.label_sources.join(", ")
    };
    page.push_str(&format!("- Label provenance: {sources}"));
    if let Some(rule) = &workload.label_rule {
        page.push_str(&format!("; rule `{rule}`"));
    }
    if !workload.agreement.is_empty() {
        let ceilings: Vec<String> = workload
            .agreement
            .iter()
            .map(|(family, ceiling)| format!("{family} ≤ {ceiling}"))
            .collect();
        page.push_str(&format!("; agreement ceiling {}", ceilings.join(", ")));
    } else {
        page.push_str("; agreement ceiling unstated");
    }
    page.push_str(". Accuracy cannot exceed the agreement its labels rest on.\n");
}

fn render_status(snapshot: &Snapshot, page: &mut String) {
    page.push_str("## Service status\n\n");
    match &snapshot.status {
        None => page.push_str(
            "No live service status is published with this snapshot. The evidence \
             above is historical measurement; it says nothing about whether the \
             service is reachable now.\n",
        ),
        Some(note) => {
            page.push_str(&format!(
                "`{}` reported **{}** at {}",
                note.source, note.state, note.checked_at
            ));
            if !note.detail.is_empty() {
                page.push_str(&format!(" — {}", note.detail));
            }
            page.push_str(".\n\n");
            if note.stale {
                page.push_str(
                    "The source declares this status **stale**; treat it as history, \
                     not current health.\n",
                );
            }
            match (
                rfc3339_seconds(&note.checked_at),
                rfc3339_seconds(&snapshot.generated_at),
            ) {
                (Some(checked), Some(generated)) if generated >= checked => {
                    let age = generated - checked;
                    page.push_str(&format!(
                        "Freshness: the status was checked {} before this snapshot \
                         was generated.\n",
                        render_age(age)
                    ));
                }
                _ => page.push_str(
                    "Freshness: unknown — the check time could not be read against \
                     the generation time.\n",
                ),
            }
        }
    }
}

fn render_history(snapshot: &Snapshot, prior: &[IndexEntry], page: &mut String) {
    page.push_str("\n## Earlier snapshots\n\n");
    if prior.is_empty() {
        page.push_str("This is the first published snapshot.\n");
        return;
    }
    page.push_str(
        "Earlier snapshots are preserved in `index.json` with their evidence \
         references; each remains checkable against the store it cites.\n\n",
    );
    for entry in prior {
        page.push_str(&format!(
            "- `{}` ({}): coverage {}, head `{}`{}.\n",
            entry.id,
            entry.generated_at,
            entry.coverage,
            entry.head.as_deref().unwrap_or("unknown"),
            entry
                .commitment_digest
                .as_ref()
                .map(|digest| format!(", commitment `{digest}`"))
                .unwrap_or_default()
        ));
    }
    let _ = snapshot;
}

/// An age in seconds, said plainly.
fn render_age(seconds: u64) -> String {
    if seconds < 120 {
        format!("{seconds} second(s)")
    } else if seconds < 7_200 {
        format!("{} minute(s)", seconds / 60)
    } else if seconds < 172_800 {
        format!("{} hour(s)", seconds / 3_600)
    } else {
        format!("{} day(s)", seconds / 86_400)
    }
}

/// Seconds since the epoch of an RFC 3339 `YYYY-MM-DDTHH:MM:SSZ`
/// timestamp, or `None` when it does not parse. Only the UTC form this
/// crate writes is accepted — a status note in another offset is a
/// freshness nobody can read, and `None` says so.
#[must_use]
pub fn rfc3339_seconds(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != 20 || bytes.get(10) != Some(&b'T') || bytes.get(19) != Some(&b'Z') {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<u64> { text.get(from..to)?.parse().ok() };
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    // Days from civil, Howard Hinnant's algorithm — the inverse of
    // `eval::utc_from_unix`.
    let year = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64) as u64)
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::coverage::Expected;
    use crate::row::RefusalCode;
    use crate::suite::Partition;

    fn suite() -> Suite {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/caller-v1/suite.json");
        Suite::load_file(path.to_str().unwrap()).unwrap()
    }

    fn provenance(suite: &Suite) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/caller-v1/suite.json");
        let text = std::fs::read_to_string(path).unwrap();
        let _ = suite;
        serde_json::from_str::<Value>(&text)
            .unwrap()
            .get("provenance")
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn row(suite: &Suite, item: &crate::suite::Item, door: &str) -> Row {
        let mut distribution = IndexMap::new();
        distribution.insert("a".to_string(), 0.9);
        distribution.insert("b".to_string(), 0.1);
        Row {
            recorded_at: "2026-09-21T00:00:00Z".to_string(),
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: Some("caller-v1".to_string()),
            question_digest: Some("question-digest".to_string()),
            split: item.partition.as_str().to_string(),
            family: item.family.clone(),
            item_id: item.id.clone(),
            door: door.to_string(),
            door_identity: DoorIdentity {
                model: "kev-quiet".to_string(),
                artifact_signature: "artifact-1".to_string(),
                execution: [("backend".to_string(), "metal".to_string())]
                    .into_iter()
                    .collect(),
                verified: true,
                ..DoorIdentity::default()
            },
            estimator: "greedy".to_string(),
            samples: Some(1),
            seed_base: Some(0),
            latency_ms: Some(180.0),
            ..Row::default()
        }
        .scored(distribution, true)
    }

    fn declared(suite: &Suite, doors: &[&str]) -> Expected {
        Expected::of(
            suite,
            &[Partition::Development],
            None,
            None,
            doors.iter().map(|door| door.to_string()).collect(),
        )
        .unwrap()
    }

    fn development_rows(suite: &Suite, door: &str) -> Vec<Row> {
        suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .map(|item| row(suite, item, door))
            .collect()
    }

    fn snapshot(rows: &[Row], expected: Option<&Expected>, suite: &Suite) -> Snapshot {
        build(
            "20260921T000000Z-deadbeef",
            "2026-09-21T00:00:00Z",
            rows,
            expected,
            Some(suite),
            None,
            StoreRef {
                sha256: "store-sha".to_string(),
                head: Some("deadbeef".to_string()),
                rows: rows.len(),
            },
            None,
            None,
            &[],
        )
    }

    #[test]
    fn a_complete_run_reads_complete() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        assert_eq!(snapshot.coverage_word(), "complete");
        let page = render(&snapshot, &[]);
        assert!(page.contains("**complete**"), "{page}");
        assert!(!page.contains("**partial**"), "{page}");
    }

    #[test]
    fn a_missing_item_is_partial_never_complete() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let mut rows = development_rows(&suite, "kev");
        rows.remove(0);
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        assert_eq!(snapshot.coverage_word(), "partial");
        let page = render(&snapshot, &[]);
        assert!(page.contains("**partial**"), "{page}");
        assert!(page.contains("1 missing"), "{page}");
        let coverage = snapshot.workloads[0].coverage.as_ref().unwrap();
        assert_eq!(coverage.missing.len(), 1);
    }

    #[test]
    fn an_undeclared_selection_cannot_read_as_complete() {
        let suite = suite();
        let rows = development_rows(&suite, "kev");
        let snapshot = snapshot(&rows, None, &suite);
        assert_eq!(snapshot.coverage_word(), "undeclared");
        let page = render(&snapshot, &[]);
        assert!(page.contains("Coverage is not declared"), "{page}");
        assert!(
            page.contains("cannot read as a completed evaluation"),
            "{page}"
        );
        assert!(snapshot.workloads[0].coverage.is_none());
    }

    #[test]
    fn mixed_identities_are_separate_workloads_never_ranked() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let mut rows = development_rows(&suite, "kev");
        let mut other = development_rows(&suite, "kev");
        for row in &mut other {
            row.door_identity.artifact_signature = "artifact-2".to_string();
        }
        rows.extend(other);
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        assert_eq!(snapshot.workloads.len(), 2);
        let page = render(&snapshot, &[]);
        assert!(
            page.contains("a controlled ranking across them is not supported"),
            "{page}"
        );
    }

    #[test]
    fn a_refusal_is_reported_not_hidden() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let mut rows = development_rows(&suite, "kev");
        let mut refused = rows[1].clone();
        refused.answered = false;
        refused.correct = None;
        refused.raw_top = None;
        refused.distribution = None;
        refused.selected = None;
        rows[1] = refused.refused(RefusalCode::Busy);
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        let coverage = snapshot.workloads[0].coverage.as_ref().unwrap();
        assert!(coverage.complete());
        assert_eq!(coverage.refused, 1);
        let page = render(&snapshot, &[]);
        assert!(
            page.contains("refused and missing items are workload the door did not complete"),
            "{page}"
        );
    }

    #[test]
    fn untimed_is_not_a_zero() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let mut rows = development_rows(&suite, "kev");
        for row in &mut rows {
            row.latency_ms = None;
        }
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        let page = render(&snapshot, &[]);
        assert!(page.contains("**not timed**"), "{page}");
        assert!(!page.contains("p50 0"), "{page}");
    }

    #[test]
    fn a_metered_price_needs_its_source_and_unit() {
        let unsourced = DeclaredCost {
            door: "kev".to_string(),
            cost: Cost::Metered {
                usd_per_decision: 0.002,
            },
            source: None,
            unit: Some("usd_per_decision".to_string()),
        };
        assert!(unsourced.check().unwrap_err().contains("source"));
        let ununit = DeclaredCost {
            source: Some("price-list-2026-09".to_string()),
            unit: None,
            ..unsourced.clone()
        };
        assert!(ununit.check().unwrap_err().contains("unit"));
        let declared = DeclaredCost {
            unit: Some("usd_per_decision".to_string()),
            ..ununit
        };
        assert!(declared.check().is_ok());
    }

    #[test]
    fn unmetered_is_not_zero_and_unmeasured_is_unmeasured() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let unmetered = build(
            "id",
            "2026-09-21T00:00:00Z",
            &rows,
            Some(&expected),
            Some(&suite),
            None,
            StoreRef::default(),
            None,
            None,
            &[DeclaredCost {
                door: "kev".to_string(),
                cost: Cost::UnmeteredLocalLane,
                source: None,
                unit: None,
            }],
        );
        let page = render(&unmetered, &[]);
        assert!(page.contains("the door runs on local hardware"), "{page}");
        assert!(page.contains("Not metered does not mean free"), "{page}");
        let unmeasured = snapshot(&rows, Some(&expected), &suite);
        assert!(render(&unmeasured, &[]).contains("Cost: not measured"));
    }

    #[test]
    fn status_is_separate_and_its_freshness_is_stated() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let absent = snapshot(&rows, Some(&expected), &suite);
        let page = render(&absent, &[]);
        assert!(
            page.contains("No live service status is published"),
            "{page}"
        );
        assert!(page.contains("historical measurement"), "{page}");

        let noted = build(
            "id",
            "2026-09-21T02:00:00Z",
            &rows,
            Some(&expected),
            Some(&suite),
            None,
            StoreRef::default(),
            None,
            Some(StatusNote {
                source: "operator probe".to_string(),
                checked_at: "2026-09-21T00:30:00Z".to_string(),
                state: "operational".to_string(),
                detail: String::new(),
                stale: false,
            }),
            &[],
        );
        let page = render(&noted, &[]);
        assert!(
            page.contains("`operator probe` reported **operational**"),
            "{page}"
        );
        assert!(page.contains("90 minute(s)"), "{page}");

        let stale = StatusNote {
            stale: true,
            ..noted.status.clone().unwrap()
        };
        let stale = build(
            "id",
            "2026-09-21T02:00:00Z",
            &rows,
            Some(&expected),
            Some(&suite),
            None,
            StoreRef::default(),
            None,
            Some(stale),
            &[],
        );
        assert!(render(&stale, &[]).contains("**stale**"));
    }

    #[test]
    fn the_manifest_reproduces_its_claims() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        assert!(check(&snapshot, &rows).is_empty());
    }

    #[test]
    fn a_tampered_claim_is_a_named_divergence() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let mut snapshot = snapshot(&rows, Some(&expected), &suite);
        snapshot.workloads[0].families[0].correct += 1;
        let divergences = check(&snapshot, &rows);
        assert_eq!(divergences.len(), 1, "{divergences:?}");
        assert!(divergences[0].contains("family"), "{divergences:?}");
    }

    #[test]
    fn a_workload_the_rows_cannot_reproduce_is_named() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let mut snapshot = snapshot(&rows, Some(&expected), &suite);
        snapshot.workloads[0].identity.artifact_signature = "artifact-9".to_string();
        let divergences = check(&snapshot, &rows);
        assert!(divergences.iter().any(|line| line.contains("run key")));
    }

    #[test]
    fn the_page_carries_only_aggregates_and_safe_metadata() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let mut rows = development_rows(&suite, "kev");
        rows[0].distribution = Some(
            [("tenant-private-wording".to_string(), 1.0)]
                .into_iter()
                .collect(),
        );
        let snapshot = snapshot(&rows, Some(&expected), &suite);
        let page = render(&snapshot, &[]);
        let manifest = serde_json::to_string(&snapshot).unwrap();
        for output in [&page, &manifest] {
            assert!(!output.contains("tenant-private-wording"), "{output}");
            assert!(!output.contains("distribution"), "{output}");
            assert!(!output.contains("raw_top"), "{output}");
        }
    }

    #[test]
    fn label_provenance_and_agreement_travel_with_the_rows() {
        let suite = suite();
        let expected = declared(&suite, &["kev"]);
        let rows = development_rows(&suite, "kev");
        let snapshot = build(
            "id",
            "2026-09-21T00:00:00Z",
            &rows,
            Some(&expected),
            Some(&suite),
            Some(&provenance(&suite)),
            StoreRef::default(),
            None,
            None,
            &[],
        );
        let page = render(&snapshot, &[]);
        assert!(page.contains("Label provenance:"), "{page}");
        assert!(page.contains("author"), "{page}");
    }

    #[test]
    fn rfc3339_seconds_inverts_utc_from_unix() {
        for seconds in [0, 1_700_000_000, 1_758_787_200] {
            let text = crate::eval::utc_from_unix(seconds);
            assert_eq!(rfc3339_seconds(&text), Some(seconds), "{text}");
        }
        assert_eq!(rfc3339_seconds("not a time"), None);
        assert_eq!(rfc3339_seconds("2026-09-21 00:00:00Z"), None);
    }
}
