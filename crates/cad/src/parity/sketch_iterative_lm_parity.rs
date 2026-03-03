use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{
    CadDimensionConstraintKind, CadSketchConstraint, CadSketchEntity, CadSketchLmConfig,
    CadSketchModel, CadSketchPlane, CadSketchSolveReport,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_ITERATIVE_LM_ISSUE_ID: &str = "VCAD-PARITY-044";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchIterativeLmParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub lm_config: CadSketchLmConfig,
    pub scenario_id: String,
    pub report_snapshot: CadSketchSolveReport,
    pub report_hash: String,
    pub solved_model_hash: String,
    pub multi_iteration_observed: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

pub fn build_sketch_iterative_lm_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchIterativeLmParityManifest> {
    let lm_config = CadSketchLmConfig::default();
    let seed = iterative_lm_sample_model()?;

    let mut first_run = seed.clone();
    let first_report = first_run.solve_constraints_deterministic()?;
    let first_report_hash = solve_report_hash(&first_report)?;
    let first_model_hash = sketch_model_hash(&first_run)?;

    let mut replay_run = seed;
    let replay_report = replay_run.solve_constraints_deterministic()?;
    let replay_report_hash = solve_report_hash(&replay_report)?;
    let replay_model_hash = sketch_model_hash(&replay_run)?;

    let deterministic_replay_match = first_report == replay_report
        && first_report_hash == replay_report_hash
        && first_model_hash == replay_model_hash;

    let deterministic_signature = parity_signature(
        &lm_config,
        &first_report,
        &first_report_hash,
        &first_model_hash,
        deterministic_replay_match,
    );

    Ok(SketchIterativeLmParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_ITERATIVE_LM_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Constraint Solver section)".to_string(),
            "crates/vcad-kernel-constraints/src/solver.rs (Levenberg-Marquardt iterative solver)"
                .to_string(),
        ],
        lm_config,
        scenario_id: "coupled_constraint_profile_v1".to_string(),
        report_snapshot: first_report.clone(),
        report_hash: first_report_hash,
        solved_model_hash: first_model_hash,
        multi_iteration_observed: first_report.iteration_count > 1,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "sketch solver runs iterative LM-style passes with damping instead of fixed one-pass execution"
                .to_string(),
            "coupled constraint scenario requires >1 deterministic iteration for convergence"
                .to_string(),
            "replay from identical seed yields byte-identical solve report and solved sketch model"
                .to_string(),
        ],
    })
}

fn iterative_lm_sample_model() -> CadResult<CadSketchModel> {
    let mut model = CadSketchModel::default();
    model.insert_plane(CadSketchPlane {
        id: "plane.front".to_string(),
        name: "Front".to_string(),
        origin_mm: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        y_axis: [0.0, 1.0, 0.0],
    })?;

    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.edit".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 4.0],
        anchor_ids: [
            "anchor.edit.start".to_string(),
            "anchor.edit.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.vertical".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [30.0, 0.0],
        end_mm: [35.0, 20.0],
        anchor_ids: [
            "anchor.vert.start".to_string(),
            "anchor.vert.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.tangent".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 10.0],
        end_mm: [20.0, 10.0],
        anchor_ids: ["anchor.tan.start".to_string(), "anchor.tan.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.coincident".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [9.0, 7.0],
        anchor_id: "anchor.point.coincident".to_string(),
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Arc {
        id: "entity.arc.dimension".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [50.0, 20.0],
        radius_mm: 8.0,
        start_deg: 0.0,
        end_deg: 180.0,
        anchor_ids: [
            "anchor.dim.center".to_string(),
            "anchor.dim.start".to_string(),
            "anchor.dim.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Arc {
        id: "entity.arc.tangent".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [10.0, 0.0],
        radius_mm: 10.0,
        start_deg: 0.0,
        end_deg: 180.0,
        anchor_ids: [
            "anchor.tan.center".to_string(),
            "anchor.tan.arc_start".to_string(),
            "anchor.tan.arc_end".to_string(),
        ],
        construction: false,
    })?;

    model.insert_constraint(CadSketchConstraint::Horizontal {
        id: "constraint.horizontal.001".to_string(),
        line_entity_id: "entity.line.edit".to_string(),
    })?;
    model.insert_constraint(CadSketchConstraint::Vertical {
        id: "constraint.vertical.001".to_string(),
        line_entity_id: "entity.line.vertical".to_string(),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.zz.coincident.001".to_string(),
        first_anchor_id: "anchor.edit.end".to_string(),
        second_anchor_id: "anchor.point.coincident".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Dimension {
        id: "constraint.dimension.length.001".to_string(),
        entity_id: "entity.line.edit".to_string(),
        dimension_kind: CadDimensionConstraintKind::Length,
        target_mm: 30.0,
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Dimension {
        id: "constraint.dimension.radius.001".to_string(),
        entity_id: "entity.arc.dimension".to_string(),
        dimension_kind: CadDimensionConstraintKind::Radius,
        target_mm: 12.0,
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Tangent {
        id: "constraint.tangent.001".to_string(),
        line_entity_id: "entity.line.tangent".to_string(),
        arc_entity_id: "entity.arc.tangent".to_string(),
        at_anchor_id: None,
        tolerance_mm: Some(0.001),
    })?;

    Ok(model)
}

fn sketch_model_hash(model: &CadSketchModel) -> CadResult<String> {
    let bytes = serde_json::to_vec(model).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize LM sketch model for parity hash: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn solve_report_hash(report: &CadSketchSolveReport) -> CadResult<String> {
    let bytes = serde_json::to_vec(report).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize LM solve report for parity hash: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    config: &CadSketchLmConfig,
    report: &CadSketchSolveReport,
    report_hash: &str,
    solved_model_hash: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            config,
            report,
            report_hash,
            solved_model_hash,
            deterministic_replay_match,
        ))
        .expect("serialize iterative LM parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SKETCH_ITERATIVE_LM_ISSUE_ID, build_sketch_iterative_lm_parity_manifest};
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

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
    fn iterative_lm_manifest_reports_multi_iteration_and_replay_parity() {
        let manifest =
            build_sketch_iterative_lm_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build iterative LM parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_ITERATIVE_LM_ISSUE_ID);
        assert!(manifest.multi_iteration_observed);
        assert!(manifest.report_snapshot.passed);
        assert!(manifest.report_snapshot.iteration_count > 1);
        assert!(manifest.deterministic_replay_match);
    }
}
