use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    compiled_agent_baseline_revision_set, compiled_agent_supported_tools,
    evaluate_compiled_agent_grounded_answer, evaluate_compiled_agent_route,
    evaluate_compiled_agent_tool_arguments, evaluate_compiled_agent_tool_policy,
    evaluate_compiled_agent_verify, predict_compiled_agent_grounded_answer,
    predict_compiled_agent_route, CompiledAgentEvidenceClass, CompiledAgentModuleKind,
    CompiledAgentPublicOutcomeKind, CompiledAgentRoute, CompiledAgentRuntimeState,
    CompiledAgentToolResult, CompiledAgentVerifyVerdict,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_compiled_agent_learning_receipt_from_source,
    build_compiled_agent_learning_receipt_ledger_from_receipts,
    build_compiled_agent_replay_bundle_from_ledger,
    canonical_compiled_agent_external_benchmark_run,
    canonical_compiled_agent_external_contributor_identity,
    canonical_compiled_agent_governed_source_receipt,
    canonical_compiled_agent_promoted_artifact_contract, repo_relative_path,
    retained_compiled_agent_external_benchmark_kit, CompiledAgentArtifactContractEntry,
    CompiledAgentArtifactContractError, CompiledAgentArtifactLifecycleState,
    CompiledAgentArtifactPayload, CompiledAgentCorpusSplit, CompiledAgentDisagreementReason,
    CompiledAgentExternalBenchmarkError, CompiledAgentExternalBenchmarkRun,
    CompiledAgentExternalContributorIdentity, CompiledAgentReceiptError,
    CompiledAgentReceiptSupervisionLabel, CompiledAgentReplaySample,
    CompiledAgentReviewDisposition, CompiledAgentSourceManifest,
    CompiledAgentSourcePhaseTraceEntry, CompiledAgentSourceReceipt,
    COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH,
};

pub const COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_runtime_receipt_submission_v1.json";
pub const COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_replay_proposal_v1.json";
pub const COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_submission_staging_ledger_v1.json";
pub const COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_quarantine_report_v1.json";
pub const COMPILED_AGENT_EXTERNAL_INTAKE_DOC_PATH: &str = "docs/COMPILED_AGENT_EXTERNAL_INTAKE.md";

const COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_runtime_receipt_submission.v1";
const COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_replay_proposal.v1";
const COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_submission_staging_ledger.v1";
const COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_quarantine_report.v1";

const EXTERNAL_RUNTIME_SUBMISSION_ID: &str =
    "submission.compiled_agent.external_runtime_disagreement.alpha.v1";
const EXTERNAL_REPLAY_PROPOSAL_ID: &str = "proposal.compiled_agent.external_replay.alpha.v1";
const EXTERNAL_STAGING_LEDGER_ID: &str = "compiled_agent.external_submission_staging_ledger.v1";
const EXTERNAL_QUARANTINE_REPORT_ID: &str = "compiled_agent.external_quarantine_report.v1";
const INVALID_BENCHMARK_SUBMISSION_ID: &str =
    "submission.compiled_agent.external_benchmark_invalid.alpha.v1";

#[derive(Debug, Error)]
pub enum CompiledAgentExternalIntakeError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid external runtime receipt submission: {detail}")]
    InvalidRuntimeSubmission { detail: String },
    #[error("invalid external replay proposal: {detail}")]
    InvalidReplayProposal { detail: String },
    #[error("invalid external staging ledger: {detail}")]
    InvalidStagingLedger { detail: String },
    #[error("invalid external quarantine report: {detail}")]
    InvalidQuarantineReport { detail: String },
    #[error("missing artifact entry for module `{module}` and lifecycle `{lifecycle}`")]
    MissingArtifactEntry { module: String, lifecycle: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    ArtifactContract(#[from] CompiledAgentArtifactContractError),
    #[error(transparent)]
    ExternalBenchmark(#[from] CompiledAgentExternalBenchmarkError),
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalSubmissionKind {
    BenchmarkRun,
    RuntimeReceipt,
    ReplayProposal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalValidatorStatus {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalQuarantineStatus {
    Rejected,
    Held,
    ShadowScored,
    ReplayCandidateEligible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalReviewState {
    Accepted,
    Rejected,
    ReviewRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalAnomalyKind {
    SchemaMismatch,
    ContractMismatch,
    DigestMismatch,
    MissingEnvironmentMetadata,
    StructurallyInconsistentDisagreement,
    UnusualConfidencePattern,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalTrustPosture {
    TrustedSignal,
    Neutral,
    Watch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalIntakeChecks {
    pub schema_conformant: bool,
    pub digest_integrity: bool,
    pub contract_version_match: bool,
    pub benchmark_consistency: bool,
    pub environment_metadata_present: bool,
    pub contributor_identity_linked: bool,
    pub shadow_scored: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalRuntimeReceiptSubmission {
    pub schema_version: String,
    pub submission_id: String,
    pub contributor: CompiledAgentExternalContributorIdentity,
    pub contract_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_receipt: CompiledAgentSourceReceipt,
    pub label: CompiledAgentReceiptSupervisionLabel,
    pub detail: String,
    pub payload_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalReplayProposal {
    pub schema_version: String,
    pub proposal_id: String,
    pub contributor: CompiledAgentExternalContributorIdentity,
    pub contract_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_receipt_id: String,
    pub source_ledger_digest: String,
    pub proposed_samples: Vec<CompiledAgentReplaySample>,
    pub detail: String,
    pub payload_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalSubmissionRecord {
    pub submission_id: String,
    pub submission_kind: CompiledAgentExternalSubmissionKind,
    pub contributor_id: String,
    pub source_machine_id: String,
    pub machine_class: String,
    pub environment_class: String,
    pub contract_version: String,
    pub payload_ref: String,
    pub payload_schema_version: String,
    pub payload_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub validation_checks: CompiledAgentExternalIntakeChecks,
    pub validator_status: CompiledAgentExternalValidatorStatus,
    pub quarantine_status: CompiledAgentExternalQuarantineStatus,
    pub review_state: CompiledAgentExternalReviewState,
    pub quarantined_learning_receipt_ids: Vec<String>,
    pub replay_candidate_receipt_ids: Vec<String>,
    pub proposed_replay_sample_ids: Vec<String>,
    pub review_required_receipt_ids: Vec<String>,
    pub shadow_assessment_ids: Vec<String>,
    pub anomaly_flags: Vec<CompiledAgentExternalAnomalyKind>,
    pub trust_posture: CompiledAgentExternalTrustPosture,
    pub failure_classes: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalContributorTrustHistory {
    pub contributor_id: String,
    pub source_machine_id: String,
    pub accepted_submission_count: u32,
    pub rejected_submission_count: u32,
    pub review_required_submission_count: u32,
    pub replay_candidate_submission_count: u32,
    pub anomaly_count: u32,
    pub trust_posture: CompiledAgentExternalTrustPosture,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalSubmissionStagingLedger {
    pub schema_version: String,
    pub ledger_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub contributor_contract_version: String,
    pub promoted_artifact_contract_digest: String,
    pub total_submission_count: u32,
    pub accepted_submission_count: u32,
    pub rejected_submission_count: u32,
    pub review_required_submission_count: u32,
    pub replay_candidate_submission_count: u32,
    pub anomaly_submission_count: u32,
    pub submissions: Vec<CompiledAgentExternalSubmissionRecord>,
    pub contributor_trust_histories: Vec<CompiledAgentExternalContributorTrustHistory>,
    pub summary: String,
    pub ledger_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalShadowAssessment {
    pub assessment_id: String,
    pub submission_id: String,
    pub module: CompiledAgentModuleKind,
    pub source_receipt_id: String,
    pub promoted_artifact_id: String,
    pub candidate_artifact_id: String,
    pub candidate_label: String,
    pub promoted_output: Value,
    pub candidate_output: Value,
    pub expected_output: Value,
    pub promoted_matches_expected: bool,
    pub candidate_matches_expected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promoted_confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_confidence: Option<f32>,
    pub reason: CompiledAgentDisagreementReason,
    pub disposition: CompiledAgentReviewDisposition,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalQuarantineReport {
    pub schema_version: String,
    pub report_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub staging_ledger_digest: String,
    pub promoted_artifact_contract_digest: String,
    pub accepted_submission_ids: Vec<String>,
    pub rejected_submission_ids: Vec<String>,
    pub review_required_submission_ids: Vec<String>,
    pub replay_candidate_receipt_ids: Vec<String>,
    pub proposed_replay_sample_ids: Vec<String>,
    pub anomaly_submission_ids: Vec<String>,
    pub trusted_signal_contributor_ids: Vec<String>,
    pub watch_contributor_ids: Vec<String>,
    pub shadow_assessments: Vec<CompiledAgentExternalShadowAssessment>,
    pub summary: String,
    pub report_digest: String,
}

impl CompiledAgentExternalRuntimeReceiptSubmission {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.payload_digest.clear();
        stable_digest(
            b"compiled_agent_external_runtime_receipt_submission|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), CompiledAgentExternalIntakeError> {
        let contract = retained_compiled_agent_external_benchmark_kit()?;
        if self.schema_version != COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_SCHEMA_VERSION
        {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("schema_version drifted"),
            });
        }
        if !valid_runtime_submission_id(self.submission_id.as_str()) {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("submission_id drifted"),
            });
        }
        if self.contract_digest != contract.contract_digest {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("contract digest drifted"),
            });
        }
        if self.contributor.contract_version_accepted != contract.contributor_contract_version {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("accepted contributor contract drifted"),
            });
        }
        if !external_metadata_present(&self.contributor) {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("contributor metadata is incomplete"),
            });
        }
        if self
            .source_receipt
            .run
            .internal_trace
            .shadow_phases
            .is_empty()
        {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("runtime disagreement submission lost shadow phases"),
            });
        }
        if self.payload_digest != self.stable_digest() {
            return Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: String::from("payload digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentExternalReplayProposal {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.payload_digest.clear();
        stable_digest(b"compiled_agent_external_replay_proposal|", &clone)
    }

    pub fn validate(&self) -> Result<(), CompiledAgentExternalIntakeError> {
        let contract = retained_compiled_agent_external_benchmark_kit()?;
        if self.schema_version != COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_SCHEMA_VERSION {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.proposal_id != EXTERNAL_REPLAY_PROPOSAL_ID {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("proposal_id drifted"),
            });
        }
        if self.contract_digest != contract.contract_digest {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("contract digest drifted"),
            });
        }
        if self.contributor.contract_version_accepted != contract.contributor_contract_version {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("accepted contributor contract drifted"),
            });
        }
        if !external_metadata_present(&self.contributor) {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("contributor metadata is incomplete"),
            });
        }
        if self.proposed_samples.is_empty() {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("replay proposal must keep at least one sample"),
            });
        }
        if self.payload_digest != self.stable_digest() {
            return Err(CompiledAgentExternalIntakeError::InvalidReplayProposal {
                detail: String::from("payload digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentExternalSubmissionStagingLedger {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.ledger_digest.clear();
        stable_digest(
            b"compiled_agent_external_submission_staging_ledger|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), CompiledAgentExternalIntakeError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_SCHEMA_VERSION {
            return Err(CompiledAgentExternalIntakeError::InvalidStagingLedger {
                detail: String::from("schema_version drifted"),
            });
        }
        if !valid_staging_ledger_id(self.ledger_id.as_str()) {
            return Err(CompiledAgentExternalIntakeError::InvalidStagingLedger {
                detail: String::from("ledger_id drifted"),
            });
        }
        if self.total_submission_count != self.submissions.len() as u32 {
            return Err(CompiledAgentExternalIntakeError::InvalidStagingLedger {
                detail: String::from("submission count drifted"),
            });
        }
        let anomaly_submission_count = self
            .submissions
            .iter()
            .filter(|record| !record.anomaly_flags.is_empty())
            .count() as u32;
        if self.anomaly_submission_count != anomaly_submission_count {
            return Err(CompiledAgentExternalIntakeError::InvalidStagingLedger {
                detail: String::from("anomaly submission count drifted"),
            });
        }
        if self.ledger_digest != self.stable_digest() {
            return Err(CompiledAgentExternalIntakeError::InvalidStagingLedger {
                detail: String::from("ledger digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentExternalQuarantineReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"compiled_agent_external_quarantine_report|", &clone)
    }

    pub fn validate(
        &self,
        staging_ledger: &CompiledAgentExternalSubmissionStagingLedger,
    ) -> Result<(), CompiledAgentExternalIntakeError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_SCHEMA_VERSION {
            return Err(CompiledAgentExternalIntakeError::InvalidQuarantineReport {
                detail: String::from("schema_version drifted"),
            });
        }
        if !valid_quarantine_report_id(self.report_id.as_str()) {
            return Err(CompiledAgentExternalIntakeError::InvalidQuarantineReport {
                detail: String::from("report_id drifted"),
            });
        }
        if self.staging_ledger_digest != staging_ledger.ledger_digest {
            return Err(CompiledAgentExternalIntakeError::InvalidQuarantineReport {
                detail: String::from("staging-ledger linkage drifted"),
            });
        }
        let expected_anomaly_submission_ids = staging_ledger
            .submissions
            .iter()
            .filter(|record| !record.anomaly_flags.is_empty())
            .map(|record| record.submission_id.clone())
            .collect::<Vec<_>>();
        if self.anomaly_submission_ids != expected_anomaly_submission_ids {
            return Err(CompiledAgentExternalIntakeError::InvalidQuarantineReport {
                detail: String::from("anomaly submission ids drifted"),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(CompiledAgentExternalIntakeError::InvalidQuarantineReport {
                detail: String::from("report digest drifted"),
            });
        }
        Ok(())
    }
}

#[must_use]
pub fn compiled_agent_external_runtime_receipt_submission_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_external_replay_proposal_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_external_submission_staging_ledger_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_external_quarantine_report_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_external_runtime_receipt_submission(
) -> Result<CompiledAgentExternalRuntimeReceiptSubmission, CompiledAgentExternalIntakeError> {
    let contributor = canonical_compiled_agent_external_contributor_identity();
    build_compiled_agent_external_runtime_receipt_submission_from_fixture(
        COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH,
        EXTERNAL_RUNTIME_SUBMISSION_ID,
        "openagents_negated_wallet_receipt_v1.json",
        &contributor,
        vec![
            String::from("external"),
            String::from("runtime"),
            String::from("shadow_compare"),
            String::from("negated"),
            String::from("quarantine_training_candidate"),
        ],
        String::from(
            "External runtime disagreement receipt that keeps the promoted baseline route and the shadow learned route side by side on the admitted negated-wallet regression row. It remains review-required, but it is staged as a replay-training candidate instead of a held-out benchmark row.",
        ),
        String::from(
            "This retained external runtime receipt keeps one real admitted-family disagreement in the governed source-receipt shape so it can be shadow-scored before any replay admission.",
        ),
    )
}

pub fn build_compiled_agent_external_runtime_receipt_submission_from_fixture(
    _payload_ref: &str,
    submission_id: &str,
    fixture_name: &str,
    contributor: &CompiledAgentExternalContributorIdentity,
    tags: Vec<String>,
    operator_note: String,
    detail: String,
) -> Result<CompiledAgentExternalRuntimeReceiptSubmission, CompiledAgentExternalIntakeError> {
    let mut submission = CompiledAgentExternalRuntimeReceiptSubmission {
        schema_version: String::from(
            COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_SCHEMA_VERSION,
        ),
        submission_id: submission_id.to_string(),
        contributor: contributor.clone(),
        contract_digest: retained_compiled_agent_external_benchmark_kit()?.contract_digest,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        source_receipt: build_runtime_shadow_compare_receipt_for_fixture(
            fixture_name,
            contributor,
        )?,
        label: {
            let governed = canonical_compiled_agent_governed_source_receipt(fixture_name)?;
            CompiledAgentReceiptSupervisionLabel {
                expected_route: governed.label.expected_route,
                expected_public_response: governed.label.expected_public_response,
                corpus_split: CompiledAgentCorpusSplit::Training,
                tags,
                operator_note,
            }
        },
        detail,
        payload_digest: String::new(),
    };
    submission.payload_digest = submission.stable_digest();
    submission.validate()?;
    Ok(submission)
}

pub fn canonical_compiled_agent_external_replay_proposal(
) -> Result<CompiledAgentExternalReplayProposal, CompiledAgentExternalIntakeError> {
    let contract = retained_compiled_agent_external_benchmark_kit()?;
    let runtime_submission = canonical_compiled_agent_external_runtime_receipt_submission()?;
    let learning_receipt = build_compiled_agent_learning_receipt_from_source(
        COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH,
        &runtime_submission.source_receipt,
        &runtime_submission.label,
    )?;
    let ledger = build_compiled_agent_learning_receipt_ledger_from_receipts(
        vec![learning_receipt.clone()],
        &compiled_agent_baseline_revision_set().revision_id,
    )?;
    let replay_bundle = build_compiled_agent_replay_bundle_from_ledger(&ledger)?;
    let mut proposal = CompiledAgentExternalReplayProposal {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_SCHEMA_VERSION),
        proposal_id: String::from(EXTERNAL_REPLAY_PROPOSAL_ID),
        contributor: runtime_submission.contributor,
        contract_digest: contract.contract_digest,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        source_receipt_id: learning_receipt.receipt_id,
        source_ledger_digest: ledger.ledger_digest,
        proposed_samples: replay_bundle.samples,
        detail: String::from(
            "External replay proposal derived from one quarantined runtime disagreement receipt. The proposal is structurally valid but still requires governed review before any replay admission.",
        ),
        payload_digest: String::new(),
    };
    proposal.payload_digest = proposal.stable_digest();
    proposal.validate()?;
    Ok(proposal)
}

pub fn canonical_compiled_agent_external_submission_staging_ledger(
) -> Result<CompiledAgentExternalSubmissionStagingLedger, CompiledAgentExternalIntakeError> {
    let contract = retained_compiled_agent_external_benchmark_kit()?;
    let promoted_contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let benchmark_run = canonical_compiled_agent_external_benchmark_run()?;
    let runtime_submission = canonical_compiled_agent_external_runtime_receipt_submission()?;
    let replay_proposal = canonical_compiled_agent_external_replay_proposal()?;

    let (benchmark_record, benchmark_review_learning_receipt, benchmark_shadow_ids) =
        build_benchmark_submission_record_with_payload_ref(
            String::from("submission.compiled_agent.external_benchmark.alpha.v1"),
            String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH),
            &benchmark_run,
        )?;
    let invalid_record = build_invalid_benchmark_submission_record(&benchmark_run, &contract)?;
    let (runtime_record, runtime_learning_receipt, runtime_shadow_ids) =
        build_runtime_submission_record_with_payload_ref(
            String::from(COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH),
            &runtime_submission,
        )?;
    let replay_record = build_replay_submission_record(&replay_proposal)?;

    let mut submissions = vec![
        benchmark_record,
        invalid_record,
        runtime_record,
        replay_record,
    ];
    let contributor_trust_histories = apply_external_boundary_posture(&mut submissions);
    let accepted_submission_count = submissions
        .iter()
        .filter(|record| record.review_state != CompiledAgentExternalReviewState::Rejected)
        .count() as u32;
    let rejected_submission_count = submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::Rejected)
        .count() as u32;
    let review_required_submission_count = submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::ReviewRequired)
        .count() as u32;
    let replay_candidate_submission_count = submissions
        .iter()
        .filter(|record| !record.replay_candidate_receipt_ids.is_empty())
        .count() as u32;
    let anomaly_submission_count = submissions
        .iter()
        .filter(|record| !record.anomaly_flags.is_empty())
        .count() as u32;
    let replay_candidate_receipt_count = submissions
        .iter()
        .flat_map(|record| record.replay_candidate_receipt_ids.iter())
        .count();

    let mut ledger = CompiledAgentExternalSubmissionStagingLedger {
        schema_version: String::from(
            COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_SCHEMA_VERSION,
        ),
        ledger_id: String::from(EXTERNAL_STAGING_LEDGER_ID),
        row_id: promoted_contract.row_id,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        contributor_contract_version: contract.contributor_contract_version,
        promoted_artifact_contract_digest: promoted_contract.contract_digest,
        total_submission_count: submissions.len() as u32,
        accepted_submission_count,
        rejected_submission_count,
        review_required_submission_count,
        replay_candidate_submission_count,
        anomaly_submission_count,
        submissions,
        contributor_trust_histories,
        summary: format!(
            "External intake staging ledger retained {} submissions on the admitted compiled-agent family: {} accepted into quarantine, {} rejected before quarantine, {} marked review-required, {} carrying anomaly flags, and {} replay-candidate receipt ids held behind the same governance boundary.",
            4,
            accepted_submission_count,
            rejected_submission_count,
            review_required_submission_count,
            anomaly_submission_count,
            replay_candidate_receipt_count
        ),
        ledger_digest: String::new(),
    };
    let _ = benchmark_review_learning_receipt;
    let _ = runtime_learning_receipt;
    let _ = benchmark_shadow_ids;
    let _ = runtime_shadow_ids;
    ledger.ledger_digest = ledger.stable_digest();
    ledger.validate()?;
    Ok(ledger)
}

pub fn canonical_compiled_agent_external_quarantine_report(
) -> Result<CompiledAgentExternalQuarantineReport, CompiledAgentExternalIntakeError> {
    let staging_ledger = canonical_compiled_agent_external_submission_staging_ledger()?;
    let promoted_contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let benchmark_run = canonical_compiled_agent_external_benchmark_run()?;
    let (_, benchmark_review_learning_receipt, _) =
        build_benchmark_submission_record_with_payload_ref(
            String::from("submission.compiled_agent.external_benchmark.alpha.v1"),
            String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH),
            &benchmark_run,
        )?;
    let runtime_submission = canonical_compiled_agent_external_runtime_receipt_submission()?;
    let (_, runtime_learning_receipt, _) = build_runtime_submission_record_with_payload_ref(
        String::from(COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH),
        &runtime_submission,
    )?;

    let mut shadow_assessments = Vec::new();
    shadow_assessments.extend(build_shadow_assessments(
        "submission.compiled_agent.external_benchmark.alpha.v1",
        &benchmark_review_learning_receipt,
        Some(
            benchmark_run
                .row_runs
                .iter()
                .find(|row| row.row_id == "external.negated_wallet.v1")
                .map(|row| row.source_receipt.clone())
                .expect("canonical negated row must exist"),
        ),
    )?);
    shadow_assessments.extend(build_shadow_assessments(
        EXTERNAL_RUNTIME_SUBMISSION_ID,
        &runtime_learning_receipt,
        Some(runtime_submission.source_receipt),
    )?);
    shadow_assessments.sort_by(|left, right| left.assessment_id.cmp(&right.assessment_id));

    let accepted_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| record.review_state != CompiledAgentExternalReviewState::Rejected)
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let rejected_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::Rejected)
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let review_required_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| record.review_state == CompiledAgentExternalReviewState::ReviewRequired)
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let replay_candidate_receipt_ids = staging_ledger
        .submissions
        .iter()
        .flat_map(|record| record.replay_candidate_receipt_ids.clone())
        .collect::<Vec<_>>();
    let proposed_replay_sample_ids = staging_ledger
        .submissions
        .iter()
        .flat_map(|record| record.proposed_replay_sample_ids.clone())
        .collect::<Vec<_>>();
    let anomaly_submission_ids = staging_ledger
        .submissions
        .iter()
        .filter(|record| !record.anomaly_flags.is_empty())
        .map(|record| record.submission_id.clone())
        .collect::<Vec<_>>();
    let trusted_signal_contributor_ids = staging_ledger
        .contributor_trust_histories
        .iter()
        .filter(|history| history.trust_posture == CompiledAgentExternalTrustPosture::TrustedSignal)
        .map(|history| history.contributor_id.clone())
        .collect::<Vec<_>>();
    let watch_contributor_ids = staging_ledger
        .contributor_trust_histories
        .iter()
        .filter(|history| history.trust_posture == CompiledAgentExternalTrustPosture::Watch)
        .map(|history| history.contributor_id.clone())
        .collect::<Vec<_>>();

    let mut report = CompiledAgentExternalQuarantineReport {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_SCHEMA_VERSION),
        report_id: String::from(EXTERNAL_QUARANTINE_REPORT_ID),
        row_id: staging_ledger.row_id.clone(),
        evidence_class: staging_ledger.evidence_class,
        staging_ledger_digest: staging_ledger.ledger_digest.clone(),
        promoted_artifact_contract_digest: promoted_contract.contract_digest.clone(),
        accepted_submission_ids,
        rejected_submission_ids,
        review_required_submission_ids,
        replay_candidate_receipt_ids,
        proposed_replay_sample_ids,
        anomaly_submission_ids,
        trusted_signal_contributor_ids,
        watch_contributor_ids,
        shadow_assessments,
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "External quarantine retained {} accepted submissions, {} rejected submissions, {} review-required submissions, {} anomaly-flagged submissions, {} trusted-signal contributors, and {} shadow assessments while keeping outside evidence separate from runtime authority.",
        report.accepted_submission_ids.len(),
        report.rejected_submission_ids.len(),
        report.review_required_submission_ids.len(),
        report.anomaly_submission_ids.len(),
        report.trusted_signal_contributor_ids.len(),
        report.shadow_assessments.len(),
    );
    report.report_digest = report.stable_digest();
    report.validate(&staging_ledger)?;
    Ok(report)
}

pub fn write_compiled_agent_external_runtime_receipt_submission(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalRuntimeReceiptSubmission, CompiledAgentExternalIntakeError> {
    write_pretty_json(
        output_path,
        canonical_compiled_agent_external_runtime_receipt_submission,
    )
}

pub fn write_compiled_agent_external_replay_proposal(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalReplayProposal, CompiledAgentExternalIntakeError> {
    write_pretty_json(
        output_path,
        canonical_compiled_agent_external_replay_proposal,
    )
}

pub fn write_compiled_agent_external_submission_staging_ledger(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalSubmissionStagingLedger, CompiledAgentExternalIntakeError> {
    write_pretty_json(
        output_path,
        canonical_compiled_agent_external_submission_staging_ledger,
    )
}

pub fn write_compiled_agent_external_quarantine_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalQuarantineReport, CompiledAgentExternalIntakeError> {
    write_pretty_json(
        output_path,
        canonical_compiled_agent_external_quarantine_report,
    )
}

pub fn verify_compiled_agent_external_intake_fixtures(
) -> Result<(), CompiledAgentExternalIntakeError> {
    verify_fixture(
        compiled_agent_external_runtime_receipt_submission_fixture_path(),
        canonical_compiled_agent_external_runtime_receipt_submission()?,
    )?;
    verify_fixture(
        compiled_agent_external_replay_proposal_fixture_path(),
        canonical_compiled_agent_external_replay_proposal()?,
    )?;
    verify_fixture(
        compiled_agent_external_submission_staging_ledger_fixture_path(),
        canonical_compiled_agent_external_submission_staging_ledger()?,
    )?;
    verify_fixture(
        compiled_agent_external_quarantine_report_fixture_path(),
        canonical_compiled_agent_external_quarantine_report()?,
    )?;
    Ok(())
}

fn detect_submission_anomalies(
    record: &CompiledAgentExternalSubmissionRecord,
) -> Vec<CompiledAgentExternalAnomalyKind> {
    let mut anomalies = Vec::new();
    if !record.validation_checks.schema_conformant {
        anomalies.push(CompiledAgentExternalAnomalyKind::SchemaMismatch);
    }
    if !record.validation_checks.contract_version_match {
        anomalies.push(CompiledAgentExternalAnomalyKind::ContractMismatch);
    }
    if !record.validation_checks.digest_integrity {
        anomalies.push(CompiledAgentExternalAnomalyKind::DigestMismatch);
    }
    if !record.validation_checks.environment_metadata_present {
        anomalies.push(CompiledAgentExternalAnomalyKind::MissingEnvironmentMetadata);
    }
    if record.validation_checks.shadow_scored
        && record.review_state == CompiledAgentExternalReviewState::ReviewRequired
        && record.shadow_assessment_ids.is_empty()
    {
        anomalies.push(CompiledAgentExternalAnomalyKind::StructurallyInconsistentDisagreement);
    }
    if record.failure_classes.iter().any(|failure_class| {
        matches!(
            failure_class.as_str(),
            "grounded_answer_mismatch" | "unsafe_final_outcome"
        )
    }) {
        anomalies.push(CompiledAgentExternalAnomalyKind::UnusualConfidencePattern);
    }
    anomalies
}

pub(crate) fn apply_external_boundary_posture(
    submissions: &mut [CompiledAgentExternalSubmissionRecord],
) -> Vec<CompiledAgentExternalContributorTrustHistory> {
    let mut counts =
        std::collections::BTreeMap::<(String, String), (u32, u32, u32, u32, u32)>::new();
    for record in submissions.iter_mut() {
        record.anomaly_flags = detect_submission_anomalies(record);
        record.trust_posture = CompiledAgentExternalTrustPosture::Neutral;
        let entry = counts
            .entry((
                record.contributor_id.clone(),
                record.source_machine_id.clone(),
            ))
            .or_insert((0, 0, 0, 0, 0));
        if record.review_state != CompiledAgentExternalReviewState::Rejected {
            entry.0 += 1;
        }
        if record.review_state == CompiledAgentExternalReviewState::Rejected {
            entry.1 += 1;
        }
        if record.review_state == CompiledAgentExternalReviewState::ReviewRequired {
            entry.2 += 1;
        }
        if !record.replay_candidate_receipt_ids.is_empty() {
            entry.3 += 1;
        }
        entry.4 += record.anomaly_flags.len() as u32;
    }

    let trust_histories = counts
        .into_iter()
        .map(
            |(
                (contributor_id, source_machine_id),
                (
                    accepted_submission_count,
                    rejected_submission_count,
                    review_required_submission_count,
                    replay_candidate_submission_count,
                    anomaly_count,
                ),
            )| {
                let trust_posture = if rejected_submission_count > 0 || anomaly_count > 1 {
                    CompiledAgentExternalTrustPosture::Watch
                } else if replay_candidate_submission_count > 0 && anomaly_count == 0 {
                    CompiledAgentExternalTrustPosture::TrustedSignal
                } else {
                    CompiledAgentExternalTrustPosture::Neutral
                };
                let detail = match trust_posture {
                    CompiledAgentExternalTrustPosture::TrustedSignal => format!(
                        "Contributor `{contributor_id}` kept contract integrity clean and produced {} replay-candidate submissions without anomaly flags, so the intake path marks it as a bounded trusted signal source without granting any authority.",
                        replay_candidate_submission_count
                    ),
                    CompiledAgentExternalTrustPosture::Watch => format!(
                        "Contributor `{contributor_id}` triggered {} rejected submissions and {} anomaly flags, so the intake path tightens review posture while keeping the same quarantine boundary and no authority expansion.",
                        rejected_submission_count, anomaly_count
                    ),
                    CompiledAgentExternalTrustPosture::Neutral => format!(
                        "Contributor `{contributor_id}` remains neutral while the beta accumulates more accepted bounded evidence."
                    ),
                };
                CompiledAgentExternalContributorTrustHistory {
                    contributor_id,
                    source_machine_id,
                    accepted_submission_count,
                    rejected_submission_count,
                    review_required_submission_count,
                    replay_candidate_submission_count,
                    anomaly_count,
                    trust_posture,
                    detail,
                }
            },
        )
        .collect::<Vec<_>>();

    for record in submissions.iter_mut() {
        if let Some(history) = trust_histories.iter().find(|history| {
            history.contributor_id == record.contributor_id
                && history.source_machine_id == record.source_machine_id
        }) {
            record.trust_posture = history.trust_posture;
        }
    }

    trust_histories
}

pub(crate) fn build_benchmark_submission_record_with_payload_ref(
    submission_id: String,
    payload_ref: String,
    benchmark_run: &CompiledAgentExternalBenchmarkRun,
) -> Result<
    (
        CompiledAgentExternalSubmissionRecord,
        crate::CompiledAgentLearningReceipt,
        Vec<String>,
    ),
    CompiledAgentExternalIntakeError,
> {
    let contract = retained_compiled_agent_external_benchmark_kit()?;
    let schema_conformant =
        benchmark_run.schema_version == crate::COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_SCHEMA_VERSION;
    let digest_integrity = benchmark_run.run_digest == benchmark_run.stable_digest();
    let contract_version_match = benchmark_run.contributor.contract_version_accepted
        == contract.contributor_contract_version;
    let benchmark_consistency = benchmark_run.validate(&contract).is_ok();
    let environment_metadata_present = external_metadata_present(&benchmark_run.contributor);
    let contributor_identity_linked = contributor_identity_linked(&benchmark_run.contributor);

    let mut quarantined_learning_receipt_ids = Vec::new();
    let mut replay_candidate_receipt_ids = Vec::new();
    let mut review_required_receipt_ids = Vec::new();
    let mut review_learning_receipt = None;

    for row_run in &benchmark_run.row_runs {
        let learning_receipt = build_compiled_agent_learning_receipt_from_source(
            format!("{payload_ref}#{}", row_run.row_id).as_str(),
            &row_run.source_receipt,
            &CompiledAgentReceiptSupervisionLabel {
                expected_route: row_run.expected_route,
                expected_public_response: row_run.expected_public_response.clone(),
                corpus_split: row_run.corpus_split,
                tags: row_run.tags.clone(),
                operator_note: row_run.operator_note.clone(),
            },
        )?;
        if row_run.row_id == "external.negated_wallet.v1" {
            review_learning_receipt = Some(learning_receipt.clone());
        }
        quarantined_learning_receipt_ids.push(learning_receipt.receipt_id.clone());
        if row_run.validator_outcome == crate::CompiledAgentExternalValidatorOutcome::Accepted {
            replay_candidate_receipt_ids.push(learning_receipt.receipt_id);
        } else {
            review_required_receipt_ids.push(learning_receipt.receipt_id);
        }
    }
    quarantined_learning_receipt_ids.sort();
    replay_candidate_receipt_ids.sort();
    review_required_receipt_ids.sort();
    let shadow_assessment_ids = vec![
        String::from("assessment.compiled_agent.external_benchmark.negated_wallet.route.v1"),
        String::from(
            "assessment.compiled_agent.external_benchmark.negated_wallet.grounded_answer.v1",
        ),
    ];
    let record = CompiledAgentExternalSubmissionRecord {
        submission_id,
        submission_kind: CompiledAgentExternalSubmissionKind::BenchmarkRun,
        contributor_id: benchmark_run.contributor.contributor_id.clone(),
        source_machine_id: benchmark_run.contributor.source_machine_id.clone(),
        machine_class: benchmark_run.contributor.machine_class.clone(),
        environment_class: benchmark_run.contributor.environment_class.clone(),
        contract_version: benchmark_run.contributor.contract_version_accepted.clone(),
        payload_ref,
        payload_schema_version: benchmark_run.schema_version.clone(),
        payload_digest: benchmark_run.run_digest.clone(),
        evidence_class: benchmark_run.evidence_class,
        validation_checks: CompiledAgentExternalIntakeChecks {
            schema_conformant,
            digest_integrity,
            contract_version_match,
            benchmark_consistency,
            environment_metadata_present,
            contributor_identity_linked,
            shadow_scored: false,
        },
        validator_status: CompiledAgentExternalValidatorStatus::Passed,
        quarantine_status: CompiledAgentExternalQuarantineStatus::Held,
        review_state: CompiledAgentExternalReviewState::ReviewRequired,
        quarantined_learning_receipt_ids,
        replay_candidate_receipt_ids,
        proposed_replay_sample_ids: Vec::new(),
        review_required_receipt_ids,
        shadow_assessment_ids: shadow_assessment_ids.clone(),
        anomaly_flags: Vec::new(),
        trust_posture: CompiledAgentExternalTrustPosture::Neutral,
        failure_classes: vec![String::from("negated_route_false_positive")],
        detail: String::from(
            "The canonical external benchmark submission passed schema, digest, and contract checks, but one held-out negated-wallet row remains review-required. Accepted rows can enter replay-candidate state without letting the review-required row become authority.",
        ),
    };
    Ok((
        record,
        review_learning_receipt.expect("canonical benchmark run must keep the negated review row"),
        shadow_assessment_ids,
    ))
}

fn build_invalid_benchmark_submission_record(
    benchmark_run: &CompiledAgentExternalBenchmarkRun,
    contract: &crate::CompiledAgentExternalBenchmarkKit,
) -> Result<CompiledAgentExternalSubmissionRecord, CompiledAgentExternalIntakeError> {
    let mut tampered = benchmark_run.clone();
    tampered.contract_digest = String::from("tampered-contract-digest");
    tampered.contributor.source_machine_id.clear();
    let schema_conformant =
        tampered.schema_version == crate::COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_SCHEMA_VERSION;
    let digest_integrity = tampered.run_digest == tampered.stable_digest();
    let contract_version_match = tampered.contributor.contract_version_accepted
        == contract.contributor_contract_version
        && tampered.contract_digest == contract.contract_digest;
    let benchmark_consistency = tampered.validate(contract).is_ok();
    let environment_metadata_present = external_metadata_present(&tampered.contributor);
    let contributor_identity_linked = contributor_identity_linked(&tampered.contributor);

    Ok(CompiledAgentExternalSubmissionRecord {
        submission_id: String::from(INVALID_BENCHMARK_SUBMISSION_ID),
        submission_kind: CompiledAgentExternalSubmissionKind::BenchmarkRun,
        contributor_id: tampered.contributor.contributor_id.clone(),
        source_machine_id: tampered.contributor.source_machine_id.clone(),
        machine_class: tampered.contributor.machine_class.clone(),
        environment_class: tampered.contributor.environment_class.clone(),
        contract_version: tampered.contributor.contract_version_accepted.clone(),
        payload_ref: String::from("virtual://compiled_agent_external_benchmark_run_invalid.v1"),
        payload_schema_version: tampered.schema_version.clone(),
        payload_digest: tampered.run_digest.clone(),
        evidence_class: tampered.evidence_class,
        validation_checks: CompiledAgentExternalIntakeChecks {
            schema_conformant,
            digest_integrity,
            contract_version_match,
            benchmark_consistency,
            environment_metadata_present,
            contributor_identity_linked,
            shadow_scored: false,
        },
        validator_status: CompiledAgentExternalValidatorStatus::Failed,
        quarantine_status: CompiledAgentExternalQuarantineStatus::Rejected,
        review_state: CompiledAgentExternalReviewState::Rejected,
        quarantined_learning_receipt_ids: Vec::new(),
        replay_candidate_receipt_ids: Vec::new(),
        proposed_replay_sample_ids: Vec::new(),
        review_required_receipt_ids: Vec::new(),
        shadow_assessment_ids: Vec::new(),
        anomaly_flags: Vec::new(),
        trust_posture: CompiledAgentExternalTrustPosture::Neutral,
        failure_classes: vec![
            String::from("contract_digest_mismatch"),
            String::from("missing_environment_metadata"),
        ],
        detail: String::from(
            "This retained invalid submission proves the intake path rejects tampered contract linkage and missing machine metadata before any replay or training admission, and does so in a machine-legible fail-closed way.",
        ),
    })
}

pub(crate) fn build_runtime_submission_record_with_payload_ref(
    payload_ref: String,
    submission: &CompiledAgentExternalRuntimeReceiptSubmission,
) -> Result<
    (
        CompiledAgentExternalSubmissionRecord,
        crate::CompiledAgentLearningReceipt,
        Vec<String>,
    ),
    CompiledAgentExternalIntakeError,
> {
    submission.validate()?;
    let learning_receipt = build_compiled_agent_learning_receipt_from_source(
        payload_ref.as_str(),
        &submission.source_receipt,
        &submission.label,
    )?;
    let shadow_assessment_ids = vec![
        String::from("assessment.compiled_agent.external_runtime.route.v1"),
        String::from("assessment.compiled_agent.external_runtime.grounded_answer.v1"),
    ];
    let record = CompiledAgentExternalSubmissionRecord {
        submission_id: submission.submission_id.clone(),
        submission_kind: CompiledAgentExternalSubmissionKind::RuntimeReceipt,
        contributor_id: submission.contributor.contributor_id.clone(),
        source_machine_id: submission.contributor.source_machine_id.clone(),
        machine_class: submission.contributor.machine_class.clone(),
        environment_class: submission.contributor.environment_class.clone(),
        contract_version: submission.contributor.contract_version_accepted.clone(),
        payload_ref,
        payload_schema_version: submission.schema_version.clone(),
        payload_digest: submission.payload_digest.clone(),
        evidence_class: submission.evidence_class,
        validation_checks: CompiledAgentExternalIntakeChecks {
            schema_conformant: true,
            digest_integrity: true,
            contract_version_match: true,
            benchmark_consistency: true,
            environment_metadata_present: true,
            contributor_identity_linked: true,
            shadow_scored: true,
        },
        validator_status: CompiledAgentExternalValidatorStatus::Passed,
        quarantine_status: CompiledAgentExternalQuarantineStatus::ShadowScored,
        review_state: CompiledAgentExternalReviewState::ReviewRequired,
        quarantined_learning_receipt_ids: vec![learning_receipt.receipt_id.clone()],
        replay_candidate_receipt_ids: Vec::new(),
        proposed_replay_sample_ids: Vec::new(),
        review_required_receipt_ids: vec![learning_receipt.receipt_id.clone()],
        shadow_assessment_ids: shadow_assessment_ids.clone(),
        anomaly_flags: Vec::new(),
        trust_posture: CompiledAgentExternalTrustPosture::Neutral,
        failure_classes: learning_receipt.assessment.failure_classes.clone(),
        detail: String::from(
            "The external runtime disagreement receipt passed intake validation, was retained in quarantine, and was immediately shadow-scored against the promoted-versus-candidate route and grounded modules before any replay admission.",
        ),
    };
    Ok((record, learning_receipt, shadow_assessment_ids))
}

fn build_replay_submission_record(
    proposal: &CompiledAgentExternalReplayProposal,
) -> Result<CompiledAgentExternalSubmissionRecord, CompiledAgentExternalIntakeError> {
    proposal.validate()?;
    let mut proposed_replay_sample_ids = proposal
        .proposed_samples
        .iter()
        .map(|sample| sample.sample_id.clone())
        .collect::<Vec<_>>();
    proposed_replay_sample_ids.sort();
    Ok(CompiledAgentExternalSubmissionRecord {
        submission_id: proposal.proposal_id.clone(),
        submission_kind: CompiledAgentExternalSubmissionKind::ReplayProposal,
        contributor_id: proposal.contributor.contributor_id.clone(),
        source_machine_id: proposal.contributor.source_machine_id.clone(),
        machine_class: proposal.contributor.machine_class.clone(),
        environment_class: proposal.contributor.environment_class.clone(),
        contract_version: proposal.contributor.contract_version_accepted.clone(),
        payload_ref: String::from(COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH),
        payload_schema_version: proposal.schema_version.clone(),
        payload_digest: proposal.payload_digest.clone(),
        evidence_class: proposal.evidence_class,
        validation_checks: CompiledAgentExternalIntakeChecks {
            schema_conformant: true,
            digest_integrity: true,
            contract_version_match: true,
            benchmark_consistency: true,
            environment_metadata_present: true,
            contributor_identity_linked: true,
            shadow_scored: false,
        },
        validator_status: CompiledAgentExternalValidatorStatus::Passed,
        quarantine_status: CompiledAgentExternalQuarantineStatus::Held,
        review_state: CompiledAgentExternalReviewState::ReviewRequired,
        quarantined_learning_receipt_ids: Vec::new(),
        replay_candidate_receipt_ids: Vec::new(),
        proposed_replay_sample_ids,
        review_required_receipt_ids: Vec::new(),
        shadow_assessment_ids: Vec::new(),
        anomaly_flags: Vec::new(),
        trust_posture: CompiledAgentExternalTrustPosture::Neutral,
        failure_classes: Vec::new(),
        detail: String::from(
            "The external replay proposal passed schema, digest, and lineage checks, but it stays review-required until the quarantined runtime disagreement row is explicitly accepted into the replay set.",
        ),
    })
}

pub(crate) fn build_shadow_assessments(
    submission_id: &str,
    learning_receipt: &crate::CompiledAgentLearningReceipt,
    source_receipt: Option<CompiledAgentSourceReceipt>,
) -> Result<Vec<CompiledAgentExternalShadowAssessment>, CompiledAgentExternalIntakeError> {
    let contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let route_promoted = promoted_entry(&contract, CompiledAgentModuleKind::Route)?;
    let route_candidate = candidate_entry(&contract, CompiledAgentModuleKind::Route)?;
    let grounded_promoted = promoted_entry(&contract, CompiledAgentModuleKind::GroundedAnswer)?;
    let grounded_candidate = candidate_entry(&contract, CompiledAgentModuleKind::GroundedAnswer)?;
    let source_receipt = source_receipt.expect("shadow assessment source receipt must exist");

    let (candidate_route, candidate_route_confidence) =
        candidate_route_prediction(route_candidate, learning_receipt.user_request.as_str());
    let route_assessment = build_route_shadow_assessment(
        submission_id,
        learning_receipt,
        route_promoted,
        route_candidate,
        candidate_route,
        candidate_route_confidence,
    );
    let candidate_grounded = candidate_grounded_output(
        grounded_candidate,
        candidate_route,
        &tool_results_for_route(candidate_route, &source_receipt.state),
    );
    let grounded_assessment = build_grounded_shadow_assessment(
        submission_id,
        learning_receipt,
        grounded_promoted,
        grounded_candidate,
        candidate_route,
        candidate_grounded,
    );
    Ok(vec![route_assessment, grounded_assessment])
}

fn build_route_shadow_assessment(
    submission_id: &str,
    learning_receipt: &crate::CompiledAgentLearningReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    candidate_route: CompiledAgentRoute,
    candidate_confidence: Option<f32>,
) -> CompiledAgentExternalShadowAssessment {
    let promoted_route = learning_receipt.observed_route;
    let promoted_matches_expected = promoted_route == learning_receipt.expected_route;
    let candidate_matches_expected = candidate_route == learning_receipt.expected_route;
    let (reason, disposition) =
        disagreement_reason(promoted_matches_expected, candidate_matches_expected);
    CompiledAgentExternalShadowAssessment {
        assessment_id: shadow_assessment_id(submission_id, "route"),
        submission_id: submission_id.to_string(),
        module: CompiledAgentModuleKind::Route,
        source_receipt_id: learning_receipt.receipt_id.clone(),
        promoted_artifact_id: promoted_entry.artifact_id.clone(),
        candidate_artifact_id: candidate_entry.artifact_id.clone(),
        candidate_label: candidate_entry
            .candidate_label
            .clone()
            .unwrap_or_else(|| String::from("candidate")),
        promoted_output: json!({ "route": promoted_route }),
        candidate_output: json!({ "route": candidate_route }),
        expected_output: json!({ "route": learning_receipt.expected_route }),
        promoted_matches_expected,
        candidate_matches_expected,
        promoted_confidence: learning_receipt
            .primary_phase_confidences
            .get("intent_route")
            .copied(),
        candidate_confidence,
        reason,
        disposition,
        detail: format!(
            "External intake shadow-scored route authority for `{}` against promoted `{}` and candidate `{}` before any replay admission.",
            learning_receipt.receipt_id, promoted_entry.artifact_id, candidate_entry.artifact_id
        ),
    }
}

fn build_grounded_shadow_assessment(
    submission_id: &str,
    learning_receipt: &crate::CompiledAgentLearningReceipt,
    promoted_entry: &CompiledAgentArtifactContractEntry,
    candidate_entry: &CompiledAgentArtifactContractEntry,
    candidate_route: CompiledAgentRoute,
    candidate_grounded: CandidateGroundedOutput,
) -> CompiledAgentExternalShadowAssessment {
    let promoted_matches_expected = learning_receipt.observed_public_response.kind
        == learning_receipt.expected_public_response.kind
        && learning_receipt.observed_public_response.response
            == learning_receipt.expected_public_response.response;
    let candidate_matches_expected = candidate_grounded.kind
        == learning_receipt.expected_public_response.kind
        && candidate_grounded.response == learning_receipt.expected_public_response.response;
    let (reason, disposition) =
        disagreement_reason(promoted_matches_expected, candidate_matches_expected);
    CompiledAgentExternalShadowAssessment {
        assessment_id: shadow_assessment_id(submission_id, "grounded_answer"),
        submission_id: submission_id.to_string(),
        module: CompiledAgentModuleKind::GroundedAnswer,
        source_receipt_id: learning_receipt.receipt_id.clone(),
        promoted_artifact_id: promoted_entry.artifact_id.clone(),
        candidate_artifact_id: candidate_entry.artifact_id.clone(),
        candidate_label: candidate_entry
            .candidate_label
            .clone()
            .unwrap_or_else(|| String::from("candidate")),
        promoted_output: json!({
            "kind": learning_receipt.observed_public_response.kind,
            "response": learning_receipt.observed_public_response.response,
        }),
        candidate_output: json!({
            "route": candidate_route,
            "kind": candidate_grounded.kind,
            "response": candidate_grounded.response,
        }),
        expected_output: json!({
            "kind": learning_receipt.expected_public_response.kind,
            "response": learning_receipt.expected_public_response.response,
        }),
        promoted_matches_expected,
        candidate_matches_expected,
        promoted_confidence: learning_receipt
            .primary_phase_confidences
            .get("grounded_answer")
            .copied(),
        candidate_confidence: candidate_grounded.confidence,
        reason,
        disposition,
        detail: format!(
            "External intake shadow-scored grounded synthesis for `{}` against promoted `{}` and candidate `{}` before any replay admission.",
            learning_receipt.receipt_id, promoted_entry.artifact_id, candidate_entry.artifact_id
        ),
    }
}

pub(crate) fn build_runtime_shadow_compare_receipt_for_fixture(
    fixture_name: &str,
    contributor: &CompiledAgentExternalContributorIdentity,
) -> Result<CompiledAgentSourceReceipt, CompiledAgentExternalIntakeError> {
    let governed = canonical_compiled_agent_governed_source_receipt(fixture_name)?;
    let row_id = fixture_name.trim_end_matches(".json").replace('.', "_");
    build_runtime_shadow_compare_receipt(&governed.source_receipt, row_id.as_str(), contributor)
}

pub(crate) fn build_runtime_shadow_compare_receipt(
    promoted_receipt: &CompiledAgentSourceReceipt,
    row_id: &str,
    contributor: &CompiledAgentExternalContributorIdentity,
) -> Result<CompiledAgentSourceReceipt, CompiledAgentExternalIntakeError> {
    let contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let route_candidate = candidate_entry(&contract, CompiledAgentModuleKind::Route)?;
    let grounded_candidate = candidate_entry(&contract, CompiledAgentModuleKind::GroundedAnswer)?;
    let tool_policy_promoted = promoted_entry(&contract, CompiledAgentModuleKind::ToolPolicy)?;
    let tool_arguments_promoted =
        promoted_entry(&contract, CompiledAgentModuleKind::ToolArguments)?;
    let verify_promoted = promoted_entry(&contract, CompiledAgentModuleKind::Verify)?;

    let prompt = promoted_receipt.run.lineage.user_request.as_str();
    let (candidate_route, route_confidence) = candidate_route_prediction(route_candidate, prompt);
    let candidate_selected_tools =
        evaluate_compiled_agent_tool_policy(candidate_route, &compiled_agent_supported_tools());
    let candidate_selected_tool_names = candidate_selected_tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<Vec<_>>();
    let candidate_tool_calls =
        evaluate_compiled_agent_tool_arguments(&candidate_selected_tool_names);
    let candidate_tool_results = tool_results_for_route(candidate_route, &promoted_receipt.state);
    let candidate_grounded =
        candidate_grounded_output(grounded_candidate, candidate_route, &candidate_tool_results);
    let verify_revision = revision_from_entry(verify_promoted)?;
    let verify_verdict = evaluate_compiled_agent_verify(
        candidate_route,
        &candidate_selected_tool_names,
        &candidate_tool_results,
        &candidate_grounded.response,
        verify_revision,
    );

    let mut receipt = promoted_receipt.clone();
    let shadow_phases = vec![
        phase_trace_entry(
            "intent_route",
            route_candidate,
            "shadow",
            route_candidate.candidate_label.clone(),
            json!({ "user_request": prompt }),
            json!({ "route": candidate_route }),
            route_confidence.unwrap_or(0.77),
            json!({
                "artifact_id": route_candidate.artifact_id,
                "artifact_digest": route_candidate.artifact_digest,
                "captured_from_external_runtime": true,
                "row_id": row_id,
            }),
        ),
        phase_trace_entry(
            "tool_policy",
            tool_policy_promoted,
            "shadow",
            route_candidate.candidate_label.clone(),
            json!({
                "user_request": prompt,
                "route": candidate_route,
                "available_tools": compiled_agent_supported_tools(),
            }),
            json!({
                "selected_tools": candidate_selected_tools
                    .iter()
                    .map(|tool| json!({
                        "name": tool.name,
                        "description": tool.description,
                    }))
                    .collect::<Vec<_>>(),
            }),
            0.9,
            json!({
                "captured_from_external_runtime": true,
                "row_id": row_id,
            }),
        ),
        phase_trace_entry(
            "tool_arguments",
            tool_arguments_promoted,
            "shadow",
            route_candidate.candidate_label.clone(),
            json!({
                "user_request": prompt,
                "route": candidate_route,
                "selected_tools": candidate_selected_tool_names,
            }),
            json!({ "calls": candidate_tool_calls }),
            0.92,
            json!({
                "captured_from_external_runtime": true,
                "row_id": row_id,
            }),
        ),
        phase_trace_entry(
            "grounded_answer",
            grounded_candidate,
            "shadow",
            grounded_candidate.candidate_label.clone(),
            json!({
                "user_request": prompt,
                "route": candidate_route,
                "tool_results": candidate_tool_results,
            }),
            json!({
                "answer": candidate_grounded.response,
                "response_kind": candidate_grounded.kind,
            }),
            candidate_grounded.confidence.unwrap_or(0.78),
            json!({
                "captured_from_external_runtime": true,
                "row_id": row_id,
            }),
        ),
        phase_trace_entry(
            "verify",
            verify_promoted,
            "shadow",
            route_candidate.candidate_label.clone(),
            json!({
                "user_request": prompt,
                "route": candidate_route,
                "tool_calls": candidate_tool_calls,
                "tool_results": candidate_tool_results,
                "candidate_answer": candidate_grounded.response,
            }),
            json!({
                "verdict": verify_label(verify_verdict),
            }),
            0.81,
            json!({
                "captured_from_external_runtime": true,
                "row_id": row_id,
            }),
        ),
    ];
    receipt.run.internal_trace.shadow_phases = shadow_phases.clone();
    receipt.run.lineage.shadow_manifest_ids = shadow_phases
        .iter()
        .map(|phase| phase.manifest.manifest_id())
        .collect();
    receipt.captured_at_epoch_ms = contributor.attested_at_epoch_ms + 11;
    Ok(receipt)
}

fn candidate_route_prediction(
    entry: &CompiledAgentArtifactContractEntry,
    prompt: &str,
) -> (CompiledAgentRoute, Option<f32>) {
    match &entry.payload {
        CompiledAgentArtifactPayload::RouteModel { artifact } => {
            let prediction = predict_compiled_agent_route(artifact, prompt);
            (prediction.route, Some(prediction.confidence))
        }
        CompiledAgentArtifactPayload::RevisionSet { revision } => {
            (evaluate_compiled_agent_route(prompt, revision), None)
        }
        // A coordinator-head candidate is not scored by the prompt-route intake
        // path (its gate is the coordinator `ShadowComparison`). Fall back to the
        // deterministic baseline route with no model confidence.
        CompiledAgentArtifactPayload::CoordinatorHead { .. } => (
            evaluate_compiled_agent_route(prompt, &compiled_agent_baseline_revision_set()),
            None,
        ),
    }
}

struct CandidateGroundedOutput {
    kind: CompiledAgentPublicOutcomeKind,
    response: String,
    confidence: Option<f32>,
}

fn candidate_grounded_output(
    entry: &CompiledAgentArtifactContractEntry,
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> CandidateGroundedOutput {
    let revision = revision_from_entry(entry).expect("grounded entry must carry a revision");
    if let Some(artifact) = revision.grounded_answer_model_artifact.as_ref() {
        let prediction = predict_compiled_agent_grounded_answer(artifact, route, tool_results);
        return CandidateGroundedOutput {
            kind: prediction.outcome_kind,
            response: prediction.response,
            confidence: Some(prediction.confidence),
        };
    }
    CandidateGroundedOutput {
        kind: if route == CompiledAgentRoute::Unsupported {
            CompiledAgentPublicOutcomeKind::UnsupportedRefusal
        } else {
            CompiledAgentPublicOutcomeKind::GroundedAnswer
        },
        response: evaluate_compiled_agent_grounded_answer(route, tool_results, revision),
        confidence: Some(0.78),
    }
}

fn tool_results_for_route(
    route: CompiledAgentRoute,
    runtime_state: &CompiledAgentRuntimeState,
) -> Vec<CompiledAgentToolResult> {
    match route {
        CompiledAgentRoute::ProviderStatus => vec![CompiledAgentToolResult {
            tool_name: String::from("provider_status"),
            payload: json!({
                "ready": runtime_state.provider_ready,
                "blockers": runtime_state.provider_blockers,
            }),
        }],
        CompiledAgentRoute::WalletStatus => vec![CompiledAgentToolResult {
            tool_name: String::from("wallet_status"),
            payload: json!({
                "balance_sats": runtime_state.wallet_balance_sats,
                "recent_earnings_sats": runtime_state.recent_earnings_sats,
            }),
        }],
        CompiledAgentRoute::Unsupported => Vec::new(),
    }
}

fn disagreement_reason(
    promoted_matches_expected: bool,
    candidate_matches_expected: bool,
) -> (
    CompiledAgentDisagreementReason,
    CompiledAgentReviewDisposition,
) {
    if !promoted_matches_expected && candidate_matches_expected {
        (
            CompiledAgentDisagreementReason::LowConfidenceDisagreement,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    } else if promoted_matches_expected && !candidate_matches_expected {
        (
            CompiledAgentDisagreementReason::CandidateRegression,
            CompiledAgentReviewDisposition::ShadowOnly,
        )
    } else {
        (
            CompiledAgentDisagreementReason::AmbiguousRegression,
            CompiledAgentReviewDisposition::HumanReviewRequired,
        )
    }
}

fn promoted_entry(
    contract: &crate::CompiledAgentPromotedArtifactContract,
    module: CompiledAgentModuleKind,
) -> Result<&CompiledAgentArtifactContractEntry, CompiledAgentExternalIntakeError> {
    contract.promoted_entry(module).ok_or_else(|| {
        CompiledAgentExternalIntakeError::MissingArtifactEntry {
            module: format!("{module:?}"),
            lifecycle: String::from("promoted"),
        }
    })
}

fn candidate_entry(
    contract: &crate::CompiledAgentPromotedArtifactContract,
    module: CompiledAgentModuleKind,
) -> Result<&CompiledAgentArtifactContractEntry, CompiledAgentExternalIntakeError> {
    contract
        .artifacts
        .iter()
        .find(|entry| {
            entry.module == module
                && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Candidate
        })
        .ok_or_else(|| CompiledAgentExternalIntakeError::MissingArtifactEntry {
            module: format!("{module:?}"),
            lifecycle: String::from("candidate"),
        })
}

fn revision_from_entry(
    entry: &CompiledAgentArtifactContractEntry,
) -> Result<&psionic_eval::CompiledAgentModuleRevisionSet, CompiledAgentExternalIntakeError> {
    match &entry.payload {
        CompiledAgentArtifactPayload::RevisionSet { revision } => Ok(revision),
        CompiledAgentArtifactPayload::RouteModel { .. } => {
            Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: format!(
                    "module `{}` does not carry a revision-set payload",
                    entry.module_name
                ),
            })
        }
        CompiledAgentArtifactPayload::CoordinatorHead { .. } => {
            Err(CompiledAgentExternalIntakeError::InvalidRuntimeSubmission {
                detail: format!(
                    "module `{}` carries a coordinator-head payload, not a revision-set payload",
                    entry.module_name
                ),
            })
        }
    }
}

fn phase_trace_entry(
    phase: &str,
    entry: &CompiledAgentArtifactContractEntry,
    authority: &str,
    candidate_label: Option<String>,
    input: Value,
    output: Value,
    confidence: f32,
    trace: Value,
) -> CompiledAgentSourcePhaseTraceEntry {
    CompiledAgentSourcePhaseTraceEntry {
        phase: String::from(phase),
        manifest: CompiledAgentSourceManifest {
            module_name: entry.module_name.clone(),
            signature_name: entry.signature_name.clone(),
            implementation_family: entry.implementation_family.clone(),
            implementation_label: entry.implementation_label.clone(),
            version: entry.version.clone(),
            promotion_state: match entry.lifecycle_state {
                CompiledAgentArtifactLifecycleState::Promoted => String::from("promoted"),
                CompiledAgentArtifactLifecycleState::Candidate => String::from("candidate"),
            },
            confidence_floor: entry.confidence_floor,
        },
        authority: String::from(authority),
        candidate_label,
        input,
        output,
        confidence,
        trace,
    }
}

fn verify_label(verdict: CompiledAgentVerifyVerdict) -> &'static str {
    match verdict {
        CompiledAgentVerifyVerdict::AcceptGroundedAnswer => "accept_grounded_answer",
        CompiledAgentVerifyVerdict::UnsupportedRefusal => "unsupported_refusal",
        CompiledAgentVerifyVerdict::NeedsFallback => "needs_fallback",
    }
}

fn external_metadata_present(contributor: &CompiledAgentExternalContributorIdentity) -> bool {
    !contributor.source_machine_id.is_empty()
        && !contributor.machine_class.is_empty()
        && !contributor.environment_class.is_empty()
}

fn contributor_identity_linked(contributor: &CompiledAgentExternalContributorIdentity) -> bool {
    !contributor.contributor_id.is_empty() && !contributor.display_name.is_empty()
}

fn write_pretty_json<T, F>(
    output_path: impl AsRef<Path>,
    build: F,
) -> Result<T, CompiledAgentExternalIntakeError>
where
    T: Serialize,
    F: FnOnce() -> Result<T, CompiledAgentExternalIntakeError>,
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentExternalIntakeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let value = build()?;
    let json = serde_json::to_string_pretty(&value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentExternalIntakeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(value)
}

fn verify_fixture<T>(path: PathBuf, expected: T) -> Result<(), CompiledAgentExternalIntakeError>
where
    T: for<'de> Deserialize<'de> + PartialEq,
{
    let bytes = fs::read(&path).map_err(|error| CompiledAgentExternalIntakeError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let committed: T = serde_json::from_slice(&bytes)?;
    if committed != expected {
        return Err(CompiledAgentExternalIntakeError::FixtureDrift {
            path: path.display().to_string(),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("stable digest serialization must succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

fn valid_runtime_submission_id(submission_id: &str) -> bool {
    !submission_id.is_empty() && submission_id.starts_with("submission.compiled_agent.")
}

fn valid_staging_ledger_id(ledger_id: &str) -> bool {
    ledger_id == EXTERNAL_STAGING_LEDGER_ID
        || ledger_id.starts_with("compiled_agent.tailnet_submission_staging_ledger.")
}

fn valid_quarantine_report_id(report_id: &str) -> bool {
    report_id == EXTERNAL_QUARANTINE_REPORT_ID
        || report_id.starts_with("compiled_agent.tailnet_quarantine_report.")
}

fn shadow_assessment_id(submission_id: &str, module_label: &str) -> String {
    let normalized = submission_id.replace('.', "_");
    format!("assessment.{normalized}.{module_label}.v1")
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_external_quarantine_report,
        canonical_compiled_agent_external_replay_proposal,
        canonical_compiled_agent_external_runtime_receipt_submission,
        canonical_compiled_agent_external_submission_staging_ledger,
        verify_compiled_agent_external_intake_fixtures,
    };

    #[test]
    fn external_runtime_submission_is_valid() -> Result<(), Box<dyn std::error::Error>> {
        let submission = canonical_compiled_agent_external_runtime_receipt_submission()?;
        submission.validate()?;
        assert!(!submission
            .source_receipt
            .run
            .internal_trace
            .shadow_phases
            .is_empty());
        Ok(())
    }

    #[test]
    fn external_replay_proposal_is_valid() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = canonical_compiled_agent_external_replay_proposal()?;
        proposal.validate()?;
        assert_eq!(proposal.proposed_samples.len(), 2);
        Ok(())
    }

    #[test]
    fn external_staging_ledger_rejects_tampered_submission(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ledger = canonical_compiled_agent_external_submission_staging_ledger()?;
        ledger.validate()?;
        let rejected = ledger
            .submissions
            .iter()
            .find(|record| {
                record.submission_id
                    == "submission.compiled_agent.external_benchmark_invalid.alpha.v1"
            })
            .expect("invalid submission record should exist");
        assert_eq!(
            rejected.review_state,
            crate::CompiledAgentExternalReviewState::Rejected
        );
        assert!(rejected
            .failure_classes
            .iter()
            .any(|class| class == "contract_digest_mismatch"));
        Ok(())
    }

    #[test]
    fn external_quarantine_report_keeps_runtime_shadow_assessments(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = canonical_compiled_agent_external_quarantine_report()?;
        assert_eq!(report.shadow_assessments.len(), 4);
        assert!(report
            .review_required_submission_ids
            .iter()
            .any(|submission_id| submission_id
                == "submission.compiled_agent.external_runtime_disagreement.alpha.v1"));
        Ok(())
    }

    #[test]
    fn committed_external_intake_fixtures_match_canonical_output(
    ) -> Result<(), Box<dyn std::error::Error>> {
        verify_compiled_agent_external_intake_fixtures()?;
        Ok(())
    }
}
