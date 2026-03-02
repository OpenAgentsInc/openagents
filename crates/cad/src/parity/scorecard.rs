use serde::{Deserialize, Serialize};

use crate::parity::gap_matrix::ParityGapMatrix;

pub const PARITY_SCORECARD_ISSUE_ID: &str = "VCAD-PARITY-005";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityScorecard {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_gap_matrix: String,
    pub current: ScorecardCurrent,
    pub threshold_profiles: Vec<ScorecardThresholdProfile>,
    pub evaluations: Vec<ScorecardEvaluation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScorecardCurrent {
    pub docs_match_rate: f64,
    pub crates_match_rate: f64,
    pub commands_match_rate: f64,
    pub overall_match_rate: f64,
    pub docs_reference_count: usize,
    pub crates_reference_count: usize,
    pub commands_reference_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScorecardThresholdProfile {
    pub profile_id: String,
    pub docs_match_rate_min: f64,
    pub crates_match_rate_min: f64,
    pub commands_match_rate_min: f64,
    pub overall_match_rate_min: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScorecardEvaluation {
    pub profile_id: String,
    pub docs_pass: bool,
    pub crates_pass: bool,
    pub commands_pass: bool,
    pub overall_pass: bool,
    pub pass: bool,
}

pub fn default_threshold_profiles() -> Vec<ScorecardThresholdProfile> {
    vec![
        // Baseline quality gate for Phase A program scaffolding.
        ScorecardThresholdProfile {
            profile_id: "phase_a_baseline_v1".to_string(),
            docs_match_rate_min: 0.03,
            crates_match_rate_min: 0.10,
            commands_match_rate_min: 0.08,
            overall_match_rate_min: 0.07,
        },
        // End-state parity target for release signoff.
        ScorecardThresholdProfile {
            profile_id: "parity_complete_v1".to_string(),
            docs_match_rate_min: 1.0,
            crates_match_rate_min: 1.0,
            commands_match_rate_min: 1.0,
            overall_match_rate_min: 1.0,
        },
    ]
}

pub fn build_scorecard(
    matrix: &ParityGapMatrix,
    generated_from_gap_matrix: &str,
) -> ParityScorecard {
    let current = ScorecardCurrent {
        docs_match_rate: matrix.docs.match_rate,
        crates_match_rate: matrix.crates.match_rate,
        commands_match_rate: matrix.commands.match_rate,
        overall_match_rate: matrix.summary.total_match_rate,
        docs_reference_count: matrix.docs.reference_count,
        crates_reference_count: matrix.crates.reference_count,
        commands_reference_count: matrix.commands.reference_count,
    };

    let threshold_profiles = default_threshold_profiles();
    let evaluations = threshold_profiles
        .iter()
        .map(|profile| evaluate_profile(profile, &current))
        .collect();

    ParityScorecard {
        manifest_version: 1,
        issue_id: PARITY_SCORECARD_ISSUE_ID.to_string(),
        vcad_commit: matrix.vcad_commit.clone(),
        openagents_commit: matrix.openagents_commit.clone(),
        generated_from_gap_matrix: generated_from_gap_matrix.to_string(),
        current,
        threshold_profiles,
        evaluations,
    }
}

pub fn evaluate_profile(
    profile: &ScorecardThresholdProfile,
    current: &ScorecardCurrent,
) -> ScorecardEvaluation {
    let docs_pass = current.docs_match_rate >= profile.docs_match_rate_min;
    let crates_pass = current.crates_match_rate >= profile.crates_match_rate_min;
    let commands_pass = current.commands_match_rate >= profile.commands_match_rate_min;
    let overall_pass = current.overall_match_rate >= profile.overall_match_rate_min;
    ScorecardEvaluation {
        profile_id: profile.profile_id.clone(),
        docs_pass,
        crates_pass,
        commands_pass,
        overall_pass,
        pass: docs_pass && crates_pass && commands_pass && overall_pass,
    }
}

#[cfg(test)]
mod tests {
    use super::{ScorecardCurrent, ScorecardThresholdProfile, build_scorecard, evaluate_profile};
    use crate::parity::gap_matrix::{
        GapMatrixSources, GapMatrixSummary, GapMatrixSurface, ParityGapMatrix,
    };

    #[test]
    fn evaluate_profile_returns_pass_when_all_thresholds_met() {
        let profile = ScorecardThresholdProfile {
            profile_id: "test".to_string(),
            docs_match_rate_min: 0.2,
            crates_match_rate_min: 0.2,
            commands_match_rate_min: 0.2,
            overall_match_rate_min: 0.2,
        };
        let current = ScorecardCurrent {
            docs_match_rate: 0.5,
            crates_match_rate: 0.5,
            commands_match_rate: 0.5,
            overall_match_rate: 0.5,
            docs_reference_count: 1,
            crates_reference_count: 1,
            commands_reference_count: 1,
        };
        let evaluation = evaluate_profile(&profile, &current);
        assert!(evaluation.pass);
    }

    #[test]
    fn build_scorecard_carries_commit_metadata() {
        let matrix = ParityGapMatrix {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-004".to_string(),
            vcad_commit: "vcad-sha".to_string(),
            openagents_commit: "openagents-sha".to_string(),
            generated_from: GapMatrixSources {
                vcad_inventory_path: "a".to_string(),
                openagents_inventory_path: "b".to_string(),
            },
            docs: GapMatrixSurface {
                reference_count: 10,
                matched_count: 2,
                missing_count: 8,
                match_rate: 0.2,
                rows: Vec::new(),
            },
            crates: GapMatrixSurface {
                reference_count: 10,
                matched_count: 2,
                missing_count: 8,
                match_rate: 0.2,
                rows: Vec::new(),
            },
            commands: GapMatrixSurface {
                reference_count: 10,
                matched_count: 2,
                missing_count: 8,
                match_rate: 0.2,
                rows: Vec::new(),
            },
            summary: GapMatrixSummary {
                total_reference_count: 30,
                total_matched_count: 6,
                total_missing_count: 24,
                total_match_rate: 0.2,
            },
        };
        let scorecard = build_scorecard(&matrix, "gap.json");
        assert_eq!(scorecard.vcad_commit, "vcad-sha");
        assert_eq!(scorecard.openagents_commit, "openagents-sha");
        assert_eq!(scorecard.generated_from_gap_matrix, "gap.json");
        assert_eq!(scorecard.evaluations.len(), 2);
    }
}
