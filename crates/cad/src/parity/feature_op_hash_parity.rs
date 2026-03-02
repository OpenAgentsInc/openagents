use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::features::{
    BoxFeatureOp, CircularPatternFeatureOp, CutHoleFeatureOp, CylinderFeatureOp,
    FilletPlaceholderFeatureOp, FilletPlaceholderKind, LinearPatternFeatureOp, LoftFeatureOp,
    LoftFeatureProfile, SweepFeatureOp, TransformFeatureOp, evaluate_circular_pattern_feature,
    evaluate_fillet_placeholder_feature, evaluate_linear_pattern_feature, evaluate_loft_feature,
    evaluate_sweep_feature, evaluate_transform_feature,
};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_FEATURE_OP_HASH_ISSUE_ID: &str = "VCAD-PARITY-038";
pub const FEATURE_OP_HASH_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/feature_op_hash_vcad_reference_corpus.json";
const FEATURE_OP_HASH_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/feature_op_hash_vcad_reference_corpus.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeatureOpHashParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_count: usize,
    pub matched_case_count: usize,
    pub deterministic_replay_match: bool,
    pub mismatches: Vec<FeatureOpHashMismatch>,
    pub case_snapshots: Vec<FeatureOpHashSnapshot>,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureOpHashMismatch {
    pub case_id: String,
    pub expected_hash: String,
    pub openagents_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureOpHashSnapshot {
    pub case_id: String,
    pub operation: String,
    pub expected_hash: String,
    pub openagents_hash: String,
    pub matches_reference: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct FeatureOpHashReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    cases: Vec<FeatureOpHashReferenceCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum FeatureOpHashReferenceCase {
    Box {
        case_id: String,
        feature_id: String,
        width_mm: f64,
        depth_mm: f64,
        height_mm: f64,
        expected_hash: String,
    },
    Cylinder {
        case_id: String,
        feature_id: String,
        radius_mm: f64,
        height_mm: f64,
        expected_hash: String,
    },
    Transform {
        case_id: String,
        feature_id: String,
        source_feature_id: String,
        source_geometry_hash: String,
        translation_mm: [f64; 3],
        rotation_deg_xyz: [f64; 3],
        scale_xyz: [f64; 3],
        expected_hash: String,
    },
    CutHole {
        case_id: String,
        feature_id: String,
        source_feature_id: String,
        source_geometry_hash: String,
        radius_mm: f64,
        depth_mm: f64,
        tolerance_mm: f64,
        expected_hash: String,
    },
    LinearPattern {
        case_id: String,
        feature_id: String,
        source_feature_id: String,
        source_geometry_hash: String,
        count: u32,
        spacing_mm: f64,
        direction_unit_xyz: [f64; 3],
        start_index: u32,
        expected_hash: String,
    },
    CircularPattern {
        case_id: String,
        feature_id: String,
        source_feature_id: String,
        source_geometry_hash: String,
        count: u32,
        angle_deg: f64,
        radius_mm: f64,
        axis_origin_mm: [f64; 3],
        axis_direction_xyz: [f64; 3],
        start_index: u32,
        expected_hash: String,
    },
    FilletPlaceholder {
        case_id: String,
        feature_id: String,
        source_feature_id: String,
        source_geometry_hash: String,
        kind: String,
        radius_mm: f64,
        expected_hash: String,
    },
    Sweep {
        case_id: String,
        feature_id: String,
        source_feature_id: String,
        source_geometry_hash: String,
        path_points_mm: Vec<[f64; 3]>,
        twist_angle_rad: f64,
        scale_start: f64,
        scale_end: f64,
        path_segments: u32,
        expected_hash: String,
    },
    Loft {
        case_id: String,
        feature_id: String,
        source_feature_ids: Vec<String>,
        source_geometry_hashes: Vec<String>,
        closed: bool,
        profiles: Vec<ReferenceLoftProfile>,
        expected_hash: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ReferenceLoftProfile {
    profile_id: String,
    vertices_mm: Vec<[f64; 3]>,
}

impl FeatureOpHashReferenceCase {
    fn case_id(&self) -> &str {
        match self {
            Self::Box { case_id, .. }
            | Self::Cylinder { case_id, .. }
            | Self::Transform { case_id, .. }
            | Self::CutHole { case_id, .. }
            | Self::LinearPattern { case_id, .. }
            | Self::CircularPattern { case_id, .. }
            | Self::FilletPlaceholder { case_id, .. }
            | Self::Sweep { case_id, .. }
            | Self::Loft { case_id, .. } => case_id,
        }
    }

    fn operation_label(&self) -> &'static str {
        match self {
            Self::Box { .. } => "box",
            Self::Cylinder { .. } => "cylinder",
            Self::Transform { .. } => "transform",
            Self::CutHole { .. } => "cut_hole",
            Self::LinearPattern { .. } => "linear_pattern",
            Self::CircularPattern { .. } => "circular_pattern",
            Self::FilletPlaceholder { .. } => "fillet_placeholder",
            Self::Sweep { .. } => "sweep",
            Self::Loft { .. } => "loft",
        }
    }

    fn expected_hash(&self) -> &str {
        match self {
            Self::Box { expected_hash, .. }
            | Self::Cylinder { expected_hash, .. }
            | Self::Transform { expected_hash, .. }
            | Self::CutHole { expected_hash, .. }
            | Self::LinearPattern { expected_hash, .. }
            | Self::CircularPattern { expected_hash, .. }
            | Self::FilletPlaceholder { expected_hash, .. }
            | Self::Sweep { expected_hash, .. }
            | Self::Loft { expected_hash, .. } => expected_hash,
        }
    }
}

pub fn build_feature_op_hash_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> FeatureOpHashParityManifest {
    let corpus: FeatureOpHashReferenceCorpus =
        serde_json::from_str(FEATURE_OP_HASH_REFERENCE_CORPUS_JSON)
            .expect("feature-op hash reference corpus fixture should parse");

    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let mut snapshots = Vec::with_capacity(corpus.cases.len());
    let mut mismatches = Vec::new();
    let mut replay_match = true;

    for case in &corpus.cases {
        let openagents_hash =
            evaluate_reference_case(case).expect("feature-op hash case should evaluate");
        let replay_hash =
            evaluate_reference_case(case).expect("feature-op hash replay should evaluate");
        if openagents_hash != replay_hash {
            replay_match = false;
        }

        let expected_hash = case.expected_hash().to_string();
        let matches_reference = openagents_hash == expected_hash;
        if !matches_reference {
            mismatches.push(FeatureOpHashMismatch {
                case_id: case.case_id().to_string(),
                expected_hash: expected_hash.clone(),
                openagents_hash: openagents_hash.clone(),
            });
        }

        snapshots.push(FeatureOpHashSnapshot {
            case_id: case.case_id().to_string(),
            operation: case.operation_label().to_string(),
            expected_hash,
            openagents_hash,
            matches_reference,
        });
    }

    let matched_case_count = snapshots
        .iter()
        .filter(|snapshot| snapshot.matches_reference)
        .count();
    let reference_corpus_sha256 = sha256_hex(FEATURE_OP_HASH_REFERENCE_CORPUS_JSON.as_bytes());

    let deterministic_signature = parity_signature(
        &snapshots,
        &mismatches,
        reference_commit_match,
        replay_match,
        &reference_corpus_sha256,
    );

    FeatureOpHashParityManifest {
        manifest_version: 1,
        issue_id: PARITY_FEATURE_OP_HASH_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: FEATURE_OP_HASH_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_count: snapshots.len(),
        matched_case_count,
        deterministic_replay_match: replay_match,
        mismatches,
        case_snapshots: snapshots,
        deterministic_signature,
        parity_contracts: vec![
            "feature-op hash fixtures compare OpenAgents hashes against a pinned vcad reference corpus"
                .to_string(),
            "reference corpus is anchored to vcad materializer feature semantics and pinned vcad commit"
                .to_string(),
            "box/cylinder/transform/cut_hole/pattern/fillet_placeholder/sweep/loft hash contracts are covered"
                .to_string(),
            "hash replay is deterministic for identical feature-op payloads".to_string(),
        ],
    }
}

fn evaluate_reference_case(case: &FeatureOpHashReferenceCase) -> CadResult<String> {
    match case {
        FeatureOpHashReferenceCase::Box {
            feature_id,
            width_mm,
            depth_mm,
            height_mm,
            ..
        } => {
            let params = params_with_entries(&[
                ("width_mm", *width_mm, ScalarUnit::Millimeter),
                ("depth_mm", *depth_mm, ScalarUnit::Millimeter),
                ("height_mm", *height_mm, ScalarUnit::Millimeter),
            ])?;
            let op = BoxFeatureOp {
                feature_id: feature_id.clone(),
                width_param: "width_mm".to_string(),
                depth_param: "depth_mm".to_string(),
                height_param: "height_mm".to_string(),
            };
            let primitive = op.resolve_primitive(&params)?;
            Ok(op.geometry_hash(&primitive))
        }
        FeatureOpHashReferenceCase::Cylinder {
            feature_id,
            radius_mm,
            height_mm,
            ..
        } => {
            let params = params_with_entries(&[
                ("radius_mm", *radius_mm, ScalarUnit::Millimeter),
                ("height_mm", *height_mm, ScalarUnit::Millimeter),
            ])?;
            let op = CylinderFeatureOp {
                feature_id: feature_id.clone(),
                radius_param: "radius_mm".to_string(),
                height_param: "height_mm".to_string(),
            };
            let primitive = op.resolve_primitive(&params)?;
            Ok(op.geometry_hash(&primitive))
        }
        FeatureOpHashReferenceCase::Transform {
            feature_id,
            source_feature_id,
            source_geometry_hash,
            translation_mm,
            rotation_deg_xyz,
            scale_xyz,
            ..
        } => Ok(evaluate_transform_feature(
            &TransformFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_id: source_feature_id.clone(),
                translation_mm: *translation_mm,
                rotation_deg_xyz: *rotation_deg_xyz,
                scale_xyz: *scale_xyz,
            },
            source_geometry_hash,
        )?
        .geometry_hash),
        FeatureOpHashReferenceCase::CutHole {
            feature_id,
            source_feature_id,
            source_geometry_hash,
            radius_mm,
            depth_mm,
            tolerance_mm,
            ..
        } => {
            let params = params_with_entries(&[
                ("radius_mm", *radius_mm, ScalarUnit::Millimeter),
                ("depth_mm", *depth_mm, ScalarUnit::Millimeter),
            ])?;
            let op = CutHoleFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_id: source_feature_id.clone(),
                radius_param: "radius_mm".to_string(),
                depth_param: "depth_mm".to_string(),
                tolerance_mm: Some(*tolerance_mm),
            };
            let cutter = op.resolve_cutter(&params)?;
            Ok(op.geometry_hash(source_geometry_hash, &cutter))
        }
        FeatureOpHashReferenceCase::LinearPattern {
            feature_id,
            source_feature_id,
            source_geometry_hash,
            count,
            spacing_mm,
            direction_unit_xyz,
            start_index,
            ..
        } => {
            let params = params_with_entries(&[
                ("count", f64::from(*count), ScalarUnit::Unitless),
                ("spacing_mm", *spacing_mm, ScalarUnit::Millimeter),
            ])?;
            let op = LinearPatternFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_id: source_feature_id.clone(),
                count_param: "count".to_string(),
                spacing_param: "spacing_mm".to_string(),
                direction_unit_xyz: *direction_unit_xyz,
                start_index: *start_index,
            };
            Ok(evaluate_linear_pattern_feature(&op, &params, source_geometry_hash)?.pattern_hash)
        }
        FeatureOpHashReferenceCase::CircularPattern {
            feature_id,
            source_feature_id,
            source_geometry_hash,
            count,
            angle_deg,
            radius_mm,
            axis_origin_mm,
            axis_direction_xyz,
            start_index,
            ..
        } => {
            let params = params_with_entries(&[
                ("count", f64::from(*count), ScalarUnit::Unitless),
                ("angle_deg", *angle_deg, ScalarUnit::Unitless),
                ("radius_mm", *radius_mm, ScalarUnit::Millimeter),
            ])?;
            let op = CircularPatternFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_id: source_feature_id.clone(),
                count_param: "count".to_string(),
                angle_deg_param: "angle_deg".to_string(),
                radius_param: "radius_mm".to_string(),
                axis_origin_mm: *axis_origin_mm,
                axis_direction_xyz: *axis_direction_xyz,
                start_index: *start_index,
            };
            Ok(evaluate_circular_pattern_feature(&op, &params, source_geometry_hash)?.pattern_hash)
        }
        FeatureOpHashReferenceCase::FilletPlaceholder {
            feature_id,
            source_feature_id,
            source_geometry_hash,
            kind,
            radius_mm,
            ..
        } => {
            let params = params_with_entries(&[("radius_mm", *radius_mm, ScalarUnit::Millimeter)])?;
            let kind = match kind.as_str() {
                "fillet" => FilletPlaceholderKind::Fillet,
                "chamfer" => FilletPlaceholderKind::Chamfer,
                other => {
                    return Err(CadError::InvalidPrimitive {
                        reason: format!(
                            "unsupported reference fillet placeholder kind '{}'",
                            other
                        ),
                    });
                }
            };
            let op = FilletPlaceholderFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_id: source_feature_id.clone(),
                radius_param: "radius_mm".to_string(),
                kind,
            };
            Ok(
                evaluate_fillet_placeholder_feature(&op, &params, source_geometry_hash)?
                    .geometry_hash,
            )
        }
        FeatureOpHashReferenceCase::Sweep {
            feature_id,
            source_feature_id,
            source_geometry_hash,
            path_points_mm,
            twist_angle_rad,
            scale_start,
            scale_end,
            path_segments,
            ..
        } => {
            let params = params_with_entries(&[
                ("twist_angle_rad", *twist_angle_rad, ScalarUnit::Unitless),
                ("scale_start", *scale_start, ScalarUnit::Unitless),
                ("scale_end", *scale_end, ScalarUnit::Unitless),
            ])?;
            let op = SweepFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_id: source_feature_id.clone(),
                path_points_mm: path_points_mm.clone(),
                twist_angle_param: "twist_angle_rad".to_string(),
                scale_start_param: "scale_start".to_string(),
                scale_end_param: "scale_end".to_string(),
                path_segments: *path_segments,
            };
            Ok(evaluate_sweep_feature(&op, &params, source_geometry_hash)?.geometry_hash)
        }
        FeatureOpHashReferenceCase::Loft {
            feature_id,
            source_feature_ids,
            source_geometry_hashes,
            closed,
            profiles,
            ..
        } => {
            let op = LoftFeatureOp {
                feature_id: feature_id.clone(),
                source_feature_ids: source_feature_ids.clone(),
                profiles: profiles
                    .iter()
                    .map(|profile| LoftFeatureProfile {
                        profile_id: profile.profile_id.clone(),
                        vertices_mm: profile.vertices_mm.clone(),
                    })
                    .collect(),
                closed: *closed,
            };
            Ok(evaluate_loft_feature(&op, source_geometry_hashes)?.geometry_hash)
        }
    }
}

fn params_with_entries(entries: &[(&str, f64, ScalarUnit)]) -> CadResult<ParameterStore> {
    let mut params = ParameterStore::default();
    for (name, value, unit) in entries {
        params.set(
            *name,
            ScalarValue {
                value: *value,
                unit: *unit,
            },
        )?;
    }
    Ok(params)
}

fn parity_signature(
    snapshots: &[FeatureOpHashSnapshot],
    mismatches: &[FeatureOpHashMismatch],
    reference_commit_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshots,
            mismatches,
            reference_commit_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize feature-op hash parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{PARITY_FEATURE_OP_HASH_ISSUE_ID, build_feature_op_hash_parity_manifest};
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "1b59e7948efcdb848d8dba6848785d57aa310e81".to_string(),
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
    fn build_manifest_tracks_feature_op_hash_reference_corpus() {
        let manifest = build_feature_op_hash_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_FEATURE_OP_HASH_ISSUE_ID);
        assert!(manifest.reference_commit_match);
        assert!(manifest.case_count >= 10);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.matched_case_count, manifest.case_count);
        assert!(manifest.mismatches.is_empty());
    }
}
