use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::parity::fixture_corpus::{ParityFixtureCorpus, ParityFixtureSeed};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_RISK_REGISTER_ISSUE_ID: &str = "VCAD-PARITY-009";
pub const HARD_BLOCKER_PRIORITY: &str = "p0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityRiskRegister {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_fixture_corpus: String,
    pub generated_from_scorecard: String,
    pub workflow: BlockerWorkflow,
    pub risks: Vec<ParityRisk>,
    pub summary: RiskRegisterSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockerWorkflow {
    pub hard_blocker_priority: String,
    pub profiles: Vec<BlockerProfile>,
    pub evaluations: Vec<BlockerEvaluation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockerProfile {
    pub profile_id: String,
    pub description: String,
    pub max_open_hard_blockers: usize,
    pub max_open_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockerEvaluation {
    pub profile_id: String,
    pub open_hard_blockers: usize,
    pub open_total: usize,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityRisk {
    pub risk_id: String,
    pub fixture_id: String,
    pub surface: String,
    pub priority: String,
    pub severity: String,
    pub status: String,
    pub blocker_state: String,
    pub reference_key: String,
    pub openagents_key: Option<String>,
    pub description: String,
    pub mitigation: String,
    pub owner_lane: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskRegisterSummary {
    pub open_total: usize,
    pub open_hard_blockers: usize,
    pub by_priority: BTreeMap<String, usize>,
    pub by_surface: BTreeMap<String, usize>,
}

pub fn default_blocker_profiles() -> Vec<BlockerProfile> {
    vec![
        BlockerProfile {
            profile_id: "phase_a_baseline_v1".to_string(),
            description: "Program baseline gate: ensure hard blockers are tracked and bounded."
                .to_string(),
            max_open_hard_blockers: 24,
            max_open_total: 32,
        },
        BlockerProfile {
            profile_id: "parity_complete_v1".to_string(),
            description: "Final parity signoff: no open blockers and no open risks.".to_string(),
            max_open_hard_blockers: 0,
            max_open_total: 0,
        },
    ]
}

pub fn build_risk_register(
    fixture_corpus: &ParityFixtureCorpus,
    scorecard: &ParityScorecard,
    fixture_corpus_path: &str,
    scorecard_path: &str,
) -> ParityRiskRegister {
    let mut by_priority = BTreeMap::new();
    let mut by_surface = BTreeMap::new();
    let mut missing: Vec<&ParityFixtureSeed> = fixture_corpus
        .fixtures
        .iter()
        .filter(|fixture| fixture.state == "missing")
        .collect();
    missing.sort_by(|left, right| left.fixture_id.cmp(&right.fixture_id));

    let risks: Vec<ParityRisk> = missing
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            *by_priority.entry(fixture.priority.clone()).or_insert(0) += 1;
            *by_surface.entry(fixture.surface.clone()).or_insert(0) += 1;
            risk_from_fixture(index, fixture)
        })
        .collect();

    let summary = RiskRegisterSummary {
        open_total: risks.len(),
        open_hard_blockers: risks
            .iter()
            .filter(|risk| risk.priority == HARD_BLOCKER_PRIORITY)
            .count(),
        by_priority,
        by_surface,
    };

    let profiles = default_blocker_profiles();
    let evaluations = profiles
        .iter()
        .map(|profile| evaluate_profile(profile, &summary))
        .collect();

    ParityRiskRegister {
        manifest_version: 1,
        issue_id: PARITY_RISK_REGISTER_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_fixture_corpus: fixture_corpus_path.to_string(),
        generated_from_scorecard: scorecard_path.to_string(),
        workflow: BlockerWorkflow {
            hard_blocker_priority: HARD_BLOCKER_PRIORITY.to_string(),
            profiles,
            evaluations,
        },
        risks,
        summary,
    }
}

pub fn evaluate_profile(
    profile: &BlockerProfile,
    summary: &RiskRegisterSummary,
) -> BlockerEvaluation {
    let pass = summary.open_hard_blockers <= profile.max_open_hard_blockers
        && summary.open_total <= profile.max_open_total;
    BlockerEvaluation {
        profile_id: profile.profile_id.clone(),
        open_hard_blockers: summary.open_hard_blockers,
        open_total: summary.open_total,
        pass,
    }
}

fn risk_from_fixture(index: usize, fixture: &ParityFixtureSeed) -> ParityRisk {
    let severity = match fixture.priority.as_str() {
        "p0" => "critical",
        "p1" => "high",
        _ => "medium",
    };

    let blocker_state = if fixture.priority == HARD_BLOCKER_PRIORITY {
        "open_blocker"
    } else {
        "open_risk"
    };

    ParityRisk {
        risk_id: format!("risk.{:03}", index + 1),
        fixture_id: fixture.fixture_id.clone(),
        surface: fixture.surface.clone(),
        priority: fixture.priority.clone(),
        severity: severity.to_string(),
        status: "open".to_string(),
        blocker_state: blocker_state.to_string(),
        reference_key: fixture.reference_key.clone(),
        openagents_key: fixture.openagents_key.clone(),
        description: format!(
            "Missing parity capability for {} ({})",
            fixture.reference_key, fixture.surface
        ),
        mitigation: mitigation_for(fixture).to_string(),
        owner_lane: "crates/cad".to_string(),
        evidence_refs: vec![
            "crates/cad/parity/fixtures/parity_fixture_corpus.json".to_string(),
            "crates/cad/parity/parity_scorecard.json".to_string(),
        ],
    }
}

fn mitigation_for(fixture: &ParityFixtureSeed) -> &'static str {
    match (fixture.surface.as_str(), fixture.priority.as_str()) {
        ("commands", "p0") => {
            "prioritize command-surface parity implementation and replay fixtures"
        }
        ("docs", "p0") => {
            "close documentation capability gaps with deterministic contract coverage"
        }
        ("crates", "p1") => "add crate-level parity adapters and integration fixtures",
        _ => "track in parity queue and resolve before parity-complete gate",
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockerProfile, RiskRegisterSummary, evaluate_profile, mitigation_for};
    use crate::parity::fixture_corpus::ParityFixtureSeed;

    #[test]
    fn evaluate_profile_passes_when_summary_within_limits() {
        let profile = BlockerProfile {
            profile_id: "test".to_string(),
            description: "test".to_string(),
            max_open_hard_blockers: 3,
            max_open_total: 5,
        };
        let summary = RiskRegisterSummary {
            open_total: 5,
            open_hard_blockers: 3,
            by_priority: Default::default(),
            by_surface: Default::default(),
        };
        let evaluation = evaluate_profile(&profile, &summary);
        assert!(evaluation.pass);
    }

    #[test]
    fn mitigation_for_returns_surface_priority_specific_message() {
        let fixture = ParityFixtureSeed {
            fixture_id: "commands.missing.01".to_string(),
            surface: "commands".to_string(),
            state: "missing".to_string(),
            reference_key: "vcad_cli".to_string(),
            reference_label: "vcad cli".to_string(),
            reference_status: None,
            openagents_key: None,
            openagents_label: None,
            score: 0.0,
            priority: "p0".to_string(),
        };
        assert_eq!(
            mitigation_for(&fixture),
            "prioritize command-surface parity implementation and replay fixtures"
        );
    }
}
