use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{
    CadSketchConstraint, CadSketchEntity, CadSketchLmPipelineSummary, CadSketchModel,
    CadSketchPlane,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_JACOBIAN_RESIDUAL_ISSUE_ID: &str = "VCAD-PARITY-045";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchJacobianResidualParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub pipeline_summary: CadSketchLmPipelineSummary,
    pub rank_deficient_warning_observed: bool,
    pub rank_case_diagnostic_codes: Vec<String>,
    pub sample_model_hash: String,
    pub rank_case_report_hash: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

pub fn build_sketch_jacobian_residual_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchJacobianResidualParityManifest> {
    let model = jacobian_residual_sample_model()?;
    let pipeline_summary = model.lm_pipeline_summary()?;
    let sample_model_hash = sketch_model_hash(&model)?;

    let rank_case_report = rank_deficient_case_report()?;
    let rank_case_report_hash = solve_report_hash(&rank_case_report)?;
    let rank_case_diagnostic_codes = rank_case_report
        .diagnostics
        .iter()
        .filter(|entry| entry.constraint_id == "lm.pipeline")
        .map(|entry| entry.code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rank_deficient_warning_observed = rank_case_diagnostic_codes
        .iter()
        .any(|code| code == "SKETCH_LM_JACOBIAN_RANK_DEFICIENT");

    let replay_model = jacobian_residual_sample_model()?;
    let replay_pipeline_summary = replay_model.lm_pipeline_summary()?;
    let replay_sample_model_hash = sketch_model_hash(&replay_model)?;
    let replay_rank_case_report = rank_deficient_case_report()?;
    let replay_rank_case_report_hash = solve_report_hash(&replay_rank_case_report)?;

    let deterministic_replay_match = pipeline_summary == replay_pipeline_summary
        && sample_model_hash == replay_sample_model_hash
        && rank_case_report == replay_rank_case_report
        && rank_case_report_hash == replay_rank_case_report_hash;

    let deterministic_signature = parity_signature(
        &pipeline_summary,
        rank_deficient_warning_observed,
        &rank_case_diagnostic_codes,
        &sample_model_hash,
        &rank_case_report_hash,
        deterministic_replay_match,
    );

    Ok(SketchJacobianResidualParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_JACOBIAN_RESIDUAL_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Solver Algorithm section)".to_string(),
            "crates/vcad-kernel-constraints/src/jacobian.rs (finite-difference Jacobian reference)"
                .to_string(),
            "crates/vcad-kernel-constraints/src/residual.rs (constraint residual vector reference)"
                .to_string(),
        ],
        pipeline_summary,
        rank_deficient_warning_observed,
        rank_case_diagnostic_codes,
        sample_model_hash,
        rank_case_report_hash,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "LM pipeline computes deterministic residual vector + finite-difference Jacobian summaries"
                .to_string(),
            "pipeline exposes stable residual/jacobian hashes for replay-safe parity checks"
                .to_string(),
            "rank-deficient unsolved systems emit deterministic SKETCH_LM_JACOBIAN_RANK_DEFICIENT diagnostics"
                .to_string(),
        ],
    })
}

fn jacobian_residual_sample_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.a".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 5.0],
        anchor_ids: [
            "anchor.line.a.start".to_string(),
            "anchor.line.a.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.b".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 10.0],
        end_mm: [20.0, 15.0],
        anchor_ids: [
            "anchor.line.b.start".to_string(),
            "anchor.line.b.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.a".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [5.0, 8.0],
        anchor_id: "anchor.point.a".to_string(),
        construction: false,
    })?;

    model.insert_constraint(CadSketchConstraint::Horizontal {
        id: "constraint.horizontal".to_string(),
        line_entity_id: "entity.line.a".to_string(),
    })?;
    model.insert_constraint(CadSketchConstraint::PointOnLine {
        id: "constraint.point_on_line".to_string(),
        point_anchor_id: "anchor.point.a".to_string(),
        line_entity_id: "entity.line.a".to_string(),
        tolerance_mm: Some(0.01),
    })?;
    model.insert_constraint(CadSketchConstraint::Distance {
        id: "constraint.distance".to_string(),
        first_anchor_id: "anchor.line.a.start".to_string(),
        second_anchor_id: "anchor.point.a".to_string(),
        target_mm: 6.0,
        tolerance_mm: Some(0.01),
    })?;
    model.insert_constraint(CadSketchConstraint::Parallel {
        id: "constraint.parallel".to_string(),
        first_line_entity_id: "entity.line.a".to_string(),
        second_line_entity_id: "entity.line.b".to_string(),
        tolerance_mm: Some(0.01),
    })?;

    Ok(model)
}

fn rank_deficient_case_report() -> CadResult<crate::sketch::CadSketchSolveReport> {
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
        id: "entity.line.a".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 5.0],
        anchor_ids: [
            "anchor.line.a.start".to_string(),
            "anchor.line.a.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.a".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [5.0, 8.0],
        anchor_id: "anchor.point.a".to_string(),
        construction: false,
    })?;
    model.insert_constraint(CadSketchConstraint::PointOnLine {
        id: "constraint.point_on_line.a".to_string(),
        point_anchor_id: "anchor.point.a".to_string(),
        line_entity_id: "entity.line.a".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::PointOnLine {
        id: "constraint.point_on_line.b".to_string(),
        point_anchor_id: "anchor.point.a".to_string(),
        line_entity_id: "entity.line.a".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.solve_constraints_deterministic()
}

fn sketch_model_hash(model: &CadSketchModel) -> CadResult<String> {
    let bytes = serde_json::to_vec(model).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed to serialize sketch Jacobian/residual sample model for parity hash: {error}"
        ),
    })?;
    Ok(short_sha256(&bytes))
}

fn solve_report_hash(report: &crate::sketch::CadSketchSolveReport) -> CadResult<String> {
    let bytes = serde_json::to_vec(report).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed to serialize sketch Jacobian/residual solve report for parity hash: {error}"
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
    summary: &CadSketchLmPipelineSummary,
    rank_deficient_warning_observed: bool,
    rank_case_diagnostic_codes: &[String],
    model_hash: &str,
    report_hash: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            summary,
            rank_deficient_warning_observed,
            rank_case_diagnostic_codes,
            model_hash,
            report_hash,
            deterministic_replay_match,
        ))
        .expect("serialize sketch Jacobian/residual parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_JACOBIAN_RESIDUAL_ISSUE_ID, build_sketch_jacobian_residual_parity_manifest,
    };
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
    fn jacobian_residual_manifest_reports_pipeline_and_rank_diagnostic_contracts() {
        let manifest =
            build_sketch_jacobian_residual_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build sketch Jacobian/residual parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_JACOBIAN_RESIDUAL_ISSUE_ID);
        assert!(manifest.pipeline_summary.parameter_count > 0);
        assert!(manifest.pipeline_summary.residual_component_count > 0);
        assert!(manifest.rank_deficient_warning_observed);
        assert!(manifest.deterministic_replay_match);
    }
}
