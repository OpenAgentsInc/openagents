use psionic_data::DatasetKey;
use psionic_eval::{
    AppleAdapterBaseVsAdapterAcceptancePolicy, AppleAdapterBaseVsAdapterBenchmarkReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable ABI version for first-run Apple adapter experiment manifests.
pub const APPLE_ADAPTER_EXPERIMENT_MANIFEST_ABI_VERSION: &str =
    "psionic.apple_adapter_experiment_manifest.v2";
/// Stable ABI version for first-run Apple adapter trend ledgers.
pub const APPLE_ADAPTER_EXPERIMENT_TREND_LEDGER_ABI_VERSION: &str =
    "psionic.apple_adapter_experiment_trend_ledger.v1";
/// Canonical experiment id for the first real architecture-explainer run.
pub const APPLE_ARCHITECTURE_EXPLAINER_EXPERIMENT_ID: &str =
    "apple_adapter.psionic_architecture_explainer.first_real_run";

/// Benchmark gate variant used when judging whether one Apple adapter is useful.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterUsefulAdapterBenchmarkMode {
    /// The normal benchmark gate for the standard architecture-explainer run.
    Standard,
    /// The weaker benchmark-overfit gate that still requires non-zero movement.
    OverfitNonZero,
}

impl AppleAdapterUsefulAdapterBenchmarkMode {
    /// Returns the stable label for this benchmark mode.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::OverfitNonZero => "overfit_non_zero",
        }
    }
}

/// Frozen success contract for the first useful-adapter Apple reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterUsefulAdapterAcceptanceGate {
    /// Whether runtime smoke must pass before usefulness can be claimed.
    pub runtime_smoke_required: bool,
    /// Standard benchmark gate for the normal architecture-explainer run.
    pub standard_benchmark_policy: AppleAdapterBaseVsAdapterAcceptancePolicy,
    /// Separate non-zero overfit gate used to prove the lane can actually move.
    pub overfit_non_zero_policy: AppleAdapterBaseVsAdapterAcceptancePolicy,
}

impl AppleAdapterUsefulAdapterAcceptanceGate {
    /// Canonical useful-adapter gate for the first architecture-explainer lane.
    #[must_use]
    pub const fn architecture_explainer_default() -> Self {
        Self {
            runtime_smoke_required: true,
            standard_benchmark_policy:
                AppleAdapterBaseVsAdapterAcceptancePolicy::architecture_explainer_default(),
            overfit_non_zero_policy: AppleAdapterBaseVsAdapterAcceptancePolicy {
                minimum_adapter_score_bps: 1,
                minimum_adapter_pass_rate_bps: 1,
                minimum_score_delta_bps: 1,
                minimum_pass_rate_delta_bps: 1,
                minimum_improved_case_count: 1,
            },
        }
    }

    /// Returns the selected benchmark policy for one useful-adapter mode.
    #[must_use]
    pub const fn policy_for_mode(
        &self,
        mode: AppleAdapterUsefulAdapterBenchmarkMode,
    ) -> &AppleAdapterBaseVsAdapterAcceptancePolicy {
        match mode {
            AppleAdapterUsefulAdapterBenchmarkMode::Standard => &self.standard_benchmark_policy,
            AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero => &self.overfit_non_zero_policy,
        }
    }

    fn validate(&self) -> Result<(), AppleAdapterExperimentError> {
        validate_acceptance_policy(
            &self.standard_benchmark_policy,
            "useful_adapter_gate.standard_benchmark_policy",
        )?;
        validate_acceptance_policy(
            &self.overfit_non_zero_policy,
            "useful_adapter_gate.overfit_non_zero_policy",
        )?;
        Ok(())
    }
}

/// One frozen experiment manifest for a concrete Apple adapter training attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExperimentManifest {
    /// Stable manifest ABI version.
    pub abi_version: String,
    /// Stable experiment identifier.
    pub experiment_id: String,
    /// Stable training target identifier.
    pub target_id: String,
    /// Frozen dataset identity.
    pub dataset: DatasetKey,
    /// Frozen train split digest.
    pub train_split_digest: String,
    /// Frozen held-out split digest.
    pub held_out_split_digest: String,
    /// Frozen benchmark split digest.
    pub benchmark_split_digest: String,
    /// Frozen curated-corpus manifest digest.
    pub corpus_manifest_digest: String,
    /// Runtime-derived Apple base-model compatibility anchor.
    pub base_model_signature: String,
    /// Frozen tokenizer digest for the run.
    pub tokenizer_digest: String,
    /// Frozen prompt-shaping digest for the run.
    pub prompt_shaping_digest: String,
    /// Frozen Apple environment ref.
    pub environment_ref: String,
    /// Frozen benchmark ref.
    pub benchmark_ref: String,
    /// Fidelity plan used by the backend for this run.
    pub fidelity_plan_id: String,
    /// Input width for the repo-owned backend.
    pub input_width: usize,
    /// Output width for the repo-owned backend.
    pub output_width: usize,
    /// Stable LoRA target identifiers.
    pub lora_targets: Vec<String>,
    /// Shared LoRA rank for the selected targets.
    pub lora_rank: usize,
    /// Fixed-budget max step count.
    pub max_steps: u64,
    /// Frozen useful-adapter contract for the run.
    pub useful_adapter_gate: AppleAdapterUsefulAdapterAcceptanceGate,
}

impl AppleAdapterExperimentManifest {
    /// Validates the frozen experiment manifest.
    pub fn validate(&self) -> Result<(), AppleAdapterExperimentError> {
        if self.abi_version != APPLE_ADAPTER_EXPERIMENT_MANIFEST_ABI_VERSION {
            return Err(AppleAdapterExperimentError::UnsupportedManifestAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.experiment_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingExperimentId);
        }
        if self.target_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingTargetId);
        }
        if self.dataset.dataset_ref.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingDatasetRef);
        }
        if self.dataset.version.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingDatasetVersion);
        }
        for (label, digest) in [
            ("train", self.train_split_digest.as_str()),
            ("held_out", self.held_out_split_digest.as_str()),
            ("benchmark", self.benchmark_split_digest.as_str()),
            ("corpus_manifest", self.corpus_manifest_digest.as_str()),
        ] {
            if digest.trim().is_empty() {
                return Err(AppleAdapterExperimentError::MissingSplitDigest {
                    split: label.to_string(),
                });
            }
        }
        if self.base_model_signature.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingBaseModelSignature);
        }
        if self.tokenizer_digest.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingTokenizerDigest);
        }
        if self.prompt_shaping_digest.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingPromptShapingDigest);
        }
        if self.environment_ref.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingEnvironmentRef);
        }
        if self.benchmark_ref.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingBenchmarkRef);
        }
        if self.fidelity_plan_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingFidelityPlanId);
        }
        if self.input_width == 0 || self.output_width == 0 {
            return Err(AppleAdapterExperimentError::InvalidFeatureWidth);
        }
        if self.lora_targets.is_empty() {
            return Err(AppleAdapterExperimentError::MissingLoraTargets);
        }
        if self.lora_rank == 0 {
            return Err(AppleAdapterExperimentError::InvalidLoraRank);
        }
        if self.max_steps == 0 {
            return Err(AppleAdapterExperimentError::InvalidMaxSteps);
        }
        self.useful_adapter_gate.validate()?;
        Ok(())
    }

    /// Returns the stable digest for the frozen experiment definition.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_apple_adapter_experiment_manifest|");
        hasher.update(self.abi_version.as_bytes());
        hasher.update(b"|");
        hasher.update(self.experiment_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.target_id.as_bytes());
        hasher.update(b"|dataset|");
        hasher.update(self.dataset.storage_key().as_bytes());
        for digest in [
            self.train_split_digest.as_str(),
            self.held_out_split_digest.as_str(),
            self.benchmark_split_digest.as_str(),
            self.corpus_manifest_digest.as_str(),
        ] {
            hasher.update(b"|");
            hasher.update(digest.as_bytes());
        }
        hasher.update(b"|");
        hasher.update(self.base_model_signature.as_bytes());
        hasher.update(b"|");
        hasher.update(self.tokenizer_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.prompt_shaping_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.environment_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(self.benchmark_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(self.fidelity_plan_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.input_width.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.output_width.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.lora_rank.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(self.max_steps.to_string().as_bytes());
        for target in &self.lora_targets {
            hasher.update(b"|target|");
            hasher.update(target.as_bytes());
        }
        hasher.update(b"|useful_adapter_gate|");
        hasher.update(
            serde_json::to_vec(&self.useful_adapter_gate)
                .unwrap_or_default()
                .as_slice(),
        );
        hex::encode(hasher.finalize())
    }
}

/// Reduced benchmark summary kept in experiment manifests, ledgers, and selection records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExperimentBenchmarkSummary {
    /// Stable benchmark ref.
    pub benchmark_ref: String,
    /// Base-model aggregate score.
    pub base_score_bps: u32,
    /// Adapted-model aggregate score.
    pub adapted_score_bps: u32,
    /// Aggregate score delta.
    pub aggregate_score_delta_bps: i32,
    /// Base-model aggregate pass rate.
    pub base_pass_rate_bps: u32,
    /// Adapted-model aggregate pass rate.
    pub adapted_pass_rate_bps: u32,
    /// Aggregate pass-rate delta.
    pub aggregate_pass_rate_delta_bps: i32,
    /// Improved-case count.
    pub improved_case_count: u32,
    /// Whether the benchmark gate accepted the candidate.
    pub accepted: bool,
    /// Machine-legible reason codes surfaced by the benchmark gate.
    #[serde(default)]
    pub reason_codes: Vec<String>,
}

impl AppleAdapterExperimentBenchmarkSummary {
    /// Creates a benchmark summary directly from one machine-legible benchmark report.
    #[must_use]
    pub fn from_report(report: &AppleAdapterBaseVsAdapterBenchmarkReport) -> Self {
        Self {
            benchmark_ref: report.benchmark_key.benchmark_ref.clone(),
            base_score_bps: report.base_summary.aggregate_score_bps.unwrap_or(0),
            adapted_score_bps: report.adapted_summary.aggregate_score_bps.unwrap_or(0),
            aggregate_score_delta_bps: report.acceptance.aggregate_score_delta_bps,
            base_pass_rate_bps: report.base_summary.aggregate_pass_rate_bps,
            adapted_pass_rate_bps: report.adapted_summary.aggregate_pass_rate_bps,
            aggregate_pass_rate_delta_bps: report.acceptance.aggregate_pass_rate_delta_bps,
            improved_case_count: report.acceptance.improved_case_count,
            accepted: report.acceptance.accepted,
            reason_codes: report
                .acceptance
                .reason_codes
                .iter()
                .map(|code| format!("{code:?}"))
                .collect(),
        }
    }
}

/// One checkpoint/package candidate compared inside the experiment program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExperimentCheckpointCandidate {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Stable checkpoint ref produced by the run.
    pub checkpoint_ref: String,
    /// Stable exported package digest.
    pub package_digest: String,
    /// Reduced benchmark summary for selection and trend tracking.
    pub benchmark: AppleAdapterExperimentBenchmarkSummary,
}

impl AppleAdapterExperimentCheckpointCandidate {
    fn validate(&self) -> Result<(), AppleAdapterExperimentError> {
        if self.candidate_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingCandidateId);
        }
        if self.checkpoint_ref.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingCheckpointRef {
                candidate_id: self.candidate_id.clone(),
            });
        }
        if self.package_digest.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingPackageDigest {
                candidate_id: self.candidate_id.clone(),
            });
        }
        Ok(())
    }
}

/// Selection result for one frozen experiment iteration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterCheckpointSelection {
    /// Winning candidate id.
    pub selected_candidate_id: String,
    /// Winning checkpoint ref.
    pub selected_checkpoint_ref: String,
    /// Winning package digest.
    pub selected_package_digest: String,
    /// Whether the winning candidate actually cleared the acceptance bar.
    pub accepted: bool,
    /// Machine-legible rationale summary.
    pub rationale: String,
}

/// One experiment result entry persisted into the benchmark trend ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExperimentTrendEntry {
    /// Stable operator run id.
    pub run_id: String,
    /// Stable digest of the frozen experiment manifest used for the run.
    pub manifest_digest: String,
    /// Final checkpoint selection.
    pub selection: AppleAdapterCheckpointSelection,
    /// Benchmark summary for the selected candidate.
    pub benchmark: AppleAdapterExperimentBenchmarkSummary,
}

impl AppleAdapterExperimentTrendEntry {
    fn validate(&self) -> Result<(), AppleAdapterExperimentError> {
        if self.run_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingRunId);
        }
        if self.manifest_digest.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingManifestDigest);
        }
        if self.selection.selected_candidate_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingSelectedCandidateId);
        }
        if self.selection.selected_checkpoint_ref.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingSelectedCheckpointRef);
        }
        if self.selection.selected_package_digest.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingSelectedPackageDigest);
        }
        Ok(())
    }
}

/// Ordered benchmark-trend ledger for repeated experiment iterations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExperimentTrendLedger {
    /// Stable ledger ABI version.
    pub abi_version: String,
    /// Stable experiment identifier.
    pub experiment_id: String,
    /// Ordered experiment entries.
    #[serde(default)]
    pub entries: Vec<AppleAdapterExperimentTrendEntry>,
}

impl AppleAdapterExperimentTrendLedger {
    /// Validates the trend ledger.
    pub fn validate(&self) -> Result<(), AppleAdapterExperimentError> {
        if self.abi_version != APPLE_ADAPTER_EXPERIMENT_TREND_LEDGER_ABI_VERSION {
            return Err(
                AppleAdapterExperimentError::UnsupportedTrendLedgerAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.experiment_id.trim().is_empty() {
            return Err(AppleAdapterExperimentError::MissingExperimentId);
        }
        for entry in &self.entries {
            entry.validate()?;
        }
        Ok(())
    }
}

/// Machine-legible regression reasons surfaced by the experiment ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterExperimentRegressionReasonCode {
    /// The latest run failed the acceptance bar after an earlier accepted run.
    AcceptanceDropped,
    /// Aggregate score regressed against the best prior run.
    AggregateScoreDropped,
    /// Aggregate pass rate regressed against the best prior run.
    AggregatePassRateDropped,
    /// Improved-case count regressed against the best prior run.
    ImprovedCaseCountDropped,
}

/// Regression report comparing the latest run to the best prior run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterExperimentRegressionReport {
    /// Prior run used as the baseline.
    pub baseline_run_id: String,
    /// Latest run id being evaluated.
    pub candidate_run_id: String,
    /// Latest minus baseline aggregate-score delta.
    pub aggregate_score_delta_bps: i32,
    /// Latest minus baseline aggregate-pass-rate delta.
    pub aggregate_pass_rate_delta_bps: i32,
    /// Latest minus baseline improved-case-count delta.
    pub improved_case_delta: i32,
    /// Machine-legible reasons for the detected regression.
    pub reason_codes: Vec<AppleAdapterExperimentRegressionReasonCode>,
}

/// Experiment selection, ledger, or manifest failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleAdapterExperimentError {
    /// Unsupported manifest ABI version.
    #[error("unsupported Apple adapter experiment manifest ABI version `{abi_version}`")]
    UnsupportedManifestAbiVersion { abi_version: String },
    /// Unsupported trend-ledger ABI version.
    #[error("unsupported Apple adapter experiment trend-ledger ABI version `{abi_version}`")]
    UnsupportedTrendLedgerAbiVersion { abi_version: String },
    /// Missing experiment id.
    #[error("Apple adapter experiment manifest is missing `experiment_id`")]
    MissingExperimentId,
    /// Missing target id.
    #[error("Apple adapter experiment manifest is missing `target_id`")]
    MissingTargetId,
    /// Missing dataset ref.
    #[error("Apple adapter experiment manifest is missing `dataset.dataset_ref`")]
    MissingDatasetRef,
    /// Missing dataset version.
    #[error("Apple adapter experiment manifest is missing `dataset.version`")]
    MissingDatasetVersion,
    /// Missing one split digest.
    #[error("Apple adapter experiment manifest is missing `{split}` split digest")]
    MissingSplitDigest { split: String },
    /// Missing base-model compatibility anchor.
    #[error("Apple adapter experiment manifest is missing `base_model_signature`")]
    MissingBaseModelSignature,
    /// Missing tokenizer digest.
    #[error("Apple adapter experiment manifest is missing `tokenizer_digest`")]
    MissingTokenizerDigest,
    /// Missing prompt-shaping digest.
    #[error("Apple adapter experiment manifest is missing `prompt_shaping_digest`")]
    MissingPromptShapingDigest,
    /// Missing environment ref.
    #[error("Apple adapter experiment manifest is missing `environment_ref`")]
    MissingEnvironmentRef,
    /// Missing benchmark ref.
    #[error("Apple adapter experiment manifest is missing `benchmark_ref`")]
    MissingBenchmarkRef,
    /// Missing fidelity plan id.
    #[error("Apple adapter experiment manifest is missing `fidelity_plan_id`")]
    MissingFidelityPlanId,
    /// Invalid feature width.
    #[error("Apple adapter experiment manifest requires `input_width > 0` and `output_width > 0`")]
    InvalidFeatureWidth,
    /// Missing LoRA targets.
    #[error("Apple adapter experiment manifest requires at least one LoRA target")]
    MissingLoraTargets,
    /// Invalid LoRA rank.
    #[error("Apple adapter experiment manifest requires `lora_rank > 0`")]
    InvalidLoraRank,
    /// Invalid step budget.
    #[error("Apple adapter experiment manifest requires `max_steps > 0`")]
    InvalidMaxSteps,
    /// One acceptance-policy value was outside the supported range.
    #[error(
        "Apple adapter experiment manifest field `{field}` must stay within its supported benchmark-policy range"
    )]
    InvalidAcceptancePolicyField { field: String },
    /// No candidate checkpoints were available for selection.
    #[error("Apple adapter experiment selection requires at least one candidate")]
    MissingCheckpointCandidates,
    /// Candidate id was empty.
    #[error("Apple adapter experiment candidate is missing `candidate_id`")]
    MissingCandidateId,
    /// Checkpoint ref was empty.
    #[error("Apple adapter experiment candidate `{candidate_id}` is missing `checkpoint_ref`")]
    MissingCheckpointRef { candidate_id: String },
    /// Package digest was empty.
    #[error("Apple adapter experiment candidate `{candidate_id}` is missing `package_digest`")]
    MissingPackageDigest { candidate_id: String },
    /// Run id was empty.
    #[error("Apple adapter experiment trend entry is missing `run_id`")]
    MissingRunId,
    /// Manifest digest was empty.
    #[error("Apple adapter experiment trend entry is missing `manifest_digest`")]
    MissingManifestDigest,
    /// Selected candidate id was empty.
    #[error("Apple adapter experiment trend entry is missing `selection.selected_candidate_id`")]
    MissingSelectedCandidateId,
    /// Selected checkpoint ref was empty.
    #[error("Apple adapter experiment trend entry is missing `selection.selected_checkpoint_ref`")]
    MissingSelectedCheckpointRef,
    /// Selected package digest was empty.
    #[error("Apple adapter experiment trend entry is missing `selection.selected_package_digest`")]
    MissingSelectedPackageDigest,
}

fn validate_acceptance_policy(
    policy: &AppleAdapterBaseVsAdapterAcceptancePolicy,
    prefix: &str,
) -> Result<(), AppleAdapterExperimentError> {
    for (field, value) in [
        (
            format!("{prefix}.minimum_adapter_score_bps"),
            policy.minimum_adapter_score_bps > 10_000,
        ),
        (
            format!("{prefix}.minimum_adapter_pass_rate_bps"),
            policy.minimum_adapter_pass_rate_bps > 10_000,
        ),
    ] {
        if value {
            return Err(AppleAdapterExperimentError::InvalidAcceptancePolicyField { field });
        }
    }
    for (field, value) in [
        (
            format!("{prefix}.minimum_score_delta_bps"),
            policy.minimum_score_delta_bps.unsigned_abs() > 10_000,
        ),
        (
            format!("{prefix}.minimum_pass_rate_delta_bps"),
            policy.minimum_pass_rate_delta_bps.unsigned_abs() > 10_000,
        ),
    ] {
        if value {
            return Err(AppleAdapterExperimentError::InvalidAcceptancePolicyField { field });
        }
    }
    Ok(())
}

/// Selects the best checkpoint candidate for one experiment iteration.
pub fn select_checkpoint_candidate(
    candidates: &[AppleAdapterExperimentCheckpointCandidate],
) -> Result<AppleAdapterCheckpointSelection, AppleAdapterExperimentError> {
    for candidate in candidates {
        candidate.validate()?;
    }
    let winner = candidates
        .iter()
        .max_by_key(|candidate| {
            (
                candidate.benchmark.accepted,
                candidate.benchmark.adapted_score_bps,
                candidate.benchmark.aggregate_score_delta_bps,
                candidate.benchmark.improved_case_count,
                candidate.candidate_id.clone(),
            )
        })
        .ok_or(AppleAdapterExperimentError::MissingCheckpointCandidates)?;
    Ok(AppleAdapterCheckpointSelection {
        selected_candidate_id: winner.candidate_id.clone(),
        selected_checkpoint_ref: winner.checkpoint_ref.clone(),
        selected_package_digest: winner.package_digest.clone(),
        accepted: winner.benchmark.accepted,
        rationale: format!(
            "selected {} with adapted_score_bps={} score_delta_bps={} improved_case_count={} accepted={}",
            winner.candidate_id,
            winner.benchmark.adapted_score_bps,
            winner.benchmark.aggregate_score_delta_bps,
            winner.benchmark.improved_case_count,
            winner.benchmark.accepted,
        ),
    })
}

/// Appends one experiment iteration to the ordered ledger and returns any
/// newly detected regression against the best prior run.
pub fn append_experiment_trend_entry(
    ledger: &mut AppleAdapterExperimentTrendLedger,
    entry: AppleAdapterExperimentTrendEntry,
) -> Result<Option<AppleAdapterExperimentRegressionReport>, AppleAdapterExperimentError> {
    ledger.validate()?;
    entry.validate()?;
    ledger.entries.push(entry);
    Ok(detect_experiment_regression(ledger))
}

/// Detects whether the latest ledger entry regressed against the best prior run.
pub fn detect_experiment_regression(
    ledger: &AppleAdapterExperimentTrendLedger,
) -> Option<AppleAdapterExperimentRegressionReport> {
    if ledger.entries.len() < 2 {
        return None;
    }
    let latest = ledger.entries.last()?;
    let baseline = ledger.entries[..ledger.entries.len() - 1]
        .iter()
        .max_by_key(|entry| {
            (
                entry.benchmark.accepted,
                entry.benchmark.adapted_score_bps,
                entry.benchmark.aggregate_score_delta_bps,
                entry.benchmark.improved_case_count,
            )
        })?;
    let mut reason_codes = Vec::new();
    if baseline.benchmark.accepted && !latest.benchmark.accepted {
        reason_codes.push(AppleAdapterExperimentRegressionReasonCode::AcceptanceDropped);
    }
    if latest.benchmark.adapted_score_bps < baseline.benchmark.adapted_score_bps {
        reason_codes.push(AppleAdapterExperimentRegressionReasonCode::AggregateScoreDropped);
    }
    if latest.benchmark.adapted_pass_rate_bps < baseline.benchmark.adapted_pass_rate_bps {
        reason_codes.push(AppleAdapterExperimentRegressionReasonCode::AggregatePassRateDropped);
    }
    if latest.benchmark.improved_case_count < baseline.benchmark.improved_case_count {
        reason_codes.push(AppleAdapterExperimentRegressionReasonCode::ImprovedCaseCountDropped);
    }
    if reason_codes.is_empty() {
        return None;
    }
    Some(AppleAdapterExperimentRegressionReport {
        baseline_run_id: baseline.run_id.clone(),
        candidate_run_id: latest.run_id.clone(),
        aggregate_score_delta_bps: latest.benchmark.adapted_score_bps as i32
            - baseline.benchmark.adapted_score_bps as i32,
        aggregate_pass_rate_delta_bps: latest.benchmark.adapted_pass_rate_bps as i32
            - baseline.benchmark.adapted_pass_rate_bps as i32,
        improved_case_delta: latest.benchmark.improved_case_count as i32
            - baseline.benchmark.improved_case_count as i32,
        reason_codes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> AppleAdapterExperimentManifest {
        AppleAdapterExperimentManifest {
            abi_version: APPLE_ADAPTER_EXPERIMENT_MANIFEST_ABI_VERSION.to_string(),
            experiment_id: APPLE_ARCHITECTURE_EXPLAINER_EXPERIMENT_ID.to_string(),
            target_id: String::from("apple_adapter.psionic_architecture_explainer"),
            dataset: DatasetKey::new(
                "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                "2026.03.15.2",
            ),
            train_split_digest: String::from("sha256:train-split-digest"),
            held_out_split_digest: String::from("sha256:held-out-split-digest"),
            benchmark_split_digest: String::from("sha256:benchmark-split-digest"),
            corpus_manifest_digest: String::from("sha256:corpus-manifest-digest"),
            base_model_signature: String::from("9799725ff8e851184037110b422d891ad3b92ec1"),
            tokenizer_digest: String::from("sha256:tokenizer"),
            prompt_shaping_digest: String::from("sha256:prompt"),
            environment_ref: String::from("env.openagents.apple_adapter.helpdesk.core"),
            benchmark_ref: String::from(
                "benchmark://openagents/apple_adapter/psionic_architecture_explainer/base_vs_adapter",
            ),
            fidelity_plan_id: String::from("openagents.apple.token_sequence_reference.v1"),
            input_width: 48,
            output_width: 24,
            lora_targets: vec![
                String::from("decoder.attn.q_proj"),
                String::from("decoder.ffn.up_proj"),
            ],
            lora_rank: 4,
            max_steps: 8,
            useful_adapter_gate:
                AppleAdapterUsefulAdapterAcceptanceGate::architecture_explainer_default(),
        }
    }

    fn benchmark_summary(
        adapted_score_bps: u32,
        adapted_pass_rate_bps: u32,
        improved_case_count: u32,
        accepted: bool,
    ) -> AppleAdapterExperimentBenchmarkSummary {
        AppleAdapterExperimentBenchmarkSummary {
            benchmark_ref: String::from(
                "benchmark://openagents/apple_adapter/psionic_architecture_explainer/base_vs_adapter",
            ),
            base_score_bps: 7600,
            adapted_score_bps,
            aggregate_score_delta_bps: adapted_score_bps as i32 - 7600,
            base_pass_rate_bps: 7500,
            adapted_pass_rate_bps,
            aggregate_pass_rate_delta_bps: adapted_pass_rate_bps as i32 - 7500,
            improved_case_count,
            accepted,
            reason_codes: Vec::new(),
        }
    }

    #[test]
    fn experiment_manifest_validates_and_hashes_stably() {
        let manifest = sample_manifest();
        manifest.validate().expect("manifest should validate");
        assert_eq!(manifest.stable_digest(), manifest.stable_digest());
    }

    #[test]
    fn useful_adapter_gate_exposes_standard_and_overfit_policies() {
        let gate = AppleAdapterUsefulAdapterAcceptanceGate::architecture_explainer_default();
        assert!(gate.runtime_smoke_required);
        assert_eq!(
            gate.policy_for_mode(AppleAdapterUsefulAdapterBenchmarkMode::Standard),
            &AppleAdapterBaseVsAdapterAcceptancePolicy::architecture_explainer_default()
        );
        assert_eq!(
            gate.policy_for_mode(AppleAdapterUsefulAdapterBenchmarkMode::OverfitNonZero)
                .minimum_improved_case_count,
            1
        );
    }

    #[test]
    fn manifest_rejects_out_of_range_acceptance_policy_values() {
        let mut manifest = sample_manifest();
        manifest
            .useful_adapter_gate
            .standard_benchmark_policy
            .minimum_adapter_score_bps = 10_001;
        assert_eq!(
            manifest.validate(),
            Err(AppleAdapterExperimentError::InvalidAcceptancePolicyField {
                field: String::from(
                    "useful_adapter_gate.standard_benchmark_policy.minimum_adapter_score_bps"
                ),
            })
        );
    }

    #[test]
    fn checkpoint_selection_prefers_accepted_higher_scoring_candidate() {
        let candidates = vec![
            AppleAdapterExperimentCheckpointCandidate {
                candidate_id: String::from("run-a"),
                checkpoint_ref: String::from("checkpoint://run-a/final"),
                package_digest: String::from("pkg-a"),
                benchmark: benchmark_summary(8700, 8600, 3, false),
            },
            AppleAdapterExperimentCheckpointCandidate {
                candidate_id: String::from("run-b"),
                checkpoint_ref: String::from("checkpoint://run-b/final"),
                package_digest: String::from("pkg-b"),
                benchmark: benchmark_summary(9200, 9100, 5, true),
            },
        ];
        let selection = select_checkpoint_candidate(candidates.as_slice()).expect("selection");
        assert_eq!(selection.selected_candidate_id, "run-b");
        assert!(selection.accepted);
    }

    #[test]
    fn regression_report_flags_latest_drop_from_best_prior_run() {
        let ledger = AppleAdapterExperimentTrendLedger {
            abi_version: APPLE_ADAPTER_EXPERIMENT_TREND_LEDGER_ABI_VERSION.to_string(),
            experiment_id: APPLE_ARCHITECTURE_EXPLAINER_EXPERIMENT_ID.to_string(),
            entries: vec![
                AppleAdapterExperimentTrendEntry {
                    run_id: String::from("run-1"),
                    manifest_digest: String::from("manifest-1"),
                    selection: AppleAdapterCheckpointSelection {
                        selected_candidate_id: String::from("run-1"),
                        selected_checkpoint_ref: String::from("checkpoint://run-1/final"),
                        selected_package_digest: String::from("pkg-1"),
                        accepted: true,
                        rationale: String::from("baseline"),
                    },
                    benchmark: benchmark_summary(9300, 9200, 6, true),
                },
                AppleAdapterExperimentTrendEntry {
                    run_id: String::from("run-2"),
                    manifest_digest: String::from("manifest-2"),
                    selection: AppleAdapterCheckpointSelection {
                        selected_candidate_id: String::from("run-2"),
                        selected_checkpoint_ref: String::from("checkpoint://run-2/final"),
                        selected_package_digest: String::from("pkg-2"),
                        accepted: false,
                        rationale: String::from("regressed"),
                    },
                    benchmark: benchmark_summary(8800, 8700, 4, false),
                },
            ],
        };
        let regression =
            detect_experiment_regression(&ledger).expect("regression should be detected");
        assert_eq!(regression.baseline_run_id, "run-1");
        assert_eq!(regression.candidate_run_id, "run-2");
        assert!(
            regression
                .reason_codes
                .contains(&AppleAdapterExperimentRegressionReasonCode::AcceptanceDropped)
        );
        assert!(
            regression
                .reason_codes
                .contains(&AppleAdapterExperimentRegressionReasonCode::AggregateScoreDropped)
        );
    }

    #[test]
    fn append_trend_entry_surfaces_new_regression() {
        let mut ledger = AppleAdapterExperimentTrendLedger {
            abi_version: APPLE_ADAPTER_EXPERIMENT_TREND_LEDGER_ABI_VERSION.to_string(),
            experiment_id: APPLE_ARCHITECTURE_EXPLAINER_EXPERIMENT_ID.to_string(),
            entries: vec![AppleAdapterExperimentTrendEntry {
                run_id: String::from("run-1"),
                manifest_digest: String::from("manifest-1"),
                selection: AppleAdapterCheckpointSelection {
                    selected_candidate_id: String::from("run-1"),
                    selected_checkpoint_ref: String::from("checkpoint://run-1/final"),
                    selected_package_digest: String::from("pkg-1"),
                    accepted: true,
                    rationale: String::from("baseline"),
                },
                benchmark: benchmark_summary(9300, 9200, 6, true),
            }],
        };
        let regression = append_experiment_trend_entry(
            &mut ledger,
            AppleAdapterExperimentTrendEntry {
                run_id: String::from("run-2"),
                manifest_digest: String::from("manifest-2"),
                selection: AppleAdapterCheckpointSelection {
                    selected_candidate_id: String::from("run-2"),
                    selected_checkpoint_ref: String::from("checkpoint://run-2/final"),
                    selected_package_digest: String::from("pkg-2"),
                    accepted: false,
                    rationale: String::from("regressed"),
                },
                benchmark: benchmark_summary(8800, 8700, 4, false),
            },
        )
        .expect("entry should append");
        let regression = regression.expect("regression should be detected");
        assert_eq!(ledger.entries.len(), 2);
        assert_eq!(regression.baseline_run_id, "run-1");
        assert_eq!(regression.candidate_run_id, "run-2");
    }

    #[test]
    fn fixture_manifest_and_trend_ledger_roundtrip() {
        let manifest = serde_json::from_str::<AppleAdapterExperimentManifest>(include_str!(
            "../../fixtures/apple_adapter/experiments/psionic_architecture_explainer_first_real_run_v1.json"
        ))
        .expect("manifest fixture should decode");
        manifest
            .validate()
            .expect("manifest fixture should validate");

        let ledger = serde_json::from_str::<AppleAdapterExperimentTrendLedger>(include_str!(
            "../../fixtures/apple_adapter/experiments/psionic_architecture_explainer_trend_ledger_v1.json"
        ))
        .expect("ledger fixture should decode");
        ledger.validate().expect("ledger fixture should validate");
        assert!(
            detect_experiment_regression(&ledger).is_some(),
            "ledger fixture should carry one explicit regression example"
        );
    }
}
