use serde::{Deserialize, Serialize};

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
}
