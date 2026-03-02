use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CadResult;
use crate::parity::scorecard::ParityScorecard;
use crate::sketch::{CadSketchEntity, CadSketchModel, CadSketchPlane};
use crate::sketch_feature_ops::{
    SketchProfileFeatureKind, SketchProfileFeatureSpec, convert_sketch_profile_to_feature_node,
};

pub const PARITY_SKETCH_ENTITY_SET_ISSUE_ID: &str = "VCAD-PARITY-041";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchEntitySetParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub supported_entity_kinds: Vec<String>,
    pub kind_counts: BTreeMap<String, usize>,
    pub entity_kind_summaries: Vec<SketchEntityKindSummary>,
    pub sample_model_hash: String,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SketchEntityKindSummary {
    pub entity_id: String,
    pub kind: String,
    pub anchor_count: usize,
    pub profile_closed_loop: bool,
    pub warning_codes: Vec<String>,
}

pub fn build_sketch_entity_set_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchEntitySetParityManifest> {
    let model = sample_sketch_entity_model()?;
    let summaries = build_entity_summaries(&model)?;
    let kind_counts = build_kind_counts(&summaries);
    let supported_entity_kinds = vec![
        "line".to_string(),
        "rectangle".to_string(),
        "circle".to_string(),
        "arc".to_string(),
        "spline".to_string(),
    ];
    let sample_model_hash = sketch_model_hash(&model)?;

    let replay_model = sample_sketch_entity_model()?;
    let replay_summaries = build_entity_summaries(&replay_model)?;
    let replay_kind_counts = build_kind_counts(&replay_summaries);
    let replay_model_hash = sketch_model_hash(&replay_model)?;
    let deterministic_replay_match = summaries == replay_summaries
        && kind_counts == replay_kind_counts
        && sample_model_hash == replay_model_hash;
    let deterministic_signature = parity_signature(
        &supported_entity_kinds,
        &kind_counts,
        &summaries,
        &sample_model_hash,
        deterministic_replay_match,
    );

    Ok(SketchEntitySetParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_ENTITY_SET_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Drawing Tools)".to_string(),
            "crates/vcad-kernel-sketch/src/profile.rs (SketchProfile::rectangle/circle)"
                .to_string(),
        ],
        supported_entity_kinds,
        kind_counts,
        entity_kind_summaries: summaries,
        sample_model_hash,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "sketch entity schema includes line, rectangle, circle, arc, and spline kinds"
                .to_string(),
            "rectangle/circle/closed-spline entities map to closed profile conversion semantics"
                .to_string(),
            "line/arc entities preserve open-profile warning semantics".to_string(),
            "sample sketch entity corpus replays deterministically".to_string(),
        ],
    })
}

fn sample_sketch_entity_model() -> CadResult<CadSketchModel> {
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
        id: "entity.line.001".to_string(),
        plane_id: "plane.front".to_string(),
        start_mm: [0.0, 0.0],
        end_mm: [20.0, 0.0],
        anchor_ids: [
            "anchor.line.start.001".to_string(),
            "anchor.line.end.001".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Rectangle {
        id: "entity.rectangle.001".to_string(),
        plane_id: "plane.front".to_string(),
        min_mm: [24.0, 0.0],
        max_mm: [44.0, 10.0],
        anchor_ids: [
            "anchor.rect.00.001".to_string(),
            "anchor.rect.10.001".to_string(),
            "anchor.rect.11.001".to_string(),
            "anchor.rect.01.001".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Circle {
        id: "entity.circle.001".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [56.0, 5.0],
        radius_mm: 5.0,
        anchor_ids: [
            "anchor.circle.center.001".to_string(),
            "anchor.circle.radius.001".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Arc {
        id: "entity.arc.001".to_string(),
        plane_id: "plane.front".to_string(),
        center_mm: [72.0, 5.0],
        radius_mm: 5.0,
        start_deg: 0.0,
        end_deg: 180.0,
        anchor_ids: [
            "anchor.arc.center.001".to_string(),
            "anchor.arc.start.001".to_string(),
            "anchor.arc.end.001".to_string(),
        ],
        construction: false,
    })?;
    model.insert_entity(CadSketchEntity::Spline {
        id: "entity.spline.001".to_string(),
        plane_id: "plane.front".to_string(),
        control_points_mm: vec![[84.0, 0.0], [88.0, 8.0], [94.0, 6.0], [92.0, -1.0]],
        anchor_ids: vec![
            "anchor.spline.0.001".to_string(),
            "anchor.spline.1.001".to_string(),
            "anchor.spline.2.001".to_string(),
            "anchor.spline.3.001".to_string(),
        ],
        closed: true,
        construction: false,
    })?;
    Ok(model)
}

fn build_entity_summaries(model: &CadSketchModel) -> CadResult<Vec<SketchEntityKindSummary>> {
    let mut summaries = Vec::with_capacity(model.entities.len());
    for (entity_id, entity) in &model.entities {
        let warning_codes = profile_warning_codes(model, entity_id)?;
        summaries.push(SketchEntityKindSummary {
            entity_id: entity_id.clone(),
            kind: entity_kind_label(entity).to_string(),
            anchor_count: entity.anchor_ids().len(),
            profile_closed_loop: profile_closed_loop_flag(model, entity_id)?,
            warning_codes,
        });
    }
    summaries.sort_by(|left, right| left.entity_id.cmp(&right.entity_id));
    Ok(summaries)
}

fn build_kind_counts(summaries: &[SketchEntityKindSummary]) -> BTreeMap<String, usize> {
    let mut kind_counts = BTreeMap::<String, usize>::new();
    for summary in summaries {
        let count = kind_counts.entry(summary.kind.clone()).or_insert(0);
        *count = count.saturating_add(1);
    }
    kind_counts
}

fn profile_closed_loop_flag(model: &CadSketchModel, entity_id: &str) -> CadResult<bool> {
    let spec = SketchProfileFeatureSpec {
        feature_id: format!("feature.{entity_id}.parity"),
        profile_id: format!("profile.{entity_id}.parity"),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec![entity_id.to_string()],
        kind: SketchProfileFeatureKind::Extrude,
        source_feature_id: None,
        depth_mm: Some(1.0),
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        tolerance_mm: Some(0.001),
    };
    let conversion = convert_sketch_profile_to_feature_node(model, &spec)?;
    Ok(conversion
        .node
        .params
        .get("profile_closed_loop")
        .map(String::as_str)
        == Some("true"))
}

fn profile_warning_codes(model: &CadSketchModel, entity_id: &str) -> CadResult<Vec<String>> {
    let spec = SketchProfileFeatureSpec {
        feature_id: format!("feature.{entity_id}.warning"),
        profile_id: format!("profile.{entity_id}.warning"),
        plane_id: "plane.front".to_string(),
        profile_entity_ids: vec![entity_id.to_string()],
        kind: SketchProfileFeatureKind::Extrude,
        source_feature_id: None,
        depth_mm: Some(1.0),
        revolve_angle_deg: None,
        axis_anchor_ids: None,
        sweep_path_entity_ids: None,
        sweep_twist_deg: None,
        sweep_scale_start: None,
        sweep_scale_end: None,
        tolerance_mm: Some(0.001),
    };
    let conversion = convert_sketch_profile_to_feature_node(model, &spec)?;
    let mut warning_codes: Vec<String> = conversion
        .warnings
        .iter()
        .map(|warning| warning.code.stable_code().to_string())
        .collect();
    warning_codes.sort();
    warning_codes.dedup();
    Ok(warning_codes)
}

fn entity_kind_label(entity: &CadSketchEntity) -> &'static str {
    match entity {
        CadSketchEntity::Line { .. } => "line",
        CadSketchEntity::Rectangle { .. } => "rectangle",
        CadSketchEntity::Circle { .. } => "circle",
        CadSketchEntity::Arc { .. } => "arc",
        CadSketchEntity::Spline { .. } => "spline",
        CadSketchEntity::Point { .. } => "point",
    }
}

fn sketch_model_hash(model: &CadSketchModel) -> CadResult<String> {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(model).map_err(|error| crate::CadError::ParseFailed {
            reason: format!("failed to serialize sketch model for parity hash: {error}"),
        })?,
    );
    Ok(format!("{:x}", hasher.finalize())[..16].to_string())
}

fn parity_signature(
    supported_entity_kinds: &[String],
    kind_counts: &BTreeMap<String, usize>,
    summaries: &[SketchEntityKindSummary],
    sample_model_hash: &str,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            supported_entity_kinds,
            kind_counts,
            summaries,
            sample_model_hash,
            deterministic_replay_match,
        ))
        .expect("serialize sketch entity set parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_SKETCH_ENTITY_SET_ISSUE_ID, build_sketch_entity_set_parity_manifest,
        sample_sketch_entity_model,
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
    fn sketch_entity_set_sample_model_carries_expected_kinds() {
        let model = sample_sketch_entity_model().expect("sample sketch model should build");
        assert_eq!(model.entities.len(), 5);
        assert!(
            model
                .entities
                .values()
                .any(|entity| matches!(entity, crate::sketch::CadSketchEntity::Rectangle { .. }))
        );
        assert!(
            model
                .entities
                .values()
                .any(|entity| matches!(entity, crate::sketch::CadSketchEntity::Circle { .. }))
        );
        assert!(
            model
                .entities
                .values()
                .any(|entity| matches!(entity, crate::sketch::CadSketchEntity::Spline { .. }))
        );
    }

    #[test]
    fn build_manifest_tracks_entity_set_support() {
        let manifest = build_sketch_entity_set_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("build sketch entity set manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_ENTITY_SET_ISSUE_ID);
        assert_eq!(manifest.supported_entity_kinds.len(), 5);
        assert_eq!(manifest.kind_counts.get("line"), Some(&1));
        assert_eq!(manifest.kind_counts.get("rectangle"), Some(&1));
        assert_eq!(manifest.kind_counts.get("circle"), Some(&1));
        assert_eq!(manifest.kind_counts.get("arc"), Some(&1));
        assert_eq!(manifest.kind_counts.get("spline"), Some(&1));
        assert!(manifest.deterministic_replay_match);
    }
}
