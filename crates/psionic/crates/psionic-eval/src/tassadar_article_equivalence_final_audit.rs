use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_article_equivalence_claim_checker_report,
    TassadarArticleEquivalenceBlockerCategory, TassadarArticleEquivalenceClaimCheckerError,
    TassadarArticleEquivalenceClaimCheckerReport, TassadarArticleEquivalenceExclusionReview,
    TASSADAR_ARTICLE_EQUIVALENCE_CLAIM_CHECKER_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_CHECKER_REF,
};

pub const TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_final_audit_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditLineMatchRow {
    pub claim_ref_id: String,
    pub line_start: u16,
    pub line_end: u16,
    pub claim_paraphrase: String,
    pub matched_blocker_ids: Vec<String>,
    pub matched_blocker_categories: Vec<TassadarArticleEquivalenceBlockerCategory>,
    pub matched_issue_ids: Vec<String>,
    pub matched_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditVerdictReview {
    pub mechanistic_verdict_green: bool,
    pub behavioral_verdict_green: bool,
    pub operational_verdict_green: bool,
    pub all_categories_green: bool,
    pub public_article_equivalence_claim_allowed: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditCanonicalClosureReview {
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_decode_mode: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditMachineMatrixReview {
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceFinalAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub claim_checker_report_ref: String,
    pub claim_checker_report: TassadarArticleEquivalenceClaimCheckerReport,
    pub matched_article_line_rows: Vec<TassadarArticleEquivalenceFinalAuditLineMatchRow>,
    pub matched_article_line_count: usize,
    pub all_article_lines_matched: bool,
    pub verdict_review: TassadarArticleEquivalenceFinalAuditVerdictReview,
    pub canonical_closure_review: TassadarArticleEquivalenceFinalAuditCanonicalClosureReview,
    pub machine_matrix_review: TassadarArticleEquivalenceFinalAuditMachineMatrixReview,
    pub exclusion_review: TassadarArticleEquivalenceExclusionReview,
    pub public_article_equivalence_claim_allowed: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleEquivalenceFinalAuditError {
    #[error(transparent)]
    ClaimChecker(#[from] TassadarArticleEquivalenceClaimCheckerError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_equivalence_final_audit_report(
) -> Result<TassadarArticleEquivalenceFinalAuditReport, TassadarArticleEquivalenceFinalAuditError> {
    let claim_checker_report = build_tassadar_article_equivalence_claim_checker_report()?;
    let matched_article_line_rows = build_matched_article_line_rows(&claim_checker_report);
    let matched_article_line_count = matched_article_line_rows.len();
    let all_article_lines_matched = matched_article_line_count > 0
        && matched_article_line_rows
            .iter()
            .all(|row| row.matched_green);
    let verdict_review = TassadarArticleEquivalenceFinalAuditVerdictReview {
        mechanistic_verdict_green: claim_checker_report.mechanistic_verdict_green,
        behavioral_verdict_green: claim_checker_report.behavioral_verdict_green,
        operational_verdict_green: claim_checker_report.operational_verdict_green,
        all_categories_green: claim_checker_report.mechanistic_verdict_green
            && claim_checker_report.behavioral_verdict_green
            && claim_checker_report.operational_verdict_green,
        public_article_equivalence_claim_allowed: claim_checker_report
            .public_article_equivalence_claim_allowed,
        article_equivalence_green: claim_checker_report.article_equivalence_green
            && all_article_lines_matched,
        detail: format!(
            "mechanistic_verdict_green={} behavioral_verdict_green={} operational_verdict_green={} all_categories_green={} public_article_equivalence_claim_allowed={} article_equivalence_green={}",
            claim_checker_report.mechanistic_verdict_green,
            claim_checker_report.behavioral_verdict_green,
            claim_checker_report.operational_verdict_green,
            claim_checker_report.mechanistic_verdict_green
                && claim_checker_report.behavioral_verdict_green
                && claim_checker_report.operational_verdict_green,
            claim_checker_report.public_article_equivalence_claim_allowed,
            claim_checker_report.article_equivalence_green && all_article_lines_matched,
        ),
    };
    let canonical_closure_review = TassadarArticleEquivalenceFinalAuditCanonicalClosureReview {
        canonical_model_id: claim_checker_report
            .canonical_identity_review
            .canonical_model_id
            .clone(),
        canonical_weight_artifact_id: claim_checker_report
            .canonical_identity_review
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: claim_checker_report
            .canonical_identity_review
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: claim_checker_report
            .canonical_identity_review
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: claim_checker_report
            .canonical_identity_review
            .canonical_route_id
            .clone(),
        canonical_route_descriptor_digest: claim_checker_report
            .canonical_identity_review
            .canonical_route_descriptor_digest
            .clone(),
        canonical_decode_mode: claim_checker_report
            .canonical_identity_review
            .canonical_decode_mode
            .clone(),
        detail: claim_checker_report
            .canonical_identity_review
            .detail
            .clone(),
    };
    let machine_matrix_review = TassadarArticleEquivalenceFinalAuditMachineMatrixReview {
        current_host_machine_class_id: claim_checker_report
            .canonical_identity_review
            .current_host_machine_class_id
            .clone(),
        supported_machine_class_ids: claim_checker_report
            .canonical_identity_review
            .supported_machine_class_ids
            .clone(),
        detail: format!(
            "current_host_machine_class_id=`{}` supported_machine_class_count={}",
            claim_checker_report
                .canonical_identity_review
                .current_host_machine_class_id,
            claim_checker_report
                .canonical_identity_review
                .supported_machine_class_ids
                .len(),
        ),
    };

    let public_article_equivalence_claim_allowed =
        verdict_review.public_article_equivalence_claim_allowed && all_article_lines_matched;
    let article_equivalence_green = verdict_review.article_equivalence_green;
    let exclusion_review = claim_checker_report.exclusion_review.clone();
    let mut report = TassadarArticleEquivalenceFinalAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_equivalence.final_audit.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_CHECKER_REF),
        claim_checker_report_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_CLAIM_CHECKER_REPORT_REF),
        claim_checker_report,
        matched_article_line_rows,
        matched_article_line_count,
        all_article_lines_matched,
        verdict_review,
        canonical_closure_review,
        machine_matrix_review,
        exclusion_review,
        public_article_equivalence_claim_allowed,
        article_equivalence_green,
        claim_boundary: String::from(
            "this final audit closes bounded article equivalence only for the declared article envelope: the owned `psionic-transformer` stack, the canonical trained article model and weight artifact, the direct deterministic HullCache claim route, and the declared CPU machine matrix. It does not widen the public claim to arbitrary C ingress, arbitrary Wasm ingress, planner-mediated or hybrid canonical routes, resumed or stochastic execution, generic interpreter-in-weights claims outside the declared envelope, or the post-article universality bridge.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Final article-equivalence audit now records matched_article_lines={}, all_article_lines_matched={}, mechanistic_verdict_green={}, behavioral_verdict_green={}, operational_verdict_green={}, supported_machine_classes={}, optional_open_issues={}, public_article_equivalence_claim_allowed={}, and article_equivalence_green={}.",
        report.matched_article_line_count,
        report.all_article_lines_matched,
        report.verdict_review.mechanistic_verdict_green,
        report.verdict_review.behavioral_verdict_green,
        report.verdict_review.operational_verdict_green,
        report.machine_matrix_review.supported_machine_class_ids.len(),
        report.exclusion_review.optional_open_issue_ids.len(),
        report.public_article_equivalence_claim_allowed,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_equivalence_final_audit_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_article_equivalence_final_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF)
}

pub fn write_tassadar_article_equivalence_final_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleEquivalenceFinalAuditReport, TassadarArticleEquivalenceFinalAuditError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEquivalenceFinalAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_equivalence_final_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEquivalenceFinalAuditError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_matched_article_line_rows(
    claim_checker_report: &TassadarArticleEquivalenceClaimCheckerReport,
) -> Vec<TassadarArticleEquivalenceFinalAuditLineMatchRow> {
    #[derive(Clone)]
    struct Accumulator {
        claim_ref_id: String,
        line_start: u16,
        line_end: u16,
        claim_paraphrase: String,
        matched_blocker_ids: Vec<String>,
        matched_blocker_categories: Vec<TassadarArticleEquivalenceBlockerCategory>,
        matched_issue_ids: Vec<String>,
        matched_green: bool,
    }

    let open_blocker_ids = &claim_checker_report.blocker_matrix_report.open_blocker_ids;
    let mut by_line = BTreeMap::<(String, u16, u16, String), Accumulator>::new();
    for blocker in &claim_checker_report.blocker_matrix_report.blockers {
        let blocker_green = !open_blocker_ids.contains(&blocker.blocker_id);
        for line_ref in &blocker.article_line_refs {
            let key = (
                line_ref.claim_ref_id.clone(),
                line_ref.line_start,
                line_ref.line_end,
                line_ref.claim_paraphrase.clone(),
            );
            let entry = by_line.entry(key).or_insert_with(|| Accumulator {
                claim_ref_id: line_ref.claim_ref_id.clone(),
                line_start: line_ref.line_start,
                line_end: line_ref.line_end,
                claim_paraphrase: line_ref.claim_paraphrase.clone(),
                matched_blocker_ids: Vec::new(),
                matched_blocker_categories: Vec::new(),
                matched_issue_ids: Vec::new(),
                matched_green: true,
            });
            entry.matched_blocker_ids.push(blocker.blocker_id.clone());
            entry.matched_blocker_categories.push(blocker.category);
            entry
                .matched_issue_ids
                .extend(blocker.covered_by_issue_ids.iter().cloned());
            entry.matched_green &= blocker_green;
        }
    }

    by_line
        .into_values()
        .map(|mut row| {
            row.matched_blocker_ids.sort();
            row.matched_blocker_ids.dedup();
            row.matched_blocker_categories.sort();
            row.matched_blocker_categories.dedup();
            row.matched_issue_ids.sort();
            row.matched_issue_ids.dedup();
            TassadarArticleEquivalenceFinalAuditLineMatchRow {
                claim_ref_id: row.claim_ref_id,
                line_start: row.line_start,
                line_end: row.line_end,
                claim_paraphrase: row.claim_paraphrase,
                matched_blocker_ids: row.matched_blocker_ids.clone(),
                matched_blocker_categories: row.matched_blocker_categories,
                matched_issue_ids: row.matched_issue_ids.clone(),
                matched_green: row.matched_green,
                detail: format!(
                    "matched_blockers={} matched_issues={} matched_green={}",
                    row.matched_blocker_ids.len(),
                    row.matched_issue_ids.len(),
                    row.matched_green,
                ),
            }
        })
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;

    use super::{
        build_tassadar_article_equivalence_final_audit_report,
        tassadar_article_equivalence_final_audit_report_path,
        write_tassadar_article_equivalence_final_audit_report,
        TassadarArticleEquivalenceFinalAuditReport,
        TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
    };

    #[test]
    fn article_equivalence_final_audit_is_green_and_bounded() {
        let report = build_tassadar_article_equivalence_final_audit_report().expect("report");

        assert!(report.all_article_lines_matched);
        assert!(report.matched_article_line_count > 0);
        assert!(report.verdict_review.mechanistic_verdict_green);
        assert!(report.verdict_review.behavioral_verdict_green);
        assert!(report.verdict_review.operational_verdict_green);
        assert_eq!(
            report.canonical_closure_review.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            report.canonical_closure_review.canonical_decode_mode,
            "hull_cache"
        );
        assert_eq!(
            report
                .machine_matrix_review
                .supported_machine_class_ids
                .len(),
            2
        );
        assert_eq!(
            report.exclusion_review.optional_open_issue_ids,
            vec![String::from("TAS-R1")]
        );
        assert!(report.public_article_equivalence_claim_allowed);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_equivalence_final_audit_matches_committed_truth() {
        let generated = build_tassadar_article_equivalence_final_audit_report().expect("report");
        let committed: TassadarArticleEquivalenceFinalAuditReport =
            read_json(tassadar_article_equivalence_final_audit_report_path()).expect("committed");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_equivalence_final_audit_report.json"
        );
    }

    #[test]
    fn write_article_equivalence_final_audit_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_equivalence_final_audit_report.json");
        let written =
            write_tassadar_article_equivalence_final_audit_report(&output_path).expect("write");
        let persisted: TassadarArticleEquivalenceFinalAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_equivalence_final_audit_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_equivalence_final_audit_report.json")
        );
    }

    fn read_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }
}
