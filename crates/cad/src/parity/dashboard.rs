use serde::{Deserialize, Serialize};

use crate::parity::ci_artifacts::ParityCiArtifactManifest;
use crate::parity::risk_register::ParityRiskRegister;
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_DASHBOARD_ISSUE_ID: &str = "VCAD-PARITY-010";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityDashboard {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from: DashboardSources,
    pub summary: DashboardSummary,
    pub profile_status: Vec<DashboardProfileStatus>,
    pub artifacts: DashboardArtifacts,
    pub phase_status: String,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardSources {
    pub scorecard_path: String,
    pub risk_register_path: String,
    pub ci_manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardSummary {
    pub docs_match_rate: f64,
    pub crates_match_rate: f64,
    pub commands_match_rate: f64,
    pub overall_match_rate: f64,
    pub open_risks: usize,
    pub open_hard_blockers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardProfileStatus {
    pub profile_id: String,
    pub lane: String,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardArtifacts {
    pub source_artifact_count: usize,
    pub artifact_ids: Vec<String>,
}

pub fn build_dashboard(
    scorecard: &ParityScorecard,
    risk_register: &ParityRiskRegister,
    ci_manifest: &ParityCiArtifactManifest,
    scorecard_path: &str,
    risk_register_path: &str,
    ci_manifest_path: &str,
) -> ParityDashboard {
    let summary = DashboardSummary {
        docs_match_rate: scorecard.current.docs_match_rate,
        crates_match_rate: scorecard.current.crates_match_rate,
        commands_match_rate: scorecard.current.commands_match_rate,
        overall_match_rate: scorecard.current.overall_match_rate,
        open_risks: risk_register.summary.open_total,
        open_hard_blockers: risk_register.summary.open_hard_blockers,
    };

    let mut profile_status = Vec::new();
    profile_status.extend(
        scorecard
            .evaluations
            .iter()
            .map(|evaluation| DashboardProfileStatus {
                profile_id: evaluation.profile_id.clone(),
                lane: "scorecard".to_string(),
                pass: evaluation.pass,
            }),
    );
    profile_status.extend(risk_register.workflow.evaluations.iter().map(|evaluation| {
        DashboardProfileStatus {
            profile_id: evaluation.profile_id.clone(),
            lane: "risk_register".to_string(),
            pass: evaluation.pass,
        }
    }));
    profile_status.sort_by(|left, right| {
        left.profile_id
            .cmp(&right.profile_id)
            .then_with(|| left.lane.cmp(&right.lane))
    });

    let mut artifact_ids: Vec<String> = ci_manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect();
    artifact_ids.sort();
    let artifacts = DashboardArtifacts {
        source_artifact_count: ci_manifest.source_artifact_count,
        artifact_ids,
    };

    let baseline_scorecard_pass = scorecard
        .evaluations
        .iter()
        .any(|evaluation| evaluation.profile_id == "phase_a_baseline_v1" && evaluation.pass);
    let baseline_risk_pass = risk_register
        .workflow
        .evaluations
        .iter()
        .any(|evaluation| evaluation.profile_id == "phase_a_baseline_v1" && evaluation.pass);
    let has_phase_c_checkpoint = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "core_modeling_checkpoint_parity_manifest");
    let phase_status = if baseline_scorecard_pass && baseline_risk_pass && has_phase_c_checkpoint {
        "phase_c_core_modeling_complete".to_string()
    } else if baseline_scorecard_pass && baseline_risk_pass {
        "phase_a_baseline_complete".to_string()
    } else {
        "phase_a_baseline_at_risk".to_string()
    };
    let next_actions = if phase_status == "phase_c_core_modeling_complete" {
        vec![
            "Execute VCAD-PARITY-041 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else {
        vec![
            "Execute VCAD-PARITY-011 through VCAD-PARITY-025 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    };

    ParityDashboard {
        manifest_version: 1,
        issue_id: PARITY_DASHBOARD_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from: DashboardSources {
            scorecard_path: scorecard_path.to_string(),
            risk_register_path: risk_register_path.to_string(),
            ci_manifest_path: ci_manifest_path.to_string(),
        },
        summary,
        profile_status,
        artifacts,
        phase_status,
        next_actions,
    }
}

pub fn render_dashboard_markdown(dashboard: &ParityDashboard) -> String {
    let mut lines = Vec::new();
    lines.push("# Baseline Parity Dashboard".to_string());
    lines.push(String::new());
    lines.push(format!("Issue coverage: `{}`", dashboard.issue_id));
    lines.push(String::new());
    lines.push("## Snapshot".to_string());
    lines.push(String::new());
    lines.push("| Metric | Value |".to_string());
    lines.push("| --- | --- |".to_string());
    lines.push(format!("| vcad commit | `{}` |", dashboard.vcad_commit));
    lines.push(format!(
        "| openagents commit (plan baseline) | `{}` |",
        dashboard.openagents_commit
    ));
    lines.push(format!("| phase status | `{}` |", dashboard.phase_status));
    lines.push(format!(
        "| overall match rate | `{:.6}` |",
        dashboard.summary.overall_match_rate
    ));
    lines.push(format!(
        "| docs match rate | `{:.6}` |",
        dashboard.summary.docs_match_rate
    ));
    lines.push(format!(
        "| crates match rate | `{:.6}` |",
        dashboard.summary.crates_match_rate
    ));
    lines.push(format!(
        "| commands match rate | `{:.6}` |",
        dashboard.summary.commands_match_rate
    ));
    lines.push(format!(
        "| open risks | `{}` |",
        dashboard.summary.open_risks
    ));
    lines.push(format!(
        "| open hard blockers (p0) | `{}` |",
        dashboard.summary.open_hard_blockers
    ));
    lines.push(format!(
        "| CI source artifact count | `{}` |",
        dashboard.artifacts.source_artifact_count
    ));
    lines.push(String::new());
    lines.push("## Profile Gates".to_string());
    lines.push(String::new());
    lines.push("| Lane | Profile | Pass |".to_string());
    lines.push("| --- | --- | --- |".to_string());
    for profile in &dashboard.profile_status {
        lines.push(format!(
            "| `{}` | `{}` | `{}` |",
            profile.lane, profile.profile_id, profile.pass
        ));
    }
    lines.push(String::new());
    lines.push("## CI Evidence Artifacts".to_string());
    lines.push(String::new());
    for artifact_id in &dashboard.artifacts.artifact_ids {
        lines.push(format!("- `{}`", artifact_id));
    }
    lines.push(String::new());
    lines.push("## Next Actions".to_string());
    lines.push(String::new());
    for action in &dashboard.next_actions {
        lines.push(format!("- {}", action));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{ParityDashboard, render_dashboard_markdown};

    #[test]
    fn render_dashboard_markdown_emits_header() {
        let dashboard = ParityDashboard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-010".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from: super::DashboardSources {
                scorecard_path: "a".to_string(),
                risk_register_path: "b".to_string(),
                ci_manifest_path: "c".to_string(),
            },
            summary: super::DashboardSummary {
                docs_match_rate: 0.1,
                crates_match_rate: 0.1,
                commands_match_rate: 0.1,
                overall_match_rate: 0.1,
                open_risks: 1,
                open_hard_blockers: 1,
            },
            profile_status: vec![super::DashboardProfileStatus {
                profile_id: "phase_a_baseline_v1".to_string(),
                lane: "scorecard".to_string(),
                pass: true,
            }],
            artifacts: super::DashboardArtifacts {
                source_artifact_count: 1,
                artifact_ids: vec!["artifact".to_string()],
            },
            phase_status: "phase_c_core_modeling_complete".to_string(),
            next_actions: vec!["x".to_string()],
        };
        let markdown = render_dashboard_markdown(&dashboard);
        assert!(markdown.contains("# Baseline Parity Dashboard"));
        assert!(markdown.contains("phase_c_core_modeling_complete"));
    }
}
