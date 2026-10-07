use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for the canonical Tassadar default-train-lane contract.
pub const TASSADAR_DEFAULT_TRAIN_LANE_SCHEMA_VERSION: &str =
    "tassadar.default_train_lane_contract.v1";

/// Stable lane identifier for the canonical Tassadar default-train lane.
pub const TASSADAR_DEFAULT_TRAIN_LANE_ID: &str =
    "tassadar_article_transformer_trace_bound_trained_v0";

/// Stable launcher path for the canonical Tassadar default-train lane.
pub const TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH: &str = "./TRAIN_TASSADAR";

/// Stable fixture path for the canonical Tassadar default-train-lane contract.
pub const TASSADAR_DEFAULT_TRAIN_LANE_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_default_train_lane_contract_v1.json";

/// Stable doc path for the canonical Tassadar default-train-lane contract.
pub const TASSADAR_DEFAULT_TRAIN_LANE_DOC_PATH: &str = "docs/TASSADAR_DEFAULT_TRAIN_LANE.md";

/// Stable checker path for the canonical Tassadar default-train lane.
pub const TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH: &str =
    "scripts/check-tassadar-default-train-lane.sh";

const WEIGHT_PRODUCTION_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_article_transformer_weight_production_v1/article_transformer_weight_production_bundle.json";
const TRAINED_V0_LINEAGE_CONTRACT_REF: &str = "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json";
const TASSADAR_ACCEPTANCE_CHECKER_PATH: &str = "scripts/check-tassadar-acceptance.sh";

/// Machine-checkable contract for the canonical Tassadar default-train lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDefaultTrainLaneContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable launcher path.
    pub launcher_path: String,
    /// Stable launcher surface identifier.
    pub launcher_surface_id: String,
    /// Stable stage-program identifier.
    pub stage_program_id: String,
    /// Stable training run profile.
    pub training_run_profile: String,
    /// Stable hardware-profile identifier.
    pub hardware_profile_id: String,
    /// Stable writer-node identity surfaced by the retained checkpoint family.
    pub writer_node_id: String,
    /// Stable output-root family for the default lane.
    pub run_root_family: String,
    /// Stable evidence-family identifier for the default lane.
    pub evidence_family: String,
    /// Stable checker-bundle identifier.
    pub checker_bundle_id: String,
    /// Stable checker refs.
    pub checker_refs: Vec<String>,
    /// Explicit restart posture for the default lane.
    pub restart_posture: String,
    /// Stable promotion-target model identifier.
    pub promotion_target_model_id: String,
    /// Stable promotion-target descriptor ref.
    pub promotion_target_descriptor_ref: String,
    /// Stable promotion-target artifact ref.
    pub promotion_target_artifact_ref: String,
    /// Stable promotion-target artifact id.
    pub promotion_target_artifact_id: String,
    /// Stable promotion-target lineage ref.
    pub promotion_target_lineage_ref: String,
    /// Stable anchor run-bundle ref proving the default lane.
    pub anchor_run_bundle_ref: String,
    /// Stable digest of the anchor run bundle.
    pub anchor_run_bundle_digest: String,
    /// Short summary of the lane boundary.
    pub summary: String,
}

impl TassadarDefaultTrainLaneContract {
    /// Validates the default-lane contract.
    pub fn validate(&self) -> Result<(), TassadarDefaultTrainLaneContractError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "tassadar_default_train_lane.schema_version",
        )?;
        if self.schema_version != TASSADAR_DEFAULT_TRAIN_LANE_SCHEMA_VERSION {
            return Err(
                TassadarDefaultTrainLaneContractError::SchemaVersionMismatch {
                    expected: String::from(TASSADAR_DEFAULT_TRAIN_LANE_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(self.lane_id.as_str(), "tassadar_default_train_lane.lane_id")?;
        if self.lane_id != TASSADAR_DEFAULT_TRAIN_LANE_ID {
            return Err(TassadarDefaultTrainLaneContractError::LaneIdMismatch {
                expected: String::from(TASSADAR_DEFAULT_TRAIN_LANE_ID),
                actual: self.lane_id.clone(),
            });
        }
        ensure_nonempty(
            self.launcher_path.as_str(),
            "tassadar_default_train_lane.launcher_path",
        )?;
        if self.launcher_path != TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH {
            return Err(
                TassadarDefaultTrainLaneContractError::LauncherPathMismatch {
                    expected: String::from(TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH),
                    actual: self.launcher_path.clone(),
                },
            );
        }
        ensure_nonempty(
            self.launcher_surface_id.as_str(),
            "tassadar_default_train_lane.launcher_surface_id",
        )?;
        ensure_nonempty(
            self.stage_program_id.as_str(),
            "tassadar_default_train_lane.stage_program_id",
        )?;
        ensure_nonempty(
            self.training_run_profile.as_str(),
            "tassadar_default_train_lane.training_run_profile",
        )?;
        if self.training_run_profile != "bounded_article_weight_production" {
            return Err(
                TassadarDefaultTrainLaneContractError::TrainingRunProfileMismatch {
                    expected: String::from("bounded_article_weight_production"),
                    actual: self.training_run_profile.clone(),
                },
            );
        }
        ensure_nonempty(
            self.hardware_profile_id.as_str(),
            "tassadar_default_train_lane.hardware_profile_id",
        )?;
        if self.hardware_profile_id != "cpu_reference" {
            return Err(
                TassadarDefaultTrainLaneContractError::HardwareProfileMismatch {
                    expected: String::from("cpu_reference"),
                    actual: self.hardware_profile_id.clone(),
                },
            );
        }
        ensure_nonempty(
            self.writer_node_id.as_str(),
            "tassadar_default_train_lane.writer_node_id",
        )?;
        ensure_nonempty(
            self.run_root_family.as_str(),
            "tassadar_default_train_lane.run_root_family",
        )?;
        if self.run_root_family
            != "fixtures/tassadar/runs/tassadar_article_transformer_weight_production_v1"
        {
            return Err(
                TassadarDefaultTrainLaneContractError::RunRootFamilyMismatch {
                    expected: String::from(
                        "fixtures/tassadar/runs/tassadar_article_transformer_weight_production_v1",
                    ),
                    actual: self.run_root_family.clone(),
                },
            );
        }
        ensure_nonempty(
            self.evidence_family.as_str(),
            "tassadar_default_train_lane.evidence_family",
        )?;
        if self.evidence_family != "train.tassadar.article_transformer.weight_production" {
            return Err(
                TassadarDefaultTrainLaneContractError::EvidenceFamilyMismatch {
                    expected: String::from("train.tassadar.article_transformer.weight_production"),
                    actual: self.evidence_family.clone(),
                },
            );
        }
        ensure_nonempty(
            self.checker_bundle_id.as_str(),
            "tassadar_default_train_lane.checker_bundle_id",
        )?;
        if !self
            .checker_refs
            .iter()
            .any(|value| value == TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH)
        {
            return Err(TassadarDefaultTrainLaneContractError::MissingCheckerRef {
                checker_ref: String::from(TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH),
            });
        }
        if !self
            .checker_refs
            .iter()
            .any(|value| value == TASSADAR_ACCEPTANCE_CHECKER_PATH)
        {
            return Err(TassadarDefaultTrainLaneContractError::MissingCheckerRef {
                checker_ref: String::from(TASSADAR_ACCEPTANCE_CHECKER_PATH),
            });
        }
        ensure_nonempty(
            self.restart_posture.as_str(),
            "tassadar_default_train_lane.restart_posture",
        )?;
        if self.restart_posture != "restart_from_trace_bound_base_v0" {
            return Err(
                TassadarDefaultTrainLaneContractError::RestartPostureMismatch {
                    expected: String::from("restart_from_trace_bound_base_v0"),
                    actual: self.restart_posture.clone(),
                },
            );
        }
        ensure_nonempty(
            self.promotion_target_model_id.as_str(),
            "tassadar_default_train_lane.promotion_target_model_id",
        )?;
        if self.promotion_target_model_id != "tassadar-article-transformer-trace-bound-trained-v0" {
            return Err(
                TassadarDefaultTrainLaneContractError::PromotionTargetModelMismatch {
                    expected: String::from("tassadar-article-transformer-trace-bound-trained-v0"),
                    actual: self.promotion_target_model_id.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "tassadar_default_train_lane.promotion_target_descriptor_ref",
                self.promotion_target_descriptor_ref.as_str(),
            ),
            (
                "tassadar_default_train_lane.promotion_target_artifact_ref",
                self.promotion_target_artifact_ref.as_str(),
            ),
            (
                "tassadar_default_train_lane.promotion_target_artifact_id",
                self.promotion_target_artifact_id.as_str(),
            ),
            (
                "tassadar_default_train_lane.promotion_target_lineage_ref",
                self.promotion_target_lineage_ref.as_str(),
            ),
            (
                "tassadar_default_train_lane.anchor_run_bundle_ref",
                self.anchor_run_bundle_ref.as_str(),
            ),
            (
                "tassadar_default_train_lane.anchor_run_bundle_digest",
                self.anchor_run_bundle_digest.as_str(),
            ),
            ("tassadar_default_train_lane.summary", self.summary.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }
        Ok(())
    }
}

/// Builds the canonical Tassadar default-train-lane contract from retained repo truth.
pub fn builtin_tassadar_default_train_lane_contract(
    workspace_root: &Path,
) -> Result<TassadarDefaultTrainLaneContract, TassadarDefaultTrainLaneContractError> {
    let bundle: WeightProductionBundle = read_json(workspace_root, WEIGHT_PRODUCTION_BUNDLE_REF)?;
    let lineage: LineageContract = read_json(workspace_root, TRAINED_V0_LINEAGE_CONTRACT_REF)?;
    let descriptor: ProducedDescriptor =
        read_json(workspace_root, lineage.produced_descriptor_ref.as_str())?;
    let hardware_profile_id =
        derive_hardware_profile_id(bundle.checkpoint.writer_node_id.as_str())?;

    let contract = TassadarDefaultTrainLaneContract {
        schema_version: String::from(TASSADAR_DEFAULT_TRAIN_LANE_SCHEMA_VERSION),
        lane_id: String::from(TASSADAR_DEFAULT_TRAIN_LANE_ID),
        launcher_path: String::from(TASSADAR_DEFAULT_TRAIN_LAUNCHER_PATH),
        launcher_surface_id: String::from("tassadar_train_default_start"),
        stage_program_id: String::from("tassadar_article_transformer_weight_production_v1"),
        training_run_profile: String::from("bounded_article_weight_production"),
        hardware_profile_id,
        writer_node_id: bundle.checkpoint.writer_node_id,
        run_root_family: String::from(
            "fixtures/tassadar/runs/tassadar_article_transformer_weight_production_v1",
        ),
        evidence_family: bundle.checkpoint_family,
        checker_bundle_id: String::from("tassadar_default_train_lane_checker_bundle_v1"),
        checker_refs: vec![
            String::from(TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH),
            String::from(TASSADAR_ACCEPTANCE_CHECKER_PATH),
        ],
        restart_posture: String::from("restart_from_trace_bound_base_v0"),
        promotion_target_model_id: descriptor.model.model_id,
        promotion_target_descriptor_ref: lineage.produced_descriptor_ref,
        promotion_target_artifact_ref: lineage.produced_artifact_ref,
        promotion_target_artifact_id: descriptor.artifact_binding.artifact_id,
        promotion_target_lineage_ref: String::from(TRAINED_V0_LINEAGE_CONTRACT_REF),
        anchor_run_bundle_ref: String::from(WEIGHT_PRODUCTION_BUNDLE_REF),
        anchor_run_bundle_digest: bundle.bundle_digest,
        summary: String::from(
            "Canonical Tassadar train default is the bounded trace-bound article-transformer weight-production lane that yields `tassadar-article-transformer-trace-bound-trained-v0`, because that is the retained model family later served, checked, and promoted across the repo. The older 4x4 and 9x9 learned lanes remain bounded benchmark evidence, and the 4080 executor candidate remains a later replacement track above this incumbent instead of the default operator meaning of `train Tassadar`.",
        ),
    };
    contract.validate()?;
    Ok(contract)
}

/// Writes the canonical Tassadar default-train-lane contract fixture.
pub fn write_builtin_tassadar_default_train_lane_contract(
    workspace_root: &Path,
) -> Result<TassadarDefaultTrainLaneContract, TassadarDefaultTrainLaneContractError> {
    let contract = builtin_tassadar_default_train_lane_contract(workspace_root)?;
    let output_path = workspace_root.join(TASSADAR_DEFAULT_TRAIN_LANE_FIXTURE_PATH);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarDefaultTrainLaneContractError::Write {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contents = serde_json::to_vec_pretty(&contract)?;
    fs::write(&output_path, contents).map_err(|error| {
        TassadarDefaultTrainLaneContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

/// Validation errors for the canonical Tassadar default-train-lane contract.
#[derive(Debug, Error)]
pub enum TassadarDefaultTrainLaneContractError {
    #[error("tassadar default-train lane field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "tassadar default-train lane schema version mismatch: expected `{expected}`, got `{actual}`"
    )]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("tassadar default-train lane id mismatch: expected `{expected}`, got `{actual}`")]
    LaneIdMismatch { expected: String, actual: String },
    #[error("tassadar default-train launcher path mismatch: expected `{expected}`, got `{actual}`")]
    LauncherPathMismatch { expected: String, actual: String },
    #[error("tassadar default-train profile mismatch: expected `{expected}`, got `{actual}`")]
    TrainingRunProfileMismatch { expected: String, actual: String },
    #[error(
        "tassadar default-train hardware profile mismatch: expected `{expected}`, got `{actual}`"
    )]
    HardwareProfileMismatch { expected: String, actual: String },
    #[error(
        "tassadar default-train run-root family mismatch: expected `{expected}`, got `{actual}`"
    )]
    RunRootFamilyMismatch { expected: String, actual: String },
    #[error(
        "tassadar default-train evidence family mismatch: expected `{expected}`, got `{actual}`"
    )]
    EvidenceFamilyMismatch { expected: String, actual: String },
    #[error("tassadar default-train contract is missing checker ref `{checker_ref}`")]
    MissingCheckerRef { checker_ref: String },
    #[error(
        "tassadar default-train restart posture mismatch: expected `{expected}`, got `{actual}`"
    )]
    RestartPostureMismatch { expected: String, actual: String },
    #[error(
        "tassadar default-train promotion target mismatch: expected `{expected}`, got `{actual}`"
    )]
    PromotionTargetModelMismatch { expected: String, actual: String },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(
        "tassadar default-train lane cannot derive a hardware profile from writer node `{writer_node_id}`"
    )]
    UnsupportedWriterNode { writer_node_id: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct WeightProductionBundle {
    bundle_digest: String,
    checkpoint_family: String,
    checkpoint: WeightProductionCheckpoint,
}

#[derive(Debug, Deserialize)]
struct WeightProductionCheckpoint {
    writer_node_id: String,
}

#[derive(Debug, Deserialize)]
struct LineageContract {
    produced_descriptor_ref: String,
    produced_artifact_ref: String,
}

#[derive(Debug, Deserialize)]
struct ProducedDescriptor {
    model: ProducedModelIdentity,
    artifact_binding: ProducedArtifactBinding,
}

#[derive(Debug, Deserialize)]
struct ProducedModelIdentity {
    model_id: String,
}

#[derive(Debug, Deserialize)]
struct ProducedArtifactBinding {
    artifact_id: String,
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), TassadarDefaultTrainLaneContractError> {
    if value.trim().is_empty() {
        return Err(TassadarDefaultTrainLaneContractError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn derive_hardware_profile_id(
    writer_node_id: &str,
) -> Result<String, TassadarDefaultTrainLaneContractError> {
    if writer_node_id.contains("cpu_reference") {
        return Ok(String::from("cpu_reference"));
    }
    Err(
        TassadarDefaultTrainLaneContractError::UnsupportedWriterNode {
            writer_node_id: String::from(writer_node_id),
        },
    )
}

fn read_json<T: for<'de> Deserialize<'de>>(
    workspace_root: &Path,
    relative_ref: &str,
) -> Result<T, TassadarDefaultTrainLaneContractError> {
    let path = workspace_root.join(relative_ref);
    let contents =
        fs::read_to_string(&path).map_err(|error| TassadarDefaultTrainLaneContractError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_str(&contents).map_err(|error| TassadarDefaultTrainLaneContractError::Parse {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH, TASSADAR_DEFAULT_TRAIN_LANE_FIXTURE_PATH,
        TassadarDefaultTrainLaneContract, builtin_tassadar_default_train_lane_contract,
        workspace_root,
    };

    fn contract_fixture() -> TassadarDefaultTrainLaneContract {
        serde_json::from_str(include_str!(
            "../../../fixtures/tassadar/operator/tassadar_default_train_lane_contract_v1.json"
        ))
        .expect("tassadar default-train lane fixture should parse")
    }

    #[test]
    fn tassadar_default_train_lane_fixture_validates() {
        contract_fixture()
            .validate()
            .expect("tassadar default-train lane fixture should validate");
    }

    #[test]
    fn builtin_contract_matches_committed_fixture() {
        let root = workspace_root();
        let expected = contract_fixture();
        let actual =
            builtin_tassadar_default_train_lane_contract(root.as_path()).expect("contract");
        assert_eq!(actual, expected);
    }

    #[test]
    fn tassadar_default_train_lane_requires_canonical_checker() {
        let mut contract = contract_fixture();
        contract
            .checker_refs
            .retain(|value| value != TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH);
        let error = contract
            .validate()
            .expect_err("missing checker should fail");
        assert!(matches!(
            error,
            super::TassadarDefaultTrainLaneContractError::MissingCheckerRef { checker_ref }
            if checker_ref == TASSADAR_DEFAULT_TRAIN_LANE_CHECKER_PATH
        ));
    }

    #[test]
    fn default_train_lane_fixture_path_stays_stable() {
        assert_eq!(
            TASSADAR_DEFAULT_TRAIN_LANE_FIXTURE_PATH,
            "fixtures/tassadar/operator/tassadar_default_train_lane_contract_v1.json"
        );
    }
}
