use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_text::{FontRegistry, TextAlignment, text_bounds, text_to_profiles};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_TEXT_ISSUE_ID: &str = "VCAD-PARITY-022";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelTextParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub sample_text: String,
    pub left_alignment: TextGeometrySnapshot,
    pub center_alignment: TextGeometrySnapshot,
    pub right_alignment: TextGeometrySnapshot,
    pub multiline_bounds_mm: [f64; 2],
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextGeometrySnapshot {
    pub profile_count: usize,
    pub hole_profile_count: usize,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub deterministic_signature: String,
}

pub fn build_kernel_text_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelTextParityManifest {
    let font = FontRegistry::builtin_sans();
    let sample_text = "OpenAgents 90".to_string();

    let left_profiles = text_to_profiles(&sample_text, &font, 10.0, 1.0, 1.2, TextAlignment::Left)
        .expect("left profiles");
    let center_profiles =
        text_to_profiles(&sample_text, &font, 10.0, 1.0, 1.2, TextAlignment::Center)
            .expect("center profiles");
    let right_profiles =
        text_to_profiles(&sample_text, &font, 10.0, 1.0, 1.2, TextAlignment::Right)
            .expect("right profiles");
    let bounds = text_bounds("Open\nAgents", &font, 10.0, 1.0, 1.2).expect("bounds");

    KernelTextParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_TEXT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        sample_text,
        left_alignment: snapshot(&left_profiles),
        center_alignment: snapshot(&center_profiles),
        right_alignment: snapshot(&right_profiles),
        multiline_bounds_mm: [bounds.0, bounds.1],
        parity_contracts: vec![
            "text_to_profiles is deterministic for fixed text/layout/font inputs".to_string(),
            "alignment offsets shift profile x-range while preserving profile counts".to_string(),
            "invalid text layout inputs map to CadError::InvalidParameter".to_string(),
            "multiline bounds use line_spacing scaling".to_string(),
        ],
    }
}

fn snapshot(profiles: &[crate::kernel_text::TextProfile]) -> TextGeometrySnapshot {
    let profile_count = profiles.len();
    let hole_profile_count = profiles.iter().filter(|profile| profile.is_hole).count();
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for profile in profiles {
        for point in &profile.points {
            min_x = min_x.min(point[0]);
            max_x = max_x.max(point[0]);
            min_y = min_y.min(point[1]);
            max_y = max_y.max(point[1]);
        }
    }
    if !min_x.is_finite() {
        min_x = 0.0;
        max_x = 0.0;
        min_y = 0.0;
        max_y = 0.0;
    }

    let signature_payload: Vec<(bool, usize, i64, i64)> = profiles
        .iter()
        .map(|profile| {
            let first = profile.points.first().copied().unwrap_or([0.0, 0.0]);
            (
                profile.is_hole,
                profile.points.len(),
                (first[0] * 1_000_000.0).round() as i64,
                (first[1] * 1_000_000.0).round() as i64,
            )
        })
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&signature_payload).expect("serialize signature payload"));
    let digest = hasher.finalize();

    TextGeometrySnapshot {
        profile_count,
        hole_profile_count,
        min_x,
        max_x,
        min_y,
        max_y,
        deterministic_signature: format!("{:x}", digest)[..16].to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_TEXT_ISSUE_ID, build_kernel_text_parity_manifest};
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
    fn build_manifest_has_alignment_snapshots() {
        let manifest = build_kernel_text_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_TEXT_ISSUE_ID);
        assert!(manifest.left_alignment.profile_count > 0);
        assert_eq!(
            manifest.left_alignment.profile_count,
            manifest.center_alignment.profile_count
        );
        assert_eq!(
            manifest.left_alignment.profile_count,
            manifest.right_alignment.profile_count
        );
        assert!(manifest.center_alignment.min_x < manifest.left_alignment.min_x);
        assert!(manifest.right_alignment.min_x < manifest.center_alignment.min_x);
    }
}
