use std::{fs, path::Path};

use psionic_models::{
    TASSADAR_RECEIPT_SUPERVISION_BUNDLE_REF, TASSADAR_RECEIPT_SUPERVISION_REPORT_REF,
    TassadarAcceptedOutcomeLabel, TassadarReceiptSupervisionCase,
    TassadarReceiptSupervisionEvidenceBundle, TassadarReceiptSupervisionReport,
    seeded_tassadar_receipt_supervision_cases, tassadar_receipt_supervision_publication,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_RECEIPT_SUPERVISION_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_receipt_supervision_v1";
pub const TASSADAR_RECEIPT_SUPERVISION_FILE: &str = "receipt_supervision_evidence_bundle.json";

#[derive(Debug, Error)]
pub enum TassadarReceiptSupervisionError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn execute_tassadar_receipt_supervision_bundle(
    output_dir: &Path,
) -> Result<TassadarReceiptSupervisionEvidenceBundle, TassadarReceiptSupervisionError> {
    fs::create_dir_all(output_dir).map_err(|error| TassadarReceiptSupervisionError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;
    let cases = seeded_tassadar_receipt_supervision_cases();
    let mut bundle = TassadarReceiptSupervisionEvidenceBundle {
        publication: tassadar_receipt_supervision_publication(),
        heuristic_route_quality_bps: route_quality_bps(&cases, false),
        receipt_supervised_route_quality_bps: route_quality_bps(&cases, true),
        heuristic_refusal_quality_bps: refusal_quality_bps(&cases, false),
        receipt_supervised_refusal_quality_bps: refusal_quality_bps(&cases, true),
        heuristic_accepted_outcome_quality_bps: accepted_outcome_quality_bps(&cases, false),
        receipt_supervised_accepted_outcome_quality_bps: accepted_outcome_quality_bps(&cases, true),
        cases,
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Receipt-supervision bundle now freezes {} seeded cases from `{}` with route_quality {}->{} bps, refusal_quality {}->{}, and accepted_outcome_quality {}->{} bps.",
        bundle.cases.len(),
        TASSADAR_RECEIPT_SUPERVISION_BUNDLE_REF,
        bundle.heuristic_route_quality_bps,
        bundle.receipt_supervised_route_quality_bps,
        bundle.heuristic_refusal_quality_bps,
        bundle.receipt_supervised_refusal_quality_bps,
        bundle.heuristic_accepted_outcome_quality_bps,
        bundle.receipt_supervised_accepted_outcome_quality_bps,
    );
    bundle.bundle_digest = stable_digest(b"psionic_tassadar_receipt_supervision_bundle|", &bundle);

    let output_path = output_dir.join(TASSADAR_RECEIPT_SUPERVISION_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarReceiptSupervisionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn build_tassadar_receipt_supervision_report(
    bundle: &TassadarReceiptSupervisionEvidenceBundle,
) -> TassadarReceiptSupervisionReport {
    let accepted_count = bundle
        .cases
        .iter()
        .filter(|case| case.accepted_outcome_label == TassadarAcceptedOutcomeLabel::Accepted)
        .count() as u32;
    let rejected_for_evidence_count = bundle
        .cases
        .iter()
        .filter(|case| {
            case.accepted_outcome_label == TassadarAcceptedOutcomeLabel::RejectedForEvidence
        })
        .count() as u32;
    let rejected_for_refusal_quality_count = bundle
        .cases
        .iter()
        .filter(|case| {
            case.accepted_outcome_label == TassadarAcceptedOutcomeLabel::RejectedForRefusalQuality
        })
        .count() as u32;
    let accepted_after_delegation_count = bundle
        .cases
        .iter()
        .filter(|case| {
            case.accepted_outcome_label == TassadarAcceptedOutcomeLabel::AcceptedAfterDelegation
        })
        .count() as u32;
    let mut report = TassadarReceiptSupervisionReport {
        report_id: String::from("tassadar.receipt_supervision.report.v1"),
        bundle_ref: String::from(TASSADAR_RECEIPT_SUPERVISION_BUNDLE_REF),
        heuristic_route_quality_bps: bundle.heuristic_route_quality_bps,
        receipt_supervised_route_quality_bps: bundle.receipt_supervised_route_quality_bps,
        heuristic_refusal_quality_bps: bundle.heuristic_refusal_quality_bps,
        receipt_supervised_refusal_quality_bps: bundle.receipt_supervised_refusal_quality_bps,
        heuristic_accepted_outcome_quality_bps: bundle.heuristic_accepted_outcome_quality_bps,
        receipt_supervised_accepted_outcome_quality_bps: bundle
            .receipt_supervised_accepted_outcome_quality_bps,
        accepted_count,
        rejected_for_evidence_count,
        rejected_for_refusal_quality_count,
        accepted_after_delegation_count,
        claim_boundary: String::from(
            "this report is a benchmark-bound planner-learning surface over explicit receipt bundles, validator outcomes, and accepted-outcome labels. It does not collapse planner quality into authority closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Receipt-supervision report keeps route_quality {}->{} bps, refusal_quality {}->{}, and accepted_outcome_quality {}->{} bps explicit across {} accepted / {} evidence rejections / {} refusal-quality rejections / {} delegated accepts.",
        report.heuristic_route_quality_bps,
        report.receipt_supervised_route_quality_bps,
        report.heuristic_refusal_quality_bps,
        report.receipt_supervised_refusal_quality_bps,
        report.heuristic_accepted_outcome_quality_bps,
        report.receipt_supervised_accepted_outcome_quality_bps,
        report.accepted_count,
        report.rejected_for_evidence_count,
        report.rejected_for_refusal_quality_count,
        report.accepted_after_delegation_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_receipt_supervision_report|", &report);
    report
}

pub fn write_tassadar_receipt_supervision_report(
    report: &TassadarReceiptSupervisionReport,
) -> Result<(), TassadarReceiptSupervisionError> {
    let path = Path::new(TASSADAR_RECEIPT_SUPERVISION_REPORT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarReceiptSupervisionError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let json = serde_json::to_string_pretty(report)?;
    fs::write(path, format!("{json}\n")).map_err(|error| TassadarReceiptSupervisionError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn route_quality_bps(cases: &[TassadarReceiptSupervisionCase], receipt_supervised: bool) -> u32 {
    let good = cases
        .iter()
        .filter(|case| {
            let route = if receipt_supervised {
                case.receipt_supervised_route_family
            } else {
                case.heuristic_route_family
            };
            route == case.receipt_supervised_route_family
        })
        .count() as u32;
    ratio_bps(good, cases.len() as u32)
}

fn refusal_quality_bps(cases: &[TassadarReceiptSupervisionCase], receipt_supervised: bool) -> u32 {
    let good = cases
        .iter()
        .filter(|case| match case.accepted_outcome_label {
            TassadarAcceptedOutcomeLabel::RejectedForRefusalQuality => {
                receipt_supervised
                    && case.receipt_supervised_route_family != case.heuristic_route_family
            }
            _ => true,
        })
        .count() as u32;
    ratio_bps(good, cases.len() as u32)
}

fn accepted_outcome_quality_bps(
    cases: &[TassadarReceiptSupervisionCase],
    receipt_supervised: bool,
) -> u32 {
    let good = cases
        .iter()
        .filter(|case| match case.accepted_outcome_label {
            TassadarAcceptedOutcomeLabel::Accepted => true,
            TassadarAcceptedOutcomeLabel::AcceptedAfterDelegation
            | TassadarAcceptedOutcomeLabel::RejectedForEvidence
            | TassadarAcceptedOutcomeLabel::RejectedForRefusalQuality => {
                receipt_supervised
                    && case.receipt_supervised_route_family != case.heuristic_route_family
            }
        })
        .count() as u32;
    ratio_bps(good, cases.len() as u32)
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        numerator.saturating_mul(10_000) / denominator
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_RECEIPT_SUPERVISION_FILE, build_tassadar_receipt_supervision_report,
        execute_tassadar_receipt_supervision_bundle,
    };

    #[test]
    fn receipt_supervision_bundle_and_report_keep_lifts_explicit() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let bundle = execute_tassadar_receipt_supervision_bundle(temp_dir.path()).expect("bundle");
        let report = build_tassadar_receipt_supervision_report(&bundle);

        assert_eq!(bundle.cases.len(), 5);
        assert!(bundle.receipt_supervised_route_quality_bps > bundle.heuristic_route_quality_bps);
        assert!(
            bundle.receipt_supervised_refusal_quality_bps > bundle.heuristic_refusal_quality_bps
        );
        assert!(
            bundle.receipt_supervised_accepted_outcome_quality_bps
                > bundle.heuristic_accepted_outcome_quality_bps
        );
        assert_eq!(report.accepted_after_delegation_count, 1);
        assert!(
            temp_dir
                .path()
                .join(TASSADAR_RECEIPT_SUPERVISION_FILE)
                .exists()
        );
    }
}
