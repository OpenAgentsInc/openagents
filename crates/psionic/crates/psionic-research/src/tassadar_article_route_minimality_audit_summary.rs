use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::{
    tassadar_article_route_minimality_audit_report_path, TassadarArticleRouteMinimalityAuditReport,
    TassadarArticleRouteMinimalityPublicPosture,
};

pub const TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_route_minimality_audit_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityAuditSummary {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: String,
    pub canonical_claim_route_id: String,
    pub route_descriptor_digest: String,
    pub selected_decode_mode: String,
    pub operator_verdict_green: bool,
    pub public_posture: TassadarArticleRouteMinimalityPublicPosture,
    pub public_verdict_green: bool,
    pub route_minimality_audit_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
    pub summary_digest: String,
}

pub fn build_tassadar_article_route_minimality_audit_summary_from_report(
    report: &TassadarArticleRouteMinimalityAuditReport,
) -> TassadarArticleRouteMinimalityAuditSummary {
    let mut summary = TassadarArticleRouteMinimalityAuditSummary {
        report_id: report.report_id.clone(),
        tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
        tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
        blocked_issue_frontier: report
            .public_verdict_review
            .blocked_issue_ids
            .first()
            .cloned()
            .unwrap_or_else(|| String::from("none")),
        canonical_claim_route_id: report
            .canonical_claim_route_review
            .canonical_claim_route_id
            .clone(),
        route_descriptor_digest: report
            .canonical_claim_route_review
            .projected_route_descriptor_digest
            .clone(),
        selected_decode_mode: report
            .canonical_claim_route_review
            .selected_decode_mode
            .as_str()
            .to_string(),
        operator_verdict_green: report.operator_verdict_review.operator_verdict_green,
        public_posture: report.public_verdict_review.posture,
        public_verdict_green: report.public_verdict_review.public_verdict_green,
        route_minimality_audit_green: report.route_minimality_audit_green,
        article_equivalence_green: report.article_equivalence_green,
        detail: format!(
            "TAS-185A now records canonical_claim_route_id=`{}`, selected_decode_mode=`{}`, operator_verdict_green={}, public_posture={:?}, public_verdict_green={}, route_minimality_audit_green={}, and article_equivalence_green={}.",
            report.canonical_claim_route_review.canonical_claim_route_id,
            report.canonical_claim_route_review.selected_decode_mode.as_str(),
            report.operator_verdict_review.operator_verdict_green,
            report.public_verdict_review.posture,
            report.public_verdict_review.public_verdict_green,
            report.route_minimality_audit_green,
            report.article_equivalence_green,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_article_route_minimality_audit_summary|",
        &summary,
    );
    summary
}

pub fn build_tassadar_article_route_minimality_audit_summary(
) -> Result<TassadarArticleRouteMinimalityAuditSummary, Box<dyn std::error::Error>> {
    let report: TassadarArticleRouteMinimalityAuditReport = serde_json::from_slice(&fs::read(
        tassadar_article_route_minimality_audit_report_path(),
    )?)?;
    Ok(build_tassadar_article_route_minimality_audit_summary_from_report(&report))
}

pub fn tassadar_article_route_minimality_audit_summary_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_SUMMARY_REF)
}

pub fn write_tassadar_article_route_minimality_audit_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleRouteMinimalityAuditSummary, Box<dyn std::error::Error>> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_article_route_minimality_audit_summary()?;
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
        build_tassadar_article_route_minimality_audit_summary,
        build_tassadar_article_route_minimality_audit_summary_from_report,
        tassadar_article_route_minimality_audit_summary_path,
        write_tassadar_article_route_minimality_audit_summary,
        TassadarArticleRouteMinimalityAuditSummary,
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_SUMMARY_REF,
    };
    use psionic_eval::{
        TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityPublicPosture,
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
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
    fn article_route_minimality_audit_summary_tracks_public_green_closeout(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let summary = build_tassadar_article_route_minimality_audit_summary()?;

        assert_eq!(summary.tied_requirement_id, "TAS-185A");
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.blocked_issue_frontier, "none");
        assert_eq!(
            summary.canonical_claim_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            summary.selected_decode_mode,
            "tassadar.decode.hull_cache.v1"
        );
        assert!(summary.operator_verdict_green);
        assert_eq!(
            summary.public_posture,
            TassadarArticleRouteMinimalityPublicPosture::GreenBounded
        );
        assert!(summary.public_verdict_green);
        assert!(summary.route_minimality_audit_green);
        assert!(summary.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_route_minimality_audit_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report: TassadarArticleRouteMinimalityAuditReport =
            read_repo_json(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF)?;
        let generated = build_tassadar_article_route_minimality_audit_summary_from_report(&report);
        let committed: TassadarArticleRouteMinimalityAuditSummary =
            read_repo_json(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_SUMMARY_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_route_minimality_audit_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_route_minimality_audit_summary.json");
        let written = write_tassadar_article_route_minimality_audit_summary(&output_path)?;
        let persisted: TassadarArticleRouteMinimalityAuditSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_route_minimality_audit_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_route_minimality_audit_summary.json")
        );
        Ok(())
    }
}
