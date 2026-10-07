use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical committed report for the blocked Parameter Golf record-track contract.
pub const PARAMETER_GOLF_RECORD_TRACK_CONTRACT_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_record_track_contract.json";

/// Current disposition of the record-track contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfRecordTrackDisposition {
    Blocked,
    Ready,
}

/// Status of one required record-track surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfRecordTrackSurfaceStatus {
    Satisfied,
    Blocked,
}

/// One required surface for the record-track contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordTrackRequiredSurface {
    /// Stable surface identifier.
    pub surface_id: String,
    /// Current surface status.
    pub status: ParameterGolfRecordTrackSurfaceStatus,
    /// Honest detail about the surface.
    pub detail: String,
    /// Ordered evidence refs for the surface.
    pub evidence_refs: Vec<String>,
}

/// One explicit blocker for record-track promotion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordTrackBlocker {
    /// Stable blocker identifier.
    pub blocker_id: String,
    /// Plain-language blocker detail.
    pub detail: String,
    /// Required surfaces blocked by this condition.
    pub blocking_surface_ids: Vec<String>,
}

/// Committed blocked contract for the Parameter Golf record-track lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordTrackContractReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Current maximum honest claim posture.
    pub current_max_claim_posture: String,
    /// Claim posture unlocked once throughput closure is green but runtime story stays blocked.
    pub next_claim_posture_after_throughput_closure: String,
    /// Final target claim posture.
    pub target_claim_posture: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Record-track disposition.
    pub disposition: ParameterGolfRecordTrackDisposition,
    /// Stable digest of the acceptance report that gates this contract.
    pub acceptance_report_digest: String,
    /// Stable digest of the committed research harness report.
    pub research_harness_report_digest: String,
    /// Baseline non-record package version.
    pub baseline_submission_package_version: String,
    /// Stable digest of the shipped submission entrypoint.
    pub baseline_entrypoint_artifact_digest: String,
    /// Stable digest of the counted compressed-model artifact.
    pub baseline_model_artifact_digest: String,
    /// Stable digest of the non-record accounting receipt.
    pub baseline_accounting_receipt_digest: String,
    /// Stable digest of the exported submission run-evidence report.
    pub baseline_submission_run_evidence_report_digest: String,
    /// Stable digest of the export-surface contract report.
    pub baseline_export_surface_contract_report_digest: String,
    /// Stable digest of the exported folder replay-verification report.
    pub baseline_record_folder_replay_verification_report_digest: String,
    /// Stable digest of the final PR-bundle report.
    pub baseline_final_pr_bundle_report_digest: String,
    /// Stable digest of the local challenge-clone dry-run report.
    pub baseline_local_clone_dry_run_report_digest: String,
    /// Ordered counted-component identifiers carried forward from the package lane.
    pub required_counted_component_ids: Vec<String>,
    /// Ordered required surfaces.
    pub required_surfaces: Vec<ParameterGolfRecordTrackRequiredSurface>,
    /// Ordered explicit blockers.
    pub blockers: Vec<ParameterGolfRecordTrackBlocker>,
    /// Honest claim boundary for the current contract.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl ParameterGolfRecordTrackContractReport {
    fn new(
        current_max_claim_posture: impl Into<String>,
        acceptance_report_digest: impl Into<String>,
        research_harness_report_digest: impl Into<String>,
        baseline_submission_package_version: impl Into<String>,
        baseline_entrypoint_artifact_digest: impl Into<String>,
        baseline_model_artifact_digest: impl Into<String>,
        baseline_accounting_receipt_digest: impl Into<String>,
        baseline_submission_run_evidence_report_digest: impl Into<String>,
        baseline_export_surface_contract_report_digest: impl Into<String>,
        baseline_record_folder_replay_verification_report_digest: impl Into<String>,
        baseline_final_pr_bundle_report_digest: impl Into<String>,
        baseline_local_clone_dry_run_report_digest: impl Into<String>,
        required_counted_component_ids: Vec<String>,
        required_surfaces: Vec<ParameterGolfRecordTrackRequiredSurface>,
        blockers: Vec<ParameterGolfRecordTrackBlocker>,
    ) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("parameter_golf.record_track_contract.v1"),
            current_max_claim_posture: current_max_claim_posture.into(),
            next_claim_posture_after_throughput_closure: String::from(
                "record_candidate_blocked_on_accounting",
            ),
            target_claim_posture: String::from("record_ready"),
            benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
            disposition: if blockers.is_empty() {
                ParameterGolfRecordTrackDisposition::Ready
            } else {
                ParameterGolfRecordTrackDisposition::Blocked
            },
            acceptance_report_digest: acceptance_report_digest.into(),
            research_harness_report_digest: research_harness_report_digest.into(),
            baseline_submission_package_version: baseline_submission_package_version.into(),
            baseline_entrypoint_artifact_digest: baseline_entrypoint_artifact_digest.into(),
            baseline_model_artifact_digest: baseline_model_artifact_digest.into(),
            baseline_accounting_receipt_digest: baseline_accounting_receipt_digest.into(),
            baseline_submission_run_evidence_report_digest:
                baseline_submission_run_evidence_report_digest.into(),
            baseline_export_surface_contract_report_digest:
                baseline_export_surface_contract_report_digest.into(),
            baseline_record_folder_replay_verification_report_digest:
                baseline_record_folder_replay_verification_report_digest.into(),
            baseline_final_pr_bundle_report_digest:
                baseline_final_pr_bundle_report_digest.into(),
            baseline_local_clone_dry_run_report_digest:
                baseline_local_clone_dry_run_report_digest.into(),
            required_counted_component_ids,
            required_surfaces,
            blockers,
            claim_boundary: String::from(
                "record-track contract is now explicit, the shipped runtime-byte contract is defended, and the current lane remains blocked on reproducible 8xH100 challenge-speed execution",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_parameter_golf_record_track_contract_report|",
            &report,
        );
        report
    }
}

/// Failure while building or persisting the record-track contract report.
#[derive(Debug, Error)]
pub enum ParameterGolfRecordTrackContractError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize)]
struct AcceptanceReportFixture {
    current_claim_posture: String,
    report_digest: String,
    categories: Vec<AcceptanceCategoryFixture>,
}

#[derive(Clone, Debug, Deserialize)]
struct AcceptanceCategoryFixture {
    category_id: String,
    matrix_status: String,
    current_repo_truth: String,
    boundary_note: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ResearchHarnessFixture {
    report_digest: String,
    comparison_surface: ResearchHarnessComparisonSurfaceFixture,
}

#[derive(Clone, Debug, Deserialize)]
struct ResearchHarnessComparisonSurfaceFixture {
    counted_component_ids: Vec<String>,
    baseline_submission_package_version: String,
    baseline_entrypoint_artifact_digest: String,
    baseline_model_artifact_digest: String,
    baseline_accounting_receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct SubmissionRunEvidenceFixture {
    report_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ExportSurfaceContractFixture {
    report_digest: String,
    judgment: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ReplayVerificationFixture {
    report_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FinalPrBundleFixture {
    report_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct LocalCloneDryRunFixture {
    report_digest: String,
}

/// Builds the committed blocked record-track contract report.
#[must_use]
pub fn build_parameter_golf_record_track_contract_report() -> ParameterGolfRecordTrackContractReport
{
    let acceptance: AcceptanceReportFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_acceptance_report.json"
    ))
    .expect("acceptance report fixture should decode");
    let research: ResearchHarnessFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_research_harness_report.json"
    ))
    .expect("research harness fixture should decode");
    let submission_run_evidence: SubmissionRunEvidenceFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_submission_run_evidence.json"
    ))
    .expect("submission run evidence fixture should decode");
    let export_surface_contract: ExportSurfaceContractFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_record_export_surface_contract.json"
    ))
    .expect("export surface contract fixture should decode");
    let replay_verification: ReplayVerificationFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_record_folder_replay_verification.json"
    ))
    .expect("record-folder replay fixture should decode");
    let final_pr_bundle: FinalPrBundleFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_final_pr_bundle.json"
    ))
    .expect("final PR bundle fixture should decode");
    let local_clone_dry_run: LocalCloneDryRunFixture = serde_json::from_str(include_str!(
        "../../../fixtures/parameter_golf/reports/parameter_golf_local_clone_dry_run.json"
    ))
    .expect("local clone dry-run fixture should decode");

    let oracle = category(&acceptance.categories, "challenge-oracle-parity");
    let packaging = category(&acceptance.categories, "packaging-readiness");
    let distributed = category(&acceptance.categories, "distributed-throughput-closure");
    let record = category(&acceptance.categories, "record-track-readiness");

    let required_surfaces = vec![
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("challenge_oracle_parity"),
            status: if oracle.matrix_status == "implemented" {
                ParameterGolfRecordTrackSurfaceStatus::Satisfied
            } else {
                ParameterGolfRecordTrackSurfaceStatus::Blocked
            },
            detail: oracle.current_repo_truth.clone(),
            evidence_refs: vec![String::from(
                "fixtures/parameter_golf/reports/parameter_golf_acceptance_report.json",
            )],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("reproducible_record_folder_output"),
            status: if packaging.matrix_status == "implemented" {
                ParameterGolfRecordTrackSurfaceStatus::Satisfied
            } else {
                ParameterGolfRecordTrackSurfaceStatus::Blocked
            },
            detail: packaging.current_repo_truth.clone(),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_acceptance_report.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("public_counted_byte_contract"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: format!(
                "The counted-byte vocabulary is explicit and frozen through the committed non-record package surface: {}",
                research
                    .comparison_surface
                    .counted_component_ids
                    .join(", ")
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_ACCOUNTING.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_research_harness_report.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("record_runtime_entrypoint"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: format!(
                "The current shipped train_gpt.py surface is a real non-record submission launcher keyed by entrypoint digest `{}`; it execs a shipped Psionic runtime payload, replays the bounded local-reference validation path, and writes a runtime receipt inside the exported folder.",
                research.comparison_surface.baseline_entrypoint_artifact_digest
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md"),
                String::from("docs/PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY.md"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("record_export_surface_contract"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: format!(
                "The repo now preserves one explicit export-surface judgment against the README train_gpt.py contract. Current judgment=`{}`: the folder is not literally train_gpt.py-only, but the launcher-plus-runtime equivalence argument is machine-readable and bound to exact shipped bytes.",
                export_surface_contract.judgment
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md"),
                String::from("docs/PARAMETER_GOLF_EXPORTED_SUBMISSION_EVIDENCE.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_record_export_surface_contract.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("folder_local_replay_verification"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: String::from(
                "The exported folder now carries a machine-readable replay verifier that replays offline execution, checks final metrics against train.log plus runtime and benchmark receipts, and checks counted bytes against the shipped accounting contract.",
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_EXPORTED_SUBMISSION_EVIDENCE.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_record_folder_replay_verification.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("maintainer_facing_pr_bundle"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: String::from(
                "Psionic now owns a deterministic final PR-bundle generator with explicit review artifacts, promotion receipt posture, and maintainer-facing checklist text for the live challenge repo.",
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_PR_SUBMISSION_FLOW.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_final_pr_bundle.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("local_challenge_clone_dry_run"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: String::from(
                "The current generated folder has already been staged into a local parameter-golf clone, verified there, and cleaned back out with a committed dry-run report.",
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_PR_SUBMISSION_FLOW.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_local_clone_dry_run.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("counted_runtime_story_for_record_track"),
            status: ParameterGolfRecordTrackSurfaceStatus::Satisfied,
            detail: String::from(
                "The current shipped runtime-byte story is explicit and defended: the exported folder counts the top-level train_gpt.py launcher, the shipped replay and single-H100 trainer binaries, zero additional wrapper bytes, and zero in-folder build-dependency bytes, while preserving JSON contracts and receipts as data/config sidecars rather than counted code bytes.",
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_ACCOUNTING.md"),
                String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_research_harness_report.json"),
            ],
        },
        ParameterGolfRecordTrackRequiredSurface {
            surface_id: String::from("reproducible_8xh100_record_execution"),
            status: if distributed.matrix_status == "implemented" {
                ParameterGolfRecordTrackSurfaceStatus::Satisfied
            } else {
                ParameterGolfRecordTrackSurfaceStatus::Blocked
            },
            detail: format!(
                "{} Benchmark ref remains `{}`.",
                distributed.current_repo_truth, PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF
            ),
            evidence_refs: vec![
                String::from("docs/PARAMETER_GOLF_DISTRIBUTED_8XH100.md"),
                String::from("fixtures/parameter_golf/reports/parameter_golf_acceptance_report.json"),
            ],
        },
    ];
    let blockers = vec![ParameterGolfRecordTrackBlocker {
        blocker_id: String::from("reproducible_8xh100_record_execution_not_yet_closed"),
        detail: format!("{} {}", distributed.boundary_note, record.boundary_note),
        blocking_surface_ids: vec![String::from("reproducible_8xh100_record_execution")],
    }];

    ParameterGolfRecordTrackContractReport::new(
        acceptance.current_claim_posture,
        acceptance.report_digest,
        research.report_digest,
        research
            .comparison_surface
            .baseline_submission_package_version,
        research
            .comparison_surface
            .baseline_entrypoint_artifact_digest,
        research.comparison_surface.baseline_model_artifact_digest,
        research
            .comparison_surface
            .baseline_accounting_receipt_digest,
        submission_run_evidence.report_digest,
        export_surface_contract.report_digest,
        replay_verification.report_digest,
        final_pr_bundle.report_digest,
        local_clone_dry_run.report_digest,
        research.comparison_surface.counted_component_ids,
        required_surfaces,
        blockers,
    )
}

/// Returns the canonical absolute path for the committed record-track contract report.
#[must_use]
pub fn parameter_golf_record_track_contract_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_RECORD_TRACK_CONTRACT_REPORT_REF)
}

/// Writes the committed record-track contract report.
pub fn write_parameter_golf_record_track_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<ParameterGolfRecordTrackContractReport, ParameterGolfRecordTrackContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfRecordTrackContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_parameter_golf_record_track_contract_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        ParameterGolfRecordTrackContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn category<'a>(
    categories: &'a [AcceptanceCategoryFixture],
    category_id: &str,
) -> &'a AcceptanceCategoryFixture {
    categories
        .iter()
        .find(|category| category.category_id == category_id)
        .expect("required acceptance category should exist")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, ParameterGolfRecordTrackContractError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| ParameterGolfRecordTrackContractError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfRecordTrackContractError::Deserialize {
            artifact_kind: String::from("parameter_golf_record_track_contract_report"),
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_parameter_golf_record_track_contract_report,
        parameter_golf_record_track_contract_report_path, read_repo_json,
        write_parameter_golf_record_track_contract_report, ParameterGolfRecordTrackContractReport,
        ParameterGolfRecordTrackDisposition, ParameterGolfRecordTrackSurfaceStatus,
        PARAMETER_GOLF_RECORD_TRACK_CONTRACT_REPORT_REF,
    };

    #[test]
    fn parameter_golf_record_track_contract_stays_explicitly_blocked() {
        let report = build_parameter_golf_record_track_contract_report();

        assert_eq!(
            report.disposition,
            ParameterGolfRecordTrackDisposition::Blocked
        );
        assert_eq!(report.current_max_claim_posture, "non_record_submission");
        assert!(report.required_surfaces.iter().any(|surface| {
            surface.surface_id == "record_runtime_entrypoint"
                && surface.status == ParameterGolfRecordTrackSurfaceStatus::Satisfied
        }));
        assert!(report.required_surfaces.iter().any(|surface| {
            surface.surface_id == "counted_runtime_story_for_record_track"
                && surface.status == ParameterGolfRecordTrackSurfaceStatus::Satisfied
        }));
        assert!(report.required_surfaces.iter().any(|surface| {
            surface.surface_id == "record_export_surface_contract"
                && surface.status == ParameterGolfRecordTrackSurfaceStatus::Satisfied
        }));
        assert!(report
            .required_surfaces
            .iter()
            .any(|surface| surface.surface_id == "reproducible_8xh100_record_execution"));
    }

    #[test]
    fn parameter_golf_record_track_contract_report_matches_committed_truth() {
        let generated = build_parameter_golf_record_track_contract_report();
        let committed: ParameterGolfRecordTrackContractReport =
            read_repo_json(PARAMETER_GOLF_RECORD_TRACK_CONTRACT_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_parameter_golf_record_track_contract_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("parameter_golf_record_track_contract.json");
        let written =
            write_parameter_golf_record_track_contract_report(&output_path).expect("write report");
        let persisted: ParameterGolfRecordTrackContractReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_record_track_contract_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("parameter_golf_record_track_contract.json")
        );
    }
}
