//! Rust-native optimizer substrate contracts for Psionic.
//!
//! The first landing keeps candidate identity, lineage state, persisted run
//! state, and top-level run receipts explicit and machine-readable. Later
//! issues add evaluation contracts, search loops, reflection, merge, and proof
//! lanes above these core artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const RUN_SPEC_PREFIX: &[u8] = b"psionic_optimize_run_spec|";
const CANDIDATE_MANIFEST_PREFIX: &[u8] = b"psionic_optimize_candidate_manifest|";
const CASE_MANIFEST_PREFIX: &[u8] = b"psionic_optimize_case_manifest|";
const LINEAGE_STATE_PREFIX: &[u8] = b"psionic_optimize_lineage_state|";
const CASE_RECEIPT_PREFIX: &[u8] = b"psionic_optimize_case_receipt|";
const BATCH_RECEIPT_PREFIX: &[u8] = b"psionic_optimize_batch_receipt|";
const FRONTIER_SNAPSHOT_PREFIX: &[u8] = b"psionic_optimize_frontier_snapshot|";
const ITERATION_RECEIPT_PREFIX: &[u8] = b"psionic_optimize_iteration_receipt|";
const SEARCH_STATE_PREFIX: &[u8] = b"psionic_optimize_search_state|";
const REFLECTIVE_DATASET_PREFIX: &[u8] = b"psionic_optimize_reflective_dataset|";
const REFLECTION_PROMPT_PREFIX: &[u8] = b"psionic_optimize_reflection_prompt|";
const PROPOSER_RECEIPT_PREFIX: &[u8] = b"psionic_optimize_proposer_receipt|";
const MERGE_RECEIPT_PREFIX: &[u8] = b"psionic_optimize_merge_receipt|";
const RUN_RECEIPT_PREFIX: &[u8] = b"psionic_optimize_run_receipt|";
const RETAINED_EVAL_RUN_BUNDLE_PREFIX: &[u8] = b"psionic_optimize_retained_eval_run_bundle|";
const CANDIDATE_COMPARISON_MANIFEST_PREFIX: &[u8] =
    b"psionic_optimize_candidate_comparison_manifest|";

/// Stable frontier mode identifier declared by one optimization run spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationFrontierMode {
    /// One scalar objective and one retained frontier.
    Scalar,
    /// Multiple named objectives will later produce a hybrid frontier.
    Hybrid,
    /// Retain winners for each case-by-objective pair in addition to hybrid rows.
    Cartesian,
}

/// Final stop reason recorded by one optimization run receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationStopReason {
    /// The operator or caller stopped the run intentionally.
    Manual,
    /// The run reached its configured iteration budget.
    IterationBudgetReached,
    /// The run reached its configured candidate budget.
    CandidateBudgetReached,
    /// The proposer or sampler could not produce another honest search step.
    ProposalExhausted,
    /// The run completed without entering a proposal loop.
    NoSearchRequired,
}

/// Stable run spec for one optimizer invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationRunSpec {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable run identifier.
    pub run_id: String,
    /// Candidate family under optimization.
    pub family_id: String,
    /// Ordered dataset or retained-surface refs bound to this run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dataset_refs: Vec<String>,
    /// Optional issue or queue ref that owns the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_ref: Option<String>,
    /// Configured frontier mode.
    pub frontier_mode: OptimizationFrontierMode,
    /// Optional iteration budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration_budget: Option<u32>,
    /// Optional candidate budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_budget: Option<u32>,
    /// Stable digest over the run spec.
    pub spec_digest: String,
}

impl OptimizationRunSpec {
    /// Creates a run spec with default schema metadata and a stable digest.
    #[must_use]
    pub fn new(run_id: impl Into<String>, family_id: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            run_id: run_id.into(),
            family_id: family_id.into(),
            dataset_refs: Vec::new(),
            issue_ref: None,
            frontier_mode: OptimizationFrontierMode::Scalar,
            iteration_budget: None,
            candidate_budget: None,
            spec_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns a copy with the given dataset refs.
    #[must_use]
    pub fn with_dataset_refs(mut self, dataset_refs: Vec<String>) -> Self {
        self.dataset_refs = dataset_refs;
        self.with_stable_digest()
    }

    /// Returns a copy with the given issue ref.
    #[must_use]
    pub fn with_issue_ref(mut self, issue_ref: impl Into<String>) -> Self {
        self.issue_ref = Some(issue_ref.into());
        self.with_stable_digest()
    }

    /// Returns a copy with the given frontier mode.
    #[must_use]
    pub fn with_frontier_mode(mut self, frontier_mode: OptimizationFrontierMode) -> Self {
        self.frontier_mode = frontier_mode;
        self.with_stable_digest()
    }

    /// Returns a copy with the given iteration budget.
    #[must_use]
    pub fn with_iteration_budget(mut self, iteration_budget: u32) -> Self {
        self.iteration_budget = Some(iteration_budget);
        self.with_stable_digest()
    }

    /// Returns a copy with the given candidate budget.
    #[must_use]
    pub fn with_candidate_budget(mut self, candidate_budget: u32) -> Self {
        self.candidate_budget = Some(candidate_budget);
        self.with_stable_digest()
    }

    /// Returns the stable digest over the run spec payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.spec_digest.clear();
        stable_digest(RUN_SPEC_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.spec_digest = self.stable_digest();
        self
    }
}

/// Stable manifest for one candidate artifact under optimization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCandidateManifest {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Candidate family under optimization.
    pub family_id: String,
    /// Stable run id that first materialized this candidate.
    pub originating_run_id: String,
    /// Text-renderable named component map for the candidate.
    pub components: BTreeMap<String, String>,
    /// Parent candidate ids in deterministic order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_candidate_ids: Vec<String>,
    /// Ordered provenance refs that justify how the candidate was materialized.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_refs: Vec<String>,
    /// Optional issue or queue ref that owns the candidate row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_ref: Option<String>,
    /// Stable digest over the candidate manifest payload.
    pub manifest_digest: String,
}

impl OptimizationCandidateManifest {
    /// Creates a candidate manifest with stable digest metadata.
    #[must_use]
    pub fn new(
        candidate_id: impl Into<String>,
        family_id: impl Into<String>,
        originating_run_id: impl Into<String>,
        components: BTreeMap<String, String>,
    ) -> Self {
        Self {
            schema_version: 1,
            candidate_id: candidate_id.into(),
            family_id: family_id.into(),
            originating_run_id: originating_run_id.into(),
            components,
            parent_candidate_ids: Vec::new(),
            provenance_refs: Vec::new(),
            issue_ref: None,
            manifest_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns a copy with the given parent candidate ids.
    #[must_use]
    pub fn with_parent_candidate_ids(mut self, parent_candidate_ids: Vec<String>) -> Self {
        self.parent_candidate_ids = parent_candidate_ids;
        self.with_stable_digest()
    }

    /// Returns a copy with the given provenance refs.
    #[must_use]
    pub fn with_provenance_refs(mut self, provenance_refs: Vec<String>) -> Self {
        self.provenance_refs = provenance_refs;
        self.with_stable_digest()
    }

    /// Returns a copy with the given issue ref.
    #[must_use]
    pub fn with_issue_ref(mut self, issue_ref: impl Into<String>) -> Self {
        self.issue_ref = Some(issue_ref.into());
        self.with_stable_digest()
    }

    /// Returns the stable digest over the candidate manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.manifest_digest.clear();
        stable_digest(CANDIDATE_MANIFEST_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.manifest_digest = self.stable_digest();
        self
    }
}

/// Split membership for one retained evaluation case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationCaseSplit {
    /// Train-time batch sampling surface.
    Train,
    /// Held-out retained validation surface.
    Validation,
    /// Test-only surface.
    Test,
    /// Shadow-only comparison surface.
    Shadow,
}

/// Stable manifest for one retained evaluation case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCaseManifest {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable case identifier.
    pub case_id: String,
    /// Split membership for the case.
    pub split: OptimizationCaseSplit,
    /// Optional label or expected outcome identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Structured string metadata for the case.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Ordered evidence refs that justify the case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    /// Stable digest over the case manifest payload.
    pub case_digest: String,
}

impl OptimizationCaseManifest {
    /// Creates a case manifest with stable digest metadata.
    #[must_use]
    pub fn new(case_id: impl Into<String>, split: OptimizationCaseSplit) -> Self {
        Self {
            schema_version: 1,
            case_id: case_id.into(),
            split,
            label: None,
            metadata: BTreeMap::new(),
            evidence_refs: Vec::new(),
            case_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns a copy with the given label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.with_stable_digest()
    }

    /// Returns a copy with the given metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: BTreeMap<String, String>) -> Self {
        self.metadata = metadata;
        self.with_stable_digest()
    }

    /// Returns a copy with the given evidence refs.
    #[must_use]
    pub fn with_evidence_refs(mut self, evidence_refs: Vec<String>) -> Self {
        self.evidence_refs = evidence_refs;
        self.with_stable_digest()
    }

    /// Returns the stable digest over the case manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.case_digest.clear();
        stable_digest(CASE_MANIFEST_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.case_digest = self.stable_digest();
        self
    }
}

/// Shared typed evaluator feedback for one case.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationSharedFeedback {
    /// Short summary of the evaluation result.
    pub summary: String,
    /// Ordered detail rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

impl OptimizationSharedFeedback {
    /// Creates one shared feedback record.
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    /// Returns a copy with the given detail rows.
    #[must_use]
    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }
}

/// Typed evaluator feedback scoped to one candidate component.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationComponentFeedback {
    /// Short summary of the component-local feedback.
    pub summary: String,
    /// Ordered detail rows for the component.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

impl OptimizationComponentFeedback {
    /// Creates one component feedback record.
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    /// Returns a copy with the given detail rows.
    #[must_use]
    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }
}

/// Case-level evaluation receipt for one candidate against one retained case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCaseEvaluationReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Candidate identifier under evaluation.
    pub candidate_id: String,
    /// Candidate manifest digest under evaluation.
    pub candidate_manifest_digest: String,
    /// Retained case identifier.
    pub case_id: String,
    /// Retained case digest.
    pub case_digest: String,
    /// Scalar score for the case.
    pub scalar_score: i64,
    /// Named objective scores for the case.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub objective_scores: BTreeMap<String, i64>,
    /// Shared evaluator feedback.
    pub shared_feedback: OptimizationSharedFeedback,
    /// Per-component feedback map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub component_feedback: BTreeMap<String, OptimizationComponentFeedback>,
    /// Unified cache key used by the optimizer substrate.
    pub cache_key: String,
    /// Stable digest over the case receipt payload.
    pub receipt_digest: String,
}

impl OptimizationCaseEvaluationReceipt {
    /// Creates one case receipt with a stable cache key and digest.
    #[must_use]
    pub fn new(
        candidate: &OptimizationCandidateManifest,
        case: &OptimizationCaseManifest,
        scalar_score: i64,
        objective_scores: BTreeMap<String, i64>,
        shared_feedback: OptimizationSharedFeedback,
        component_feedback: BTreeMap<String, OptimizationComponentFeedback>,
    ) -> Self {
        Self {
            schema_version: 1,
            candidate_id: candidate.candidate_id.clone(),
            candidate_manifest_digest: candidate.manifest_digest.clone(),
            case_id: case.case_id.clone(),
            case_digest: case.case_digest.clone(),
            scalar_score,
            objective_scores,
            shared_feedback,
            component_feedback,
            cache_key: OptimizationEvaluationCache::cache_key(candidate, case),
            receipt_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns the stable digest over the case receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(CASE_RECEIPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Unified case-evaluation cache for the optimizer substrate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationEvaluationCache {
    /// Cached case receipts keyed by candidate-manifest digest plus case digest.
    pub entries: BTreeMap<String, OptimizationCaseEvaluationReceipt>,
}

impl OptimizationEvaluationCache {
    /// Builds the stable cache key for one candidate-plus-case pair.
    #[must_use]
    pub fn cache_key(
        candidate: &OptimizationCandidateManifest,
        case: &OptimizationCaseManifest,
    ) -> String {
        format!("{}:{}", candidate.manifest_digest, case.case_digest)
    }

    /// Returns one cached receipt when present.
    #[must_use]
    pub fn lookup(
        &self,
        candidate: &OptimizationCandidateManifest,
        case: &OptimizationCaseManifest,
    ) -> Option<&OptimizationCaseEvaluationReceipt> {
        self.entries.get(&Self::cache_key(candidate, case))
    }

    /// Inserts or replaces one cached case receipt.
    pub fn insert(
        &mut self,
        candidate: &OptimizationCandidateManifest,
        case: &OptimizationCaseManifest,
        receipt: OptimizationCaseEvaluationReceipt,
    ) {
        self.entries
            .insert(Self::cache_key(candidate, case), receipt);
    }
}

/// Aggregated batch receipt for one candidate across a retained case batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationBatchEvaluationReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Candidate identifier under evaluation.
    pub candidate_id: String,
    /// Candidate manifest digest under evaluation.
    pub candidate_manifest_digest: String,
    /// Ordered case receipts for the batch.
    pub case_receipts: Vec<OptimizationCaseEvaluationReceipt>,
    /// Sum of scalar case scores for the batch.
    pub aggregated_scalar_score: i64,
    /// Sum of objective scores for the batch.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aggregated_objective_scores: BTreeMap<String, i64>,
    /// Number of cache hits for the batch.
    pub cache_hit_count: u32,
    /// Number of cache misses for the batch.
    pub cache_miss_count: u32,
    /// Stable digest over the batch receipt payload.
    pub receipt_digest: String,
}

impl OptimizationBatchEvaluationReceipt {
    /// Creates one batch receipt and aggregates objective totals from the cases.
    #[must_use]
    pub fn new(
        run_id: impl Into<String>,
        candidate: &OptimizationCandidateManifest,
        case_receipts: Vec<OptimizationCaseEvaluationReceipt>,
        cache_hit_count: u32,
        cache_miss_count: u32,
    ) -> Self {
        let aggregated_scalar_score = case_receipts.iter().map(|case| case.scalar_score).sum();
        let mut aggregated_objective_scores = BTreeMap::new();
        for case_receipt in &case_receipts {
            for (objective_name, objective_score) in &case_receipt.objective_scores {
                *aggregated_objective_scores
                    .entry(objective_name.clone())
                    .or_insert(0) += objective_score;
            }
        }
        Self {
            schema_version: 1,
            report_id: String::from("psionic.optimize.batch_evaluation_receipt.v1"),
            run_id: run_id.into(),
            candidate_id: candidate.candidate_id.clone(),
            candidate_manifest_digest: candidate.manifest_digest.clone(),
            case_receipts,
            aggregated_scalar_score,
            aggregated_objective_scores,
            cache_hit_count,
            cache_miss_count,
            receipt_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns the stable digest over the batch receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(BATCH_RECEIPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Winner row for one retained case in the frontier snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCaseFrontierRow {
    /// Stable case identifier.
    pub case_id: String,
    /// Winning candidate identifier.
    pub winning_candidate_id: String,
    /// Winning scalar score for the case.
    pub winning_scalar_score: i64,
}

/// Winner row for one named objective in the frontier snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationObjectiveFrontierRow {
    /// Named objective identifier.
    pub objective_name: String,
    /// Winning candidate identifier.
    pub winning_candidate_id: String,
    /// Winning aggregated objective score.
    pub winning_score: i64,
}

/// Winner row for one case-by-objective pair in the frontier snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCartesianFrontierRow {
    /// Stable case identifier.
    pub case_id: String,
    /// Named objective identifier.
    pub objective_name: String,
    /// Winning candidate identifier.
    pub winning_candidate_id: String,
    /// Winning objective score for the case-objective pair.
    pub winning_score: i64,
}

/// Snapshot of the retained frontier for one optimizer run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationFrontierSnapshot {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Frontier mode used to interpret the snapshot.
    pub frontier_mode: OptimizationFrontierMode,
    /// Source candidates considered for the frontier.
    pub source_candidate_ids: Vec<String>,
    /// Ordered per-case frontier rows.
    pub case_frontier: Vec<OptimizationCaseFrontierRow>,
    /// Ordered per-objective frontier rows.
    pub objective_frontier: Vec<OptimizationObjectiveFrontierRow>,
    /// Ordered per-case-by-objective frontier rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cartesian_frontier: Vec<OptimizationCartesianFrontierRow>,
    /// Deduplicated candidate ids retained by the hybrid frontier.
    pub hybrid_candidate_ids: Vec<String>,
    /// Stable digest over the frontier snapshot payload.
    pub snapshot_digest: String,
}

impl OptimizationFrontierSnapshot {
    /// Builds a frontier snapshot from candidate batch receipts.
    #[must_use]
    pub fn from_batches(
        run_id: impl Into<String>,
        frontier_mode: OptimizationFrontierMode,
        batches: &[OptimizationBatchEvaluationReceipt],
    ) -> Self {
        let run_id = run_id.into();
        let mut case_winners = BTreeMap::<String, (String, i64)>::new();
        let mut objective_winners = BTreeMap::<String, (String, i64)>::new();
        let mut cartesian_winners = BTreeMap::<(String, String), (String, i64)>::new();
        let mut source_candidate_ids = Vec::new();

        for batch in batches {
            push_unique(&mut source_candidate_ids, batch.candidate_id.clone());
            for case_receipt in &batch.case_receipts {
                let candidate_score = case_receipt.scalar_score;
                let candidate_id = case_receipt.candidate_id.clone();
                case_winners
                    .entry(case_receipt.case_id.clone())
                    .and_modify(|winner| {
                        if candidate_score > winner.1 {
                            *winner = (candidate_id.clone(), candidate_score);
                        }
                    })
                    .or_insert((candidate_id, candidate_score));
                if frontier_mode == OptimizationFrontierMode::Cartesian {
                    for (objective_name, objective_score) in &case_receipt.objective_scores {
                        let key = (case_receipt.case_id.clone(), objective_name.clone());
                        let candidate_id = case_receipt.candidate_id.clone();
                        cartesian_winners
                            .entry(key)
                            .and_modify(|winner| {
                                if *objective_score > winner.1 {
                                    *winner = (candidate_id.clone(), *objective_score);
                                }
                            })
                            .or_insert((candidate_id, *objective_score));
                    }
                }
            }
            for (objective_name, objective_score) in &batch.aggregated_objective_scores {
                let candidate_id = batch.candidate_id.clone();
                objective_winners
                    .entry(objective_name.clone())
                    .and_modify(|winner| {
                        if *objective_score > winner.1 {
                            *winner = (candidate_id.clone(), *objective_score);
                        }
                    })
                    .or_insert((candidate_id, *objective_score));
            }
        }

        let case_frontier = case_winners
            .into_iter()
            .map(|(case_id, (winning_candidate_id, winning_scalar_score))| {
                OptimizationCaseFrontierRow {
                    case_id,
                    winning_candidate_id,
                    winning_scalar_score,
                }
            })
            .collect::<Vec<_>>();
        let objective_frontier = objective_winners
            .into_iter()
            .map(|(objective_name, (winning_candidate_id, winning_score))| {
                OptimizationObjectiveFrontierRow {
                    objective_name,
                    winning_candidate_id,
                    winning_score,
                }
            })
            .collect::<Vec<_>>();
        let cartesian_frontier = cartesian_winners
            .into_iter()
            .map(
                |((case_id, objective_name), (winning_candidate_id, winning_score))| {
                    OptimizationCartesianFrontierRow {
                        case_id,
                        objective_name,
                        winning_candidate_id,
                        winning_score,
                    }
                },
            )
            .collect::<Vec<_>>();

        let mut hybrid_candidate_ids = Vec::new();
        for row in &case_frontier {
            push_unique(&mut hybrid_candidate_ids, row.winning_candidate_id.clone());
        }
        if frontier_mode == OptimizationFrontierMode::Hybrid
            || frontier_mode == OptimizationFrontierMode::Cartesian
        {
            for row in &objective_frontier {
                push_unique(&mut hybrid_candidate_ids, row.winning_candidate_id.clone());
            }
        }
        if frontier_mode == OptimizationFrontierMode::Cartesian {
            for row in &cartesian_frontier {
                push_unique(&mut hybrid_candidate_ids, row.winning_candidate_id.clone());
            }
        }

        Self {
            schema_version: 1,
            report_id: String::from("psionic.optimize.frontier_snapshot.v1"),
            run_id,
            frontier_mode,
            source_candidate_ids,
            case_frontier,
            objective_frontier,
            cartesian_frontier,
            hybrid_candidate_ids,
            snapshot_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns the stable digest over the frontier snapshot payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.snapshot_digest.clear();
        stable_digest(FRONTIER_SNAPSHOT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.snapshot_digest = self.stable_digest();
        self
    }
}

/// One reflective dataset row derived from typed evaluation feedback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationReflectiveDatasetRow {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable case digest.
    pub case_digest: String,
    /// Optional retained case label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_label: Option<String>,
    /// Scalar score observed for the case.
    pub scalar_score: i64,
    /// Objective scores observed for the case.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub objective_scores: BTreeMap<String, i64>,
    /// Shared typed evaluator feedback.
    pub shared_feedback: OptimizationSharedFeedback,
    /// Per-component typed feedback for the case.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub component_feedback: BTreeMap<String, OptimizationComponentFeedback>,
}

/// Reflective dataset built from one minibatch receipt plus retained case metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationReflectiveDataset {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Parent candidate identifier.
    pub candidate_id: String,
    /// Source minibatch receipt digest.
    pub source_batch_receipt_digest: String,
    /// Selected components targeted by reflection.
    pub selected_component_ids: Vec<String>,
    /// Snapshot of selected component values before mutation.
    pub current_component_values: BTreeMap<String, String>,
    /// Ordered reflective dataset rows.
    pub rows: Vec<OptimizationReflectiveDatasetRow>,
    /// Stable digest over the reflective dataset payload.
    pub dataset_digest: String,
}

impl OptimizationReflectiveDataset {
    /// Returns the stable digest over the reflective dataset payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.dataset_digest.clear();
        stable_digest(REFLECTIVE_DATASET_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.dataset_digest = self.stable_digest();
        self
    }
}

/// Rendered reflection prompt for one selected candidate component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationReflectionPrompt {
    /// Stable component identifier under mutation.
    pub component_id: String,
    /// Prompt family or rendering mode.
    pub prompt_kind: String,
    /// Rendered prompt text.
    pub prompt_text: String,
    /// Prompt-local metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Stable digest over the prompt payload.
    pub prompt_digest: String,
}

impl OptimizationReflectionPrompt {
    /// Returns the stable digest over the prompt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.prompt_digest.clear();
        stable_digest(REFLECTION_PROMPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.prompt_digest = self.stable_digest();
        self
    }
}

/// Component-level mutation diff proposed by one proposer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationComponentDiff {
    /// Stable component identifier.
    pub component_id: String,
    /// Previous component value before mutation.
    pub previous_value: String,
    /// Proposed component value after mutation.
    pub proposed_value: String,
}

/// Receipt for one proposer invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationProposerReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Proposer implementation identifier.
    pub proposer_kind: String,
    /// Parent candidate identifier.
    pub parent_candidate_id: String,
    /// Proposed candidate identifier.
    pub proposed_candidate_id: String,
    /// Source minibatch receipt digest.
    pub source_batch_receipt_digest: String,
    /// Optional reflective dataset digest when one was built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflective_dataset_digest: Option<String>,
    /// Selected components inspected by the proposer.
    pub selected_component_ids: Vec<String>,
    /// Actual component diffs emitted by the proposer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_diffs: Vec<OptimizationComponentDiff>,
    /// Rendered prompts used by the proposer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<OptimizationReflectionPrompt>,
    /// Extra proposer metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Stable digest over the proposer receipt payload.
    pub receipt_digest: String,
}

impl OptimizationProposerReceipt {
    /// Returns the stable digest over the proposer receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(PROPOSER_RECEIPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Merge context attached to one proposed candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationMergeContext {
    /// Shared ancestor candidate identifier that justified the merge.
    pub ancestor_candidate_id: String,
    /// Component-to-source-parent map for the merged candidate.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub component_source_candidate_ids: BTreeMap<String, String>,
}

/// Merge receipt outcome kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationMergeOutcome {
    /// A merge candidate was materialized and scheduled for gating.
    Attempted,
    /// The merge candidate beat the parent score floor and was retained.
    Accepted,
    /// The merge candidate did not beat the parent score floor.
    Rejected,
}

/// Receipt for one lineage-aware merge attempt or outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationMergeReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Proposer receipt digest that materialized the merge candidate.
    pub proposer_receipt_digest: String,
    /// Merged candidate identifier.
    pub merged_candidate_id: String,
    /// Parent candidate identifiers used by the merge.
    pub parent_candidate_ids: Vec<String>,
    /// Shared ancestor candidate identifier.
    pub ancestor_candidate_id: String,
    /// Component-to-source-parent map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub component_source_candidate_ids: BTreeMap<String, String>,
    /// Candidate ids used as the gating score floor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gating_candidate_ids: Vec<String>,
    /// Merge receipt outcome kind.
    pub outcome: OptimizationMergeOutcome,
    /// Proposed minibatch scalar score when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_minibatch_score: Option<i64>,
    /// Parent gating floor when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gating_score_floor: Option<i64>,
    /// Frontier snapshot digest after accepting the merge when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_snapshot_digest: Option<String>,
    /// Optional plain-language reason for the outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Stable digest over the merge receipt payload.
    pub receipt_digest: String,
}

impl OptimizationMergeReceipt {
    /// Returns the stable digest over the merge receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(MERGE_RECEIPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Candidate proposal plus the receipt that explains how it was materialized.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCandidateProposal {
    /// Proposed candidate manifest.
    pub candidate: OptimizationCandidateManifest,
    /// Proposer receipt for this mutation attempt.
    pub proposer_receipt: OptimizationProposerReceipt,
    /// Additional candidate ids the proposal must beat on the same minibatch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gating_candidate_ids: Vec<String>,
    /// Optional merge context for lineage-aware recombination proposals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_context: Option<OptimizationMergeContext>,
}

/// Evaluator contract for one optimizer candidate family.
pub trait OptimizationEvaluator {
    /// Evaluates one candidate across the supplied retained cases.
    fn evaluate_candidate(
        &mut self,
        run_id: &str,
        candidate: &OptimizationCandidateManifest,
        cases: &[OptimizationCaseManifest],
        cache: &mut OptimizationEvaluationCache,
    ) -> OptimizationBatchEvaluationReceipt;
}

/// Proposer contract for one optimizer candidate family.
pub trait OptimizationCandidateProposer {
    /// Returns the next candidate proposal when one exists.
    fn propose_candidate(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
    ) -> Option<OptimizationCandidateProposal>;
}

/// Selects which candidate components should be targeted by reflection.
pub trait OptimizationReflectionComponentSelector {
    /// Returns the ordered component ids to inspect or mutate.
    fn select_components(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
    ) -> Vec<String>;
}

/// Builds reflective datasets from typed evaluation feedback.
pub trait OptimizationReflectiveDatasetBuilder {
    /// Builds one reflective dataset scoped to the selected components.
    fn build_dataset(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
        selected_component_ids: &[String],
    ) -> OptimizationReflectiveDataset;
}

/// Renders component-scoped prompts from a reflective dataset.
pub trait OptimizationReflectionPromptBuilder {
    /// Builds one prompt per selected component.
    fn build_prompts(
        &mut self,
        current_candidate: &OptimizationCandidateManifest,
        reflective_dataset: &OptimizationReflectiveDataset,
    ) -> Vec<OptimizationReflectionPrompt>;
}

/// Mutates selected component values from reflective datasets plus prompts.
pub trait OptimizationMutationStrategy {
    /// Returns proposed new values keyed by component id.
    fn propose_component_values(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        reflective_dataset: &OptimizationReflectiveDataset,
        prompts: &[OptimizationReflectionPrompt],
    ) -> BTreeMap<String, String>;
}

/// Feedback-driven component selector that prefers components mentioned in typed feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationFeedbackComponentSelector {
    /// Upper bound on selected components.
    pub max_components: usize,
    /// Whether to fall back to all candidate components when feedback is silent.
    pub fallback_to_all_components: bool,
}

impl OptimizationFeedbackComponentSelector {
    /// Creates a feedback-driven selector.
    #[must_use]
    pub fn new(max_components: usize, fallback_to_all_components: bool) -> Self {
        Self {
            max_components: max_components.max(1),
            fallback_to_all_components,
        }
    }
}

impl OptimizationReflectionComponentSelector for OptimizationFeedbackComponentSelector {
    fn select_components(
        &mut self,
        _state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
    ) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let feedback_component_ids = minibatch_receipt
            .case_receipts
            .iter()
            .flat_map(|receipt| receipt.component_feedback.keys().cloned())
            .collect::<BTreeSet<_>>();
        let mut selected = current_candidate
            .components
            .keys()
            .filter(|component_id| feedback_component_ids.contains(*component_id))
            .filter_map(|component_id| {
                if seen.insert(component_id.clone()) {
                    Some(component_id.clone())
                } else {
                    None
                }
            })
            .take(self.max_components)
            .collect::<Vec<_>>();
        if selected.is_empty() && self.fallback_to_all_components {
            selected = current_candidate
                .components
                .keys()
                .take(self.max_components)
                .cloned()
                .collect();
        }
        selected
    }
}

/// Builds a reflective dataset directly from typed batch feedback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationTypedFeedbackDatasetBuilder;

impl OptimizationReflectiveDatasetBuilder for OptimizationTypedFeedbackDatasetBuilder {
    fn build_dataset(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
        selected_component_ids: &[String],
    ) -> OptimizationReflectiveDataset {
        let mut case_lookup = BTreeMap::new();
        for case in state
            .train_cases
            .iter()
            .chain(state.validation_cases.iter())
            .cloned()
        {
            case_lookup.insert(case.case_id.clone(), case);
        }

        let rows = minibatch_receipt
            .case_receipts
            .iter()
            .map(|receipt| OptimizationReflectiveDatasetRow {
                case_id: receipt.case_id.clone(),
                case_digest: receipt.case_digest.clone(),
                case_label: case_lookup
                    .get(receipt.case_id.as_str())
                    .and_then(|case| case.label.clone()),
                scalar_score: receipt.scalar_score,
                objective_scores: receipt.objective_scores.clone(),
                shared_feedback: receipt.shared_feedback.clone(),
                component_feedback: receipt.component_feedback.clone(),
            })
            .collect::<Vec<_>>();
        let current_component_values = selected_component_ids
            .iter()
            .filter_map(|component_id| {
                current_candidate
                    .components
                    .get(component_id)
                    .map(|value| (component_id.clone(), value.clone()))
            })
            .collect::<BTreeMap<_, _>>();

        OptimizationReflectiveDataset {
            schema_version: 1,
            report_id: String::from("psionic.optimize.reflective_dataset.v1"),
            run_id: state.run_spec.run_id.clone(),
            candidate_id: current_candidate.candidate_id.clone(),
            source_batch_receipt_digest: minibatch_receipt.receipt_digest.clone(),
            selected_component_ids: selected_component_ids.to_vec(),
            current_component_values,
            rows,
            dataset_digest: String::new(),
        }
        .with_stable_digest()
    }
}

/// Deterministic prompt renderer over one reflective dataset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationDefaultReflectionPromptBuilder {
    /// Maximum reflective rows to render for each component prompt.
    pub max_rows_per_component: usize,
}

impl OptimizationDefaultReflectionPromptBuilder {
    /// Creates a prompt builder with a bounded case-row budget.
    #[must_use]
    pub fn new(max_rows_per_component: usize) -> Self {
        Self {
            max_rows_per_component: max_rows_per_component.max(1),
        }
    }
}

impl OptimizationReflectionPromptBuilder for OptimizationDefaultReflectionPromptBuilder {
    fn build_prompts(
        &mut self,
        current_candidate: &OptimizationCandidateManifest,
        reflective_dataset: &OptimizationReflectiveDataset,
    ) -> Vec<OptimizationReflectionPrompt> {
        reflective_dataset
            .selected_component_ids
            .iter()
            .map(|component_id| {
                let filtered_rows = reflective_dataset
                    .rows
                    .iter()
                    .filter(|row| row.component_feedback.contains_key(component_id))
                    .take(self.max_rows_per_component)
                    .collect::<Vec<_>>();
                let rows = if filtered_rows.is_empty() {
                    reflective_dataset
                        .rows
                        .iter()
                        .take(self.max_rows_per_component)
                        .collect::<Vec<_>>()
                } else {
                    filtered_rows
                };
                let mut prompt_text = format!(
                    "Optimize component `{component_id}` for candidate `{}` in family `{}`.\nCurrent value:\n{}\n",
                    current_candidate.candidate_id,
                    current_candidate.family_id,
                    current_candidate
                        .components
                        .get(component_id)
                        .cloned()
                        .unwrap_or_default()
                );
                for row in &rows {
                    prompt_text.push_str(&format!(
                        "\nCase `{}` label={:?} scalar_score={}\nShared feedback: {}\n",
                        row.case_id, row.case_label, row.scalar_score, row.shared_feedback.summary
                    ));
                    if !row.shared_feedback.details.is_empty() {
                        prompt_text.push_str("Shared details:\n");
                        for detail in &row.shared_feedback.details {
                            prompt_text.push_str(&format!("- {detail}\n"));
                        }
                    }
                    if let Some(component_feedback) = row.component_feedback.get(component_id) {
                        prompt_text.push_str(&format!(
                            "Component feedback: {}\n",
                            component_feedback.summary
                        ));
                        for detail in &component_feedback.details {
                            prompt_text.push_str(&format!("- {detail}\n"));
                        }
                    }
                }

                OptimizationReflectionPrompt {
                    component_id: component_id.clone(),
                    prompt_kind: String::from("component_reflection_v1"),
                    prompt_text,
                    metadata: BTreeMap::from([
                        (String::from("row_count"), rows.len().to_string()),
                        (
                            String::from("source_batch_receipt_digest"),
                            reflective_dataset.source_batch_receipt_digest.clone(),
                        ),
                    ]),
                    prompt_digest: String::new(),
                }
                .with_stable_digest()
            })
            .collect()
    }
}

/// Manual mutation strategy with explicit component-value overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationManualMutationStrategy {
    /// Explicit proposed values keyed by component id.
    pub proposed_values: BTreeMap<String, String>,
}

impl OptimizationMutationStrategy for OptimizationManualMutationStrategy {
    fn propose_component_values(
        &mut self,
        _state: &OptimizationSearchState,
        _current_candidate: &OptimizationCandidateManifest,
        reflective_dataset: &OptimizationReflectiveDataset,
        _prompts: &[OptimizationReflectionPrompt],
    ) -> BTreeMap<String, String> {
        reflective_dataset
            .selected_component_ids
            .iter()
            .filter_map(|component_id| {
                self.proposed_values
                    .get(component_id)
                    .map(|value| (component_id.clone(), value.clone()))
            })
            .collect()
    }
}

/// Feedback-driven test-double mutation strategy that adopts the first case label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCaseLabelMutationStrategy;

impl OptimizationMutationStrategy for OptimizationCaseLabelMutationStrategy {
    fn propose_component_values(
        &mut self,
        _state: &OptimizationSearchState,
        _current_candidate: &OptimizationCandidateManifest,
        reflective_dataset: &OptimizationReflectiveDataset,
        _prompts: &[OptimizationReflectionPrompt],
    ) -> BTreeMap<String, String> {
        let first_label = reflective_dataset
            .rows
            .iter()
            .find_map(|row| row.case_label.clone());
        let Some(first_label) = first_label else {
            return BTreeMap::new();
        };
        reflective_dataset
            .selected_component_ids
            .iter()
            .map(|component_id| (component_id.clone(), first_label.clone()))
            .collect()
    }
}

/// Generic reflective mutation proposer over explicit component selectors and mutation strategies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimizationReflectiveMutationProposer<S, D, P, M> {
    /// Stable proposer identifier.
    pub proposer_kind: String,
    /// Candidate id prefix for newly materialized variants.
    pub candidate_id_prefix: String,
    /// Extra provenance refs to copy into proposed candidates.
    pub provenance_refs: Vec<String>,
    /// Component selector.
    pub component_selector: S,
    /// Dataset builder.
    pub dataset_builder: D,
    /// Prompt builder.
    pub prompt_builder: P,
    /// Mutation strategy.
    pub mutation_strategy: M,
}

impl<S, D, P, M> OptimizationReflectiveMutationProposer<S, D, P, M> {
    /// Creates a reflective proposer from explicit subcomponents.
    #[must_use]
    pub fn new(
        proposer_kind: impl Into<String>,
        candidate_id_prefix: impl Into<String>,
        component_selector: S,
        dataset_builder: D,
        prompt_builder: P,
        mutation_strategy: M,
    ) -> Self {
        Self {
            proposer_kind: proposer_kind.into(),
            candidate_id_prefix: candidate_id_prefix.into(),
            provenance_refs: Vec::new(),
            component_selector,
            dataset_builder,
            prompt_builder,
            mutation_strategy,
        }
    }

    /// Returns a copy with the given extra provenance refs.
    #[must_use]
    pub fn with_provenance_refs(mut self, provenance_refs: Vec<String>) -> Self {
        self.provenance_refs = provenance_refs;
        self
    }
}

impl<S, D, P, M> OptimizationCandidateProposer
    for OptimizationReflectiveMutationProposer<S, D, P, M>
where
    S: OptimizationReflectionComponentSelector,
    D: OptimizationReflectiveDatasetBuilder,
    P: OptimizationReflectionPromptBuilder,
    M: OptimizationMutationStrategy,
{
    fn propose_candidate(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
    ) -> Option<OptimizationCandidateProposal> {
        let selected_component_ids =
            self.component_selector
                .select_components(state, current_candidate, minibatch_receipt);
        if selected_component_ids.is_empty() {
            return None;
        }

        let reflective_dataset = self.dataset_builder.build_dataset(
            state,
            current_candidate,
            minibatch_receipt,
            selected_component_ids.as_slice(),
        );
        let prompts = self
            .prompt_builder
            .build_prompts(current_candidate, &reflective_dataset);
        let proposed_values = self.mutation_strategy.propose_component_values(
            state,
            current_candidate,
            &reflective_dataset,
            prompts.as_slice(),
        );
        let mut updated_components = current_candidate.components.clone();
        let component_diffs = selected_component_ids
            .iter()
            .filter_map(|component_id| {
                let previous_value = current_candidate.components.get(component_id)?.clone();
                let proposed_value = proposed_values.get(component_id)?.clone();
                if proposed_value == previous_value {
                    return None;
                }
                updated_components.insert(component_id.clone(), proposed_value.clone());
                Some(OptimizationComponentDiff {
                    component_id: component_id.clone(),
                    previous_value,
                    proposed_value,
                })
            })
            .collect::<Vec<_>>();
        if component_diffs.is_empty() {
            return None;
        }

        let next_candidate_id = format!(
            "{}_{:04}",
            self.candidate_id_prefix,
            state.lineage_state.discovery_order.len() + 1
        );
        let mut provenance_refs = self.provenance_refs.clone();
        provenance_refs.push(format!(
            "reflective_dataset_digest:{}",
            reflective_dataset.dataset_digest
        ));
        provenance_refs.push(format!(
            "source_batch_receipt_digest:{}",
            minibatch_receipt.receipt_digest
        ));
        provenance_refs.push(format!("proposer_kind:{}", self.proposer_kind));
        let candidate = OptimizationCandidateManifest::new(
            next_candidate_id.clone(),
            current_candidate.family_id.clone(),
            current_candidate.originating_run_id.clone(),
            updated_components,
        )
        .with_parent_candidate_ids(vec![current_candidate.candidate_id.clone()])
        .with_provenance_refs(provenance_refs);
        let proposer_receipt = OptimizationProposerReceipt {
            schema_version: 1,
            report_id: String::from("psionic.optimize.proposer_receipt.v1"),
            run_id: state.run_spec.run_id.clone(),
            proposer_kind: self.proposer_kind.clone(),
            parent_candidate_id: current_candidate.candidate_id.clone(),
            proposed_candidate_id: next_candidate_id,
            source_batch_receipt_digest: minibatch_receipt.receipt_digest.clone(),
            reflective_dataset_digest: Some(reflective_dataset.dataset_digest.clone()),
            selected_component_ids,
            component_diffs,
            prompts,
            metadata: BTreeMap::from([(
                String::from("source_candidate_manifest_digest"),
                current_candidate.manifest_digest.clone(),
            )]),
            receipt_digest: String::new(),
        }
        .with_stable_digest();

        Some(OptimizationCandidateProposal {
            candidate,
            proposer_receipt,
            gating_candidate_ids: Vec::new(),
            merge_context: None,
        })
    }
}

/// Merge proposer that recombines compatible retained candidates through a shared ancestor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptimizationLineageAwareMergeProposer {
    /// Stable proposer identifier.
    pub proposer_kind: String,
    /// Candidate id prefix for merged variants.
    pub candidate_id_prefix: String,
    /// Extra provenance refs copied into merged candidates.
    pub provenance_refs: Vec<String>,
}

impl OptimizationLineageAwareMergeProposer {
    /// Creates a lineage-aware merge proposer.
    #[must_use]
    pub fn new(proposer_kind: impl Into<String>, candidate_id_prefix: impl Into<String>) -> Self {
        Self {
            proposer_kind: proposer_kind.into(),
            candidate_id_prefix: candidate_id_prefix.into(),
            provenance_refs: Vec::new(),
        }
    }

    /// Returns a copy with extra provenance refs.
    #[must_use]
    pub fn with_provenance_refs(mut self, provenance_refs: Vec<String>) -> Self {
        self.provenance_refs = provenance_refs;
        self
    }
}

impl OptimizationCandidateProposer for OptimizationLineageAwareMergeProposer {
    fn propose_candidate(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &OptimizationBatchEvaluationReceipt,
    ) -> Option<OptimizationCandidateProposal> {
        let current_score = state
            .accepted_validation_batches
            .get(current_candidate.candidate_id.as_str())
            .map_or(i64::MIN, |batch| batch.aggregated_scalar_score);
        let mut sibling_candidate_ids = state
            .latest_frontier_snapshot
            .as_ref()
            .map(|snapshot| snapshot.hybrid_candidate_ids.clone())
            .unwrap_or_else(|| state.lineage_state.retained_candidate_ids.clone())
            .into_iter()
            .filter(|candidate_id| candidate_id != &current_candidate.candidate_id)
            .collect::<Vec<_>>();
        sibling_candidate_ids.sort_by_key(|candidate_id| {
            std::cmp::Reverse(
                state
                    .accepted_validation_batches
                    .get(candidate_id.as_str())
                    .map_or(i64::MIN, |batch| batch.aggregated_scalar_score),
            )
        });

        for sibling_candidate_id in sibling_candidate_ids {
            let Some(sibling_candidate) =
                state.lineage_state.candidate(sibling_candidate_id.as_str())
            else {
                continue;
            };
            let sibling_score = state
                .accepted_validation_batches
                .get(sibling_candidate_id.as_str())
                .map_or(i64::MIN, |batch| batch.aggregated_scalar_score);
            let Some(ancestor_candidate_id) = latest_common_ancestor(
                &state.lineage_state,
                current_candidate.candidate_id.as_str(),
                sibling_candidate.candidate_id.as_str(),
            ) else {
                continue;
            };
            let Some(ancestor_candidate) = state
                .lineage_state
                .candidate(ancestor_candidate_id.as_str())
            else {
                continue;
            };
            let component_ids = current_candidate
                .components
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            if component_ids
                != sibling_candidate
                    .components
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                || component_ids
                    != ancestor_candidate
                        .components
                        .keys()
                        .cloned()
                        .collect::<BTreeSet<_>>()
            {
                continue;
            }

            let mut merged_components = BTreeMap::new();
            let mut component_source_candidate_ids = BTreeMap::new();
            let mut component_diffs = Vec::new();
            let mut used_sibling = false;
            for component_id in component_ids {
                let ancestor_value = match ancestor_candidate.components.get(component_id.as_str())
                {
                    Some(value) => value,
                    None => continue,
                };
                let current_value = match current_candidate.components.get(component_id.as_str()) {
                    Some(value) => value,
                    None => continue,
                };
                let sibling_value = match sibling_candidate.components.get(component_id.as_str()) {
                    Some(value) => value,
                    None => continue,
                };
                let (chosen_source_id, chosen_value) = if current_value == sibling_value {
                    (
                        current_candidate.candidate_id.clone(),
                        current_value.clone(),
                    )
                } else if current_value == ancestor_value && sibling_value != ancestor_value {
                    used_sibling = true;
                    (
                        sibling_candidate.candidate_id.clone(),
                        sibling_value.clone(),
                    )
                } else if sibling_value == ancestor_value && current_value != ancestor_value {
                    (
                        current_candidate.candidate_id.clone(),
                        current_value.clone(),
                    )
                } else if current_value != ancestor_value && sibling_value != ancestor_value {
                    if sibling_score > current_score {
                        used_sibling = true;
                        (
                            sibling_candidate.candidate_id.clone(),
                            sibling_value.clone(),
                        )
                    } else {
                        (
                            current_candidate.candidate_id.clone(),
                            current_value.clone(),
                        )
                    }
                } else {
                    continue;
                };
                if chosen_value != *current_value {
                    component_diffs.push(OptimizationComponentDiff {
                        component_id: component_id.clone(),
                        previous_value: current_value.clone(),
                        proposed_value: chosen_value.clone(),
                    });
                }
                component_source_candidate_ids.insert(component_id.clone(), chosen_source_id);
                merged_components.insert(component_id, chosen_value);
            }

            if !used_sibling
                || component_diffs.is_empty()
                || merged_components == current_candidate.components
                || merged_components == sibling_candidate.components
            {
                continue;
            }

            let next_candidate_id = format!(
                "{}_{:04}",
                self.candidate_id_prefix,
                state.lineage_state.discovery_order.len() + 1
            );
            let mut provenance_refs = self.provenance_refs.clone();
            provenance_refs.push(format!(
                "merge_ancestor_candidate_id:{}",
                ancestor_candidate_id
            ));
            provenance_refs.push(format!(
                "source_batch_receipt_digest:{}",
                minibatch_receipt.receipt_digest
            ));
            provenance_refs.push(format!(
                "secondary_parent_candidate_id:{}",
                sibling_candidate.candidate_id
            ));
            let candidate = OptimizationCandidateManifest::new(
                next_candidate_id.clone(),
                current_candidate.family_id.clone(),
                current_candidate.originating_run_id.clone(),
                merged_components,
            )
            .with_parent_candidate_ids(vec![
                current_candidate.candidate_id.clone(),
                sibling_candidate.candidate_id.clone(),
            ])
            .with_provenance_refs(provenance_refs);
            let proposer_receipt = OptimizationProposerReceipt {
                schema_version: 1,
                report_id: String::from("psionic.optimize.proposer_receipt.v1"),
                run_id: state.run_spec.run_id.clone(),
                proposer_kind: self.proposer_kind.clone(),
                parent_candidate_id: current_candidate.candidate_id.clone(),
                proposed_candidate_id: next_candidate_id,
                source_batch_receipt_digest: minibatch_receipt.receipt_digest.clone(),
                reflective_dataset_digest: None,
                selected_component_ids: component_diffs
                    .iter()
                    .map(|diff| diff.component_id.clone())
                    .collect(),
                component_diffs,
                prompts: Vec::new(),
                metadata: BTreeMap::from([
                    (
                        String::from("ancestor_candidate_id"),
                        ancestor_candidate_id.clone(),
                    ),
                    (
                        String::from("secondary_parent_candidate_id"),
                        sibling_candidate.candidate_id.clone(),
                    ),
                ]),
                receipt_digest: String::new(),
            }
            .with_stable_digest();

            return Some(OptimizationCandidateProposal {
                candidate,
                proposer_receipt,
                gating_candidate_ids: vec![
                    current_candidate.candidate_id.clone(),
                    sibling_candidate.candidate_id.clone(),
                ],
                merge_context: Some(OptimizationMergeContext {
                    ancestor_candidate_id,
                    component_source_candidate_ids,
                }),
            });
        }

        None
    }
}

fn latest_common_ancestor(
    lineage_state: &OptimizationLineageState,
    left_candidate_id: &str,
    right_candidate_id: &str,
) -> Option<String> {
    let left_ancestors = ancestor_candidate_ids(lineage_state, left_candidate_id);
    let right_ancestors = ancestor_candidate_ids(lineage_state, right_candidate_id);
    let discovery_order = lineage_state
        .discovery_order
        .iter()
        .enumerate()
        .map(|(index, candidate_id)| (candidate_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    left_ancestors
        .intersection(&right_ancestors)
        .max_by_key(|candidate_id| discovery_order.get(*candidate_id).copied().unwrap_or(0))
        .cloned()
}

fn ancestor_candidate_ids(
    lineage_state: &OptimizationLineageState,
    candidate_id: &str,
) -> BTreeSet<String> {
    fn visit(
        lineage_state: &OptimizationLineageState,
        candidate_id: &str,
        seen: &mut BTreeSet<String>,
    ) {
        let Some(candidate) = lineage_state.candidate(candidate_id) else {
            return;
        };
        for parent_candidate_id in &candidate.parent_candidate_ids {
            if seen.insert(parent_candidate_id.clone()) {
                visit(lineage_state, parent_candidate_id.as_str(), seen);
            }
        }
    }

    let mut seen = BTreeSet::new();
    visit(lineage_state, candidate_id, &mut seen);
    seen
}

/// Minibatch sampler for train-time proposal gating.
pub trait OptimizationMinibatchSampler {
    /// Samples one minibatch from the retained train cases.
    fn sample_minibatch(
        &mut self,
        iteration_index: u32,
        train_cases: &[OptimizationCaseManifest],
    ) -> Vec<OptimizationCaseManifest>;
}

/// Deterministic sequential minibatch sampler over retained train cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationSequentialMinibatchSampler {
    /// Number of cases to include in each minibatch.
    pub minibatch_size: usize,
}

impl OptimizationSequentialMinibatchSampler {
    /// Creates a sequential sampler with the given minibatch size.
    #[must_use]
    pub fn new(minibatch_size: usize) -> Self {
        Self {
            minibatch_size: minibatch_size.max(1),
        }
    }
}

impl OptimizationMinibatchSampler for OptimizationSequentialMinibatchSampler {
    fn sample_minibatch(
        &mut self,
        iteration_index: u32,
        train_cases: &[OptimizationCaseManifest],
    ) -> Vec<OptimizationCaseManifest> {
        if train_cases.is_empty() {
            return Vec::new();
        }
        let start = iteration_index as usize % train_cases.len();
        (0..self.minibatch_size)
            .map(|offset| train_cases[(start + offset) % train_cases.len()].clone())
            .collect()
    }
}

/// Iteration receipt emitted by the cheap-first optimization engine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationIterationReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Zero-based iteration index.
    pub iteration_index: u32,
    /// Baseline candidate under consideration.
    pub current_candidate_id: String,
    /// Proposed candidate id.
    pub proposed_candidate_id: String,
    /// Proposer receipt digest for the mutation attempt.
    pub proposer_receipt_digest: String,
    /// Ordered minibatch case ids.
    pub minibatch_case_ids: Vec<String>,
    /// Baseline minibatch scalar score.
    pub current_minibatch_score: i64,
    /// Proposed minibatch scalar score.
    pub proposed_minibatch_score: i64,
    /// Whether the proposal survived cheap-first gating.
    pub accepted: bool,
    /// Candidate id retained after the iteration.
    pub retained_candidate_id: String,
    /// Full validation scalar score for the retained candidate when re-evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_validation_score: Option<i64>,
    /// Latest frontier snapshot digest after the iteration when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_snapshot_digest: Option<String>,
    /// Total cache hits observed during the iteration.
    pub cache_hit_count: u32,
    /// Total cache misses observed during the iteration.
    pub cache_miss_count: u32,
    /// Stable digest over the iteration receipt payload.
    pub receipt_digest: String,
}

impl OptimizationIterationReceipt {
    /// Returns the stable digest over the iteration receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(ITERATION_RECEIPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Persisted optimizer search state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationSearchState {
    /// Stable schema version.
    pub schema_version: u16,
    /// Bound run spec for the search.
    pub run_spec: OptimizationRunSpec,
    /// Lineage state for all materialized candidates.
    pub lineage_state: OptimizationLineageState,
    /// Candidate currently driving proposal generation.
    pub current_candidate_id: String,
    /// Retained train cases for cheap-first gating.
    pub train_cases: Vec<OptimizationCaseManifest>,
    /// Retained validation cases for full evaluation.
    pub validation_cases: Vec<OptimizationCaseManifest>,
    /// Unified optimizer cache across minibatch and validation evaluation.
    pub evaluation_cache: OptimizationEvaluationCache,
    /// Full validation batches for accepted candidates keyed by candidate id.
    pub accepted_validation_batches: BTreeMap<String, OptimizationBatchEvaluationReceipt>,
    /// Latest frontier snapshot when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_frontier_snapshot: Option<OptimizationFrontierSnapshot>,
    /// Ordered iteration receipts emitted so far.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iteration_receipts: Vec<OptimizationIterationReceipt>,
    /// Ordered proposer receipts emitted so far.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposer_receipts: Vec<OptimizationProposerReceipt>,
    /// Ordered merge attempt and outcome receipts emitted so far.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merge_receipts: Vec<OptimizationMergeReceipt>,
    /// Current zero-based iteration count.
    pub current_iteration: u32,
    /// Total case evaluations across all batch receipts.
    pub total_case_evaluations: u32,
    /// Accepted proposal count.
    pub accepted_proposal_count: u32,
    /// Rejected proposal count.
    pub rejected_proposal_count: u32,
    /// Stable digest over the persisted search state.
    pub state_digest: String,
}

impl OptimizationSearchState {
    /// Returns the stable digest over the search state payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.state_digest.clear();
        stable_digest(SEARCH_STATE_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.state_digest = self.stable_digest();
        self
    }

    /// Writes the search state as pretty JSON.
    pub fn write_json(&self, output_path: impl AsRef<Path>) -> Result<(), OptimizationIoError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| OptimizationIoError::CreateDir {
                path: parent.display().to_string(),
                error,
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| OptimizationIoError::Write {
            path: output_path.display().to_string(),
            error,
        })
    }

    /// Loads the search state from one JSON artifact.
    pub fn read_json(input_path: impl AsRef<Path>) -> Result<Self, OptimizationIoError> {
        let input_path = input_path.as_ref();
        let body = fs::read_to_string(input_path).map_err(|error| OptimizationIoError::Read {
            path: input_path.display().to_string(),
            error,
        })?;
        let state: Self =
            serde_json::from_str(&body).map_err(|error| OptimizationIoError::Deserialize {
                path: input_path.display().to_string(),
                error,
            })?;
        Ok(state)
    }
}

/// Final outcome for one optimizer engine run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationEngineRunOutcome {
    /// Final persisted search state.
    pub state: OptimizationSearchState,
    /// Final top-level run receipt.
    pub run_receipt: OptimizationRunReceipt,
}

/// Cheap-first optimizer engine over manifest-backed candidates and retained cases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OptimizationEngine;

impl OptimizationEngine {
    /// Initializes one search state from a seed candidate and retained cases.
    pub fn initialize<E>(
        run_spec: OptimizationRunSpec,
        seed_candidate: OptimizationCandidateManifest,
        train_cases: Vec<OptimizationCaseManifest>,
        validation_cases: Vec<OptimizationCaseManifest>,
        evaluator: &mut E,
    ) -> Result<OptimizationSearchState, OptimizationEngineError>
    where
        E: OptimizationEvaluator,
    {
        if train_cases.is_empty() {
            return Err(OptimizationEngineError::MissingTrainCases {
                run_id: run_spec.run_id.clone(),
            });
        }
        if validation_cases.is_empty() {
            return Err(OptimizationEngineError::MissingValidationCases {
                run_id: run_spec.run_id.clone(),
            });
        }
        let validation_case_count = validation_cases.len() as u32;

        let mut lineage_state = OptimizationLineageState::new(run_spec.clone());
        lineage_state.register_candidate(seed_candidate.clone())?;

        let mut evaluation_cache = OptimizationEvaluationCache::default();
        let validation_batch = evaluator.evaluate_candidate(
            run_spec.run_id.as_str(),
            &seed_candidate,
            &validation_cases,
            &mut evaluation_cache,
        );
        let frontier_snapshot = OptimizationFrontierSnapshot::from_batches(
            run_spec.run_id.as_str(),
            run_spec.frontier_mode,
            std::slice::from_ref(&validation_batch),
        );
        lineage_state.set_retained_candidates(frontier_snapshot.hybrid_candidate_ids.clone())?;

        Ok(OptimizationSearchState {
            schema_version: 1,
            run_spec,
            lineage_state,
            current_candidate_id: seed_candidate.candidate_id.clone(),
            train_cases,
            validation_cases,
            evaluation_cache,
            accepted_validation_batches: BTreeMap::from([(
                seed_candidate.candidate_id,
                validation_batch,
            )]),
            latest_frontier_snapshot: Some(frontier_snapshot),
            iteration_receipts: Vec::new(),
            proposer_receipts: Vec::new(),
            merge_receipts: Vec::new(),
            current_iteration: 0,
            total_case_evaluations: validation_case_count,
            accepted_proposal_count: 0,
            rejected_proposal_count: 0,
            state_digest: String::new(),
        }
        .with_stable_digest())
    }

    /// Runs the engine until a stop reason is reached or the optional iteration cap is hit.
    pub fn run<E, P, S>(
        mut state: OptimizationSearchState,
        evaluator: &mut E,
        proposer: &mut P,
        sampler: &mut S,
        max_additional_iterations: Option<u32>,
    ) -> Result<OptimizationEngineRunOutcome, OptimizationEngineError>
    where
        E: OptimizationEvaluator,
        P: OptimizationCandidateProposer,
        S: OptimizationMinibatchSampler,
    {
        let mut executed_iterations = 0_u32;

        let stop_reason = loop {
            if let Some(iteration_budget) = state.run_spec.iteration_budget {
                if state.current_iteration >= iteration_budget {
                    break OptimizationStopReason::IterationBudgetReached;
                }
            }
            if let Some(candidate_budget) = state.run_spec.candidate_budget {
                if state.lineage_state.candidates.len() as u32 >= candidate_budget {
                    break OptimizationStopReason::CandidateBudgetReached;
                }
            }
            if max_additional_iterations.is_some_and(|limit| executed_iterations >= limit) {
                break OptimizationStopReason::Manual;
            }

            let current_candidate = state
                .lineage_state
                .candidate(state.current_candidate_id.as_str())
                .cloned()
                .ok_or_else(|| OptimizationEngineError::UnknownCurrentCandidate {
                    run_id: state.run_spec.run_id.clone(),
                    candidate_id: state.current_candidate_id.clone(),
                })?;
            let minibatch_cases =
                sampler.sample_minibatch(state.current_iteration, state.train_cases.as_slice());
            if minibatch_cases.is_empty() {
                break OptimizationStopReason::ProposalExhausted;
            }

            let current_batch = evaluator.evaluate_candidate(
                state.run_spec.run_id.as_str(),
                &current_candidate,
                minibatch_cases.as_slice(),
                &mut state.evaluation_cache,
            );
            let Some(proposal) =
                proposer.propose_candidate(&state, &current_candidate, &current_batch)
            else {
                break OptimizationStopReason::ProposalExhausted;
            };
            let proposed_candidate = proposal.candidate;
            let proposer_receipt = proposal.proposer_receipt;
            let gating_candidate_ids = proposal.gating_candidate_ids;
            let merge_context = proposal.merge_context;

            state
                .lineage_state
                .register_candidate(proposed_candidate.clone())?;
            state.proposer_receipts.push(proposer_receipt.clone());
            if let Some(merge_context) = &merge_context {
                state.merge_receipts.push(
                    OptimizationMergeReceipt {
                        schema_version: 1,
                        report_id: String::from("psionic.optimize.merge_receipt.v1"),
                        run_id: state.run_spec.run_id.clone(),
                        proposer_receipt_digest: proposer_receipt.receipt_digest.clone(),
                        merged_candidate_id: proposed_candidate.candidate_id.clone(),
                        parent_candidate_ids: proposed_candidate.parent_candidate_ids.clone(),
                        ancestor_candidate_id: merge_context.ancestor_candidate_id.clone(),
                        component_source_candidate_ids: merge_context
                            .component_source_candidate_ids
                            .clone(),
                        gating_candidate_ids: gating_candidate_ids.clone(),
                        outcome: OptimizationMergeOutcome::Attempted,
                        merged_minibatch_score: None,
                        gating_score_floor: None,
                        frontier_snapshot_digest: None,
                        reason: None,
                        receipt_digest: String::new(),
                    }
                    .with_stable_digest(),
                );
            }
            let proposed_batch = evaluator.evaluate_candidate(
                state.run_spec.run_id.as_str(),
                &proposed_candidate,
                minibatch_cases.as_slice(),
                &mut state.evaluation_cache,
            );
            let mut gating_score_floor = current_batch.aggregated_scalar_score;
            let mut additional_gating_batches = Vec::new();
            for gating_candidate_id in gating_candidate_ids
                .iter()
                .filter(|candidate_id| candidate_id.as_str() != current_candidate.candidate_id)
            {
                let gating_candidate = state
                    .lineage_state
                    .candidate(gating_candidate_id.as_str())
                    .cloned()
                    .ok_or_else(|| OptimizationEngineError::UnknownGatingCandidate {
                        run_id: state.run_spec.run_id.clone(),
                        candidate_id: gating_candidate_id.clone(),
                    })?;
                let gating_batch = evaluator.evaluate_candidate(
                    state.run_spec.run_id.as_str(),
                    &gating_candidate,
                    minibatch_cases.as_slice(),
                    &mut state.evaluation_cache,
                );
                gating_score_floor = gating_score_floor.max(gating_batch.aggregated_scalar_score);
                additional_gating_batches.push(gating_batch);
            }

            state.total_case_evaluations += (current_batch.case_receipts.len()
                + proposed_batch.case_receipts.len()
                + additional_gating_batches
                    .iter()
                    .map(|batch| batch.case_receipts.len())
                    .sum::<usize>()) as u32;

            let accepted = proposed_batch.aggregated_scalar_score > gating_score_floor;
            let mut retained_candidate_id = current_candidate.candidate_id.clone();
            let mut retained_validation_score = None;
            let mut frontier_snapshot_digest = state
                .latest_frontier_snapshot
                .as_ref()
                .map(|snapshot| snapshot.snapshot_digest.clone());
            if accepted {
                let validation_batch = evaluator.evaluate_candidate(
                    state.run_spec.run_id.as_str(),
                    &proposed_candidate,
                    state.validation_cases.as_slice(),
                    &mut state.evaluation_cache,
                );
                state.total_case_evaluations += validation_batch.case_receipts.len() as u32;
                retained_validation_score = Some(validation_batch.aggregated_scalar_score);
                retained_candidate_id = proposed_candidate.candidate_id.clone();
                state.current_candidate_id = proposed_candidate.candidate_id.clone();
                state
                    .accepted_validation_batches
                    .insert(proposed_candidate.candidate_id.clone(), validation_batch);
                let frontier_batches = state
                    .accepted_validation_batches
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                let frontier_snapshot = OptimizationFrontierSnapshot::from_batches(
                    state.run_spec.run_id.as_str(),
                    state.run_spec.frontier_mode,
                    frontier_batches.as_slice(),
                );
                frontier_snapshot_digest = Some(frontier_snapshot.snapshot_digest.clone());
                state
                    .lineage_state
                    .set_retained_candidates(frontier_snapshot.hybrid_candidate_ids.clone())?;
                state.latest_frontier_snapshot = Some(frontier_snapshot);
                state.accepted_proposal_count += 1;
            } else {
                state.rejected_proposal_count += 1;
            }
            if let Some(merge_context) = &merge_context {
                state.merge_receipts.push(
                    OptimizationMergeReceipt {
                        schema_version: 1,
                        report_id: String::from("psionic.optimize.merge_receipt.v1"),
                        run_id: state.run_spec.run_id.clone(),
                        proposer_receipt_digest: proposer_receipt.receipt_digest.clone(),
                        merged_candidate_id: proposed_candidate.candidate_id.clone(),
                        parent_candidate_ids: proposed_candidate.parent_candidate_ids.clone(),
                        ancestor_candidate_id: merge_context.ancestor_candidate_id.clone(),
                        component_source_candidate_ids: merge_context
                            .component_source_candidate_ids
                            .clone(),
                        gating_candidate_ids: gating_candidate_ids.clone(),
                        outcome: if accepted {
                            OptimizationMergeOutcome::Accepted
                        } else {
                            OptimizationMergeOutcome::Rejected
                        },
                        merged_minibatch_score: Some(proposed_batch.aggregated_scalar_score),
                        gating_score_floor: Some(gating_score_floor),
                        frontier_snapshot_digest: frontier_snapshot_digest.clone(),
                        reason: if accepted {
                            None
                        } else {
                            Some(format!(
                                "merged candidate score {} did not beat gating floor {}",
                                proposed_batch.aggregated_scalar_score, gating_score_floor
                            ))
                        },
                        receipt_digest: String::new(),
                    }
                    .with_stable_digest(),
                );
            }

            let cache_hit_count = current_batch.cache_hit_count
                + proposed_batch.cache_hit_count
                + additional_gating_batches
                    .iter()
                    .map(|batch| batch.cache_hit_count)
                    .sum::<u32>()
                + if accepted {
                    state
                        .accepted_validation_batches
                        .get(retained_candidate_id.as_str())
                        .map_or(0, |batch| batch.cache_hit_count)
                } else {
                    0
                };
            let cache_miss_count = current_batch.cache_miss_count
                + proposed_batch.cache_miss_count
                + additional_gating_batches
                    .iter()
                    .map(|batch| batch.cache_miss_count)
                    .sum::<u32>()
                + if accepted {
                    state
                        .accepted_validation_batches
                        .get(retained_candidate_id.as_str())
                        .map_or(0, |batch| batch.cache_miss_count)
                } else {
                    0
                };

            state.iteration_receipts.push(
                OptimizationIterationReceipt {
                    schema_version: 1,
                    report_id: String::from("psionic.optimize.iteration_receipt.v1"),
                    run_id: state.run_spec.run_id.clone(),
                    iteration_index: state.current_iteration,
                    current_candidate_id: current_candidate.candidate_id.clone(),
                    proposed_candidate_id: proposed_candidate.candidate_id.clone(),
                    proposer_receipt_digest: proposer_receipt.receipt_digest.clone(),
                    minibatch_case_ids: minibatch_cases
                        .iter()
                        .map(|case| case.case_id.clone())
                        .collect(),
                    current_minibatch_score: current_batch.aggregated_scalar_score,
                    proposed_minibatch_score: proposed_batch.aggregated_scalar_score,
                    accepted,
                    retained_candidate_id,
                    retained_validation_score,
                    frontier_snapshot_digest,
                    cache_hit_count,
                    cache_miss_count,
                    receipt_digest: String::new(),
                }
                .with_stable_digest(),
            );
            state.current_iteration += 1;
            executed_iterations += 1;
            state.state_digest = state.stable_digest();
        };

        let frontier_snapshot_refs = state
            .latest_frontier_snapshot
            .as_ref()
            .map(|snapshot| {
                vec![format!(
                    "frontier_snapshot_digest:{}",
                    snapshot.snapshot_digest
                )]
            })
            .unwrap_or_default();
        let run_receipt = OptimizationRunReceipt::from_state(
            &state.lineage_state,
            frontier_snapshot_refs,
            stop_reason,
        );
        Ok(OptimizationEngineRunOutcome { state, run_receipt })
    }
}

/// Engine-level failures for the cheap-first optimizer loop.
#[derive(Debug, Error)]
pub enum OptimizationEngineError {
    /// The run had no train cases for minibatch gating.
    #[error("optimization run `{run_id}` requires at least one train case")]
    MissingTrainCases {
        /// Stable run identifier.
        run_id: String,
    },
    /// The run had no validation cases for retained evaluation.
    #[error("optimization run `{run_id}` requires at least one validation case")]
    MissingValidationCases {
        /// Stable run identifier.
        run_id: String,
    },
    /// The current candidate id was not present in lineage state.
    #[error("optimization run `{run_id}` does not know current candidate `{candidate_id}`")]
    UnknownCurrentCandidate {
        /// Stable run identifier.
        run_id: String,
        /// Missing candidate id.
        candidate_id: String,
    },
    /// One configured gating candidate id was not present in lineage state.
    #[error("optimization run `{run_id}` does not know gating candidate `{candidate_id}`")]
    UnknownGatingCandidate {
        /// Stable run identifier.
        run_id: String,
        /// Missing candidate id.
        candidate_id: String,
    },
    /// Lineage state validation failed.
    #[error(transparent)]
    Lineage(#[from] OptimizationLineageStateError),
}

/// Ordered lineage state for one optimization run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationLineageState {
    /// Stable schema version.
    pub schema_version: u16,
    /// Run spec that owns the lineage state.
    pub run_spec: OptimizationRunSpec,
    /// Materialized candidates keyed by candidate id.
    pub candidates: BTreeMap<String, OptimizationCandidateManifest>,
    /// Candidate ids in discovery order.
    pub discovery_order: Vec<String>,
    /// Root candidates for the run in deterministic order.
    pub root_candidate_ids: Vec<String>,
    /// Retained candidate ids for the current run state.
    pub retained_candidate_ids: Vec<String>,
    /// Stable digest over the lineage state payload.
    pub state_digest: String,
}

impl OptimizationLineageState {
    /// Creates an empty lineage state for the given run.
    #[must_use]
    pub fn new(run_spec: OptimizationRunSpec) -> Self {
        Self {
            schema_version: 1,
            run_spec,
            candidates: BTreeMap::new(),
            discovery_order: Vec::new(),
            root_candidate_ids: Vec::new(),
            retained_candidate_ids: Vec::new(),
            state_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Registers one candidate in the lineage state.
    pub fn register_candidate(
        &mut self,
        candidate: OptimizationCandidateManifest,
    ) -> Result<(), OptimizationLineageStateError> {
        if candidate.family_id != self.run_spec.family_id {
            return Err(OptimizationLineageStateError::FamilyMismatch {
                expected_family_id: self.run_spec.family_id.clone(),
                actual_family_id: candidate.family_id,
            });
        }
        if candidate.originating_run_id != self.run_spec.run_id {
            return Err(OptimizationLineageStateError::RunMismatch {
                expected_run_id: self.run_spec.run_id.clone(),
                actual_run_id: candidate.originating_run_id,
            });
        }
        if self.candidates.contains_key(&candidate.candidate_id) {
            return Err(OptimizationLineageStateError::DuplicateCandidate {
                candidate_id: candidate.candidate_id,
            });
        }
        for parent_candidate_id in &candidate.parent_candidate_ids {
            if !self.candidates.contains_key(parent_candidate_id) {
                return Err(OptimizationLineageStateError::UnknownParent {
                    candidate_id: candidate.candidate_id.clone(),
                    parent_candidate_id: parent_candidate_id.clone(),
                });
            }
        }

        let candidate_id = candidate.candidate_id.clone();
        if candidate.parent_candidate_ids.is_empty() {
            push_unique(&mut self.root_candidate_ids, candidate_id.clone());
        }
        self.discovery_order.push(candidate_id.clone());
        self.candidates.insert(candidate_id.clone(), candidate);
        push_unique(&mut self.retained_candidate_ids, candidate_id);
        self.state_digest = self.stable_digest();
        Ok(())
    }

    /// Replaces the retained set with the supplied deterministic ids.
    pub fn set_retained_candidates(
        &mut self,
        retained_candidate_ids: Vec<String>,
    ) -> Result<(), OptimizationLineageStateError> {
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        for candidate_id in retained_candidate_ids {
            if !self.candidates.contains_key(&candidate_id) {
                return Err(OptimizationLineageStateError::UnknownRetainedCandidate {
                    candidate_id,
                });
            }
            if seen.insert(candidate_id.clone()) {
                ordered.push(candidate_id);
            }
        }
        self.retained_candidate_ids = ordered;
        self.state_digest = self.stable_digest();
        Ok(())
    }

    /// Returns a candidate manifest by id when one exists.
    #[must_use]
    pub fn candidate(&self, candidate_id: &str) -> Option<&OptimizationCandidateManifest> {
        self.candidates.get(candidate_id)
    }

    /// Returns the stable digest over the lineage state payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.state_digest.clear();
        stable_digest(LINEAGE_STATE_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.state_digest = self.stable_digest();
        self
    }

    /// Writes the lineage state as pretty JSON.
    pub fn write_json(&self, output_path: impl AsRef<Path>) -> Result<(), OptimizationIoError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| OptimizationIoError::CreateDir {
                path: parent.display().to_string(),
                error,
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| OptimizationIoError::Write {
            path: output_path.display().to_string(),
            error,
        })
    }

    /// Loads the lineage state from one JSON artifact.
    pub fn read_json(input_path: impl AsRef<Path>) -> Result<Self, OptimizationIoError> {
        let input_path = input_path.as_ref();
        let body = fs::read_to_string(input_path).map_err(|error| OptimizationIoError::Read {
            path: input_path.display().to_string(),
            error,
        })?;
        let state: Self =
            serde_json::from_str(&body).map_err(|error| OptimizationIoError::Deserialize {
                path: input_path.display().to_string(),
                error,
            })?;
        Ok(state)
    }
}

/// Top-level run receipt that points at retained candidates and frontier refs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationRunReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Candidate family under optimization.
    pub family_id: String,
    /// Bound run spec digest.
    pub run_spec_digest: String,
    /// Final lineage state digest.
    pub lineage_state_digest: String,
    /// Total materialized candidate count.
    pub candidate_count: u32,
    /// Retained candidate ids in deterministic order.
    pub retained_candidate_ids: Vec<String>,
    /// Frontier snapshot refs retained for this run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontier_snapshot_refs: Vec<String>,
    /// Explicit stop reason for the run.
    pub stop_reason: OptimizationStopReason,
    /// Plain-language claim boundary for the receipt.
    pub claim_boundary: String,
    /// Stable digest over the receipt payload.
    pub receipt_digest: String,
}

impl OptimizationRunReceipt {
    /// Builds a run receipt from the final lineage state.
    #[must_use]
    pub fn from_state(
        state: &OptimizationLineageState,
        frontier_snapshot_refs: Vec<String>,
        stop_reason: OptimizationStopReason,
    ) -> Self {
        Self {
            schema_version: 1,
            report_id: String::from("psionic.optimize.run_receipt.v1"),
            run_id: state.run_spec.run_id.clone(),
            family_id: state.run_spec.family_id.clone(),
            run_spec_digest: state.run_spec.spec_digest.clone(),
            lineage_state_digest: state.state_digest.clone(),
            candidate_count: state.candidates.len() as u32,
            retained_candidate_ids: state.retained_candidate_ids.clone(),
            frontier_snapshot_refs,
            stop_reason,
            claim_boundary: String::from(
                "This receipt records optimizer run identity, retained candidates, and frontier refs for one bounded offline optimization run. It does not claim evaluation quality, search validity, or runtime promotion by itself.",
            ),
            receipt_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns the stable digest over the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(RUN_RECEIPT_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.receipt_digest = self.stable_digest();
        self
    }
}

/// Artifact kind published by the optimizer substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationArtifactKind {
    /// Published run-spec artifact.
    RunSpec,
    /// Published lineage-state artifact.
    LineageState,
    /// Published persisted search-state artifact.
    SearchState,
    /// Published top-level run receipt.
    RunReceipt,
    /// Published frontier snapshot.
    FrontierSnapshot,
    /// Published case manifest.
    CaseManifest,
    /// Published batch evaluation receipt.
    BatchEvaluationReceipt,
    /// Published retained eval bundle.
    RetainedEvalRunBundle,
    /// Published candidate comparison manifest.
    CandidateComparisonManifest,
}

/// Typed reference to one published optimizer artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationArtifactRef {
    /// Artifact kind.
    pub artifact_kind: OptimizationArtifactKind,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl OptimizationArtifactRef {
    /// Creates one typed artifact reference.
    #[must_use]
    pub fn new(
        artifact_kind: OptimizationArtifactKind,
        artifact_id: impl Into<String>,
        artifact_digest: impl Into<String>,
    ) -> Self {
        Self {
            artifact_kind,
            artifact_id: artifact_id.into(),
            artifact_digest: artifact_digest.into(),
        }
    }
}

/// Published summary for one retained validation case inside an eval bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationRetainedEvalCaseSummary {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable case digest.
    pub case_digest: String,
    /// Split membership for the retained case.
    pub split: OptimizationCaseSplit,
    /// Optional retained label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Ordered evidence refs carried by the case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Published summary for one candidate validation batch inside an eval bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationPublishedCandidateEvaluation {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Stable candidate manifest digest.
    pub candidate_manifest_digest: String,
    /// Typed validation-batch reference.
    pub validation_batch_ref: OptimizationArtifactRef,
    /// Aggregated scalar score across retained validation cases.
    pub aggregated_scalar_score: i64,
    /// Aggregated objective totals across retained validation cases.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aggregated_objective_scores: BTreeMap<String, i64>,
}

/// Published retained eval bundle for one bounded optimizer run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationRetainedEvalRunBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Candidate family under optimization.
    pub family_id: String,
    /// Optional issue or queue ref bound to the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue_ref: Option<String>,
    /// Frontier mode used by the bound run.
    pub frontier_mode: OptimizationFrontierMode,
    /// Ordered supporting artifact refs for the run.
    pub source_artifacts: Vec<OptimizationArtifactRef>,
    /// Ordered retained validation-case summaries.
    pub validation_cases: Vec<OptimizationRetainedEvalCaseSummary>,
    /// Ordered published candidate validation summaries.
    pub candidate_evaluations: Vec<OptimizationPublishedCandidateEvaluation>,
    /// Retained candidate ids for the run receipt.
    pub retained_candidate_ids: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the bundle payload.
    pub bundle_digest: String,
}

impl OptimizationRetainedEvalRunBundle {
    /// Publishes one retained eval bundle from a completed optimizer outcome.
    #[must_use]
    pub fn from_run_outcome(outcome: &OptimizationEngineRunOutcome) -> Self {
        let run_id = outcome.state.run_spec.run_id.clone();
        let bundle_id = format!("{run_id}:retained_eval_bundle");
        let mut source_artifacts = vec![
            OptimizationArtifactRef::new(
                OptimizationArtifactKind::RunSpec,
                format!("{run_id}:run_spec"),
                outcome.state.run_spec.spec_digest.clone(),
            ),
            OptimizationArtifactRef::new(
                OptimizationArtifactKind::LineageState,
                format!("{run_id}:lineage_state"),
                outcome.state.lineage_state.state_digest.clone(),
            ),
            OptimizationArtifactRef::new(
                OptimizationArtifactKind::SearchState,
                format!("{run_id}:search_state"),
                outcome.state.state_digest.clone(),
            ),
            OptimizationArtifactRef::new(
                OptimizationArtifactKind::RunReceipt,
                format!("{run_id}:run_receipt"),
                outcome.run_receipt.receipt_digest.clone(),
            ),
        ];
        if let Some(frontier_snapshot) = outcome.state.latest_frontier_snapshot.as_ref() {
            source_artifacts.push(OptimizationArtifactRef::new(
                OptimizationArtifactKind::FrontierSnapshot,
                format!("{run_id}:frontier_snapshot"),
                frontier_snapshot.snapshot_digest.clone(),
            ));
        }

        let validation_cases = outcome
            .state
            .validation_cases
            .iter()
            .map(|case| {
                source_artifacts.push(OptimizationArtifactRef::new(
                    OptimizationArtifactKind::CaseManifest,
                    format!("{run_id}:case:{}", case.case_id),
                    case.case_digest.clone(),
                ));
                OptimizationRetainedEvalCaseSummary {
                    case_id: case.case_id.clone(),
                    case_digest: case.case_digest.clone(),
                    split: case.split,
                    label: case.label.clone(),
                    evidence_refs: case.evidence_refs.clone(),
                }
            })
            .collect::<Vec<_>>();

        let candidate_evaluations = outcome
            .state
            .accepted_validation_batches
            .values()
            .map(|batch| {
                let validation_batch_ref = OptimizationArtifactRef::new(
                    OptimizationArtifactKind::BatchEvaluationReceipt,
                    format!("{run_id}:validation_batch:{}", batch.candidate_id),
                    batch.receipt_digest.clone(),
                );
                source_artifacts.push(validation_batch_ref.clone());
                OptimizationPublishedCandidateEvaluation {
                    candidate_id: batch.candidate_id.clone(),
                    candidate_manifest_digest: batch.candidate_manifest_digest.clone(),
                    validation_batch_ref,
                    aggregated_scalar_score: batch.aggregated_scalar_score,
                    aggregated_objective_scores: batch.aggregated_objective_scores.clone(),
                }
            })
            .collect::<Vec<_>>();

        Self {
            schema_version: 1,
            report_id: String::from("psionic.optimize.retained_eval_run_bundle.v1"),
            bundle_id,
            run_id,
            family_id: outcome.state.run_spec.family_id.clone(),
            issue_ref: outcome.state.run_spec.issue_ref.clone(),
            frontier_mode: outcome.state.run_spec.frontier_mode,
            source_artifacts,
            validation_cases,
            candidate_evaluations,
            retained_candidate_ids: outcome.run_receipt.retained_candidate_ids.clone(),
            claim_boundary: String::from(
                "This bundle publishes the retained validation cases and accepted validation receipts Psionic already materialized for one bounded optimizer run. It does not claim campaign admission, product promotion, or payout readiness by itself.",
            ),
            bundle_digest: String::new(),
        }
        .with_stable_digest()
    }

    /// Returns the stable digest over the retained eval bundle payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.bundle_digest.clear();
        stable_digest(RETAINED_EVAL_RUN_BUNDLE_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.bundle_digest = self.stable_digest();
        self
    }

    /// Writes the retained eval bundle as pretty JSON.
    pub fn write_json(&self, output_path: impl AsRef<Path>) -> Result<(), OptimizationIoError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| OptimizationIoError::CreateDir {
                path: parent.display().to_string(),
                error,
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| OptimizationIoError::Write {
            path: output_path.display().to_string(),
            error,
        })
    }

    /// Loads the retained eval bundle from one JSON artifact.
    pub fn read_json(input_path: impl AsRef<Path>) -> Result<Self, OptimizationIoError> {
        let input_path = input_path.as_ref();
        let body = fs::read_to_string(input_path).map_err(|error| OptimizationIoError::Read {
            path: input_path.display().to_string(),
            error,
        })?;
        let bundle: Self =
            serde_json::from_str(&body).map_err(|error| OptimizationIoError::Deserialize {
                path: input_path.display().to_string(),
                error,
            })?;
        Ok(bundle)
    }
}

/// Top-level comparison outcome between two retained candidate evaluations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationCandidateComparisonOutcome {
    /// The challenger scored higher than the baseline.
    ChallengerAhead,
    /// The baseline scored higher than the challenger.
    BaselineAhead,
    /// Both candidates tied on aggregate scalar score.
    Tied,
}

/// Published candidate comparison summary suitable for campaign gating.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationCandidateComparisonManifest {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable comparison identifier.
    pub comparison_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Candidate family under comparison.
    pub family_id: String,
    /// Retained eval bundle that owns the published evidence.
    pub retained_eval_bundle_ref: OptimizationArtifactRef,
    /// Baseline candidate identifier.
    pub baseline_candidate_id: String,
    /// Baseline candidate manifest digest.
    pub baseline_candidate_manifest_digest: String,
    /// Baseline validation-batch reference.
    pub baseline_validation_batch_ref: OptimizationArtifactRef,
    /// Challenger candidate identifier.
    pub challenger_candidate_id: String,
    /// Challenger candidate manifest digest.
    pub challenger_candidate_manifest_digest: String,
    /// Challenger validation-batch reference.
    pub challenger_validation_batch_ref: OptimizationArtifactRef,
    /// Aggregate comparison outcome.
    pub outcome: OptimizationCandidateComparisonOutcome,
    /// Baseline aggregate scalar score.
    pub baseline_scalar_score: i64,
    /// Challenger aggregate scalar score.
    pub challenger_scalar_score: i64,
    /// Challenger minus baseline aggregate scalar delta.
    pub scalar_score_delta: i64,
    /// Challenger minus baseline objective deltas.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub objective_score_deltas: BTreeMap<String, i64>,
    /// Validation cases where the challenger improved over the baseline.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub improved_case_ids: Vec<String>,
    /// Validation cases where the challenger regressed versus the baseline.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regressed_case_ids: Vec<String>,
    /// Validation cases where both candidates tied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tied_case_ids: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the comparison payload.
    pub comparison_digest: String,
}

impl OptimizationCandidateComparisonManifest {
    /// Publishes one candidate comparison from a completed optimizer outcome.
    pub fn from_run_outcome(
        outcome: &OptimizationEngineRunOutcome,
        baseline_candidate_id: impl AsRef<str>,
        challenger_candidate_id: impl AsRef<str>,
    ) -> Result<Self, OptimizationPublicationError> {
        let baseline_candidate_id = baseline_candidate_id.as_ref();
        let challenger_candidate_id = challenger_candidate_id.as_ref();
        let run_id = outcome.state.run_spec.run_id.clone();
        let baseline_batch = outcome
            .state
            .accepted_validation_batches
            .get(baseline_candidate_id)
            .ok_or_else(|| OptimizationPublicationError::MissingValidationBatch {
                run_id: run_id.clone(),
                candidate_id: baseline_candidate_id.to_string(),
            })?;
        let challenger_batch = outcome
            .state
            .accepted_validation_batches
            .get(challenger_candidate_id)
            .ok_or_else(|| OptimizationPublicationError::MissingValidationBatch {
                run_id: run_id.clone(),
                candidate_id: challenger_candidate_id.to_string(),
            })?;

        let baseline_cases = baseline_batch
            .case_receipts
            .iter()
            .map(|receipt| (receipt.case_id.clone(), receipt))
            .collect::<BTreeMap<_, _>>();
        let challenger_cases = challenger_batch
            .case_receipts
            .iter()
            .map(|receipt| (receipt.case_id.clone(), receipt))
            .collect::<BTreeMap<_, _>>();
        if baseline_cases.keys().collect::<Vec<_>>() != challenger_cases.keys().collect::<Vec<_>>()
        {
            return Err(OptimizationPublicationError::ValidationCaseSetMismatch {
                baseline_candidate_id: baseline_candidate_id.to_string(),
                challenger_candidate_id: challenger_candidate_id.to_string(),
            });
        }

        let mut improved_case_ids = Vec::new();
        let mut regressed_case_ids = Vec::new();
        let mut tied_case_ids = Vec::new();
        for case in &outcome.state.validation_cases {
            let baseline_case = baseline_cases.get(&case.case_id).ok_or_else(|| {
                OptimizationPublicationError::ValidationCaseSetMismatch {
                    baseline_candidate_id: baseline_candidate_id.to_string(),
                    challenger_candidate_id: challenger_candidate_id.to_string(),
                }
            })?;
            let challenger_case = challenger_cases.get(&case.case_id).ok_or_else(|| {
                OptimizationPublicationError::ValidationCaseSetMismatch {
                    baseline_candidate_id: baseline_candidate_id.to_string(),
                    challenger_candidate_id: challenger_candidate_id.to_string(),
                }
            })?;
            if challenger_case.scalar_score > baseline_case.scalar_score {
                improved_case_ids.push(case.case_id.clone());
            } else if challenger_case.scalar_score < baseline_case.scalar_score {
                regressed_case_ids.push(case.case_id.clone());
            } else {
                tied_case_ids.push(case.case_id.clone());
            }
        }

        let objective_names = baseline_batch
            .aggregated_objective_scores
            .keys()
            .chain(challenger_batch.aggregated_objective_scores.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let objective_score_deltas = objective_names
            .into_iter()
            .map(|objective_name| {
                let baseline = baseline_batch
                    .aggregated_objective_scores
                    .get(&objective_name)
                    .copied()
                    .unwrap_or_default();
                let challenger = challenger_batch
                    .aggregated_objective_scores
                    .get(&objective_name)
                    .copied()
                    .unwrap_or_default();
                (objective_name, challenger - baseline)
            })
            .collect::<BTreeMap<_, _>>();
        let scalar_score_delta =
            challenger_batch.aggregated_scalar_score - baseline_batch.aggregated_scalar_score;
        let outcome_kind = if scalar_score_delta > 0 {
            OptimizationCandidateComparisonOutcome::ChallengerAhead
        } else if scalar_score_delta < 0 {
            OptimizationCandidateComparisonOutcome::BaselineAhead
        } else {
            OptimizationCandidateComparisonOutcome::Tied
        };
        let bundle = OptimizationRetainedEvalRunBundle::from_run_outcome(outcome);

        Ok(Self {
            schema_version: 1,
            report_id: String::from("psionic.optimize.candidate_comparison_manifest.v1"),
            comparison_id: format!(
                "{run_id}:candidate_comparison:{baseline_candidate_id}:{challenger_candidate_id}"
            ),
            run_id: run_id.clone(),
            family_id: outcome.state.run_spec.family_id.clone(),
            retained_eval_bundle_ref: OptimizationArtifactRef::new(
                OptimizationArtifactKind::RetainedEvalRunBundle,
                bundle.bundle_id.clone(),
                bundle.bundle_digest.clone(),
            ),
            baseline_candidate_id: baseline_candidate_id.to_string(),
            baseline_candidate_manifest_digest: baseline_batch.candidate_manifest_digest.clone(),
            baseline_validation_batch_ref: OptimizationArtifactRef::new(
                OptimizationArtifactKind::BatchEvaluationReceipt,
                format!("{run_id}:validation_batch:{baseline_candidate_id}"),
                baseline_batch.receipt_digest.clone(),
            ),
            challenger_candidate_id: challenger_candidate_id.to_string(),
            challenger_candidate_manifest_digest: challenger_batch
                .candidate_manifest_digest
                .clone(),
            challenger_validation_batch_ref: OptimizationArtifactRef::new(
                OptimizationArtifactKind::BatchEvaluationReceipt,
                format!("{run_id}:validation_batch:{challenger_candidate_id}"),
                challenger_batch.receipt_digest.clone(),
            ),
            outcome: outcome_kind,
            baseline_scalar_score: baseline_batch.aggregated_scalar_score,
            challenger_scalar_score: challenger_batch.aggregated_scalar_score,
            scalar_score_delta,
            objective_score_deltas,
            improved_case_ids,
            regressed_case_ids,
            tied_case_ids,
            claim_boundary: String::from(
                "This comparison reports aggregate and per-case deltas between two accepted validation batches already materialized by Psionic. It does not claim product admission, live traffic promotion, or rollback authority by itself.",
            ),
            comparison_digest: String::new(),
        }
        .with_stable_digest())
    }

    /// Returns the stable digest over the comparison payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.comparison_digest.clear();
        stable_digest(CANDIDATE_COMPARISON_MANIFEST_PREFIX, &digestible)
    }

    /// Populates the stable digest field.
    #[must_use]
    pub fn with_stable_digest(mut self) -> Self {
        self.comparison_digest = self.stable_digest();
        self
    }

    /// Writes the comparison manifest as pretty JSON.
    pub fn write_json(&self, output_path: impl AsRef<Path>) -> Result<(), OptimizationIoError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| OptimizationIoError::CreateDir {
                path: parent.display().to_string(),
                error,
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| OptimizationIoError::Write {
            path: output_path.display().to_string(),
            error,
        })
    }

    /// Loads the comparison manifest from one JSON artifact.
    pub fn read_json(input_path: impl AsRef<Path>) -> Result<Self, OptimizationIoError> {
        let input_path = input_path.as_ref();
        let body = fs::read_to_string(input_path).map_err(|error| OptimizationIoError::Read {
            path: input_path.display().to_string(),
            error,
        })?;
        let manifest: Self =
            serde_json::from_str(&body).map_err(|error| OptimizationIoError::Deserialize {
                path: input_path.display().to_string(),
                error,
            })?;
        Ok(manifest)
    }
}

/// Failure while publishing optimizer-facing eval artifacts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OptimizationPublicationError {
    /// Accepted validation batch missing for one candidate.
    #[error(
        "run `{run_id}` is missing an accepted validation batch for candidate `{candidate_id}`"
    )]
    MissingValidationBatch {
        /// Bound run id.
        run_id: String,
        /// Missing candidate id.
        candidate_id: String,
    },
    /// The compared candidates did not cover the same retained validation cases.
    #[error(
        "comparison candidates `{baseline_candidate_id}` and `{challenger_candidate_id}` do not cover the same retained validation cases"
    )]
    ValidationCaseSetMismatch {
        /// Baseline candidate identifier.
        baseline_candidate_id: String,
        /// Challenger candidate identifier.
        challenger_candidate_id: String,
    },
}

/// Failure while mutating lineage state.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OptimizationLineageStateError {
    /// Candidate family did not match the bound run spec.
    #[error(
        "candidate family `{actual_family_id}` did not match run family `{expected_family_id}`"
    )]
    FamilyMismatch {
        /// Expected family id from the bound run spec.
        expected_family_id: String,
        /// Actual family id from the candidate manifest.
        actual_family_id: String,
    },
    /// Candidate originating run did not match the bound run spec.
    #[error("candidate run `{actual_run_id}` did not match run `{expected_run_id}`")]
    RunMismatch {
        /// Expected run id from the bound run spec.
        expected_run_id: String,
        /// Actual originating run id from the candidate manifest.
        actual_run_id: String,
    },
    /// Candidate id was already registered.
    #[error("candidate `{candidate_id}` is already registered in this run state")]
    DuplicateCandidate {
        /// Duplicate candidate id.
        candidate_id: String,
    },
    /// Parent candidate id was not present in the state.
    #[error("candidate `{candidate_id}` references unknown parent `{parent_candidate_id}`")]
    UnknownParent {
        /// Candidate that declared the unknown parent.
        candidate_id: String,
        /// Unknown parent id.
        parent_candidate_id: String,
    },
    /// Retained candidate id was not present in the state.
    #[error("retained candidate `{candidate_id}` is not registered in this run state")]
    UnknownRetainedCandidate {
        /// Unknown retained candidate id.
        candidate_id: String,
    },
}

/// Failure while persisting or loading optimizer JSON artifacts.
#[derive(Debug, Error)]
pub enum OptimizationIoError {
    /// Failed to create a parent directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Parent path.
        path: String,
        /// Underlying error.
        error: std::io::Error,
    },
    /// Failed to write one artifact.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// Output path.
        path: String,
        /// Underlying error.
        error: std::io::Error,
    },
    /// Failed to read one artifact.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// Input path.
        path: String,
        /// Underlying error.
        error: std::io::Error,
    },
    /// Failed to decode one artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        /// Input path.
        path: String,
        /// Underlying error.
        error: serde_json::Error,
    },
    /// Failed to encode JSON.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn push_unique(target: &mut Vec<String>, value: String) {
    if !target.iter().any(|existing| existing == &value) {
        target.push(value);
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::{
        OptimizationArtifactKind, OptimizationBatchEvaluationReceipt,
        OptimizationCandidateComparisonManifest, OptimizationCandidateComparisonOutcome,
        OptimizationCandidateManifest, OptimizationCandidateProposal,
        OptimizationCandidateProposer, OptimizationCaseEvaluationReceipt,
        OptimizationCaseLabelMutationStrategy, OptimizationCaseManifest, OptimizationCaseSplit,
        OptimizationComponentFeedback, OptimizationDefaultReflectionPromptBuilder,
        OptimizationEngine, OptimizationEngineRunOutcome, OptimizationEvaluationCache,
        OptimizationEvaluator, OptimizationFeedbackComponentSelector, OptimizationFrontierMode,
        OptimizationFrontierSnapshot, OptimizationLineageAwareMergeProposer,
        OptimizationLineageState, OptimizationLineageStateError, OptimizationProposerReceipt,
        OptimizationPublicationError, OptimizationReflectiveMutationProposer,
        OptimizationRetainedEvalRunBundle, OptimizationRunReceipt, OptimizationRunSpec,
        OptimizationSearchState, OptimizationSequentialMinibatchSampler,
        OptimizationSharedFeedback, OptimizationStopReason,
        OptimizationTypedFeedbackDatasetBuilder,
    };

    fn candidate_components(selected_tool: &str, reason: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            (String::from("selected_tool"), String::from(selected_tool)),
            (String::from("reason"), String::from(reason)),
        ])
    }

    fn route_components(route_name: &str) -> BTreeMap<String, String> {
        candidate_components(route_name, "route decision evidence goes here")
    }

    fn route_case(case_id: &str, label: &str) -> OptimizationCaseManifest {
        OptimizationCaseManifest::new(case_id, OptimizationCaseSplit::Validation)
            .with_label(label)
            .with_metadata(BTreeMap::from([(
                String::from("decision_family"),
                String::from("tool_route"),
            )]))
    }

    fn train_route_case(case_id: &str, label: &str) -> OptimizationCaseManifest {
        OptimizationCaseManifest::new(case_id, OptimizationCaseSplit::Train)
            .with_label(label)
            .with_metadata(BTreeMap::from([(
                String::from("decision_family"),
                String::from("tool_route"),
            )]))
    }

    fn route_candidate(
        candidate_id: &str,
        route_name: &str,
        parent_candidate_ids: Vec<String>,
    ) -> OptimizationCandidateManifest {
        OptimizationCandidateManifest::new(
            candidate_id,
            "probe.tool_route",
            "run_a",
            route_components(route_name),
        )
        .with_parent_candidate_ids(parent_candidate_ids)
    }

    fn policy_candidate(
        candidate_id: &str,
        selected_tool: &str,
        reason: &str,
        parent_candidate_ids: Vec<String>,
    ) -> OptimizationCandidateManifest {
        OptimizationCandidateManifest::new(
            candidate_id,
            "probe.tool_route",
            "run_a",
            candidate_components(selected_tool, reason),
        )
        .with_parent_candidate_ids(parent_candidate_ids)
    }

    fn publication_outcome() -> OptimizationEngineRunOutcome {
        let run_spec = OptimizationRunSpec::new("run_publication", "probe.tool_route")
            .with_issue_ref("OpenAgentsInc/psionic#820")
            .with_frontier_mode(OptimizationFrontierMode::Scalar)
            .with_iteration_budget(4)
            .with_candidate_budget(4);
        let baseline = OptimizationCandidateManifest::new(
            "baseline",
            "probe.tool_route",
            "run_publication",
            route_components("read_file"),
        );
        let train_cases = vec![train_route_case("train_apply_patch", "apply_patch")];
        let validation_cases = vec![
            route_case("case_improved", "apply_patch"),
            route_case("case_regressed", "read_file"),
            route_case("case_tied", "shell"),
        ];
        let mut evaluator = DeterministicRouteEvaluator;
        let mut state = OptimizationEngine::initialize(
            run_spec,
            baseline.clone(),
            train_cases,
            validation_cases.clone(),
            &mut evaluator,
        )
        .expect("initialize publication state");
        let challenger = OptimizationCandidateManifest::new(
            "challenger",
            "probe.tool_route",
            "run_publication",
            route_components("apply_patch"),
        )
        .with_parent_candidate_ids(vec![String::from("baseline")]);
        state
            .lineage_state
            .register_candidate(challenger.clone())
            .expect("register challenger");
        let challenger_batch = evaluator.evaluate_candidate(
            "run_publication",
            &challenger,
            validation_cases.as_slice(),
            &mut state.evaluation_cache,
        );
        state
            .accepted_validation_batches
            .insert(challenger.candidate_id.clone(), challenger_batch);
        let frontier_batches = state
            .accepted_validation_batches
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let frontier = OptimizationFrontierSnapshot::from_batches(
            "run_publication",
            OptimizationFrontierMode::Scalar,
            frontier_batches.as_slice(),
        );
        state
            .lineage_state
            .set_retained_candidates(frontier.hybrid_candidate_ids.clone())
            .expect("set retained frontier");
        state.latest_frontier_snapshot = Some(frontier.clone());
        state.state_digest = state.stable_digest();

        OptimizationEngineRunOutcome {
            run_receipt: OptimizationRunReceipt::from_state(
                &state.lineage_state,
                vec![format!(
                    "frontier_snapshot_digest:{}",
                    frontier.snapshot_digest
                )],
                OptimizationStopReason::Manual,
            ),
            state,
        }
    }

    #[derive(Debug, Default)]
    struct DeterministicRouteEvaluator;

    impl OptimizationEvaluator for DeterministicRouteEvaluator {
        fn evaluate_candidate(
            &mut self,
            run_id: &str,
            candidate: &OptimizationCandidateManifest,
            cases: &[OptimizationCaseManifest],
            cache: &mut OptimizationEvaluationCache,
        ) -> OptimizationBatchEvaluationReceipt {
            let selected_tool = candidate
                .components
                .get("selected_tool")
                .cloned()
                .expect("selected_tool component");
            let mut case_receipts = Vec::new();
            let mut cache_hit_count = 0_u32;
            let mut cache_miss_count = 0_u32;

            for case in cases {
                if let Some(cached) = cache.lookup(candidate, case).cloned() {
                    cache_hit_count += 1;
                    case_receipts.push(cached);
                    continue;
                }

                cache_miss_count += 1;
                let expected_route = case.label.clone().expect("case label");
                let matched = selected_tool == expected_route;
                let scalar_score = if matched { 10_000 } else { 0 };
                let receipt = OptimizationCaseEvaluationReceipt::new(
                    candidate,
                    case,
                    scalar_score,
                    BTreeMap::from([(String::from("correctness_bps"), scalar_score)]),
                    if matched {
                        OptimizationSharedFeedback::new("candidate matched expected route")
                    } else {
                        OptimizationSharedFeedback::new("candidate missed expected route")
                    }
                    .with_details(vec![format!(
                        "expected `{expected_route}` and observed `{selected_tool}`"
                    )]),
                    BTreeMap::from([(
                        String::from("selected_tool"),
                        if matched {
                            OptimizationComponentFeedback::new(
                                "selected tool matched retained route label",
                            )
                        } else {
                            OptimizationComponentFeedback::new(
                                "selected tool diverged from retained route label",
                            )
                        },
                    )]),
                );
                cache.insert(candidate, case, receipt.clone());
                case_receipts.push(receipt);
            }

            OptimizationBatchEvaluationReceipt::new(
                run_id,
                candidate,
                case_receipts,
                cache_hit_count,
                cache_miss_count,
            )
        }
    }

    fn component_case(
        case_id: &str,
        split: OptimizationCaseSplit,
        active_component: &str,
        expected_tool: &str,
        expected_reason: &str,
    ) -> OptimizationCaseManifest {
        OptimizationCaseManifest::new(case_id, split)
            .with_label(expected_tool)
            .with_metadata(BTreeMap::from([
                (String::from("decision_family"), String::from("tool_route")),
                (
                    String::from("active_component"),
                    String::from(active_component),
                ),
                (
                    String::from("expected_reason"),
                    String::from(expected_reason),
                ),
            ]))
    }

    #[derive(Debug, Default)]
    struct DeterministicMergeEvaluator;

    impl OptimizationEvaluator for DeterministicMergeEvaluator {
        fn evaluate_candidate(
            &mut self,
            run_id: &str,
            candidate: &OptimizationCandidateManifest,
            cases: &[OptimizationCaseManifest],
            cache: &mut OptimizationEvaluationCache,
        ) -> OptimizationBatchEvaluationReceipt {
            let selected_tool = candidate
                .components
                .get("selected_tool")
                .cloned()
                .expect("selected_tool component");
            let reason = candidate
                .components
                .get("reason")
                .cloned()
                .expect("reason component");
            let mut case_receipts = Vec::new();
            let mut cache_hit_count = 0_u32;
            let mut cache_miss_count = 0_u32;

            for case in cases {
                if let Some(cached) = cache.lookup(candidate, case).cloned() {
                    cache_hit_count += 1;
                    case_receipts.push(cached);
                    continue;
                }

                cache_miss_count += 1;
                let expected_tool = case.label.clone().expect("case label");
                let expected_reason = case
                    .metadata
                    .get("expected_reason")
                    .cloned()
                    .expect("expected_reason metadata");
                let active_component = case
                    .metadata
                    .get("active_component")
                    .cloned()
                    .expect("active_component metadata");
                let tool_score =
                    if active_component == "selected_tool" && selected_tool == expected_tool {
                        10_000
                    } else {
                        0
                    };
                let reason_score = if active_component == "reason" && reason == expected_reason {
                    10_000
                } else {
                    0
                };
                let scalar_score = tool_score + reason_score;
                let receipt = OptimizationCaseEvaluationReceipt::new(
                    candidate,
                    case,
                    scalar_score,
                    BTreeMap::from([
                        (String::from("tool_bps"), tool_score),
                        (String::from("reason_bps"), reason_score),
                    ]),
                    OptimizationSharedFeedback::new("component-policy evaluation").with_details(
                        vec![format!(
                            "expected tool `{expected_tool}` and reason `{expected_reason}`"
                        )],
                    ),
                    BTreeMap::from([
                        (
                            String::from("selected_tool"),
                            if tool_score > 0 {
                                OptimizationComponentFeedback::new(
                                    "selected tool matched retained label",
                                )
                            } else {
                                OptimizationComponentFeedback::new(
                                    "selected tool missed retained label",
                                )
                            },
                        ),
                        (
                            String::from("reason"),
                            if reason_score > 0 {
                                OptimizationComponentFeedback::new(
                                    "reason matched retained policy label",
                                )
                            } else {
                                OptimizationComponentFeedback::new(
                                    "reason missed retained policy label",
                                )
                            },
                        ),
                    ]),
                );
                cache.insert(candidate, case, receipt.clone());
                case_receipts.push(receipt);
            }

            OptimizationBatchEvaluationReceipt::new(
                run_id,
                candidate,
                case_receipts,
                cache_hit_count,
                cache_miss_count,
            )
        }
    }

    #[derive(Debug)]
    struct SequenceProposer {
        queued_candidates: Vec<OptimizationCandidateManifest>,
    }

    impl SequenceProposer {
        fn new(queued_candidates: Vec<OptimizationCandidateManifest>) -> Self {
            Self { queued_candidates }
        }
    }

    impl OptimizationCandidateProposer for SequenceProposer {
        fn propose_candidate(
            &mut self,
            state: &OptimizationSearchState,
            current_candidate: &OptimizationCandidateManifest,
            minibatch_receipt: &OptimizationBatchEvaluationReceipt,
        ) -> Option<OptimizationCandidateProposal> {
            let candidate = self.queued_candidates.first().cloned()?;
            self.queued_candidates.remove(0);
            let component_diffs = candidate
                .components
                .iter()
                .filter_map(|(component_id, proposed_value)| {
                    let previous_value = current_candidate.components.get(component_id)?;
                    if previous_value == proposed_value {
                        None
                    } else {
                        Some(super::OptimizationComponentDiff {
                            component_id: component_id.clone(),
                            previous_value: previous_value.clone(),
                            proposed_value: proposed_value.clone(),
                        })
                    }
                })
                .collect::<Vec<_>>();
            let proposer_receipt = OptimizationProposerReceipt {
                schema_version: 1,
                report_id: String::from("psionic.optimize.proposer_receipt.v1"),
                run_id: state.run_spec.run_id.clone(),
                proposer_kind: String::from("sequence_test_double"),
                parent_candidate_id: current_candidate.candidate_id.clone(),
                proposed_candidate_id: candidate.candidate_id.clone(),
                source_batch_receipt_digest: minibatch_receipt.receipt_digest.clone(),
                reflective_dataset_digest: None,
                selected_component_ids: candidate.components.keys().cloned().collect(),
                component_diffs,
                prompts: Vec::new(),
                metadata: BTreeMap::new(),
                receipt_digest: String::new(),
            }
            .with_stable_digest();
            Some(OptimizationCandidateProposal {
                candidate,
                proposer_receipt,
                gating_candidate_ids: Vec::new(),
                merge_context: None,
            })
        }
    }

    #[test]
    fn run_spec_stable_digest_changes_with_fields() {
        let baseline = OptimizationRunSpec::new("run_a", "probe.tool_route")
            .with_frontier_mode(OptimizationFrontierMode::Scalar);
        let widened = baseline
            .clone()
            .with_frontier_mode(OptimizationFrontierMode::Hybrid);
        assert_ne!(baseline.spec_digest, widened.spec_digest);
    }

    #[test]
    fn lineage_state_registers_roots_and_children() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route");
        let mut state = OptimizationLineageState::new(run_spec);
        let baseline = OptimizationCandidateManifest::new(
            "baseline",
            "probe.tool_route",
            "run_a",
            route_components("read_file"),
        );
        state
            .register_candidate(baseline)
            .expect("register baseline");

        let candidate = OptimizationCandidateManifest::new(
            "candidate",
            "probe.tool_route",
            "run_a",
            route_components("apply_patch"),
        )
        .with_parent_candidate_ids(vec![String::from("baseline")]);
        state
            .register_candidate(candidate)
            .expect("register candidate");

        assert_eq!(state.root_candidate_ids, vec![String::from("baseline")]);
        assert_eq!(
            state.discovery_order,
            vec![String::from("baseline"), String::from("candidate")]
        );
        assert_eq!(state.retained_candidate_ids.len(), 2);
    }

    #[test]
    fn lineage_state_rejects_unknown_parent() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route");
        let mut state = OptimizationLineageState::new(run_spec);
        let candidate = OptimizationCandidateManifest::new(
            "candidate",
            "probe.tool_route",
            "run_a",
            route_components("apply_patch"),
        )
        .with_parent_candidate_ids(vec![String::from("missing_parent")]);
        let error = state
            .register_candidate(candidate)
            .expect_err("unknown parent should fail");
        assert_eq!(
            error,
            OptimizationLineageStateError::UnknownParent {
                candidate_id: String::from("candidate"),
                parent_candidate_id: String::from("missing_parent"),
            }
        );
    }

    #[test]
    fn lineage_state_round_trips_through_json() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route")
            .with_issue_ref("OpenAgentsInc/psionic#807")
            .with_iteration_budget(12)
            .with_candidate_budget(24);
        let mut state = OptimizationLineageState::new(run_spec);
        state
            .register_candidate(OptimizationCandidateManifest::new(
                "baseline",
                "probe.tool_route",
                "run_a",
                route_components("read_file"),
            ))
            .expect("register baseline");
        state
            .set_retained_candidates(vec![String::from("baseline")])
            .expect("retained set");

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("optimizer_state.json");
        state.write_json(&path).expect("write state");
        let loaded = OptimizationLineageState::read_json(&path).expect("read state");
        assert_eq!(loaded, state);
    }

    #[test]
    fn run_receipt_points_at_retained_candidates() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route");
        let mut state = OptimizationLineageState::new(run_spec);
        state
            .register_candidate(OptimizationCandidateManifest::new(
                "baseline",
                "probe.tool_route",
                "run_a",
                route_components("read_file"),
            ))
            .expect("register baseline");
        state
            .set_retained_candidates(vec![String::from("baseline")])
            .expect("retained set");

        let receipt = OptimizationRunReceipt::from_state(
            &state,
            vec![String::from("fixtures/optimizer/frontier_snapshot.json")],
            OptimizationStopReason::NoSearchRequired,
        );
        assert_eq!(receipt.run_spec_digest, state.run_spec.spec_digest);
        assert_eq!(receipt.lineage_state_digest, state.state_digest);
        assert_eq!(
            receipt.retained_candidate_ids,
            vec![String::from("baseline")]
        );
        assert!(!receipt.receipt_digest.is_empty());
    }

    #[test]
    fn retained_eval_run_bundle_publishes_stable_refs_and_round_trips() {
        let outcome = publication_outcome();
        let bundle = OptimizationRetainedEvalRunBundle::from_run_outcome(&outcome);
        assert_eq!(bundle.run_id, String::from("run_publication"));
        assert_eq!(bundle.family_id, String::from("probe.tool_route"));
        assert_eq!(
            bundle.issue_ref,
            Some(String::from("OpenAgentsInc/psionic#820"))
        );
        assert_eq!(
            bundle
                .validation_cases
                .iter()
                .map(|case| case.case_id.clone())
                .collect::<Vec<_>>(),
            vec![
                String::from("case_improved"),
                String::from("case_regressed"),
                String::from("case_tied"),
            ]
        );
        assert_eq!(
            bundle
                .candidate_evaluations
                .iter()
                .map(|candidate| candidate.candidate_id.clone())
                .collect::<Vec<_>>(),
            vec![String::from("baseline"), String::from("challenger")]
        );
        assert!(
            bundle
                .source_artifacts
                .iter()
                .any(|artifact| artifact.artifact_kind == OptimizationArtifactKind::SearchState)
        );
        assert_eq!(
            bundle.retained_candidate_ids,
            outcome.run_receipt.retained_candidate_ids
        );
        assert!(!bundle.bundle_digest.is_empty());

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("retained_eval_bundle.json");
        bundle.write_json(&path).expect("write bundle");
        let loaded =
            OptimizationRetainedEvalRunBundle::read_json(&path).expect("read retained bundle");
        assert_eq!(loaded, bundle);
    }

    #[test]
    fn candidate_comparison_manifest_tracks_case_deltas_and_round_trips() {
        let outcome = publication_outcome();
        let comparison = OptimizationCandidateComparisonManifest::from_run_outcome(
            &outcome,
            "baseline",
            "challenger",
        )
        .expect("publish comparison");
        assert_eq!(comparison.run_id, String::from("run_publication"));
        assert_eq!(
            comparison.outcome,
            OptimizationCandidateComparisonOutcome::Tied
        );
        assert_eq!(comparison.scalar_score_delta, 0);
        assert_eq!(
            comparison.objective_score_deltas.get("correctness_bps"),
            Some(&0)
        );
        assert_eq!(
            comparison.improved_case_ids,
            vec![String::from("case_improved")]
        );
        assert_eq!(
            comparison.regressed_case_ids,
            vec![String::from("case_regressed")]
        );
        assert_eq!(comparison.tied_case_ids, vec![String::from("case_tied")]);
        assert_eq!(
            comparison.retained_eval_bundle_ref.artifact_kind,
            OptimizationArtifactKind::RetainedEvalRunBundle
        );
        assert!(!comparison.comparison_digest.is_empty());

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("candidate_comparison.json");
        comparison.write_json(&path).expect("write comparison");
        let loaded =
            OptimizationCandidateComparisonManifest::read_json(&path).expect("read comparison");
        assert_eq!(loaded, comparison);
    }

    #[test]
    fn candidate_comparison_manifest_requires_existing_validation_batches() {
        let outcome = publication_outcome();
        let error = OptimizationCandidateComparisonManifest::from_run_outcome(
            &outcome,
            "missing",
            "challenger",
        )
        .expect_err("missing baseline should fail");
        assert_eq!(
            error,
            OptimizationPublicationError::MissingValidationBatch {
                run_id: String::from("run_publication"),
                candidate_id: String::from("missing"),
            }
        );
    }

    #[test]
    fn evaluation_cache_round_trips_case_receipts_by_unified_key() {
        let candidate = OptimizationCandidateManifest::new(
            "baseline",
            "probe.tool_route",
            "run_a",
            route_components("read_file"),
        );
        let case = route_case("case_a", "read_file");
        let receipt = super::OptimizationCaseEvaluationReceipt::new(
            &candidate,
            &case,
            8200,
            BTreeMap::from([(String::from("correctness_bps"), 8200)]),
            OptimizationSharedFeedback::new("baseline route was correct")
                .with_details(vec![String::from("selected_tool matched retained label")]),
            BTreeMap::from([(
                String::from("selected_tool"),
                OptimizationComponentFeedback::new("route component matched retained label"),
            )]),
        );
        let mut cache = OptimizationEvaluationCache::default();
        cache.insert(&candidate, &case, receipt.clone());

        let cached = cache.lookup(&candidate, &case).expect("cache hit");
        assert_eq!(
            cached.cache_key,
            OptimizationEvaluationCache::cache_key(&candidate, &case)
        );
        assert_eq!(cached, &receipt);
    }

    #[test]
    fn batch_receipt_aggregates_scalar_and_objective_scores() {
        let candidate = OptimizationCandidateManifest::new(
            "baseline",
            "probe.tool_route",
            "run_a",
            route_components("read_file"),
        );
        let case_a = route_case("case_a", "read_file");
        let case_b = route_case("case_b", "apply_patch");
        let receipt_a = super::OptimizationCaseEvaluationReceipt::new(
            &candidate,
            &case_a,
            5000,
            BTreeMap::from([(String::from("correctness_bps"), 5000)]),
            OptimizationSharedFeedback::new("first case"),
            BTreeMap::new(),
        );
        let receipt_b = super::OptimizationCaseEvaluationReceipt::new(
            &candidate,
            &case_b,
            4200,
            BTreeMap::from([
                (String::from("correctness_bps"), 4200),
                (String::from("latency_budget_bps"), 9700),
            ]),
            OptimizationSharedFeedback::new("second case"),
            BTreeMap::new(),
        );
        let batch = OptimizationBatchEvaluationReceipt::new(
            "run_a",
            &candidate,
            vec![receipt_a, receipt_b],
            1,
            1,
        );
        assert_eq!(batch.aggregated_scalar_score, 9200);
        assert_eq!(
            batch.aggregated_objective_scores.get("correctness_bps"),
            Some(&9200)
        );
        assert_eq!(
            batch.aggregated_objective_scores.get("latency_budget_bps"),
            Some(&9700)
        );
        assert_eq!(batch.cache_hit_count, 1);
        assert_eq!(batch.cache_miss_count, 1);
        assert!(!batch.receipt_digest.is_empty());
    }

    #[test]
    fn frontier_snapshot_keeps_case_and_objective_winners_explicit() {
        let baseline = OptimizationCandidateManifest::new(
            "baseline",
            "probe.tool_route",
            "run_a",
            route_components("read_file"),
        );
        let candidate = OptimizationCandidateManifest::new(
            "candidate",
            "probe.tool_route",
            "run_a",
            route_components("apply_patch"),
        )
        .with_parent_candidate_ids(vec![String::from("baseline")]);
        let case_a = route_case("case_a", "read_file");
        let case_b = route_case("case_b", "apply_patch");
        let baseline_batch = OptimizationBatchEvaluationReceipt::new(
            "run_a",
            &baseline,
            vec![
                super::OptimizationCaseEvaluationReceipt::new(
                    &baseline,
                    &case_a,
                    8000,
                    BTreeMap::from([
                        (String::from("correctness_bps"), 8000),
                        (String::from("latency_budget_bps"), 9600),
                    ]),
                    OptimizationSharedFeedback::new("baseline case a"),
                    BTreeMap::new(),
                ),
                super::OptimizationCaseEvaluationReceipt::new(
                    &baseline,
                    &case_b,
                    4000,
                    BTreeMap::from([
                        (String::from("correctness_bps"), 4000),
                        (String::from("latency_budget_bps"), 9600),
                    ]),
                    OptimizationSharedFeedback::new("baseline case b"),
                    BTreeMap::new(),
                ),
            ],
            0,
            2,
        );
        let candidate_batch = OptimizationBatchEvaluationReceipt::new(
            "run_a",
            &candidate,
            vec![
                super::OptimizationCaseEvaluationReceipt::new(
                    &candidate,
                    &case_a,
                    7800,
                    BTreeMap::from([
                        (String::from("correctness_bps"), 7800),
                        (String::from("latency_budget_bps"), 9900),
                    ]),
                    OptimizationSharedFeedback::new("candidate case a"),
                    BTreeMap::new(),
                ),
                super::OptimizationCaseEvaluationReceipt::new(
                    &candidate,
                    &case_b,
                    9100,
                    BTreeMap::from([
                        (String::from("correctness_bps"), 9100),
                        (String::from("latency_budget_bps"), 9900),
                    ]),
                    OptimizationSharedFeedback::new("candidate case b"),
                    BTreeMap::new(),
                ),
            ],
            0,
            2,
        );

        let frontier = OptimizationFrontierSnapshot::from_batches(
            "run_a",
            OptimizationFrontierMode::Hybrid,
            &[baseline_batch, candidate_batch],
        );
        assert_eq!(frontier.case_frontier.len(), 2);
        assert_eq!(frontier.objective_frontier.len(), 2);
        assert!(
            frontier
                .case_frontier
                .iter()
                .any(|row| row.case_id == "case_a" && row.winning_candidate_id == "baseline")
        );
        assert!(
            frontier
                .case_frontier
                .iter()
                .any(|row| row.case_id == "case_b" && row.winning_candidate_id == "candidate")
        );
        assert!(
            frontier
                .objective_frontier
                .iter()
                .any(|row| row.objective_name == "latency_budget_bps"
                    && row.winning_candidate_id == "candidate")
        );
        assert_eq!(
            frontier.hybrid_candidate_ids,
            vec![String::from("baseline"), String::from("candidate")]
        );
        assert!(!frontier.snapshot_digest.is_empty());
    }

    #[test]
    fn frontier_snapshot_materializes_cartesian_winners() {
        let candidate_a = OptimizationCandidateManifest::new(
            "candidate_a",
            "probe.tool_route",
            "run_a",
            route_components("read_file"),
        );
        let candidate_b = OptimizationCandidateManifest::new(
            "candidate_b",
            "probe.tool_route",
            "run_a",
            route_components("apply_patch"),
        );
        let case_a = route_case("case_a", "read_file");
        let case_b = route_case("case_b", "apply_patch");
        let batch_a = OptimizationBatchEvaluationReceipt::new(
            "run_a",
            &candidate_a,
            vec![
                OptimizationCaseEvaluationReceipt::new(
                    &candidate_a,
                    &case_a,
                    7_000,
                    BTreeMap::from([
                        (String::from("tool_bps"), 7_000),
                        (String::from("reason_bps"), 3_000),
                    ]),
                    OptimizationSharedFeedback::new("candidate a case a"),
                    BTreeMap::new(),
                ),
                OptimizationCaseEvaluationReceipt::new(
                    &candidate_a,
                    &case_b,
                    5_000,
                    BTreeMap::from([
                        (String::from("tool_bps"), 2_000),
                        (String::from("reason_bps"), 8_000),
                    ]),
                    OptimizationSharedFeedback::new("candidate a case b"),
                    BTreeMap::new(),
                ),
            ],
            0,
            2,
        );
        let batch_b = OptimizationBatchEvaluationReceipt::new(
            "run_a",
            &candidate_b,
            vec![
                OptimizationCaseEvaluationReceipt::new(
                    &candidate_b,
                    &case_a,
                    6_000,
                    BTreeMap::from([
                        (String::from("tool_bps"), 6_000),
                        (String::from("reason_bps"), 9_000),
                    ]),
                    OptimizationSharedFeedback::new("candidate b case a"),
                    BTreeMap::new(),
                ),
                OptimizationCaseEvaluationReceipt::new(
                    &candidate_b,
                    &case_b,
                    9_000,
                    BTreeMap::from([
                        (String::from("tool_bps"), 9_000),
                        (String::from("reason_bps"), 4_000),
                    ]),
                    OptimizationSharedFeedback::new("candidate b case b"),
                    BTreeMap::new(),
                ),
            ],
            0,
            2,
        );

        let frontier = OptimizationFrontierSnapshot::from_batches(
            "run_a",
            OptimizationFrontierMode::Cartesian,
            &[batch_a, batch_b],
        );

        assert_eq!(frontier.cartesian_frontier.len(), 4);
        assert!(frontier.cartesian_frontier.iter().any(|row| {
            row.case_id == "case_a"
                && row.objective_name == "reason_bps"
                && row.winning_candidate_id == "candidate_b"
        }));
        assert!(frontier.cartesian_frontier.iter().any(|row| {
            row.case_id == "case_b"
                && row.objective_name == "tool_bps"
                && row.winning_candidate_id == "candidate_b"
        }));
        assert_eq!(
            frontier.hybrid_candidate_ids,
            vec![String::from("candidate_a"), String::from("candidate_b")]
        );
    }

    #[test]
    fn optimizer_engine_accepts_better_candidate_from_minibatch() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route")
            .with_iteration_budget(4)
            .with_candidate_budget(4);
        let seed_candidate = route_candidate("baseline", "read_file", Vec::new());
        let train_cases = vec![train_route_case("train_apply_patch", "apply_patch")];
        let validation_cases = vec![route_case("val_apply_patch", "apply_patch")];
        let mut evaluator = DeterministicRouteEvaluator;
        let state = OptimizationEngine::initialize(
            run_spec,
            seed_candidate,
            train_cases,
            validation_cases,
            &mut evaluator,
        )
        .expect("initialize search state");
        let mut proposer = SequenceProposer::new(vec![route_candidate(
            "candidate_apply_patch",
            "apply_patch",
            vec![String::from("baseline")],
        )]);
        let mut sampler = OptimizationSequentialMinibatchSampler::new(1);

        let outcome =
            OptimizationEngine::run(state, &mut evaluator, &mut proposer, &mut sampler, Some(1))
                .expect("run optimizer");

        assert_eq!(outcome.state.current_candidate_id, "candidate_apply_patch");
        assert_eq!(outcome.state.accepted_proposal_count, 1);
        assert_eq!(outcome.state.rejected_proposal_count, 0);
        assert_eq!(outcome.state.current_iteration, 1);
        assert_eq!(outcome.state.iteration_receipts.len(), 1);
        assert_eq!(outcome.state.proposer_receipts.len(), 1);
        assert_eq!(
            outcome.state.lineage_state.retained_candidate_ids,
            vec![String::from("candidate_apply_patch")]
        );

        let receipt = &outcome.state.iteration_receipts[0];
        assert!(receipt.accepted);
        assert_eq!(
            receipt.proposer_receipt_digest,
            outcome.state.proposer_receipts[0].receipt_digest
        );
        assert_eq!(receipt.current_minibatch_score, 0);
        assert_eq!(receipt.proposed_minibatch_score, 10_000);
        assert_eq!(receipt.retained_validation_score, Some(10_000));

        assert_eq!(
            outcome.run_receipt.stop_reason,
            OptimizationStopReason::Manual
        );
        assert_eq!(
            outcome.run_receipt.retained_candidate_ids,
            vec![String::from("candidate_apply_patch")]
        );
        assert_eq!(outcome.run_receipt.candidate_count, 2);
        assert_eq!(outcome.state.total_case_evaluations, 4);
    }

    #[test]
    fn optimizer_search_state_round_trips_and_resumes() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route")
            .with_frontier_mode(OptimizationFrontierMode::Scalar)
            .with_iteration_budget(4)
            .with_candidate_budget(8);
        let seed_candidate = route_candidate("baseline", "read_file", Vec::new());
        let train_cases = vec![
            train_route_case("train_apply_patch", "apply_patch"),
            train_route_case("train_shell", "shell"),
        ];
        let validation_cases = vec![
            route_case("val_apply_patch", "apply_patch"),
            route_case("val_shell", "shell"),
        ];
        let mut evaluator = DeterministicRouteEvaluator;
        let state = OptimizationEngine::initialize(
            run_spec,
            seed_candidate,
            train_cases,
            validation_cases,
            &mut evaluator,
        )
        .expect("initialize search state");
        let mut first_proposer = SequenceProposer::new(vec![route_candidate(
            "candidate_apply_patch",
            "apply_patch",
            vec![String::from("baseline")],
        )]);
        let mut sampler = OptimizationSequentialMinibatchSampler::new(1);

        let first_outcome = OptimizationEngine::run(
            state,
            &mut evaluator,
            &mut first_proposer,
            &mut sampler,
            Some(1),
        )
        .expect("run first optimizer step");
        assert_eq!(first_outcome.state.current_iteration, 1);
        assert_eq!(
            first_outcome.state.current_candidate_id,
            "candidate_apply_patch"
        );
        assert_eq!(first_outcome.state.accepted_proposal_count, 1);
        assert_eq!(first_outcome.state.iteration_receipts.len(), 1);
        assert_eq!(first_outcome.state.proposer_receipts.len(), 1);

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("optimizer_search_state.json");
        first_outcome
            .state
            .write_json(&path)
            .expect("write search state");
        let resumed_state = OptimizationSearchState::read_json(&path).expect("reload search state");
        assert_eq!(resumed_state, first_outcome.state);

        let mut second_proposer = SequenceProposer::new(vec![route_candidate(
            "candidate_shell",
            "shell",
            vec![String::from("candidate_apply_patch")],
        )]);
        let OptimizationEngineRunOutcome { state, run_receipt } = OptimizationEngine::run(
            resumed_state,
            &mut evaluator,
            &mut second_proposer,
            &mut sampler,
            Some(1),
        )
        .expect("resume optimizer");

        assert_eq!(state.current_iteration, 2);
        assert_eq!(state.current_candidate_id, "candidate_shell");
        assert_eq!(state.accepted_proposal_count, 2);
        assert_eq!(state.rejected_proposal_count, 0);
        assert_eq!(state.iteration_receipts.len(), 2);
        assert_eq!(state.proposer_receipts.len(), 2);
        assert_eq!(
            state.lineage_state.retained_candidate_ids,
            vec![
                String::from("candidate_apply_patch"),
                String::from("candidate_shell"),
            ]
        );
        assert!(
            state
                .accepted_validation_batches
                .contains_key("candidate_apply_patch")
        );
        assert!(
            state
                .accepted_validation_batches
                .contains_key("candidate_shell")
        );
        assert_eq!(run_receipt.stop_reason, OptimizationStopReason::Manual);
        assert_eq!(run_receipt.candidate_count, 3);
    }

    #[test]
    fn reflective_mutation_proposer_builds_feedback_driven_candidate_and_receipt() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route")
            .with_iteration_budget(4)
            .with_candidate_budget(4);
        let seed_candidate = route_candidate("baseline", "read_file", Vec::new());
        let train_cases = vec![train_route_case("train_apply_patch", "apply_patch")];
        let validation_cases = vec![route_case("val_apply_patch", "apply_patch")];
        let mut evaluator = DeterministicRouteEvaluator;
        let state = OptimizationEngine::initialize(
            run_spec,
            seed_candidate,
            train_cases,
            validation_cases,
            &mut evaluator,
        )
        .expect("initialize search state");
        let mut proposer = OptimizationReflectiveMutationProposer::new(
            "feedback_label_test_double",
            "reflective_candidate",
            OptimizationFeedbackComponentSelector::new(1, true),
            OptimizationTypedFeedbackDatasetBuilder,
            OptimizationDefaultReflectionPromptBuilder::new(4),
            OptimizationCaseLabelMutationStrategy,
        )
        .with_provenance_refs(vec![String::from("issue:psionic#810")]);
        let mut sampler = OptimizationSequentialMinibatchSampler::new(1);

        let outcome =
            OptimizationEngine::run(state, &mut evaluator, &mut proposer, &mut sampler, Some(1))
                .expect("run optimizer");

        assert_eq!(
            outcome.state.current_candidate_id,
            "reflective_candidate_0002"
        );
        assert_eq!(outcome.state.proposer_receipts.len(), 1);
        let proposer_receipt = &outcome.state.proposer_receipts[0];
        assert_eq!(proposer_receipt.proposer_kind, "feedback_label_test_double");
        assert_eq!(
            proposer_receipt.selected_component_ids,
            vec![String::from("selected_tool")]
        );
        assert_eq!(proposer_receipt.component_diffs.len(), 1);
        assert_eq!(
            proposer_receipt.component_diffs[0].previous_value,
            String::from("read_file")
        );
        assert_eq!(
            proposer_receipt.component_diffs[0].proposed_value,
            String::from("apply_patch")
        );
        assert_eq!(proposer_receipt.prompts.len(), 1);
        assert!(
            proposer_receipt.prompts[0]
                .prompt_text
                .contains("Component feedback:")
        );

        let iteration_receipt = &outcome.state.iteration_receipts[0];
        assert_eq!(
            iteration_receipt.proposer_receipt_digest,
            proposer_receipt.receipt_digest
        );

        let proposed_candidate = outcome
            .state
            .lineage_state
            .candidate("reflective_candidate_0002")
            .expect("proposed candidate in lineage");
        assert_eq!(
            proposed_candidate.components.get("selected_tool"),
            Some(&String::from("apply_patch"))
        );
        assert!(
            proposed_candidate
                .provenance_refs
                .iter()
                .any(|entry| entry.starts_with("reflective_dataset_digest:"))
        );
    }

    #[test]
    fn lineage_merge_proposer_recombines_related_candidates_and_records_receipts() {
        let run_spec = OptimizationRunSpec::new("run_a", "probe.tool_route")
            .with_frontier_mode(OptimizationFrontierMode::Cartesian)
            .with_iteration_budget(4)
            .with_candidate_budget(8);
        let train_cases = vec![
            component_case(
                "train_tool",
                OptimizationCaseSplit::Train,
                "selected_tool",
                "apply_patch",
                "cautious",
            ),
            component_case(
                "train_reason",
                OptimizationCaseSplit::Train,
                "reason",
                "read_file",
                "aggressive",
            ),
        ];
        let validation_cases = vec![
            component_case(
                "val_tool",
                OptimizationCaseSplit::Validation,
                "selected_tool",
                "apply_patch",
                "cautious",
            ),
            component_case(
                "val_reason",
                OptimizationCaseSplit::Validation,
                "reason",
                "read_file",
                "aggressive",
            ),
        ];
        let baseline = policy_candidate("baseline", "read_file", "cautious", Vec::new());
        let candidate_tool = policy_candidate(
            "candidate_tool",
            "apply_patch",
            "cautious",
            vec![String::from("baseline")],
        );
        let candidate_reason = policy_candidate(
            "candidate_reason",
            "read_file",
            "aggressive",
            vec![String::from("baseline")],
        );
        let mut evaluator = DeterministicMergeEvaluator;
        let mut state = OptimizationEngine::initialize(
            run_spec,
            baseline.clone(),
            train_cases.clone(),
            validation_cases.clone(),
            &mut evaluator,
        )
        .expect("initialize search state");
        state
            .lineage_state
            .register_candidate(candidate_tool.clone())
            .expect("register tool candidate");
        state
            .lineage_state
            .register_candidate(candidate_reason.clone())
            .expect("register reason candidate");

        let tool_batch = evaluator.evaluate_candidate(
            "run_a",
            &candidate_tool,
            validation_cases.as_slice(),
            &mut state.evaluation_cache,
        );
        let reason_batch = evaluator.evaluate_candidate(
            "run_a",
            &candidate_reason,
            validation_cases.as_slice(),
            &mut state.evaluation_cache,
        );
        state
            .accepted_validation_batches
            .insert(candidate_tool.candidate_id.clone(), tool_batch);
        state
            .accepted_validation_batches
            .insert(candidate_reason.candidate_id.clone(), reason_batch);
        let frontier_batches = state
            .accepted_validation_batches
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let frontier = OptimizationFrontierSnapshot::from_batches(
            "run_a",
            OptimizationFrontierMode::Cartesian,
            frontier_batches.as_slice(),
        );
        state.current_candidate_id = String::from("candidate_tool");
        state
            .lineage_state
            .set_retained_candidates(frontier.hybrid_candidate_ids.clone())
            .expect("set retained frontier");
        state.latest_frontier_snapshot = Some(frontier);
        state.accepted_proposal_count = 2;
        state.state_digest = state.stable_digest();

        let mut proposer =
            OptimizationLineageAwareMergeProposer::new("lineage_merge_v1", "merged_candidate")
                .with_provenance_refs(vec![String::from("issue:psionic#811")]);
        let mut sampler = OptimizationSequentialMinibatchSampler::new(2);

        let outcome =
            OptimizationEngine::run(state, &mut evaluator, &mut proposer, &mut sampler, Some(1))
                .expect("run merge optimizer");

        assert_eq!(outcome.state.current_candidate_id, "merged_candidate_0004");
        let merged_candidate = outcome
            .state
            .lineage_state
            .candidate("merged_candidate_0004")
            .expect("merged candidate in lineage");
        assert_eq!(
            merged_candidate.parent_candidate_ids,
            vec![
                String::from("candidate_tool"),
                String::from("candidate_reason"),
            ]
        );
        assert_eq!(
            merged_candidate.components,
            candidate_components("apply_patch", "aggressive")
        );
        assert_eq!(outcome.state.proposer_receipts.len(), 1);
        assert_eq!(outcome.state.merge_receipts.len(), 2);
        assert_eq!(
            outcome.state.merge_receipts[0].outcome,
            super::OptimizationMergeOutcome::Attempted
        );
        assert_eq!(
            outcome.state.merge_receipts[1].outcome,
            super::OptimizationMergeOutcome::Accepted
        );
        assert_eq!(
            outcome.state.merge_receipts[1].gating_score_floor,
            Some(10_000)
        );
        assert_eq!(
            outcome.state.merge_receipts[1].merged_minibatch_score,
            Some(20_000)
        );
        assert_eq!(
            outcome.state.merge_receipts[1].component_source_candidate_ids,
            BTreeMap::from([
                (String::from("reason"), String::from("candidate_reason"),),
                (
                    String::from("selected_tool"),
                    String::from("candidate_tool"),
                ),
            ])
        );
    }
}
