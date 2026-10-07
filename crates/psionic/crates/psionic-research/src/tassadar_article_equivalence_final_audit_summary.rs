use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::{
    tassadar_article_equivalence_final_audit_report_path,
    TassadarArticleEquivalenceFinalAuditReport,
};

pub const TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_final_audit_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditSummary {
    pub report_id: String,
    pub matched_article_line_count: usize,
    pub all_article_lines_matched: bool,
    pub mechanistic_verdict_green: bool,
    pub behavioral_verdict_green: bool,
    pub operational_verdict_green: bool,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub canonical_decode_mode: String,
    pub supported_machine_class_ids: Vec<String>,
    pub optional_open_issue_ids: Vec<String>,
    pub public_article_equivalence_claim_allowed: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
    pub summary_digest: String,
}

pub fn build_tassadar_article_equivalence_final_audit_summary_from_report(
    report: &TassadarArticleEquivalenceFinalAuditReport,
) -> TassadarArticleEquivalenceFinalAuditSummary {
    let mut summary = TassadarArticleEquivalenceFinalAuditSummary {
        report_id: report.report_id.clone(),
        matched_article_line_count: report.matched_article_line_count,
        all_article_lines_matched: report.all_article_lines_matched,
        mechanistic_verdict_green: report.verdict_review.mechanistic_verdict_green,
        behavioral_verdict_green: report.verdict_review.behavioral_verdict_green,
        operational_verdict_green: report.verdict_review.operational_verdict_green,
        canonical_model_id: report
            .canonical_closure_review
            .canonical_model_id
            .clone(),
        canonical_weight_artifact_id: report
            .canonical_closure_review
            .canonical_weight_artifact_id
            .clone(),
        canonical_route_id: report
            .canonical_closure_review
            .canonical_route_id
            .clone(),
        canonical_decode_mode: report
            .canonical_closure_review
            .canonical_decode_mode
            .clone(),
        supported_machine_class_ids: report
            .machine_matrix_review
            .supported_machine_class_ids
            .clone(),
        optional_open_issue_ids: report.exclusion_review.optional_open_issue_ids.clone(),
        public_article_equivalence_claim_allowed: report.public_article_equivalence_claim_allowed,
        article_equivalence_green: report.article_equivalence_green,
        detail: format!(
            "TAS-186 now records matched_article_line_count={}, mechanistic_verdict_green={}, behavioral_verdict_green={}, operational_verdict_green={}, canonical_route_id=`{}`, supported_machine_classes={}, public_article_equivalence_claim_allowed={}, and article_equivalence_green={}.",
            report.matched_article_line_count,
            report.verdict_review.mechanistic_verdict_green,
            report.verdict_review.behavioral_verdict_green,
            report.verdict_review.operational_verdict_green,
            report.canonical_closure_review.canonical_route_id,
            report.machine_matrix_review.supported_machine_class_ids.len(),
            report.public_article_equivalence_claim_allowed,
            report.article_equivalence_green,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_article_equivalence_final_audit_summary|",
        &summary,
    );
    summary
}

pub fn build_tassadar_article_equivalence_final_audit_summary(
) -> Result<TassadarArticleEquivalenceFinalAuditSummary, Box<dyn std::error::Error>> {
    let report: TassadarArticleEquivalenceFinalAuditReport = serde_json::from_slice(&fs::read(
        tassadar_article_equivalence_final_audit_report_path(),
    )?)?;
    Ok(build_tassadar_article_equivalence_final_audit_summary_from_report(&report))
}

pub fn tassadar_article_equivalence_final_audit_summary_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_SUMMARY_REF)
}

pub fn write_tassadar_article_equivalence_final_audit_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleEquivalenceFinalAuditSummary, Box<dyn std::error::Error>> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_article_equivalence_final_audit_summary()?;
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
        build_tassadar_article_equivalence_final_audit_summary,
        build_tassadar_article_equivalence_final_audit_summary_from_report,
        tassadar_article_equivalence_final_audit_summary_path,
        write_tassadar_article_equivalence_final_audit_summary,
        TassadarArticleEquivalenceFinalAuditSummary,
        TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_SUMMARY_REF,
    };
    use psionic_eval::{
        TassadarArticleEquivalenceFinalAuditReport,
        TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
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
    fn article_equivalence_final_audit_summary_tracks_green_closeout(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let summary = build_tassadar_article_equivalence_final_audit_summary()?;

        assert!(summary.all_article_lines_matched);
        assert!(summary.mechanistic_verdict_green);
        assert!(summary.behavioral_verdict_green);
        assert!(summary.operational_verdict_green);
        assert_eq!(
            summary.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(summary.canonical_decode_mode, "hull_cache");
        assert_eq!(summary.supported_machine_class_ids.len(), 2);
        assert_eq!(
            summary.optional_open_issue_ids,
            vec![String::from("TAS-R1")]
        );
        assert!(summary.public_article_equivalence_claim_allowed);
        assert!(summary.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_equivalence_final_audit_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report: TassadarArticleEquivalenceFinalAuditReport =
            read_repo_json(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF)?;
        let generated = build_tassadar_article_equivalence_final_audit_summary_from_report(&report);
        let committed: TassadarArticleEquivalenceFinalAuditSummary =
            read_repo_json(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_SUMMARY_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_equivalence_final_audit_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_equivalence_final_audit_summary.json");
        let written = write_tassadar_article_equivalence_final_audit_summary(&output_path)?;
        let persisted: TassadarArticleEquivalenceFinalAuditSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_equivalence_final_audit_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_equivalence_final_audit_summary.json")
        );
        Ok(())
    }
}
