use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{
    CadSketchConstraint, CadSketchConstraintStatusReport, CadSketchEntity, CadSketchModel,
    CadSketchPlane,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_CONSTRAINT_STATUS_ISSUE_ID: &str = "VCAD-PARITY-046";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchConstraintStatusParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub under_case: ConstraintStatusCaseSnapshot,
    pub fully_case: ConstraintStatusCaseSnapshot,
    pub over_case: ConstraintStatusCaseSnapshot,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstraintStatusCaseSnapshot {
    pub case_id: String,
    pub status_report: CadSketchConstraintStatusReport,
    pub is_under_constrained: bool,
    pub is_fully_constrained: bool,
    pub is_over_constrained: bool,
    pub model_hash: String,
}

pub fn build_sketch_constraint_status_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchConstraintStatusParityManifest> {
    let under_case = status_case_snapshot(
        "under_constrained_point_only_v1",
        &under_constrained_model()?,
    )?;
    let fully_case = status_case_snapshot(
        "fully_constrained_fixed_point_v1",
        &fully_constrained_model()?,
    )?;
    let over_case = status_case_snapshot(
        "over_constrained_duplicate_fixed_v1",
        &over_constrained_model()?,
    )?;

    let replay_under_case = status_case_snapshot(
        "under_constrained_point_only_v1",
        &under_constrained_model()?,
    )?;
    let replay_fully_case = status_case_snapshot(
        "fully_constrained_fixed_point_v1",
        &fully_constrained_model()?,
    )?;
    let replay_over_case = status_case_snapshot(
        "over_constrained_duplicate_fixed_v1",
        &over_constrained_model()?,
    )?;

    let deterministic_replay_match = under_case == replay_under_case
        && fully_case == replay_fully_case
        && over_case == replay_over_case;

    let deterministic_signature = parity_signature(
        &under_case,
        &fully_case,
        &over_case,
        deterministic_replay_match,
    );

    Ok(SketchConstraintStatusParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_CONSTRAINT_STATUS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Constraint Status Indicator)".to_string(),
            "crates/vcad-kernel-constraints/src/sketch.rs (degrees_of_freedom + is_*_constrained)"
                .to_string(),
        ],
        under_case,
        fully_case,
        over_case,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "degrees_of_freedom follows vcad semantics: parameters - constraint residual equations"
                .to_string(),
            "under/fully/over status is classified by DOF sign (>0, =0, <0)".to_string(),
            "status helpers is_under/is_fully/is_over are deterministic and replay-stable"
                .to_string(),
        ],
    })
}

fn status_case_snapshot(
    case_id: &str,
    model: &CadSketchModel,
) -> CadResult<ConstraintStatusCaseSnapshot> {
    Ok(ConstraintStatusCaseSnapshot {
        case_id: case_id.to_string(),
        status_report: model.constraint_status_report()?,
        is_under_constrained: model.is_under_constrained()?,
        is_fully_constrained: model.is_fully_constrained()?,
        is_over_constrained: model.is_over_constrained()?,
        model_hash: sketch_model_hash(model)?,
    })
}

fn base_model_with_plane() -> CadResult<CadSketchModel> {
    let mut model = CadSketchModel::default();
    model.insert_plane(CadSketchPlane {
        id: "plane.front".to_string(),
        name: "Front".to_string(),
        origin_mm: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        y_axis: [0.0, 1.0, 0.0],
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.a".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [2.0, 3.0],
        anchor_id: "anchor.point.a".to_string(),
        construction: false,
    })?;
    Ok(model)
}

fn under_constrained_model() -> CadResult<CadSketchModel> {
    base_model_with_plane()
}

fn fully_constrained_model() -> CadResult<CadSketchModel> {
    let mut model = base_model_with_plane()?;
    model.insert_constraint(CadSketchConstraint::Fixed {
        id: "constraint.fixed.a".to_string(),
        point_anchor_id: "anchor.point.a".to_string(),
        target_mm: [2.0, 3.0],
        tolerance_mm: Some(0.001),
    })?;
    Ok(model)
}

fn over_constrained_model() -> CadResult<CadSketchModel> {
    let mut model = fully_constrained_model()?;
    model.insert_constraint(CadSketchConstraint::Fixed {
        id: "constraint.fixed.b".to_string(),
        point_anchor_id: "anchor.point.a".to_string(),
        target_mm: [4.0, 5.0],
        tolerance_mm: Some(0.001),
    })?;
    Ok(model)
}

fn sketch_model_hash(model: &CadSketchModel) -> CadResult<String> {
    let bytes = serde_json::to_vec(model).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed to serialize sketch constraint status model for parity hash: {error}"
        ),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    under_case: &ConstraintStatusCaseSnapshot,
    fully_case: &ConstraintStatusCaseSnapshot,
    over_case: &ConstraintStatusCaseSnapshot,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            under_case,
            fully_case,
            over_case,
            deterministic_replay_match,
        ))
        .expect("serialize sketch constraint status parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_CONSTRAINT_STATUS_ISSUE_ID, build_sketch_constraint_status_parity_manifest,
    };
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };
    use crate::sketch::CadSketchConstraintStatus;

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 0,
                crates_reference_count: 0,
                commands_reference_count: 0,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        }
    }

    #[test]
    fn constraint_status_manifest_emits_under_fully_over_semantics() {
        let manifest =
            build_sketch_constraint_status_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build sketch constraint status parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_CONSTRAINT_STATUS_ISSUE_ID);
        assert_eq!(
            manifest.under_case.status_report.status,
            CadSketchConstraintStatus::UnderConstrained
        );
        assert_eq!(
            manifest.fully_case.status_report.status,
            CadSketchConstraintStatus::FullyConstrained
        );
        assert_eq!(
            manifest.over_case.status_report.status,
            CadSketchConstraintStatus::OverConstrained
        );
        assert!(manifest.deterministic_replay_match);
    }
}
