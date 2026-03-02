use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{CadSketchConstraint, CadSketchEntity, CadSketchModel, CadSketchPlane};
use crate::sketch_feature_ops::{
    SketchProfileFeatureKind, SketchProfileFeatureSpec, convert_sketch_profile_to_feature_node,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_REVOLVE_ISSUE_ID: &str = "VCAD-PARITY-048";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchRevolveParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub full_case: SketchRevolveCaseSnapshot,
    pub partial_case: SketchRevolveCaseSnapshot,
    pub profile_hash_order_stable: bool,
    pub invalid_zero_angle_error: String,
    pub invalid_over_360_angle_error: String,
    pub missing_axis_error: String,
    pub unsolved_constraint_error: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchRevolveCaseSnapshot {
    pub revolve_angle_deg: f64,
    pub profile_closed_loop: bool,
    pub warning_codes: Vec<String>,
    pub profile_bounds_mm: [f64; 4],
    pub profile_hash: String,
    pub node_hash: String,
}

pub fn build_sketch_revolve_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchRevolveParityManifest> {
    let model = closed_profile_model()?;
    let full_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.revolve.full".to_string(),
        profile_id: "profile.closed.full".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec![
            "entity.line.b".to_string(),
            "entity.line.d".to_string(),
            "entity.line.a".to_string(),
            "entity.line.c".to_string(),
        ],
        kind: SketchProfileFeatureKind::Revolve,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: Some(360.0),
        axis_anchor_ids: Some(["anchor.axis.a".to_string(), "anchor.axis.b".to_string()]),
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        tolerance_mm: Some(0.001),
    };
    let partial_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.revolve.partial".to_string(),
        profile_id: "profile.closed.partial".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: full_spec.profile_entity_ids.clone(),
        kind: SketchProfileFeatureKind::Revolve,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: Some(90.0),
        axis_anchor_ids: Some(["anchor.axis.a".to_string(), "anchor.axis.b".to_string()]),
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        tolerance_mm: Some(0.001),
    };

    let full_conversion = convert_sketch_profile_to_feature_node(&model, &full_spec)?;
    let partial_conversion = convert_sketch_profile_to_feature_node(&model, &partial_spec)?;
    let full_case = case_snapshot(&full_conversion)?;
    let partial_case = case_snapshot(&partial_conversion)?;

    let reversed_full_spec = SketchProfileFeatureSpec {
        profile_entity_ids: full_spec.profile_entity_ids.iter().cloned().rev().collect(),
        ..full_spec.clone()
    };
    let reversed_full_conversion =
        convert_sketch_profile_to_feature_node(&model, &reversed_full_spec)?;
    let profile_hash_order_stable =
        full_conversion.profile_hash == reversed_full_conversion.profile_hash;

    let invalid_zero_angle_error = convert_sketch_profile_to_feature_node(
        &model,
        &SketchProfileFeatureSpec {
            revolve_angle_deg: Some(0.0),
            ..full_spec.clone()
        },
    )
    .expect_err("zero angle must fail revolve validation")
    .to_string();

    let invalid_over_360_angle_error = convert_sketch_profile_to_feature_node(
        &model,
        &SketchProfileFeatureSpec {
            revolve_angle_deg: Some(361.0),
            ..full_spec.clone()
        },
    )
    .expect_err("angle above 360 must fail revolve validation")
    .to_string();

    let missing_axis_error = convert_sketch_profile_to_feature_node(
        &model,
        &SketchProfileFeatureSpec {
            axis_anchor_ids: None,
            ..full_spec.clone()
        },
    )
    .expect_err("missing axis anchors should fail revolve validation")
    .to_string();

    let unsolved_constraint_error = convert_sketch_profile_to_feature_node(
        &unsolved_profile_model()?,
        &SketchProfileFeatureSpec {
            feature_id: "feature.sketch.revolve.unsolved".to_string(),
            profile_id: "profile.unsolved".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.line.unsolved".to_string()],
            kind: SketchProfileFeatureKind::Revolve,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: Some(90.0),
            axis_anchor_ids: Some([
                "anchor.unsolved.axis.a".to_string(),
                "anchor.unsolved.axis.b".to_string(),
            ]),
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            tolerance_mm: Some(0.001),
        },
    )
    .expect_err("unsolved constraints should block revolve conversion")
    .to_string();

    let replay_manifest = {
        let model = closed_profile_model()?;
        let full_conversion = convert_sketch_profile_to_feature_node(&model, &full_spec)?;
        let partial_conversion = convert_sketch_profile_to_feature_node(&model, &partial_spec)?;
        (
            case_snapshot(&full_conversion)?,
            case_snapshot(&partial_conversion)?,
            profile_hash_order_stable,
            invalid_zero_angle_error.clone(),
            invalid_over_360_angle_error.clone(),
            missing_axis_error.clone(),
            unsolved_constraint_error.clone(),
        )
    };

    let deterministic_replay_match = full_case == replay_manifest.0
        && partial_case == replay_manifest.1
        && profile_hash_order_stable == replay_manifest.2
        && invalid_zero_angle_error == replay_manifest.3
        && invalid_over_360_angle_error == replay_manifest.4
        && missing_axis_error == replay_manifest.5
        && unsolved_constraint_error == replay_manifest.6;

    let deterministic_signature = parity_signature(
        &full_case,
        &partial_case,
        profile_hash_order_stable,
        &invalid_zero_angle_error,
        &invalid_over_360_angle_error,
        &missing_axis_error,
        &unsolved_constraint_error,
        deterministic_replay_match,
    );

    Ok(SketchRevolveParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_REVOLVE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-operations.md (Revolve full vs partial angles)".to_string(),
            "crates/vcad-kernel-sketch/src/revolve.rs (angle bounds, axis guards, partial/full behavior)".to_string(),
        ],
        full_case,
        partial_case,
        profile_hash_order_stable,
        invalid_zero_angle_error,
        invalid_over_360_angle_error,
        missing_axis_error,
        unsolved_constraint_error,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "360-degree revolve emits deterministic sketch.revolve.v1 nodes without partial-angle seam warnings".to_string(),
            "partial-angle revolve emits deterministic CAD-WARN-SLIVER-FACE advisories".to_string(),
            "revolve rejects zero/over-360 angles, missing axis anchors, and unsolved constraints deterministically".to_string(),
        ],
    })
}

fn case_snapshot(
    conversion: &crate::sketch_feature_ops::SketchProfileFeatureConversion,
) -> CadResult<SketchRevolveCaseSnapshot> {
    let revolve_angle_deg = conversion
        .node
        .params
        .get("revolve_angle_deg")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let profile_closed_loop = conversion
        .node
        .params
        .get("profile_closed_loop")
        .map(|value| value == "true")
        .unwrap_or(false);
    let warning_codes = conversion
        .warnings
        .iter()
        .map(|warning| warning.code.stable_code().to_string())
        .collect::<Vec<_>>();
    let node_hash = stable_hash_json(&conversion.node)?;

    Ok(SketchRevolveCaseSnapshot {
        revolve_angle_deg,
        profile_closed_loop,
        warning_codes,
        profile_bounds_mm: conversion.profile_bounds_mm,
        profile_hash: conversion.profile_hash.clone(),
        node_hash,
    })
}

fn closed_profile_model() -> CadResult<CadSketchModel> {
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
        start_mm: [5.0, 0.0],
        end_mm: [8.0, 0.0],
        anchor_ids: ["anchor.a.start".to_string(), "anchor.a.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.b".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [8.0, 0.0],
        end_mm: [8.0, 10.0],
        anchor_ids: ["anchor.b.start".to_string(), "anchor.b.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.c".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [8.0, 10.0],
        end_mm: [5.0, 10.0],
        anchor_ids: ["anchor.c.start".to_string(), "anchor.c.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.d".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [5.0, 10.0],
        end_mm: [5.0, 0.0],
        anchor_ids: ["anchor.d.start".to_string(), "anchor.d.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.axis".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, -20.0],
        end_mm: [0.0, 20.0],
        anchor_ids: ["anchor.axis.a".to_string(), "anchor.axis.b".to_string()],
        construction: true,
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.01".to_string(),
        first_anchor_id: "anchor.a.end".to_string(),
        second_anchor_id: "anchor.b.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.02".to_string(),
        first_anchor_id: "anchor.b.end".to_string(),
        second_anchor_id: "anchor.c.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.03".to_string(),
        first_anchor_id: "anchor.c.end".to_string(),
        second_anchor_id: "anchor.d.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    model.insert_constraint(CadSketchConstraint::Coincident {
        id: "constraint.closed.04".to_string(),
        first_anchor_id: "anchor.d.end".to_string(),
        second_anchor_id: "anchor.a.start".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    Ok(model)
}

fn unsolved_profile_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [5.0, 0.0],
        end_mm: [8.0, 0.0],
        anchor_ids: [
            "anchor.unsolved.start".to_string(),
            "anchor.unsolved.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.axis.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, -10.0],
        end_mm: [0.0, 10.0],
        anchor_ids: [
            "anchor.unsolved.axis.a".to_string(),
            "anchor.unsolved.axis.b".to_string(),
        ],
        construction: true,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [6.0, 4.0],
        anchor_id: "anchor.point.unsolved".to_string(),
        construction: false,
    })?;
    model.insert_constraint(CadSketchConstraint::PointOnLine {
        id: "constraint.unsolved.point_on_line".to_string(),
        point_anchor_id: "anchor.point.unsolved".to_string(),
        line_entity_id: "entity.line.unsolved".to_string(),
        tolerance_mm: Some(0.001),
    })?;
    Ok(model)
}

fn stable_hash_json<T: Serialize>(value: &T) -> CadResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize sketch revolve parity payload: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    full_case: &SketchRevolveCaseSnapshot,
    partial_case: &SketchRevolveCaseSnapshot,
    profile_hash_order_stable: bool,
    invalid_zero_angle_error: &str,
    invalid_over_360_angle_error: &str,
    missing_axis_error: &str,
    unsolved_constraint_error: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            full_case,
            partial_case,
            profile_hash_order_stable,
            invalid_zero_angle_error,
            invalid_over_360_angle_error,
            missing_axis_error,
            unsolved_constraint_error,
            deterministic_replay_match,
        ))
        .expect("serialize sketch revolve parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SKETCH_REVOLVE_ISSUE_ID, build_sketch_revolve_parity_manifest};
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
    fn sketch_revolve_manifest_reports_full_partial_and_invalid_behaviors() {
        let manifest = build_sketch_revolve_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("build sketch revolve parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_REVOLVE_ISSUE_ID);
        assert_eq!(manifest.full_case.revolve_angle_deg, 360.0);
        assert!(manifest.full_case.profile_closed_loop);
        assert!(manifest.full_case.warning_codes.is_empty());
        assert_eq!(manifest.partial_case.revolve_angle_deg, 90.0);
        assert!(
            manifest
                .partial_case
                .warning_codes
                .iter()
                .any(|code| code == "CAD-WARN-SLIVER-FACE")
        );
        assert!(manifest.profile_hash_order_stable);
        assert!(
            manifest
                .invalid_zero_angle_error
                .contains("revolve_angle_deg")
        );
        assert!(
            manifest
                .invalid_over_360_angle_error
                .contains("revolve_angle_deg")
        );
        assert!(manifest.missing_axis_error.contains("axis_anchor_ids"));
        assert!(
            manifest
                .unsolved_constraint_error
                .contains("unsolved constraints")
        );
        assert!(manifest.deterministic_replay_match);
    }
}
