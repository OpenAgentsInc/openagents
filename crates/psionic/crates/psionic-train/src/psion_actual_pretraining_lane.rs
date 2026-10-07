use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for the canonical Psion actual-pretraining lane spec.
pub const PSION_ACTUAL_PRETRAINING_LANE_SPEC_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_lane_spec.v1";

/// Canonical actual-lane identity above the bounded reference pilot.
pub const PSION_ACTUAL_PRETRAINING_LANE_ID: &str = "psion_actual_pretraining_v1";

/// Machine-checkable spec for the canonical Psion actual-pretraining lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingLaneSpec {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable stage-program identifier for the actual lane.
    pub stage_program_id: String,
    /// Stable training profile name carried by the actual lane.
    pub training_run_profile: String,
    /// Stable run-root family for actual-lane operator outputs.
    pub run_root_family: String,
    /// Stable evidence-family identifier for retained actual-lane artifacts.
    pub evidence_family: String,
    /// Bounded reference lane that remains distinct from the actual lane.
    pub bounded_reference_lane_id: String,
    /// Path to the anchor bundle that proves the admitted broader-pretraining lane.
    pub anchor_run_bundle_ref: String,
    /// Stable digest of the anchor bundle.
    pub anchor_run_bundle_digest: String,
    /// Short summary of the lane boundary.
    pub summary: String,
}

impl PsionActualPretrainingLaneSpec {
    /// Validates the lane spec.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLaneSpecError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "actual_pretraining_lane.schema_version",
        )?;
        if self.schema_version != PSION_ACTUAL_PRETRAINING_LANE_SPEC_SCHEMA_VERSION {
            return Err(PsionActualPretrainingLaneSpecError::SchemaVersionMismatch {
                expected: String::from(PSION_ACTUAL_PRETRAINING_LANE_SPEC_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.lane_id.as_str(), "actual_pretraining_lane.lane_id")?;
        if self.lane_id != PSION_ACTUAL_PRETRAINING_LANE_ID {
            return Err(PsionActualPretrainingLaneSpecError::LaneIdMismatch {
                expected: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                actual: self.lane_id.clone(),
            });
        }
        ensure_nonempty(
            self.stage_program_id.as_str(),
            "actual_pretraining_lane.stage_program_id",
        )?;
        ensure_nonempty(
            self.training_run_profile.as_str(),
            "actual_pretraining_lane.training_run_profile",
        )?;
        if self.training_run_profile != "broader_pretraining" {
            return Err(PsionActualPretrainingLaneSpecError::TrainingRunProfileMismatch {
                expected: String::from("broader_pretraining"),
                actual: self.training_run_profile.clone(),
            });
        }
        ensure_nonempty(
            self.run_root_family.as_str(),
            "actual_pretraining_lane.run_root_family",
        )?;
        ensure_nonempty(
            self.evidence_family.as_str(),
            "actual_pretraining_lane.evidence_family",
        )?;
        ensure_nonempty(
            self.bounded_reference_lane_id.as_str(),
            "actual_pretraining_lane.bounded_reference_lane_id",
        )?;
        if self.bounded_reference_lane_id == self.lane_id {
            return Err(
                PsionActualPretrainingLaneSpecError::ReferenceLaneMustStayDistinct,
            );
        }
        ensure_nonempty(
            self.anchor_run_bundle_ref.as_str(),
            "actual_pretraining_lane.anchor_run_bundle_ref",
        )?;
        ensure_nonempty(
            self.anchor_run_bundle_digest.as_str(),
            "actual_pretraining_lane.anchor_run_bundle_digest",
        )?;
        ensure_nonempty(self.summary.as_str(), "actual_pretraining_lane.summary")?;
        Ok(())
    }
}

/// Validation errors for the actual-pretraining lane spec.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingLaneSpecError {
    #[error("psion actual-pretraining lane field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining lane schema version mismatch: expected `{expected}`, got `{actual}`"
    )]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("psion actual-pretraining lane id mismatch: expected `{expected}`, got `{actual}`")]
    LaneIdMismatch { expected: String, actual: String },
    #[error(
        "psion actual-pretraining lane run profile mismatch: expected `{expected}`, got `{actual}`"
    )]
    TrainingRunProfileMismatch { expected: String, actual: String },
    #[error("psion actual-pretraining lane must stay distinct from the bounded reference lane")]
    ReferenceLaneMustStayDistinct,
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionActualPretrainingLaneSpecError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingLaneSpecError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PsionActualPretrainingLaneSpec;

    fn lane_spec() -> PsionActualPretrainingLaneSpec {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_lane_spec_v1.json"
        ))
        .expect("actual pretraining lane spec fixture should parse")
    }

    #[test]
    fn actual_pretraining_lane_fixture_validates() {
        lane_spec()
            .validate()
            .expect("actual pretraining lane spec fixture should validate");
    }

    #[test]
    fn actual_pretraining_lane_rejects_reference_lane_aliasing() {
        let mut spec = lane_spec();
        spec.bounded_reference_lane_id = spec.lane_id.clone();
        let error = spec
            .validate()
            .expect_err("reference lane aliasing should be rejected");
        assert_eq!(
            error,
            super::PsionActualPretrainingLaneSpecError::ReferenceLaneMustStayDistinct
        );
    }
}
