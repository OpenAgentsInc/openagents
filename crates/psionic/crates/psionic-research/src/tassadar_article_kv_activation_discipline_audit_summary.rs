use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::{
    tassadar_article_kv_activation_discipline_audit_report_path,
    TassadarArticleKvActivationDisciplineAuditReport, TassadarArticleStateDominanceVerdictKind,
};

pub const TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_kv_activation_discipline_audit_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationDisciplineAuditSummary {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: String,
    pub ownership_gate_green: bool,
    pub feasible_constraint_case_count: usize,
    pub dominance_verdict: TassadarArticleStateDominanceVerdictKind,
    pub cache_growth_scales_with_problem_size: bool,
    pub dynamic_state_exceeds_weight_artifact_bytes: bool,
    pub cache_truncation_breaks_correctness: bool,
    pub cache_reset_breaks_correctness: bool,
    pub equivalent_behavior_survives_under_constrained_cache: bool,
    pub kv_activation_discipline_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
    pub summary_digest: String,
}

pub fn build_tassadar_article_kv_activation_discipline_audit_summary_from_report(
    report: &TassadarArticleKvActivationDisciplineAuditReport,
) -> TassadarArticleKvActivationDisciplineAuditSummary {
    let mut summary = TassadarArticleKvActivationDisciplineAuditSummary {
        report_id: report.report_id.clone(),
        tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
        tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
        blocked_issue_frontier: report
            .acceptance_gate_tie
            .blocked_issue_ids
            .first()
            .cloned()
            .unwrap_or_else(|| String::from("none")),
        ownership_gate_green: report.ownership_gate_green,
        feasible_constraint_case_count: report.growth_report.feasible_constraint_case_ids.len(),
        dominance_verdict: report.dominance_verdict.verdict,
        cache_growth_scales_with_problem_size: report
            .growth_report
            .cache_growth_scales_with_problem_size,
        dynamic_state_exceeds_weight_artifact_bytes: report
            .growth_report
            .dynamic_state_exceeds_weight_artifact_bytes,
        cache_truncation_breaks_correctness: report
            .sensitivity_review
            .cache_truncation_breaks_correctness,
        cache_reset_breaks_correctness: report.sensitivity_review.cache_reset_breaks_correctness,
        equivalent_behavior_survives_under_constrained_cache: report
            .sensitivity_review
            .equivalent_behavior_survives_under_constrained_cache,
        kv_activation_discipline_green: report.kv_activation_discipline_green,
        article_equivalence_green: report.article_equivalence_green,
        detail: format!(
            "TAS-184A now records ownership_gate_green={}, feasible_constraint_case_count={}, verdict={:?}, cache_growth_scales_with_problem_size={}, dynamic_state_exceeds_weight_artifact_bytes={}, cache_truncation_breaks_correctness={}, cache_reset_breaks_correctness={}, constrained_cache_equivalence={}, kv_activation_discipline_green={}, article_equivalence_green={}",
            report.ownership_gate_green,
            report.growth_report.feasible_constraint_case_ids.len(),
            report.dominance_verdict.verdict,
            report.growth_report.cache_growth_scales_with_problem_size,
            report.growth_report.dynamic_state_exceeds_weight_artifact_bytes,
            report.sensitivity_review.cache_truncation_breaks_correctness,
            report.sensitivity_review.cache_reset_breaks_correctness,
            report
                .sensitivity_review
                .equivalent_behavior_survives_under_constrained_cache,
            report.kv_activation_discipline_green,
            report.article_equivalence_green,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_article_kv_activation_discipline_audit_summary|",
        &summary,
    );
    summary
}

pub fn build_tassadar_article_kv_activation_discipline_audit_summary(
) -> Result<TassadarArticleKvActivationDisciplineAuditSummary, Box<dyn std::error::Error>> {
    let report: TassadarArticleKvActivationDisciplineAuditReport = serde_json::from_slice(
        &fs::read(tassadar_article_kv_activation_discipline_audit_report_path())?,
    )?;
    Ok(build_tassadar_article_kv_activation_discipline_audit_summary_from_report(&report))
}

pub fn tassadar_article_kv_activation_discipline_audit_summary_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join(TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_SUMMARY_REF)
}

pub fn write_tassadar_article_kv_activation_discipline_audit_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleKvActivationDisciplineAuditSummary, Box<dyn std::error::Error>> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_article_kv_activation_discipline_audit_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n"))?;
    Ok(summary)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("summary serialization"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_kv_activation_discipline_audit_summary,
        build_tassadar_article_kv_activation_discipline_audit_summary_from_report,
        tassadar_article_kv_activation_discipline_audit_summary_path,
        write_tassadar_article_kv_activation_discipline_audit_summary,
        TassadarArticleKvActivationDisciplineAuditSummary,
        TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_SUMMARY_REF,
    };
    use psionic_eval::{
        TassadarArticleKvActivationDisciplineAuditReport,
        TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
    };
    use tempfile::tempdir;

    fn read_repo_json<T: for<'de> serde::Deserialize<'de>>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join(repo_relative_path);
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    #[test]
    fn article_kv_activation_discipline_audit_summary_tracks_green_gate(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let summary = build_tassadar_article_kv_activation_discipline_audit_summary()?;

        assert_eq!(summary.tied_requirement_id, "TAS-184A");
        assert_eq!(summary.blocked_issue_frontier, "none");
        assert_eq!(summary.feasible_constraint_case_count, 4);
        assert_eq!(
            summary.dominance_verdict,
            psionic_eval::TassadarArticleStateDominanceVerdictKind::Mixed
        );
        assert!(summary.cache_growth_scales_with_problem_size);
        assert!(summary.dynamic_state_exceeds_weight_artifact_bytes);
        assert!(summary.cache_truncation_breaks_correctness);
        assert!(summary.cache_reset_breaks_correctness);
        assert!(!summary.equivalent_behavior_survives_under_constrained_cache);
        assert!(summary.kv_activation_discipline_green);
        assert!(summary.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_kv_activation_discipline_audit_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report: TassadarArticleKvActivationDisciplineAuditReport =
            read_repo_json(TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF)?;
        let generated =
            build_tassadar_article_kv_activation_discipline_audit_summary_from_report(&report);
        let committed: TassadarArticleKvActivationDisciplineAuditSummary =
            read_repo_json(TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_SUMMARY_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_kv_activation_discipline_audit_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_kv_activation_discipline_audit_summary.json");
        let written = write_tassadar_article_kv_activation_discipline_audit_summary(&output_path)?;
        let persisted: TassadarArticleKvActivationDisciplineAuditSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_kv_activation_discipline_audit_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_kv_activation_discipline_audit_summary.json")
        );
        Ok(())
    }
}
