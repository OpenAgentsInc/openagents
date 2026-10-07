use std::{fs, path::Path};

use psionic_eval::PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF;
use psionic_models::{
    ParameterGolfPromotedProfileContract, PARAMETER_GOLF_BASELINE_MODEL_ID,
    PARAMETER_GOLF_BASELINE_REVISION, PARAMETER_GOLF_PROMOTED_CHALLENGE_PROFILE_ID,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical committed contract report for the HOMEGOLF track.
pub const PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";
/// Canonical checker script for the HOMEGOLF track contract.
pub const PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-track-contract.sh";
/// Canonical operator doc for the HOMEGOLF track.
pub const PARAMETER_GOLF_HOMEGOLF_TRACK_DOC: &str = "docs/HOMEGOLF_TRACK.md";

/// Comparison posture admitted by the HOMEGOLF track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfComparisonClass {
    PublicBaselineComparable,
    NotPublicLeaderboardEquivalent,
}

/// Current status of one required HOMEGOLF surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfSurfaceStatus {
    Satisfied,
    Blocked,
}

/// One required HOMEGOLF execution surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfRequiredSurface {
    /// Stable surface identifier.
    pub surface_id: String,
    /// Surface status.
    pub status: ParameterGolfHomegolfSurfaceStatus,
    /// Plain-language detail.
    pub detail: String,
    /// Ordered evidence references.
    pub evidence_refs: Vec<String>,
}

/// Declared device class inside the HOMEGOLF hardware manifest contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfHardwareClass {
    /// Stable hardware class identifier.
    pub hardware_class_id: String,
    /// Whether the class is required for the contract.
    pub required: bool,
    /// Plain-language detail.
    pub detail: String,
}

/// Frozen hardware-manifest contract for HOMEGOLF runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfHardwareManifestContract {
    /// Stable manifest contract identifier.
    pub contract_id: String,
    /// Ordered admitted hardware classes.
    pub admitted_hardware_classes: Vec<ParameterGolfHomegolfHardwareClass>,
    /// Whether exact public-hardware equivalence is required.
    pub exact_public_hardware_equivalence_required: bool,
    /// Honest detail for operators.
    pub detail: String,
}

/// Comparison-policy contract for HOMEGOLF result reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfComparisonPolicy {
    /// Ordered admitted comparison classes.
    pub admitted_classes: Vec<ParameterGolfHomegolfComparisonClass>,
    /// Exact language required when comparing against the public leaderboard.
    pub leaderboard_equivalence_rule: String,
    /// Honest detail for operators.
    pub detail: String,
}

/// Frozen HOMEGOLF benchmark-track contract report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfTrackContractReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable track identifier.
    pub track_id: String,
    /// Canonical benchmark reference for score reporting.
    pub benchmark_ref: String,
    /// Stable strict-profile identifier.
    pub strict_profile_id: String,
    /// Stable baseline model identifier.
    pub baseline_model_id: String,
    /// Stable baseline revision.
    pub baseline_revision: String,
    /// Decimal artifact-size cap preserved from the public challenge.
    pub artifact_cap_bytes: u64,
    /// Maximum retained wallclock in seconds.
    pub wallclock_cap_seconds: u64,
    /// Whether exact FineWeb/SP1024 identity is part of the strict HOMEGOLF posture.
    pub exact_fineweb_sp1024_identity_required: bool,
    /// Whether exact tokenizer-agnostic BPB accounting is required.
    pub exact_contest_bpb_accounting_required: bool,
    /// Whether train-to-infer closure is required.
    pub inferable_bundle_required: bool,
    /// Canonical existing dense trainer entrypoint.
    pub canonical_dense_trainer_entrypoint: String,
    /// Canonical existing promoted bundle surface.
    pub canonical_bundle_handoff_surface: String,
    /// Frozen hardware-manifest contract.
    pub hardware_manifest_contract: ParameterGolfHomegolfHardwareManifestContract,
    /// Frozen comparison policy.
    pub comparison_policy: ParameterGolfHomegolfComparisonPolicy,
    /// Ordered required surfaces.
    pub required_surfaces: Vec<ParameterGolfHomegolfRequiredSurface>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the contract report.
    pub report_digest: String,
}

/// Failure while building or writing the HOMEGOLF contract report.
#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfTrackContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the canonical HOMEGOLF benchmark-track contract report.
#[must_use]
pub fn build_parameter_golf_homegolf_track_contract_report(
) -> ParameterGolfHomegolfTrackContractReport {
    let strict_profile = ParameterGolfPromotedProfileContract::strict_pgolf_challenge_v0();
    let required_surfaces = vec![
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("exact_dense_baseline_geometry"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "The HOMEGOLF track keeps the public 9x512 SP1024 baseline geometry instead of a PGOLF-ish approximation.",
            ),
            evidence_refs: vec![
                String::from("crates/psionic-models/src/parameter_golf.rs"),
                String::from("docs/PARAMETER_GOLF_PROMOTED_FAMILY_CONTRACT.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("strict_scorepath_contract"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "The HOMEGOLF strict posture preserves FineWeb/SP1024 identity, tokenizer-agnostic BPB accounting, the 10-minute cap, and contest-style artifact accounting even though the hardware mix is custom.",
            ),
            evidence_refs: vec![
                String::from("crates/psionic-models/src/parameter_golf.rs"),
                String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("dense_trainer_entrypoint"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "The exact dense baseline already has a Rust-owned trainer entrypoint in psionic-train.",
            ),
            evidence_refs: vec![
                String::from("crates/psionic-train/src/bin/parameter_golf_single_h100_train.rs"),
                String::from("crates/psionic-train/src/parameter_golf_single_h100_training.rs"),
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_runpod_single_h100_first_real_training_report.json",
                ),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("train_to_infer_bundle_handoff"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "Psionic already has a canonical promoted PGOLF runtime bundle surface for train-to-infer and serve closure.",
            ),
            evidence_refs: vec![
                String::from("crates/psionic-train/src/parameter_golf_reference.rs"),
                String::from("crates/psionic-models/src/parameter_golf_promoted_bundle.rs"),
                String::from("crates/psionic-serve/src/lib.rs"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("clustered_homegolf_score_surface"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now has a retained H100-backed live dense mixed-device surface that binds real same-job MLX-plus-CUDA dense execution truth to the exact dense challenge export bytes and contest-style final score, replacing the older open-adapter composed surrogate.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json",
                ),
                String::from("crates/psionic-train/src/parameter_golf_homegolf_clustered.rs"),
                String::from("docs/audits/2026-03-27-homegolf-live-dense-run-surface.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("score_relevant_dense_runtime"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now has one canonical score-relevant dense runtime report proving that the retained H100-backed mixed-device dense lane carries resident state, challenge-scale token volume, explicit phase timing, and enough projected 600-second throughput to move beyond symbolic proof updates.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_score_relevant_runtime.json",
                ),
                String::from(
                    "crates/psionic-train/src/parameter_golf_homegolf_score_runtime.rs",
                ),
                String::from(
                    "docs/audits/2026-03-27-homegolf-score-relevant-runtime-audit.md",
                ),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("public_comparison_report"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now emits one deterministic public comparison report against the public naive baseline and the current public best leaderboard row from the retained H100-backed live dense mixed-device surface, while refusing leaderboard-equivalent language for the custom-hardware track.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_public_comparison.json",
                ),
                String::from("crates/psionic-train/src/parameter_golf_homegolf_comparison.rs"),
                String::from("docs/audits/2026-03-27-homegolf-public-comparison-audit.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("artifact_accounting_report"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now emits one counted-byte report bound to the retained H100-backed live dense mixed-device surface, and the retained compressed dense export now fits inside the 16,000,000-byte contest cap under the current counted-code posture.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_artifact_accounting.json",
                ),
                String::from("crates/psionic-train/src/parameter_golf_homegolf_accounting.rs"),
                String::from("docs/audits/2026-03-27-homegolf-artifact-accounting-audit.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("strict_challenge_runnable_lane"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now has one canonical strict challenge preflight surface that binds the exact contest overlay, refuses explicitly when the exact FineWeb/SP1024 inputs are not supplied, and points operators at the actual local HOMEGOLF exact trainer entrypoint instead of the public single-H100 path.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_strict_challenge_lane.json",
                ),
                String::from(
                    "crates/psionic-train/src/parameter_golf_homegolf_strict_challenge.rs",
                ),
                String::from(
                    "docs/audits/2026-03-28-homegolf-strict-preflight-semantics-and-local-entrypoint-audit.md",
                ),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("mixed_hardware_manifest_surface"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now also has one explicit retained mixed-hardware manifest example showing that optional H100 capacity can join the same track without changing score semantics, wallclock policy, artifact accounting, or public-comparison language.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_mixed_hardware_manifest.json",
                ),
                String::from("crates/psionic-train/src/parameter_golf_homegolf_manifest.rs"),
                String::from("docs/audits/2026-03-27-homegolf-mixed-hardware-manifest-audit.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("competitive_variant_ablation_surface"),
            status: ParameterGolfHomegolfSurfaceStatus::Satisfied,
            detail: String::from(
                "HOMEGOLF now freezes one best-known competitive exact-lane variant beyond the naive baseline, plus a retained ablation report showing which already-owned public-winning surfaces are wired in versus still explicitly refused.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_competitive_ablation.json",
                ),
                String::from(
                    "crates/psionic-train/src/parameter_golf_homegolf_competitive_ablation.rs",
                ),
                String::from("docs/audits/2026-03-27-homegolf-competitive-ablation-audit.md"),
            ],
        },
        ParameterGolfHomegolfRequiredSurface {
            surface_id: String::from("mixed_device_dense_home_cluster_execution"),
            status: ParameterGolfHomegolfSurfaceStatus::Blocked,
            detail: String::from(
                "The upgraded HOMEGOLF runtime now has real score-relevant MLX-plus-H100 dense execution truth, but admitted home-cluster dense closure on the local Apple-plus-home-RTX device set remains the next implementation step.",
            ),
            evidence_refs: vec![
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json",
                ),
                String::from(
                    "fixtures/parameter_golf/reports/parameter_golf_homegolf_score_relevant_runtime.json",
                ),
                String::from("crates/psionic-train/src/first_same_job_mixed_backend_dense_run.rs"),
                String::from("docs/audits/2026-03-27-tailnet-short-run-device-audit.md"),
            ],
        },
    ];

    let hardware_manifest_contract = ParameterGolfHomegolfHardwareManifestContract {
        contract_id: String::from("parameter_golf.homegolf.hardware_manifest.v1"),
        admitted_hardware_classes: vec![
            ParameterGolfHomegolfHardwareClass {
                hardware_class_id: String::from("local_apple_silicon_metal"),
                required: true,
                detail: String::from(
                    "Primary local Apple Silicon workstation class used for HOMEGOLF development and short retained runs.",
                ),
            },
            ParameterGolfHomegolfHardwareClass {
                hardware_class_id: String::from("home_consumer_cuda_node"),
                required: true,
                detail: String::from(
                    "Consumer NVIDIA node on the home network used for mixed-device and comparative HOMEGOLF runs.",
                ),
            },
            ParameterGolfHomegolfHardwareClass {
                hardware_class_id: String::from("secondary_apple_silicon_peer"),
                required: false,
                detail: String::from(
                    "Secondary Apple Silicon peer admitted when it is awake and reachable for the retained wallclock window.",
                ),
            },
            ParameterGolfHomegolfHardwareClass {
                hardware_class_id: String::from("optional_h100_node"),
                required: false,
                detail: String::from(
                    "Future H100 capacity may join the same HOMEGOLF track without changing score semantics.",
                ),
            },
        ],
        exact_public_hardware_equivalence_required: false,
        detail: String::from(
            "HOMEGOLF preserves the public scorepath contract while explicitly refusing to claim that a mixed home cluster is hardware-equivalent to the public 8xH100 leaderboard posture.",
        ),
    };

    let comparison_policy = ParameterGolfHomegolfComparisonPolicy {
        admitted_classes: vec![
            ParameterGolfHomegolfComparisonClass::PublicBaselineComparable,
            ParameterGolfHomegolfComparisonClass::NotPublicLeaderboardEquivalent,
        ],
        leaderboard_equivalence_rule: String::from(
            "A HOMEGOLF result may be compared against the public baseline and current leaderboard by val_bpb, wallclock cap, and artifact bytes, but it must not be called public-leaderboard equivalent unless it is rerun under the official 8xH100 posture.",
        ),
        detail: String::from(
            "This keeps public-score comparison honest while still letting the custom home-cluster track measure real progress against the contest.",
        ),
    };

    let mut report = ParameterGolfHomegolfTrackContractReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_track_contract.v1"),
        track_id: String::from("parameter_golf.home_cluster_compatible_10min.v1"),
        benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
        strict_profile_id: String::from(PARAMETER_GOLF_PROMOTED_CHALLENGE_PROFILE_ID),
        baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
        baseline_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
        artifact_cap_bytes: 16_000_000,
        wallclock_cap_seconds: 600,
        exact_fineweb_sp1024_identity_required: true,
        exact_contest_bpb_accounting_required: true,
        inferable_bundle_required: true,
        canonical_dense_trainer_entrypoint: String::from(
            "crates/psionic-train/src/bin/parameter_golf_single_h100_train.rs",
        ),
        canonical_bundle_handoff_surface: String::from(
            "crates/psionic-models/src/parameter_golf_promoted_bundle.rs",
        ),
        hardware_manifest_contract,
        comparison_policy,
        required_surfaces,
        claim_boundary: format!(
            "HOMEGOLF freezes the public baseline geometry, strict scorepath semantics, and custom-hardware comparison law for a mixed cluster. It now has one canonical strict challenge preflight lane, one retained H100-backed dense mixed-device surface built from real MLX-plus-CUDA dense execution truth plus the exact dense challenge export bytes, and one explicit mixed-hardware manifest example that keeps future H100 capacity inside the same track law. It still does not claim admitted home-RTX dense closure or official 8xH100 leaderboard equivalence."
        ),
        report_digest: String::new(),
    };
    let _ = strict_profile; // keep the strict-profile contract linked in code generation.
    report.report_digest =
        stable_digest(b"psionic_parameter_golf_homegolf_track_contract|", &report);
    report
}

/// Writes the canonical HOMEGOLF benchmark-track contract report to disk.
pub fn write_parameter_golf_homegolf_track_contract_report(
    output_path: &Path,
) -> Result<ParameterGolfHomegolfTrackContractReport, ParameterGolfHomegolfTrackContractError> {
    let report = build_parameter_golf_homegolf_track_contract_report();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfTrackContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfTrackContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("HOMEGOLF contract should serialize"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_track_contract_report,
        write_parameter_golf_homegolf_track_contract_report, ParameterGolfHomegolfSurfaceStatus,
        PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REPORT_REF,
    };

    #[test]
    fn homegolf_track_contract_keeps_required_dense_and_comparison_surfaces() {
        let report = build_parameter_golf_homegolf_track_contract_report();
        assert_eq!(report.wallclock_cap_seconds, 600);
        assert_eq!(report.artifact_cap_bytes, 16_000_000);
        assert!(report.exact_fineweb_sp1024_identity_required);
        assert!(report.exact_contest_bpb_accounting_required);
        assert!(report.inferable_bundle_required);
        assert!(report
            .required_surfaces
            .iter()
            .any(|surface| surface.surface_id == "dense_trainer_entrypoint"
                && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied));
        assert!(report
            .required_surfaces
            .iter()
            .any(
                |surface| surface.surface_id == "clustered_homegolf_score_surface"
                    && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied
            ));
        assert!(report
            .required_surfaces
            .iter()
            .any(
                |surface| surface.surface_id == "mixed_hardware_manifest_surface"
                    && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied
            ));
        assert!(report
            .required_surfaces
            .iter()
            .any(
                |surface| surface.surface_id == "competitive_variant_ablation_surface"
                    && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied
            ));
        assert!(report
            .required_surfaces
            .iter()
            .any(|surface| surface.surface_id == "public_comparison_report"
                && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied));
        assert!(report
            .required_surfaces
            .iter()
            .any(|surface| surface.surface_id == "artifact_accounting_report"
                && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied));
        assert!(report
            .required_surfaces
            .iter()
            .any(
                |surface| surface.surface_id == "strict_challenge_runnable_lane"
                    && surface.status == ParameterGolfHomegolfSurfaceStatus::Satisfied
            ));
        assert!(report
            .required_surfaces
            .iter()
            .any(
                |surface| surface.surface_id == "mixed_device_dense_home_cluster_execution"
                    && surface.status == ParameterGolfHomegolfSurfaceStatus::Blocked
            ));
    }

    #[test]
    fn write_homegolf_track_contract_report_persists_current_truth() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_track_contract.json");
        let written =
            write_parameter_golf_homegolf_track_contract_report(path.as_path()).expect("write");
        let encoded = std::fs::read(path.as_path()).expect("read");
        let decoded: super::ParameterGolfHomegolfTrackContractReport =
            serde_json::from_slice(&encoded).expect("decode");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_homegolf_contract_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REPORT_REF);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfTrackContractReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        let rebuilt = build_parameter_golf_homegolf_track_contract_report();
        assert_eq!(decoded, rebuilt);
    }
}
