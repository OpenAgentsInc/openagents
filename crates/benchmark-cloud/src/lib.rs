use serde::{Deserialize, Serialize};
use serde_json::json;

pub const BENCHMARK_TASK_SCHEMA_REF: &str = "openagents.benchmark_task.v1";
pub const BENCHMARK_RESULT_SCHEMA_REF: &str = "openagents.benchmark_result.v1";
pub const BENCHMARK_EVENT_SCHEMA_REF: &str = "openagents.benchmark_event.v1";
pub const BENCHMARK_ARTIFACT_MANIFEST_SCHEMA_REF: &str =
    "openagents.benchmark_artifact_manifest.v1";
pub const BENCHMARK_PROOF_BUNDLE_SCHEMA_REF: &str = "openagents.benchmark_proof_bundle.v1";
pub const RESOURCE_USAGE_RECEIPT_SCHEMA_REF: &str = "openagents.resource_usage_receipt.v1";
pub const BENCHMARK_SPLIT_MANIFEST_SCHEMA_REF: &str = "openagents.benchmark_split_manifest.v1";
pub const BENCHMARK_RUN_MANIFEST_SCHEMA_REF: &str = "openagents.benchmark_run_manifest.v1";
pub const BENCHMARK_CAMPAIGN_SPLIT_MANIFEST_SCHEMA_REF: &str =
    "openagents.benchmark_campaign_split_manifest.v1";
pub const PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF: &str = "probe.benchmark_assignment.v1";

pub const PROBE_RUNNER_REQUIRED_ARTIFACT_FILES: [&str; 8] = [
    "result.json",
    "events.jsonl",
    "metadata.json",
    "artifact_manifest.json",
    "proof_bundle.json",
    "resource_usage_receipt.json",
    "probe-run-record.json",
    "probe-closeout.json",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkEvidenceSplit {
    Retained,
    Validation,
    Holdout,
    Live,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    PolicyBlocked,
    Errored,
}

impl BenchmarkRunStatus {
    pub fn is_terminal_failure(&self) -> bool {
        matches!(
            self,
            Self::Failed | Self::TimedOut | Self::PolicyBlocked | Self::Errored
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkRedactionState {
    PublicSafe,
    Redacted,
    Withheld,
    UnsafeBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkEventKind {
    Assigned,
    Started,
    Progress,
    ArtifactRecorded,
    ProofRecorded,
    Completed,
    Failed,
    TimedOut,
    PolicyBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkPublicClaimLevel {
    None,
    RetainedSummary,
    ValidationSummary,
    HoldoutSummary,
    LiveClaim,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSplitLane {
    RetainedFixture,
    Validation,
    FrozenHoldout,
    LocalSmokeFixture,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkDatasetRef {
    pub slug: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScorerVerifierRef {
    pub scorer_ref: String,
    pub verifier_ref: String,
    pub verifier_public_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NoCheatMetadata {
    pub policy_ref: String,
    pub no_hidden_verifier_content: bool,
    pub no_private_repo_refs: bool,
    pub no_raw_logs: bool,
    pub no_task_solution_material: bool,
}

impl NoCheatMetadata {
    pub fn is_public_safe(&self) -> bool {
        self.no_hidden_verifier_content
            && self.no_private_repo_refs
            && self.no_raw_logs
            && self.no_task_solution_material
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkTask {
    pub schema_ref: String,
    pub task_ref: String,
    pub task_run_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub benchmark_suite_ref: String,
    pub split_ref: String,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub public_task_checksum: String,
    pub runner_ref: String,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEvent {
    pub schema_ref: String,
    pub event_ref: String,
    pub run_ref: String,
    pub task_run_ref: String,
    pub event_kind: BenchmarkEventKind,
    pub observed_at: String,
    pub artifact_refs: Vec<String>,
    pub proof_bundle_refs: Vec<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkArtifactRef {
    pub artifact_ref: String,
    pub digest: String,
    pub media_type: String,
    pub public_url_ref: Option<String>,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkArtifactManifest {
    pub schema_ref: String,
    pub manifest_ref: String,
    pub run_ref: String,
    pub artifacts: Vec<BenchmarkArtifactRef>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkProofBundle {
    pub schema_ref: String,
    pub proof_bundle_ref: String,
    pub run_ref: String,
    pub artifact_manifest_refs: Vec<String>,
    pub scorer_verifier: ScorerVerifierRef,
    pub resource_usage_receipt_ref: Option<String>,
    pub probe_selected_signature_refs: Vec<String>,
    pub probe_tool_menu_ref: Option<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceTokenUsage {
    pub completion_tokens: Option<u64>,
    pub prompt_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub truth: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceUsageReceipt {
    pub schema_ref: String,
    pub receipt_ref: String,
    pub run_ref: String,
    pub worker_ref: String,
    pub duration_ms: Option<u64>,
    pub cpu_ms: Option<u64>,
    pub gpu_ms: Option<u64>,
    pub memory_peak_bytes: Option<u64>,
    pub token_usage: Option<ResourceTokenUsage>,
    pub cost_ref: Option<String>,
    pub unavailable_reason: Option<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSplitManifest {
    pub schema_ref: String,
    pub manifest_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub split_ref: String,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub task_refs: Vec<String>,
    pub frozen: bool,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRunManifest {
    pub schema_ref: String,
    pub run_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub split_manifest_refs: Vec<String>,
    pub agent_slug: String,
    pub candidate_hash: Option<String>,
    pub probe_commit: Option<String>,
    pub retry_policy_ref: String,
    pub scorer_verifier: ScorerVerifierRef,
    pub started_at: String,
    pub timeout_policy_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSplitTaskEntry {
    pub task_ref: String,
    pub task_id: String,
    pub public_task_checksum: String,
    pub lane: BenchmarkSplitLane,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub scorer_verifier: ScorerVerifierRef,
    pub allowed_claim_level: BenchmarkPublicClaimLevel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkCampaignSplitManifest {
    pub schema_ref: String,
    pub manifest_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub task_selector_version: String,
    pub task_order_ref: String,
    pub allowed_claim_state: BenchmarkPublicClaimLevel,
    pub retained_fixture_refs: Vec<String>,
    pub validation_task_refs: Vec<String>,
    pub frozen_holdout_task_refs: Vec<String>,
    pub local_smoke_fixture_refs: Vec<String>,
    pub scorer_verifier_versions: Vec<ScorerVerifierRef>,
    pub tasks: Vec<BenchmarkSplitTaskEntry>,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeBenchmarkAssignment {
    pub schema_ref: String,
    pub assignment_ref: String,
    pub benchmark_run_ref: String,
    pub task_run_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub split_ref: String,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub task_ref: String,
    pub public_task_checksum: String,
    pub probe_commit: String,
    pub runtime_ref: String,
    pub backend_profile_ref: String,
    pub backend_ref: String,
    pub model_backend_ref: String,
    pub provider_account_ref: Option<String>,
    pub auth_grant_ref: Option<String>,
    pub selected_blueprint_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub candidate_hash: String,
    pub timeout_policy_ref: String,
    pub budget_policy_ref: String,
    pub required_artifact_refs: Vec<String>,
    pub required_proof_bundle_refs: Vec<String>,
    pub callback_refs: Vec<String>,
    pub proof_sink_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeCommandInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub assignment_stdin_json: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeFakeRunnerOutcome {
    Pass,
    Timeout,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeBenchmarkRunMetadata {
    pub runner_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub task_ref: String,
    pub probe_command: ProbeCommandInvocation,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub safe_account_grant_refs: bool,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeBenchmarkRunnerArtifactSet {
    pub result_json: BenchmarkResult,
    pub events_jsonl: Vec<BenchmarkEvent>,
    pub metadata_json: ProbeBenchmarkRunMetadata,
    pub artifact_manifest_json: BenchmarkArtifactManifest,
    pub proof_bundle_json: BenchmarkProofBundle,
    pub resource_usage_receipt_json: ResourceUsageReceipt,
    pub probe_run_record_json: serde_json::Value,
    pub probe_closeout_json: serde_json::Value,
}

impl ProbeBenchmarkRunnerArtifactSet {
    pub fn file_names(&self) -> Vec<&'static str> {
        PROBE_RUNNER_REQUIRED_ARTIFACT_FILES.to_vec()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeCloseoutImport {
    pub probe_assignment_ref: String,
    pub probe_closeout_ref: String,
    pub probe_commit: String,
    pub probe_run_record_ref: String,
    pub probe_run_ref: String,
    pub candidate_hash: String,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkPublicClaimBoundary {
    pub claim_level: BenchmarkPublicClaimLevel,
    pub external_release_gate_refs: Vec<String>,
    pub public_claim_upgrade_authority: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub schema_ref: String,
    pub result_ref: String,
    pub run_ref: String,
    pub task_run_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub status: BenchmarkRunStatus,
    pub scorer_verifier: ScorerVerifierRef,
    pub score_bps: Option<u32>,
    pub failure_classification_ref: Option<String>,
    pub artifact_manifest_refs: Vec<String>,
    pub proof_bundle_refs: Vec<String>,
    pub resource_usage_receipt_ref: Option<String>,
    pub resource_unavailable_reason: Option<String>,
    pub probe_closeout_import: Option<ProbeCloseoutImport>,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
    pub public_claim_boundary: BenchmarkPublicClaimBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkContractError {
    MissingArtifactManifest {
        result_ref: String,
    },
    MissingProofBundle {
        result_ref: String,
    },
    MissingFailureClassification {
        result_ref: String,
    },
    MissingResourceUsage {
        result_ref: String,
    },
    UnsafeNoCheatMetadata {
        result_ref: String,
    },
    ProbeCloseoutImportIncomplete {
        result_ref: String,
    },
    PublicClaimUpgradeAuthority {
        result_ref: String,
    },
    NonLivePublicClaim {
        result_ref: String,
    },
    MissingPublicClaimReleaseGate {
        result_ref: String,
    },
    ManifestInvalid {
        manifest_ref: String,
        reason: String,
    },
    ProbeRunnerInvalid {
        run_ref: String,
        reason: String,
    },
}

pub fn validate_benchmark_result(result: &BenchmarkResult) -> Result<(), BenchmarkContractError> {
    if result.artifact_manifest_refs.is_empty() {
        return Err(BenchmarkContractError::MissingArtifactManifest {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.proof_bundle_refs.is_empty() {
        return Err(BenchmarkContractError::MissingProofBundle {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.status.is_terminal_failure() && result.failure_classification_ref.is_none() {
        return Err(BenchmarkContractError::MissingFailureClassification {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.resource_usage_receipt_ref.is_none() && result.resource_unavailable_reason.is_none() {
        return Err(BenchmarkContractError::MissingResourceUsage {
            result_ref: result.result_ref.clone(),
        });
    }

    if !result.no_cheat.is_public_safe() {
        return Err(BenchmarkContractError::UnsafeNoCheatMetadata {
            result_ref: result.result_ref.clone(),
        });
    }

    if let Some(import) = &result.probe_closeout_import {
        if import.probe_assignment_ref.is_empty()
            || import.probe_closeout_ref.is_empty()
            || import.probe_run_record_ref.is_empty()
            || import.probe_run_ref.is_empty()
            || import.candidate_hash.is_empty()
        {
            return Err(BenchmarkContractError::ProbeCloseoutImportIncomplete {
                result_ref: result.result_ref.clone(),
            });
        }
    }

    if result.public_claim_boundary.public_claim_upgrade_authority {
        return Err(BenchmarkContractError::PublicClaimUpgradeAuthority {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.public_claim_boundary.claim_level != BenchmarkPublicClaimLevel::None {
        if result.evidence_split != BenchmarkEvidenceSplit::Live {
            return Err(BenchmarkContractError::NonLivePublicClaim {
                result_ref: result.result_ref.clone(),
            });
        }

        if result
            .public_claim_boundary
            .external_release_gate_refs
            .is_empty()
        {
            return Err(BenchmarkContractError::MissingPublicClaimReleaseGate {
                result_ref: result.result_ref.clone(),
            });
        }
    }

    Ok(())
}

pub fn validate_campaign_split_manifest(
    manifest: &BenchmarkCampaignSplitManifest,
) -> Result<(), BenchmarkContractError> {
    if manifest.tasks.is_empty() {
        return manifest_error(
            manifest,
            "manifest must include at least one public task ref",
        );
    }

    if !manifest.no_cheat.is_public_safe() {
        return manifest_error(manifest, "no-cheat metadata must be public-safe");
    }

    if manifest.allowed_claim_state != BenchmarkPublicClaimLevel::None {
        return manifest_error(
            manifest,
            "Stage 0/1 Probe GEPA split manifests cannot authorize public claims",
        );
    }

    let mut task_refs = std::collections::BTreeSet::new();

    for task in &manifest.tasks {
        if !task_refs.insert(task.task_ref.clone()) {
            return manifest_error(manifest, "task refs must be unique");
        }

        if task.allowed_claim_level != BenchmarkPublicClaimLevel::None
            && task.evidence_split != BenchmarkEvidenceSplit::Live
        {
            return manifest_error(
                manifest,
                "non-live task entries cannot carry public claim levels",
            );
        }
    }

    require_lane_refs(
        manifest,
        &manifest.retained_fixture_refs,
        BenchmarkSplitLane::RetainedFixture,
        BenchmarkEvidenceSplit::Retained,
        "retained_fixture_refs",
    )?;
    require_lane_refs(
        manifest,
        &manifest.validation_task_refs,
        BenchmarkSplitLane::Validation,
        BenchmarkEvidenceSplit::Validation,
        "validation_task_refs",
    )?;
    require_lane_refs(
        manifest,
        &manifest.frozen_holdout_task_refs,
        BenchmarkSplitLane::FrozenHoldout,
        BenchmarkEvidenceSplit::Holdout,
        "frozen_holdout_task_refs",
    )?;
    require_lane_refs(
        manifest,
        &manifest.local_smoke_fixture_refs,
        BenchmarkSplitLane::LocalSmokeFixture,
        BenchmarkEvidenceSplit::Retained,
        "local_smoke_fixture_refs",
    )?;

    Ok(())
}

fn require_lane_refs(
    manifest: &BenchmarkCampaignSplitManifest,
    refs: &[String],
    lane: BenchmarkSplitLane,
    evidence_split: BenchmarkEvidenceSplit,
    list_name: &str,
) -> Result<(), BenchmarkContractError> {
    if refs.is_empty() {
        return manifest_error(manifest, format!("{list_name} must not be empty"));
    }

    for task_ref in refs {
        let Some(task) = manifest
            .tasks
            .iter()
            .find(|task| &task.task_ref == task_ref)
        else {
            return manifest_error(
                manifest,
                format!("{list_name} contains task ref not present in tasks: {task_ref}"),
            );
        };

        if task.lane != lane || task.evidence_split != evidence_split {
            return manifest_error(
                manifest,
                format!("{list_name} contains task ref with mismatched lane or split: {task_ref}"),
            );
        }
    }

    Ok(())
}

fn manifest_error(
    manifest: &BenchmarkCampaignSplitManifest,
    reason: impl Into<String>,
) -> Result<(), BenchmarkContractError> {
    Err(BenchmarkContractError::ManifestInvalid {
        manifest_ref: manifest.manifest_ref.clone(),
        reason: reason.into(),
    })
}

pub fn probe_assignment_from_split_task(
    manifest: &BenchmarkCampaignSplitManifest,
    task: &BenchmarkSplitTaskEntry,
    probe_commit: impl Into<String>,
    candidate_hash: impl Into<String>,
    selected_blueprint_signature_refs: Vec<String>,
    tool_menu_ref: impl Into<String>,
) -> ProbeBenchmarkAssignment {
    ProbeBenchmarkAssignment {
        schema_ref: String::from(PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF),
        assignment_ref: format!("probe_benchmark_assignment.{}", task.task_id),
        benchmark_run_ref: String::from("benchmark_run.terminal_bench_2.probe_gepa.stage_0_1"),
        task_run_ref: format!("task_run.{}", task.task_id),
        dataset: manifest.dataset.clone(),
        split_ref: manifest.manifest_ref.clone(),
        evidence_split: task.evidence_split.clone(),
        task_ref: task.task_ref.clone(),
        public_task_checksum: task.public_task_checksum.clone(),
        probe_commit: probe_commit.into(),
        runtime_ref: String::from("runtime.probe.v1"),
        backend_profile_ref: String::from("backend_profile.probe.default.v1"),
        backend_ref: String::from("probe.backend.runtime"),
        model_backend_ref: String::from("model_backend.probe.default"),
        provider_account_ref: Some(String::from("provider_account.probe.fixture")),
        auth_grant_ref: Some(String::from("provider_grant.probe.fixture")),
        selected_blueprint_signature_refs,
        tool_menu_ref: tool_menu_ref.into(),
        candidate_hash: candidate_hash.into(),
        timeout_policy_ref: String::from("timeout_policy.benchmark_cloud.probe.default.v1"),
        budget_policy_ref: String::from("budget_policy.benchmark_cloud.probe.no_spend.v1"),
        required_artifact_refs: vec![String::from("artifact_manifest.required.probe.closeout.v1")],
        required_proof_bundle_refs: vec![String::from("proof_bundle.required.probe.closeout.v1")],
        callback_refs: vec![String::from("callback.benchmark_cloud.probe.v1")],
        proof_sink_refs: vec![String::from("proof_sink.benchmark_cloud.probe.v1")],
    }
}

pub fn build_probe_command_invocation(
    assignment: &ProbeBenchmarkAssignment,
) -> Result<ProbeCommandInvocation, BenchmarkContractError> {
    validate_probe_assignment_public_refs(assignment)?;
    let assignment_stdin_json = serde_json::to_value(assignment).map_err(|error| {
        BenchmarkContractError::ProbeRunnerInvalid {
            run_ref: assignment.task_run_ref.clone(),
            reason: format!("failed to serialize Probe assignment JSON: {error}"),
        }
    })?;

    Ok(ProbeCommandInvocation {
        program: String::from("probe"),
        args: vec![
            String::from("benchmark"),
            String::from("run"),
            String::from("--assignment-json"),
            String::from("-"),
        ],
        assignment_stdin_json,
    })
}

pub fn run_fake_probe_benchmark_task(
    manifest: &BenchmarkCampaignSplitManifest,
    task: &BenchmarkSplitTaskEntry,
    assignment: &ProbeBenchmarkAssignment,
    outcome: ProbeFakeRunnerOutcome,
) -> Result<ProbeBenchmarkRunnerArtifactSet, BenchmarkContractError> {
    validate_probe_assignment_public_refs(assignment)?;
    let probe_command = build_probe_command_invocation(assignment)?;
    let status = status_for_fake_outcome(&outcome);
    let run_ref = format!("benchmark_run.probe.{}", task.task_id);
    let artifact_manifest_ref = format!("artifact_manifest.probe.{}", task.task_id);
    let proof_bundle_ref = format!("proof_bundle.probe.{}", task.task_id);
    let resource_usage_receipt_ref = format!("resource_usage.probe.{}", task.task_id);
    let probe_closeout_ref = format!("probe_closeout.{}", run_ref);
    let failure_classification_ref = status.is_terminal_failure().then(|| {
        format!(
            "failure_classification.probe.{}.{}",
            task.task_id,
            failure_family_for_fake_outcome(&outcome)
        )
    });
    let resource_usage_ref =
        matches!(outcome, ProbeFakeRunnerOutcome::Pass).then(|| resource_usage_receipt_ref.clone());
    let resource_unavailable_reason =
        (!matches!(outcome, ProbeFakeRunnerOutcome::Pass)).then(|| {
            format!(
                "{}_before_resource_meter_flush",
                failure_family_for_fake_outcome(&outcome)
            )
        });
    let events = events_for_fake_outcome(&run_ref, &assignment.task_run_ref, &outcome);
    let artifact_manifest = BenchmarkArtifactManifest {
        schema_ref: String::from(BENCHMARK_ARTIFACT_MANIFEST_SCHEMA_REF),
        manifest_ref: artifact_manifest_ref.clone(),
        run_ref: run_ref.clone(),
        artifacts: PROBE_RUNNER_REQUIRED_ARTIFACT_FILES
            .iter()
            .map(|file_name| BenchmarkArtifactRef {
                artifact_ref: format!(
                    "artifact.probe.{}.{}",
                    task.task_id,
                    file_name.replace('.', "_")
                ),
                digest: format!("sha256:{}", "0".repeat(64)),
                media_type: media_type_for_file(file_name).to_string(),
                public_url_ref: None,
                size_bytes: 1,
            })
            .collect(),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };
    let resource_usage_receipt = ResourceUsageReceipt {
        schema_ref: String::from(RESOURCE_USAGE_RECEIPT_SCHEMA_REF),
        receipt_ref: resource_usage_receipt_ref.clone(),
        run_ref: run_ref.clone(),
        worker_ref: String::from("worker.shc.probe.fake"),
        duration_ms: matches!(outcome, ProbeFakeRunnerOutcome::Pass).then_some(1000),
        cpu_ms: matches!(outcome, ProbeFakeRunnerOutcome::Pass).then_some(500),
        gpu_ms: None,
        memory_peak_bytes: Some(1024),
        token_usage: None,
        cost_ref: Some(String::from("cost.probe.fake.zero")),
        unavailable_reason: resource_unavailable_reason.clone(),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };
    let proof_bundle = BenchmarkProofBundle {
        schema_ref: String::from(BENCHMARK_PROOF_BUNDLE_SCHEMA_REF),
        proof_bundle_ref: proof_bundle_ref.clone(),
        run_ref: run_ref.clone(),
        artifact_manifest_refs: vec![artifact_manifest_ref.clone()],
        scorer_verifier: task.scorer_verifier.clone(),
        resource_usage_receipt_ref: Some(resource_usage_receipt_ref.clone()),
        probe_selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
        probe_tool_menu_ref: Some(assignment.tool_menu_ref.clone()),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };
    let result = BenchmarkResult {
        schema_ref: String::from(BENCHMARK_RESULT_SCHEMA_REF),
        result_ref: format!("benchmark_result.probe.{}", task.task_id),
        run_ref: run_ref.clone(),
        task_run_ref: assignment.task_run_ref.clone(),
        dataset: assignment.dataset.clone(),
        evidence_split: assignment.evidence_split.clone(),
        status,
        scorer_verifier: task.scorer_verifier.clone(),
        score_bps: matches!(outcome, ProbeFakeRunnerOutcome::Pass).then_some(10_000),
        failure_classification_ref,
        artifact_manifest_refs: vec![artifact_manifest_ref],
        proof_bundle_refs: vec![proof_bundle_ref],
        resource_usage_receipt_ref: resource_usage_ref,
        resource_unavailable_reason,
        probe_closeout_import: Some(ProbeCloseoutImport {
            probe_assignment_ref: assignment.assignment_ref.clone(),
            probe_closeout_ref: probe_closeout_ref.clone(),
            probe_commit: assignment.probe_commit.clone(),
            probe_run_record_ref: String::from("probe-run-record.json"),
            probe_run_ref: run_ref.clone(),
            candidate_hash: assignment.candidate_hash.clone(),
            selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
            tool_menu_ref: assignment.tool_menu_ref.clone(),
        }),
        no_cheat: manifest.no_cheat.clone(),
        redaction_state: BenchmarkRedactionState::PublicSafe,
        public_claim_boundary: BenchmarkPublicClaimBoundary {
            claim_level: BenchmarkPublicClaimLevel::None,
            external_release_gate_refs: Vec::new(),
            public_claim_upgrade_authority: false,
        },
    };

    validate_benchmark_result(&result)?;

    let artifact_set = ProbeBenchmarkRunnerArtifactSet {
        result_json: result,
        events_jsonl: events,
        metadata_json: ProbeBenchmarkRunMetadata {
            runner_ref: String::from("runner.openagents.harbor.shc.probe.fake"),
            benchmark_suite_ref: manifest.benchmark_suite_ref.clone(),
            dataset: manifest.dataset.clone(),
            task_ref: task.task_ref.clone(),
            probe_command,
            selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
            tool_menu_ref: assignment.tool_menu_ref.clone(),
            safe_account_grant_refs: true,
            redaction_state: BenchmarkRedactionState::PublicSafe,
        },
        artifact_manifest_json: artifact_manifest,
        proof_bundle_json: proof_bundle,
        resource_usage_receipt_json: resource_usage_receipt,
        probe_run_record_json: json!({
            "schema_ref": "probe.benchmark_run.v1",
            "run_ref": run_ref,
            "assignment_ref": assignment.assignment_ref,
            "candidate_hash": assignment.candidate_hash,
            "evidence_split": assignment.evidence_split,
            "status": assignment_status_for_probe_record(&outcome),
        }),
        probe_closeout_json: json!({
            "schema_ref": "probe.benchmark_closeout.v1",
            "closeout_ref": probe_closeout_ref,
            "assignment_ref": assignment.assignment_ref,
            "candidate_hash": assignment.candidate_hash,
            "selected_signature_refs": assignment.selected_blueprint_signature_refs,
            "tool_menu_ref": assignment.tool_menu_ref,
            "artifact_manifest_refs": assignment.required_artifact_refs,
            "proof_bundle_refs": assignment.required_proof_bundle_refs,
            "redaction_state": "public_safe",
        }),
    };

    if artifact_set_contains_unsafe_material(&artifact_set) {
        return Err(BenchmarkContractError::ProbeRunnerInvalid {
            run_ref: artifact_set.result_json.run_ref,
            reason: String::from("Probe runner artifact set contains unsafe material"),
        });
    }

    Ok(artifact_set)
}

pub fn artifact_set_contains_unsafe_material(
    artifact_set: &ProbeBenchmarkRunnerArtifactSet,
) -> bool {
    serde_json::to_value(artifact_set)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
}

fn validate_probe_assignment_public_refs(
    assignment: &ProbeBenchmarkAssignment,
) -> Result<(), BenchmarkContractError> {
    let fields = [
        assignment.provider_account_ref.as_deref(),
        assignment.auth_grant_ref.as_deref(),
        Some(assignment.task_ref.as_str()),
        Some(assignment.public_task_checksum.as_str()),
        Some(assignment.tool_menu_ref.as_str()),
        Some(assignment.candidate_hash.as_str()),
    ];

    for value in fields.into_iter().flatten() {
        if string_contains_unsafe_material(value) {
            return Err(BenchmarkContractError::ProbeRunnerInvalid {
                run_ref: assignment.task_run_ref.clone(),
                reason: String::from(
                    "Probe assignment contains raw credential or private trace material",
                ),
            });
        }
    }

    if assignment.selected_blueprint_signature_refs.is_empty() {
        return Err(BenchmarkContractError::ProbeRunnerInvalid {
            run_ref: assignment.task_run_ref.clone(),
            reason: String::from("Probe assignment must include selected Blueprint signature refs"),
        });
    }

    Ok(())
}

fn events_for_fake_outcome(
    run_ref: &str,
    task_run_ref: &str,
    outcome: &ProbeFakeRunnerOutcome,
) -> Vec<BenchmarkEvent> {
    let terminal_kind = match outcome {
        ProbeFakeRunnerOutcome::Pass => BenchmarkEventKind::Completed,
        ProbeFakeRunnerOutcome::Timeout => BenchmarkEventKind::TimedOut,
        ProbeFakeRunnerOutcome::Error => BenchmarkEventKind::Failed,
    };

    vec![
        BenchmarkEvent {
            schema_ref: String::from(BENCHMARK_EVENT_SCHEMA_REF),
            event_ref: format!("event.{run_ref}.started"),
            run_ref: run_ref.to_string(),
            task_run_ref: task_run_ref.to_string(),
            event_kind: BenchmarkEventKind::Started,
            observed_at: String::from("2026-06-08T00:00:00.000Z"),
            artifact_refs: Vec::new(),
            proof_bundle_refs: Vec::new(),
            redaction_state: BenchmarkRedactionState::PublicSafe,
        },
        BenchmarkEvent {
            schema_ref: String::from(BENCHMARK_EVENT_SCHEMA_REF),
            event_ref: format!("event.{run_ref}.terminal"),
            run_ref: run_ref.to_string(),
            task_run_ref: task_run_ref.to_string(),
            event_kind: terminal_kind,
            observed_at: String::from("2026-06-08T00:00:01.000Z"),
            artifact_refs: vec![format!("artifact_manifest.probe.{task_run_ref}")],
            proof_bundle_refs: vec![format!("proof_bundle.probe.{task_run_ref}")],
            redaction_state: BenchmarkRedactionState::PublicSafe,
        },
    ]
}

fn status_for_fake_outcome(outcome: &ProbeFakeRunnerOutcome) -> BenchmarkRunStatus {
    match outcome {
        ProbeFakeRunnerOutcome::Pass => BenchmarkRunStatus::Succeeded,
        ProbeFakeRunnerOutcome::Timeout => BenchmarkRunStatus::TimedOut,
        ProbeFakeRunnerOutcome::Error => BenchmarkRunStatus::Errored,
    }
}

fn assignment_status_for_probe_record(outcome: &ProbeFakeRunnerOutcome) -> &'static str {
    match outcome {
        ProbeFakeRunnerOutcome::Pass => "succeeded",
        ProbeFakeRunnerOutcome::Timeout => "timed_out",
        ProbeFakeRunnerOutcome::Error => "errored",
    }
}

fn failure_family_for_fake_outcome(outcome: &ProbeFakeRunnerOutcome) -> &'static str {
    match outcome {
        ProbeFakeRunnerOutcome::Pass => "none",
        ProbeFakeRunnerOutcome::Timeout => "timeout",
        ProbeFakeRunnerOutcome::Error => "runtime_error",
    }
}

fn media_type_for_file(file_name: &str) -> &'static str {
    if file_name.ends_with(".jsonl") {
        "application/x-ndjson"
    } else {
        "application/json"
    }
}

fn json_contains_unsafe_material(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => string_contains_unsafe_material(value),
        serde_json::Value::Array(values) => values.iter().any(json_contains_unsafe_material),
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            string_contains_unsafe_material(key) || json_contains_unsafe_material(value)
        }),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

fn string_contains_unsafe_material(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    let raw_logs_unsafe = normalized.contains("raw_logs") && !normalized.contains("no_raw_logs");

    normalized.contains("sk-")
        || normalized.contains("bearer ")
        || normalized.contains("refresh_token")
        || normalized.contains("access_token")
        || normalized.contains("auth.json")
        || normalized.contains("private_harbor_trace")
        || normalized.contains("private-repo://")
        || normalized.contains("wallet_mnemonic")
        || raw_logs_unsafe
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMINAL_BENCH_PROBE_SMOKE: &str =
        include_str!("../../../fixtures/benchmarks/terminal_bench_probe_contract_smoke.json");
    const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str = include_str!(
        "../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json"
    );

    fn no_cheat() -> NoCheatMetadata {
        NoCheatMetadata {
            policy_ref: String::from("policy.openagents.benchmark.no_cheat.public_refs_only.v1"),
            no_hidden_verifier_content: true,
            no_private_repo_refs: true,
            no_raw_logs: true,
            no_task_solution_material: true,
        }
    }

    fn scorer_verifier() -> ScorerVerifierRef {
        ScorerVerifierRef {
            scorer_ref: String::from("scorer.terminal_bench.binary.v1"),
            verifier_ref: String::from("verifier.terminal_bench.configure_git_webserver.v1"),
            verifier_public_ref: String::from("verifier_public.terminal_bench.binary_outcome.v1"),
        }
    }

    fn result_for(status: BenchmarkRunStatus) -> BenchmarkResult {
        BenchmarkResult {
            schema_ref: String::from(BENCHMARK_RESULT_SCHEMA_REF),
            result_ref: String::from("benchmark_result.terminal_bench.configure_git_webserver.1"),
            run_ref: String::from("benchmark_run.terminal_bench_2.harbor.stage_0.1"),
            task_run_ref: String::from("task_run.configure_git_webserver.1"),
            dataset: BenchmarkDatasetRef {
                slug: String::from("terminal-bench-2-harbor"),
                version: String::from("2026-06-08"),
            },
            evidence_split: BenchmarkEvidenceSplit::Retained,
            status,
            scorer_verifier: scorer_verifier(),
            score_bps: None,
            failure_classification_ref: Some(String::from(
                "failure_classification.configure_git_webserver.service_readiness",
            )),
            artifact_manifest_refs: vec![String::from(
                "artifact_manifest.probe.configure_git_webserver.1",
            )],
            proof_bundle_refs: vec![String::from("proof_bundle.probe.configure_git_webserver.1")],
            resource_usage_receipt_ref: None,
            resource_unavailable_reason: Some(String::from("meter_unavailable_in_smoke")),
            probe_closeout_import: Some(ProbeCloseoutImport {
                probe_assignment_ref: String::from(
                    "probe_benchmark_assignment.configure_git_webserver.1",
                ),
                probe_closeout_ref: String::from(
                    "probe_closeout.probe_run.configure_git_webserver.1",
                ),
                probe_commit: String::from("abc1234"),
                probe_run_record_ref: String::from("probe-run-record.json"),
                probe_run_ref: String::from("probe_run.configure_git_webserver.1"),
                candidate_hash: String::from("sha256:candidate-1"),
                selected_signature_refs: vec![String::from(
                    "program_signature.probe.benchmark.service_readiness.v1",
                )],
                tool_menu_ref: String::from("tool_menu.probe.terminal_bench.service_readiness.v1"),
            }),
            no_cheat: no_cheat(),
            redaction_state: BenchmarkRedactionState::PublicSafe,
            public_claim_boundary: BenchmarkPublicClaimBoundary {
                claim_level: BenchmarkPublicClaimLevel::None,
                external_release_gate_refs: Vec::new(),
                public_claim_upgrade_authority: false,
            },
        }
    }

    #[test]
    fn terminal_bench_harbor_probe_fixture_imports_probe_closeout_refs()
    -> Result<(), Box<dyn std::error::Error>> {
        let result: BenchmarkResult = serde_json::from_str(TERMINAL_BENCH_PROBE_SMOKE)?;

        assert_eq!(result.dataset.slug, "terminal-bench-2-harbor");
        assert_eq!(result.evidence_split, BenchmarkEvidenceSplit::Retained);
        assert!(result.probe_closeout_import.is_some());
        validate_benchmark_result(&result)
            .map_err(|error| format!("unexpected validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn result_contract_represents_all_evidence_splits() {
        let splits = [
            BenchmarkEvidenceSplit::Retained,
            BenchmarkEvidenceSplit::Validation,
            BenchmarkEvidenceSplit::Holdout,
            BenchmarkEvidenceSplit::Live,
        ];

        for split in splits {
            let mut result = result_for(BenchmarkRunStatus::Succeeded);
            result.evidence_split = split;
            result.failure_classification_ref = None;
            assert!(validate_benchmark_result(&result).is_ok());
        }
    }

    #[test]
    fn failed_timed_out_and_errored_results_require_artifact_and_proof_refs() {
        for status in [
            BenchmarkRunStatus::Failed,
            BenchmarkRunStatus::TimedOut,
            BenchmarkRunStatus::Errored,
        ] {
            let mut missing_artifact = result_for(status.clone());
            missing_artifact.artifact_manifest_refs.clear();
            assert!(matches!(
                validate_benchmark_result(&missing_artifact),
                Err(BenchmarkContractError::MissingArtifactManifest { .. })
            ));

            let mut missing_proof = result_for(status);
            missing_proof.proof_bundle_refs.clear();
            assert!(matches!(
                validate_benchmark_result(&missing_proof),
                Err(BenchmarkContractError::MissingProofBundle { .. })
            ));
        }
    }

    #[test]
    fn public_claims_require_live_evidence_and_external_gates() {
        let mut retained_result = result_for(BenchmarkRunStatus::Succeeded);
        retained_result.failure_classification_ref = None;
        retained_result.public_claim_boundary.claim_level =
            BenchmarkPublicClaimLevel::ValidationSummary;
        assert!(matches!(
            validate_benchmark_result(&retained_result),
            Err(BenchmarkContractError::NonLivePublicClaim { .. })
        ));

        let mut live_result = retained_result;
        live_result.evidence_split = BenchmarkEvidenceSplit::Live;
        assert!(matches!(
            validate_benchmark_result(&live_result),
            Err(BenchmarkContractError::MissingPublicClaimReleaseGate { .. })
        ));

        live_result
            .public_claim_boundary
            .external_release_gate_refs
            .push(String::from(
                "release_gate.openagents.public_benchmark_claim_review.v1",
            ));
        assert!(validate_benchmark_result(&live_result).is_ok());
    }

    #[test]
    fn terminal_bench_probe_gepa_split_manifest_loads_and_locks_task_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;

        assert_eq!(
            manifest.manifest_ref,
            "benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1"
        );
        assert_eq!(
            manifest.task_selector_version,
            "task_selector.terminal_bench_2.probe_gepa.stage_0_1.v1"
        );
        assert_eq!(
            manifest.allowed_claim_state,
            BenchmarkPublicClaimLevel::None
        );
        assert_eq!(
            manifest
                .tasks
                .iter()
                .map(|task| task.task_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "configure-git-webserver",
                "db-wal-recovery",
                "filter-js-from-html",
                "gcode-to-text",
                "pypi-server",
                "query-optimize",
                "runner-stall-supervision",
                "validation.db-wal-recovery",
                "validation.configure-git-webserver",
                "validation.pypi-server",
                "validation.filter-js-from-html",
                "validation.gcode-to-text",
                "validation.query-optimize",
                "holdout.openssl-selfsigned-cert",
                "holdout.vulnerable-secret",
                "local-smoke.probe-closeout-writer",
                "local-smoke.apple-fm-tool-stream",
            ]
        );
        validate_campaign_split_manifest(&manifest)
            .map_err(|error| format!("unexpected manifest validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn fake_probe_runner_pass_timeout_and_error_emit_required_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == "configure-git-webserver")
            .ok_or("missing retained configure-git-webserver task")?;
        let assignment = probe_assignment_from_split_task(
            &manifest,
            task,
            "abc1234",
            "sha256:candidate-1",
            vec![String::from(
                "program_signature.probe.benchmark.service_readiness.v1",
            )],
            "tool_menu.probe.terminal_bench.service_readiness.v1",
        );

        for outcome in [
            ProbeFakeRunnerOutcome::Pass,
            ProbeFakeRunnerOutcome::Timeout,
            ProbeFakeRunnerOutcome::Error,
        ] {
            let artifacts = run_fake_probe_benchmark_task(&manifest, task, &assignment, outcome)
                .map_err(|error| format!("unexpected fake Probe runner error: {error:?}"))?;
            assert_eq!(artifacts.file_names(), PROBE_RUNNER_REQUIRED_ARTIFACT_FILES);
            assert_eq!(
                artifacts.proof_bundle_json.probe_selected_signature_refs,
                assignment.selected_blueprint_signature_refs
            );
            assert_eq!(
                artifacts.proof_bundle_json.probe_tool_menu_ref.as_deref(),
                Some(assignment.tool_menu_ref.as_str())
            );
            assert!(!artifacts.artifact_manifest_json.artifacts.is_empty());
            assert!(!artifacts.result_json.artifact_manifest_refs.is_empty());
            assert!(!artifacts.result_json.proof_bundle_refs.is_empty());
            assert!(!artifact_set_contains_unsafe_material(&artifacts));
            validate_benchmark_result(&artifacts.result_json)
                .map_err(|error| format!("unexpected validation error: {error:?}"))?;
        }

        Ok(())
    }

    #[test]
    fn fake_probe_runner_rejects_raw_auth_material() -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == "configure-git-webserver")
            .ok_or("missing retained configure-git-webserver task")?;
        let mut assignment = probe_assignment_from_split_task(
            &manifest,
            task,
            "abc1234",
            "sha256:candidate-1",
            vec![String::from(
                "program_signature.probe.benchmark.service_readiness.v1",
            )],
            "tool_menu.probe.terminal_bench.service_readiness.v1",
        );
        assignment.auth_grant_ref = Some(String::from("Bearer raw-token"));

        assert!(matches!(
            build_probe_command_invocation(&assignment),
            Err(BenchmarkContractError::ProbeRunnerInvalid { .. })
        ));
        Ok(())
    }
}
