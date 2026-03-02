use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{CadSketchConstraint, CadSketchEntity, CadSketchModel, CadSketchPlane};
use crate::sketch_feature_ops::{
    SketchProfileFeatureKind, SketchProfileFeatureSpec, convert_sketch_profile_to_feature_node,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_SWEEP_ISSUE_ID: &str = "VCAD-PARITY-049";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchSweepParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub closed_case: SketchSweepCaseSnapshot,
    pub open_case: SketchSweepCaseSnapshot,
    pub profile_hash_order_stable: bool,
    pub empty_path_error: String,
    pub non_positive_scale_error: String,
    pub unknown_path_entity_error: String,
    pub unsolved_constraint_error: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchSweepCaseSnapshot {
    pub profile_closed_loop: bool,
    pub warning_codes: Vec<String>,
    pub sweep_path_entity_ids: Vec<String>,
    pub sweep_twist_deg: f64,
    pub sweep_scale_start: f64,
    pub sweep_scale_end: f64,
    pub profile_bounds_mm: [f64; 4],
    pub profile_hash: String,
    pub node_hash: String,
}

pub fn build_sketch_sweep_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchSweepParityManifest> {
    let closed_model = closed_profile_model()?;
    let closed_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.sweep.closed".to_string(),
        profile_id: "profile.sweep.closed".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec![
            "entity.line.b".to_string(),
            "entity.line.d".to_string(),
            "entity.line.a".to_string(),
            "entity.line.c".to_string(),
        ],
        kind: SketchProfileFeatureKind::Sweep,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: Some(vec![
            "entity.path.b".to_string(),
            "entity.path.a".to_string(),
        ]),
        sweep_twist_deg: Some(45.0),
        sweep_scale_start: Some(1.0),
        sweep_scale_end: Some(0.7),
        loft_profile_ids: None,
        loft_closed: None,
        tolerance_mm: Some(0.001),
    };
    let closed_conversion = convert_sketch_profile_to_feature_node(&closed_model, &closed_spec)?;
    let closed_case = case_snapshot(&closed_conversion)?;

    let reversed_closed_spec = SketchProfileFeatureSpec {
        profile_entity_ids: closed_spec
            .profile_entity_ids
            .iter()
            .cloned()
            .rev()
            .collect(),
        ..closed_spec.clone()
    };
    let reversed_closed_conversion =
        convert_sketch_profile_to_feature_node(&closed_model, &reversed_closed_spec)?;
    let profile_hash_order_stable =
        closed_conversion.profile_hash == reversed_closed_conversion.profile_hash;

    let open_model = open_profile_model()?;
    let open_spec = SketchProfileFeatureSpec {
        feature_id: "feature.sketch.sweep.open".to_string(),
        profile_id: "profile.sweep.open".to_string(),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec!["entity.line.open".to_string()],
        kind: SketchProfileFeatureKind::Sweep,
        source_feature_id: None,
        depth_mm: None,
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: Some(vec!["entity.path.open".to_string()]),
        sweep_twist_deg: Some(0.0),
        sweep_scale_start: Some(1.0),
        sweep_scale_end: Some(1.0),
        loft_profile_ids: None,
        loft_closed: None,
        tolerance_mm: Some(0.001),
    };
    let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)?;
    let open_case = case_snapshot(&open_conversion)?;

    let empty_path_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            sweep_path_entity_ids: Some(Vec::new()),
            ..closed_spec.clone()
        },
    )
    .expect_err("empty sweep path must fail validation")
    .to_string();

    let non_positive_scale_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            sweep_scale_end: Some(0.0),
            ..closed_spec.clone()
        },
    )
    .expect_err("non-positive sweep scale must fail validation")
    .to_string();

    let unknown_path_entity_error = convert_sketch_profile_to_feature_node(
        &closed_model,
        &SketchProfileFeatureSpec {
            sweep_path_entity_ids: Some(vec!["entity.path.missing".to_string()]),
            ..closed_spec.clone()
        },
    )
    .expect_err("unknown sweep path entity must fail conversion")
    .to_string();

    let unsolved_constraint_error = convert_sketch_profile_to_feature_node(
        &unsolved_profile_model()?,
        &SketchProfileFeatureSpec {
            feature_id: "feature.sketch.sweep.unsolved".to_string(),
            profile_id: "profile.sweep.unsolved".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.line.unsolved".to_string()],
            kind: SketchProfileFeatureKind::Sweep,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: Some(vec!["entity.path.unsolved".to_string()]),
            sweep_twist_deg: Some(0.0),
            sweep_scale_start: Some(1.0),
            sweep_scale_end: Some(1.0),
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        },
    )
    .expect_err("unsolved constraints should block sweep conversion")
    .to_string();

    let replay_manifest = {
        let closed_model = closed_profile_model()?;
        let closed_conversion =
            convert_sketch_profile_to_feature_node(&closed_model, &closed_spec)?;
        let closed_case = case_snapshot(&closed_conversion)?;
        let open_model = open_profile_model()?;
        let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)?;
        let open_case = case_snapshot(&open_conversion)?;
        (
            closed_case,
            open_case,
            profile_hash_order_stable,
            empty_path_error.clone(),
            non_positive_scale_error.clone(),
            unknown_path_entity_error.clone(),
            unsolved_constraint_error.clone(),
        )
    };

    let deterministic_replay_match = closed_case == replay_manifest.0
        && open_case == replay_manifest.1
        && profile_hash_order_stable == replay_manifest.2
        && empty_path_error == replay_manifest.3
        && non_positive_scale_error == replay_manifest.4
        && unknown_path_entity_error == replay_manifest.5
        && unsolved_constraint_error == replay_manifest.6;

    let deterministic_signature = parity_signature(
        &closed_case,
        &open_case,
        profile_hash_order_stable,
        &empty_path_error,
        &non_positive_scale_error,
        &unknown_path_entity_error,
        &unsolved_constraint_error,
        deterministic_replay_match,
    );

    Ok(SketchSweepParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_SWEEP_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-operations.md (Sweep operation + edge cases)".to_string(),
            "crates/vcad-kernel-sweep/src/sweep.rs (path/twist/scale controls)".to_string(),
        ],
        closed_case,
        open_case,
        profile_hash_order_stable,
        empty_path_error,
        non_positive_scale_error,
        unknown_path_entity_error,
        unsolved_constraint_error,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "closed sketch profiles map deterministically into sketch.sweep.v1 feature nodes with path/twist/scale controls".to_string(),
            "open sweep profiles emit deterministic CAD-WARN-NON-MANIFOLD warnings".to_string(),
            "empty-path, invalid-scale, unknown-path, and unsolved-constraint sweep inputs fail deterministically".to_string(),
        ],
    })
}

fn case_snapshot(
    conversion: &crate::sketch_feature_ops::SketchProfileFeatureConversion,
) -> CadResult<SketchSweepCaseSnapshot> {
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
    let sweep_path_entity_ids = conversion
        .node
        .params
        .get("sweep_path_entity_ids")
        .map(|value| {
            value
                .split(',')
                .filter(|segment| !segment.is_empty())
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let sweep_twist_deg = conversion
        .node
        .params
        .get("sweep_twist_deg")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let sweep_scale_start = conversion
        .node
        .params
        .get("sweep_scale_start")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(1.0);
    let sweep_scale_end = conversion
        .node
        .params
        .get("sweep_scale_end")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(1.0);
    let node_hash = stable_hash_json(&conversion.node)?;

    Ok(SketchSweepCaseSnapshot {
        profile_closed_loop,
        warning_codes,
        sweep_path_entity_ids,
        sweep_twist_deg,
        sweep_scale_start,
        sweep_scale_end,
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
        start_mm: [0.0, 0.0],
        end_mm: [40.0, 0.0],
        anchor_ids: ["anchor.a.start".to_string(), "anchor.a.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.b".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [40.0, 0.0],
        end_mm: [40.0, 20.0],
        anchor_ids: ["anchor.b.start".to_string(), "anchor.b.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.c".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [40.0, 20.0],
        end_mm: [0.0, 20.0],
        anchor_ids: ["anchor.c.start".to_string(), "anchor.c.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.line.d".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 20.0],
        end_mm: [0.0, 0.0],
        anchor_ids: ["anchor.d.start".to_string(), "anchor.d.end".to_string()],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.path.a".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [60.0, 0.0],
        end_mm: [80.0, 10.0],
        anchor_ids: [
            "anchor.path.a.start".to_string(),
            "anchor.path.a.end".to_string(),
        ],
        construction: true,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.path.b".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [80.0, 10.0],
        end_mm: [100.0, 25.0],
        anchor_ids: [
            "anchor.path.b.start".to_string(),
            "anchor.path.b.end".to_string(),
        ],
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

fn open_profile_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.open".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 0.0],
        anchor_ids: [
            "anchor.open.start".to_string(),
            "anchor.open.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.path.open".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [30.0, 0.0],
        end_mm: [50.0, 15.0],
        anchor_ids: [
            "anchor.path.open.start".to_string(),
            "anchor.path.open.end".to_string(),
        ],
        construction: true,
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
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 0.0],
        anchor_ids: [
            "anchor.unsolved.start".to_string(),
            "anchor.unsolved.end".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Line {
        id: "entity.path.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [30.0, 0.0],
        end_mm: [50.0, 15.0],
        anchor_ids: [
            "anchor.path.unsolved.start".to_string(),
            "anchor.path.unsolved.end".to_string(),
        ],
        construction: true,
    })?;
    model.insert_entity(CadSketchEntity::Point {
        id: "entity.point.unsolved".to_string(),
        plane_id: "plane.front".to_string(),
        position_mm: [8.0, 5.0],
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
        reason: format!("failed to serialize sketch sweep parity payload: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    closed_case: &SketchSweepCaseSnapshot,
    open_case: &SketchSweepCaseSnapshot,
    profile_hash_order_stable: bool,
    empty_path_error: &str,
    non_positive_scale_error: &str,
    unknown_path_entity_error: &str,
    unsolved_constraint_error: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            closed_case,
            open_case,
            profile_hash_order_stable,
            empty_path_error,
            non_positive_scale_error,
            unknown_path_entity_error,
            unsolved_constraint_error,
            deterministic_replay_match,
        ))
        .expect("serialize sketch sweep parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SKETCH_SWEEP_ISSUE_ID, build_sketch_sweep_parity_manifest};
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
    fn sketch_sweep_manifest_reports_closed_open_and_invalid_behaviors() {
        let manifest = build_sketch_sweep_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("build sketch sweep parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_SWEEP_ISSUE_ID);
        assert!(manifest.closed_case.profile_closed_loop);
        assert!(manifest.closed_case.warning_codes.is_empty());
        assert!(!manifest.open_case.profile_closed_loop);
        assert!(
            manifest
                .open_case
                .warning_codes
                .iter()
                .any(|code| code == "CAD-WARN-NON-MANIFOLD")
        );
        assert!(manifest.profile_hash_order_stable);
        assert!(manifest.empty_path_error.contains("sweep_path_entity_ids"));
        assert!(
            manifest
                .non_positive_scale_error
                .contains("sweep_scale_end")
        );
        assert!(
            manifest
                .unknown_path_entity_error
                .contains("unknown sketch entity")
        );
        assert!(
            manifest
                .unsolved_constraint_error
                .contains("unsolved constraints")
        );
        assert!(manifest.deterministic_replay_match);
    }
}
