use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_tassadar_default_train_lane_contract, write_tassadar_train_launcher_outputs,
    PsionActualPretrainingArtifactRef, TassadarDefaultTrainLaneContractError,
    TassadarTrainLauncherError, TassadarTrainLauncherPhase,
    TASSADAR_DEFAULT_TRAIN_LANE_FIXTURE_PATH, TASSADAR_DEFAULT_TRAIN_LANE_ID,
    TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH,
};

pub const TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_SCHEMA_VERSION: &str =
    "tassadar.default_train_lane_contract_checker_receipt.v1";
pub const TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_SCHEMA_VERSION: &str =
    "tassadar.default_train_acceptance_checker_receipt.v1";
pub const TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_SCHEMA_VERSION: &str =
    "tassadar.default_train_promotion_evidence.v1";
pub const TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_SCHEMA_VERSION: &str =
    "tassadar.default_train_rehearsal_bundle.v1";

pub const TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_default_train_lane_contract_checker_receipt_v1.json";
pub const TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_default_train_acceptance_checker_receipt_v1.json";
pub const TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_default_train_promotion_evidence_v1.json";
pub const TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_default_train_rehearsal_bundle_v1.json";

const DEFAULT_REHEARSAL_ID: &str = "tassadar_default_train_rehearsal_v1";
const DEFAULT_REHEARSAL_RUN_ID: &str = "run-tassadar-default-rehearsal-20260402t220000z";
const DEFAULT_REHEARSAL_EXAMPLE_ROOT: &str =
    "fixtures/tassadar/operator/tassadar_default_train_rehearsal_example";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDefaultTrainCheckerReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub lane_id: String,
    pub checker_bundle_id: String,
    pub checker_command: String,
    pub checked_artifact: PsionActualPretrainingArtifactRef,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDefaultTrainPromotionEvidence {
    pub schema_version: String,
    pub evidence_id: String,
    pub lane_id: String,
    pub promotion_target_model_id: String,
    pub promotion_target_descriptor: PsionActualPretrainingArtifactRef,
    pub promotion_target_artifact: PsionActualPretrainingArtifactRef,
    pub promotion_target_artifact_id: String,
    pub promotion_target_lineage: PsionActualPretrainingArtifactRef,
    pub anchor_run_bundle: PsionActualPretrainingArtifactRef,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDefaultTrainRehearsalArtifact {
    pub artifact_kind: String,
    pub artifact: PsionActualPretrainingArtifactRef,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDefaultTrainRehearsalGate {
    pub gate_id: String,
    pub satisfied: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDefaultTrainRehearsalBundle {
    pub schema_version: String,
    pub rehearsal_id: String,
    pub lane_id: String,
    pub launcher_path: String,
    pub launcher_surface_id: String,
    pub run_id: String,
    pub run_root: String,
    pub launch_manifest_ref: String,
    pub current_run_status_ref: String,
    pub retained_summary_ref: String,
    pub checker_receipt_refs: Vec<String>,
    pub promotion_evidence_ref: String,
    pub evidence_artifacts: Vec<TassadarDefaultTrainRehearsalArtifact>,
    pub closeout_gates: Vec<TassadarDefaultTrainRehearsalGate>,
    pub can_now_claim: Vec<String>,
    pub still_out_of_scope: Vec<String>,
    pub claim_boundary: String,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum TassadarDefaultTrainRehearsalError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid rehearsal field `{field}`: {detail}")]
    Invalid { field: String, detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    DefaultLane(#[from] TassadarDefaultTrainLaneContractError),
    #[error(transparent)]
    Launcher(#[from] TassadarTrainLauncherError),
}

impl TassadarDefaultTrainCheckerReceipt {
    pub fn validate(&self) -> Result<(), TassadarDefaultTrainRehearsalError> {
        ensure_exact(
            self.lane_id.as_str(),
            "checker_receipt.lane_id",
            TASSADAR_DEFAULT_TRAIN_LANE_ID,
        )?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "checker_receipt.schema_version",
        )?;
        ensure_nonempty(self.receipt_id.as_str(), "checker_receipt.receipt_id")?;
        ensure_nonempty(
            self.checker_bundle_id.as_str(),
            "checker_receipt.checker_bundle_id",
        )?;
        ensure_nonempty(
            self.checker_command.as_str(),
            "checker_receipt.checker_command",
        )?;
        ensure_nonempty(self.status.as_str(), "checker_receipt.status")?;
        ensure_nonempty(self.detail.as_str(), "checker_receipt.detail")?;
        validate_artifact_ref(&self.checked_artifact, "checker_receipt.checked_artifact")?;
        Ok(())
    }
}

impl TassadarDefaultTrainPromotionEvidence {
    pub fn validate(&self) -> Result<(), TassadarDefaultTrainRehearsalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "promotion_evidence.schema_version",
            TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "promotion_evidence.lane_id",
            TASSADAR_DEFAULT_TRAIN_LANE_ID,
        )?;
        ensure_nonempty(
            self.promotion_target_model_id.as_str(),
            "promotion_evidence.promotion_target_model_id",
        )?;
        ensure_nonempty(
            self.promotion_target_artifact_id.as_str(),
            "promotion_evidence.promotion_target_artifact_id",
        )?;
        ensure_nonempty(self.status.as_str(), "promotion_evidence.status")?;
        ensure_nonempty(self.detail.as_str(), "promotion_evidence.detail")?;
        validate_artifact_ref(
            &self.promotion_target_descriptor,
            "promotion_evidence.promotion_target_descriptor",
        )?;
        validate_artifact_ref(
            &self.promotion_target_artifact,
            "promotion_evidence.promotion_target_artifact",
        )?;
        validate_artifact_ref(
            &self.promotion_target_lineage,
            "promotion_evidence.promotion_target_lineage",
        )?;
        validate_artifact_ref(
            &self.anchor_run_bundle,
            "promotion_evidence.anchor_run_bundle",
        )?;
        Ok(())
    }
}

impl TassadarDefaultTrainRehearsalBundle {
    pub fn validate(&self) -> Result<(), TassadarDefaultTrainRehearsalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "rehearsal_bundle.schema_version",
            TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "rehearsal_bundle.lane_id",
            TASSADAR_DEFAULT_TRAIN_LANE_ID,
        )?;
        ensure_exact(
            self.launcher_path.as_str(),
            "rehearsal_bundle.launcher_path",
            TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH,
        )?;
        ensure_exact(
            self.launch_manifest_ref.as_str(),
            "rehearsal_bundle.launch_manifest_ref",
            "manifests/launch_manifest.json",
        )?;
        ensure_exact(
            self.current_run_status_ref.as_str(),
            "rehearsal_bundle.current_run_status_ref",
            "status/current_run_status.json",
        )?;
        ensure_exact(
            self.retained_summary_ref.as_str(),
            "rehearsal_bundle.retained_summary_ref",
            "status/retained_summary.json",
        )?;
        ensure_exact(
            self.promotion_evidence_ref.as_str(),
            "rehearsal_bundle.promotion_evidence_ref",
            "promotion/promotion_target_evidence.json",
        )?;
        if self.checker_receipt_refs.len() != 2 {
            return Err(TassadarDefaultTrainRehearsalError::Invalid {
                field: String::from("rehearsal_bundle.checker_receipt_refs"),
                detail: String::from("expected exactly two checker receipts"),
            });
        }
        if self.evidence_artifacts.is_empty() {
            return Err(TassadarDefaultTrainRehearsalError::Invalid {
                field: String::from("rehearsal_bundle.evidence_artifacts"),
                detail: String::from("expected at least one evidence artifact"),
            });
        }
        if self.closeout_gates.is_empty() {
            return Err(TassadarDefaultTrainRehearsalError::Invalid {
                field: String::from("rehearsal_bundle.closeout_gates"),
                detail: String::from("expected at least one closeout gate"),
            });
        }
        if self.can_now_claim.is_empty() || self.still_out_of_scope.is_empty() {
            return Err(TassadarDefaultTrainRehearsalError::Invalid {
                field: String::from("rehearsal_bundle.claim_lists"),
                detail: String::from("claim-boundary sections must stay non-empty"),
            });
        }
        ensure_nonempty(self.rehearsal_id.as_str(), "rehearsal_bundle.rehearsal_id")?;
        ensure_nonempty(
            self.launcher_surface_id.as_str(),
            "rehearsal_bundle.launcher_surface_id",
        )?;
        ensure_nonempty(self.run_id.as_str(), "rehearsal_bundle.run_id")?;
        ensure_nonempty(self.run_root.as_str(), "rehearsal_bundle.run_root")?;
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "rehearsal_bundle.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "rehearsal_bundle.detail")?;
        Ok(())
    }
}

pub fn write_tassadar_default_train_rehearsal_fixtures(
    workspace_root: &Path,
) -> Result<TassadarDefaultTrainRehearsalBundle, TassadarDefaultTrainRehearsalError> {
    let contract = builtin_tassadar_default_train_lane_contract(workspace_root)?;
    let example_run_root = workspace_root
        .join(DEFAULT_REHEARSAL_EXAMPLE_ROOT)
        .join(DEFAULT_REHEARSAL_RUN_ID);
    let launcher_output = write_tassadar_train_launcher_outputs(
        workspace_root,
        TassadarTrainLauncherPhase::LaunchStaged,
        Some(contract.lane_id.as_str()),
        example_run_root.as_path(),
    )?;

    let lane_contract_checker = TassadarDefaultTrainCheckerReceipt {
        schema_version: String::from(
            TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_SCHEMA_VERSION,
        ),
        receipt_id: String::from("tassadar_default_train_lane_contract_checker_receipt_v1"),
        lane_id: contract.lane_id.clone(),
        checker_bundle_id: contract.checker_bundle_id.clone(),
        checker_command: String::from("scripts/check-tassadar-default-train-lane.sh"),
        checked_artifact: artifact_ref(workspace_root, TASSADAR_DEFAULT_TRAIN_LANE_FIXTURE_PATH)?,
        status: String::from("green"),
        detail: String::from(
            "The focused default-lane checker keeps the canonical launcher, hardware profile, run-root family, checker bundle, and promotion target bound to the incumbent default lane.",
        ),
    };
    lane_contract_checker.validate()?;

    let acceptance_checker = TassadarDefaultTrainCheckerReceipt {
        schema_version: String::from(TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_SCHEMA_VERSION),
        receipt_id: String::from("tassadar_default_train_acceptance_checker_receipt_v1"),
        lane_id: contract.lane_id.clone(),
        checker_bundle_id: contract.checker_bundle_id.clone(),
        checker_command: String::from("scripts/check-tassadar-acceptance.sh"),
        checked_artifact: artifact_ref(workspace_root, contract.anchor_run_bundle_ref.as_str())?,
        status: String::from("green"),
        detail: String::from(
            "The broader Tassadar acceptance checker keeps the retained article-transformer weight-production bundle bound into the same checker bundle as the launcher default.",
        ),
    };
    acceptance_checker.validate()?;

    let promotion_evidence = TassadarDefaultTrainPromotionEvidence {
        schema_version: String::from(TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_SCHEMA_VERSION),
        evidence_id: String::from("tassadar_default_train_promotion_evidence_v1"),
        lane_id: contract.lane_id.clone(),
        promotion_target_model_id: contract.promotion_target_model_id.clone(),
        promotion_target_descriptor: artifact_ref(
            workspace_root,
            contract.promotion_target_descriptor_ref.as_str(),
        )?,
        promotion_target_artifact: artifact_ref(
            workspace_root,
            contract.promotion_target_artifact_ref.as_str(),
        )?,
        promotion_target_artifact_id: contract.promotion_target_artifact_id.clone(),
        promotion_target_lineage: artifact_ref(
            workspace_root,
            contract.promotion_target_lineage_ref.as_str(),
        )?,
        anchor_run_bundle: artifact_ref(workspace_root, contract.anchor_run_bundle_ref.as_str())?,
        status: String::from("bound_to_canonical_trained_v0"),
        detail: String::from(
            "The rehearsal carries the incumbent trained-v0 descriptor, weight artifact, lineage contract, and anchor run bundle together so the default lane stays tied to the same model family already served and cited elsewhere in the repo.",
        ),
    };
    promotion_evidence.validate()?;

    write_json(
        workspace_root.join(TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_FIXTURE_PATH),
        &lane_contract_checker,
    )?;
    write_json(
        workspace_root.join(TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_FIXTURE_PATH),
        &acceptance_checker,
    )?;
    write_json(
        workspace_root.join(TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_FIXTURE_PATH),
        &promotion_evidence,
    )?;
    write_json(
        example_run_root.join("checker/default_train_lane_contract_check.json"),
        &lane_contract_checker,
    )?;
    write_json(
        example_run_root.join("checker/acceptance_check.json"),
        &acceptance_checker,
    )?;
    write_json(
        example_run_root.join("promotion/promotion_target_evidence.json"),
        &promotion_evidence,
    )?;

    let evidence_artifacts = vec![
        rehearsal_artifact(
            workspace_root,
            "launch_manifest",
            example_run_root.join("manifests/launch_manifest.json").as_path(),
            "The bounded rehearsal keeps the start-surface launch manifest under the retained operator run root.",
        )?,
        rehearsal_artifact(
            workspace_root,
            "current_run_status",
            example_run_root.join("status/current_run_status.json").as_path(),
            "The bounded rehearsal keeps the current run status under the same operator run root.",
        )?,
        rehearsal_artifact(
            workspace_root,
            "retained_summary",
            example_run_root.join("status/retained_summary.json").as_path(),
            "The bounded rehearsal keeps the launcher retained summary under the same operator run root.",
        )?,
        rehearsal_artifact(
            workspace_root,
            "default_lane_contract_checker_receipt",
            workspace_root
                .join(TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_FIXTURE_PATH)
                .as_path(),
            "The focused default-lane checker receipt stays green for the incumbent default lane.",
        )?,
        rehearsal_artifact(
            workspace_root,
            "acceptance_checker_receipt",
            workspace_root
                .join(TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_FIXTURE_PATH)
                .as_path(),
            "The broader acceptance checker receipt keeps the retained weight-production bundle inside the same operator proof packet.",
        )?,
        rehearsal_artifact(
            workspace_root,
            "promotion_evidence",
            workspace_root
                .join(TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_FIXTURE_PATH)
                .as_path(),
            "The rehearsal keeps the incumbent trained-v0 promotion-target evidence explicit rather than implying a separate promotion path.",
        )?,
    ];

    let bundle = TassadarDefaultTrainRehearsalBundle {
        schema_version: String::from(TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_SCHEMA_VERSION),
        rehearsal_id: String::from(DEFAULT_REHEARSAL_ID),
        lane_id: contract.lane_id,
        launcher_path: contract.launcher_path,
        launcher_surface_id: launcher_output.launch_manifest.launcher_surface_id,
        run_id: launcher_output.launch_manifest.run_id,
        run_root: repo_relative_path(workspace_root, example_run_root.as_path()),
        launch_manifest_ref: String::from("manifests/launch_manifest.json"),
        current_run_status_ref: String::from("status/current_run_status.json"),
        retained_summary_ref: String::from("status/retained_summary.json"),
        checker_receipt_refs: vec![
            String::from("checker/default_train_lane_contract_check.json"),
            String::from("checker/acceptance_check.json"),
        ],
        promotion_evidence_ref: String::from("promotion/promotion_target_evidence.json"),
        evidence_artifacts,
        closeout_gates: vec![
            TassadarDefaultTrainRehearsalGate {
                gate_id: String::from("launcher_surface_bound"),
                satisfied: true,
                detail: String::from(
                    "The bounded rehearsal keeps the start-surface manifest, current status, and retained summary under one operator run root.",
                ),
            },
            TassadarDefaultTrainRehearsalGate {
                gate_id: String::from("default_checker_bound"),
                satisfied: true,
                detail: String::from(
                    "The focused default-lane checker receipt keeps the frozen lane contract and launcher meaning explicit.",
                ),
            },
            TassadarDefaultTrainRehearsalGate {
                gate_id: String::from("acceptance_checker_bound"),
                satisfied: true,
                detail: String::from(
                    "The broader acceptance checker receipt keeps the retained weight-production bundle inside the same operator proof packet.",
                ),
            },
            TassadarDefaultTrainRehearsalGate {
                gate_id: String::from("promotion_target_bound"),
                satisfied: true,
                detail: String::from(
                    "The rehearsal binds the incumbent trained-v0 descriptor, artifact, lineage contract, and anchor run bundle into one promotion-evidence packet.",
                ),
            },
        ],
        can_now_claim: vec![
            String::from(
                "Tassadar now has one bounded default-lane rehearsal that cites the same launcher path, checker bundle, and promotion-target family from retained repo truth.",
            ),
            String::from(
                "The canonical meaning of `./TRAIN_TASSADAR` is operator-legible at launcher level for the incumbent article-transformer trained-v0 lane.",
            ),
        ],
        still_out_of_scope: vec![
            String::from(
                "This rehearsal does not claim that every historical Tassadar lane has equal launcher or checker parity.",
            ),
            String::from(
                "This rehearsal does not promote any later 4080 executor candidate or redefine the incumbent default lane above the retained trained-v0 family.",
            ),
            String::from(
                "This rehearsal does not widen the article-equivalence claim boundary beyond the bounded trained-v0 artifact family already cited elsewhere in the repo.",
            ),
        ],
        claim_boundary: String::from(
            "This closeout bundle proves one bounded default-lane rehearsal for the incumbent Tassadar train operator path, including explicit start-surface outputs, two retained checker receipts, and one retained promotion-target packet. It does not claim broader lane unification, later candidate promotion, or new article-equivalence closure beyond the existing trained-v0 family.",
        ),
        detail: String::from(
            "The default-lane rehearsal ties the exact launcher run root, checker receipts, and incumbent promotion-target evidence into one operator-readable proof packet for Tassadar parity.",
        ),
    };
    bundle.validate()?;

    write_json(
        workspace_root.join(TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_FIXTURE_PATH),
        &bundle,
    )?;
    write_json(
        example_run_root.join("closeout/rehearsal_bundle.json"),
        &bundle,
    )?;
    Ok(bundle)
}

fn rehearsal_artifact(
    workspace_root: &Path,
    artifact_kind: &str,
    path: &Path,
    detail: &str,
) -> Result<TassadarDefaultTrainRehearsalArtifact, TassadarDefaultTrainRehearsalError> {
    Ok(TassadarDefaultTrainRehearsalArtifact {
        artifact_kind: String::from(artifact_kind),
        artifact: artifact_ref_absolute(workspace_root, path)?,
        detail: String::from(detail),
    })
}

fn artifact_ref(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<PsionActualPretrainingArtifactRef, TassadarDefaultTrainRehearsalError> {
    artifact_ref_absolute(workspace_root, workspace_root.join(relative_path).as_path())
}

fn artifact_ref_absolute(
    workspace_root: &Path,
    absolute_path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, TassadarDefaultTrainRehearsalError> {
    let contents =
        fs::read(absolute_path).map_err(|error| TassadarDefaultTrainRehearsalError::Read {
            path: absolute_path.display().to_string(),
            error,
        })?;
    Ok(PsionActualPretrainingArtifactRef {
        path: repo_relative_path(workspace_root, absolute_path),
        sha256: hex::encode(Sha256::digest(contents)),
    })
}

fn validate_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field: &str,
) -> Result<(), TassadarDefaultTrainRehearsalError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field}.sha256"))?;
    Ok(())
}

fn write_json<T: Serialize>(
    path: PathBuf,
    value: &T,
) -> Result<(), TassadarDefaultTrainRehearsalError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarDefaultTrainRehearsalError::Write {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contents = serde_json::to_vec_pretty(value)?;
    fs::write(&path, contents).map_err(|error| TassadarDefaultTrainRehearsalError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), TassadarDefaultTrainRehearsalError> {
    if value.trim().is_empty() {
        return Err(TassadarDefaultTrainRehearsalError::Invalid {
            field: String::from(field),
            detail: String::from("field must stay non-empty"),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), TassadarDefaultTrainRehearsalError> {
    if actual != expected {
        return Err(TassadarDefaultTrainRehearsalError::Invalid {
            field: String::from(field),
            detail: format!("expected `{expected}` but found `{actual}`"),
        });
    }
    Ok(())
}

fn repo_relative_path(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches("./")
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{
        write_tassadar_default_train_rehearsal_fixtures, TassadarDefaultTrainCheckerReceipt,
        TassadarDefaultTrainPromotionEvidence, TassadarDefaultTrainRehearsalBundle,
        TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_FIXTURE_PATH,
        TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_FIXTURE_PATH,
        TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_FIXTURE_PATH,
        TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_FIXTURE_PATH,
    };

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn tassadar_default_train_rehearsal_fixture_paths_stay_stable() {
        assert_eq!(
            TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_default_train_lane_contract_checker_receipt_v1.json"
        );
        assert_eq!(
            TASSADAR_DEFAULT_TRAIN_ACCEPTANCE_CHECKER_RECEIPT_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_default_train_acceptance_checker_receipt_v1.json"
        );
        assert_eq!(
            TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_default_train_promotion_evidence_v1.json"
        );
        assert_eq!(
            TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_default_train_rehearsal_bundle_v1.json"
        );
    }

    #[test]
    fn tassadar_default_train_rehearsal_bundle_fixture_validates() {
        let bundle: TassadarDefaultTrainRehearsalBundle = serde_json::from_str(
            &fs::read_to_string(
                workspace_root().join(TASSADAR_DEFAULT_TRAIN_REHEARSAL_BUNDLE_FIXTURE_PATH),
            )
            .expect("bundle fixture"),
        )
        .expect("bundle parse");
        bundle.validate().expect("bundle should validate");
        assert_eq!(bundle.launcher_surface_id, "tassadar_train_start");
    }

    #[test]
    fn tassadar_default_train_supporting_receipts_validate() {
        let lane_checker: TassadarDefaultTrainCheckerReceipt = serde_json::from_str(
            &fs::read_to_string(
                workspace_root()
                    .join(TASSADAR_DEFAULT_TRAIN_LANE_CONTRACT_CHECKER_RECEIPT_FIXTURE_PATH),
            )
            .expect("lane checker fixture"),
        )
        .expect("lane checker parse");
        lane_checker
            .validate()
            .expect("lane checker should validate");

        let promotion: TassadarDefaultTrainPromotionEvidence = serde_json::from_str(
            &fs::read_to_string(
                workspace_root().join(TASSADAR_DEFAULT_TRAIN_PROMOTION_EVIDENCE_FIXTURE_PATH),
            )
            .expect("promotion fixture"),
        )
        .expect("promotion parse");
        promotion.validate().expect("promotion should validate");
    }

    #[test]
    fn tassadar_default_train_fixture_writer_keeps_default_lane() {
        let bundle = write_tassadar_default_train_rehearsal_fixtures(workspace_root().as_path())
            .expect("fixture writer");
        assert_eq!(
            bundle.lane_id,
            "tassadar_article_transformer_trace_bound_trained_v0"
        );
        assert_eq!(
            bundle.checker_receipt_refs,
            vec![
                "checker/default_train_lane_contract_check.json",
                "checker/acceptance_check.json",
            ]
        );
    }
}
