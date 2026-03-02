use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kernel_primitives::make_cube;
use crate::kernel_step::{
    parse_step_entity_ids, read_step_from_buffer, tokenize_step, write_step_to_buffer,
};
use crate::kernel_topology::TopologyCounts;
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_STEP_ISSUE_ID: &str = "VCAD-PARITY-025";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelStepParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub deterministic_step_bytes: usize,
    pub deterministic_step_signature: String,
    pub token_count: usize,
    pub entity_id_count: usize,
    pub round_trip_solid_count: usize,
    pub round_trip_counts: TopologyCounts,
    pub no_solids_error: String,
    pub invalid_utf8_error: String,
    pub parity_contracts: Vec<String>,
}

pub fn build_kernel_step_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelStepParityManifest {
    let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
    let first = write_step_to_buffer(&cube).expect("first write");
    let second = write_step_to_buffer(&cube).expect("second write");
    assert_eq!(first, second, "STEP adapter write must be deterministic");

    let token_count = tokenize_step(&first).expect("tokenize").len();
    let entity_id_count = parse_step_entity_ids(&first).expect("entity ids").len();

    let round_trip = read_step_from_buffer(&first).expect("step round trip");
    let round_trip_counts = round_trip
        .first()
        .map(|solid| solid.topology.counts())
        .unwrap_or_else(|| cube.topology.counts());

    let no_solids_error = format!(
        "{}",
        read_step_from_buffer(b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n")
            .expect_err("no solids should fail")
    );
    let invalid_utf8_error = format!(
        "{}",
        read_step_from_buffer(&[0xff, 0xfe, 0xfd]).expect_err("invalid utf8 should fail")
    );

    KernelStepParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_STEP_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        deterministic_step_bytes: first.len(),
        deterministic_step_signature: sha256_hex(&first)[..16].to_string(),
        token_count,
        entity_id_count,
        round_trip_solid_count: round_trip.len(),
        round_trip_counts,
        no_solids_error,
        invalid_utf8_error,
        parity_contracts: vec![
            "write_step_to_buffer is deterministic for identical BRep input".to_string(),
            "read_step_from_buffer round-trips adapter-authored STEP summaries".to_string(),
            "tokenize_step and parse_step_entity_ids emit stable parse metadata".to_string(),
            "missing solids map to NoSolids error semantics".to_string(),
            "invalid UTF-8 input maps to parse error semantics".to_string(),
        ],
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_STEP_ISSUE_ID, build_kernel_step_parity_manifest};
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
    fn build_manifest_tracks_step_contracts() {
        let manifest = build_kernel_step_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_STEP_ISSUE_ID);
        assert!(manifest.deterministic_step_bytes > 0);
        assert!(manifest.token_count > 20);
        assert!(manifest.entity_id_count >= 3);
        assert_eq!(manifest.round_trip_solid_count, 1);
        assert_eq!(manifest.round_trip_counts.face_count, 6);
        assert!(manifest.no_solids_error.contains("No solids found"));
        assert!(manifest.invalid_utf8_error.contains("parse error"));
    }
}
