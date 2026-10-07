use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarBroadGeneralComputeValidatorBridgeReport,
    TassadarBroadGeneralComputeValidatorBridgeStatus,
    TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF,
};

use crate::{
    TassadarCompositeAcceptedOutcomeTemplateReport, TassadarExactComputeMarketReport,
    TassadarExactComputePricingPosture, TassadarExactComputeQuotePosture,
    TassadarExactComputeSettlementPosture, TASSADAR_COMPOSITE_ACCEPTED_OUTCOME_TEMPLATE_REPORT_REF,
    TASSADAR_EXACT_COMPUTE_MARKET_REPORT_REF,
};

pub const TASSADAR_BROAD_GENERAL_COMPUTE_ECONOMIC_BRIDGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_general_compute_economic_bridge_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadGeneralComputeEconomicReceiptStatus {
    ReceiptReady,
    CandidateOnlyChallengeWindow,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadGeneralComputeBridgeRefusalReason {
    PendingProfileSpecificPolicy,
    PendingPortabilityEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeAcceptedOutcomeTemplate {
    pub template_id: String,
    pub profile_id: String,
    pub world_mount_template_ref: String,
    pub kernel_policy_template_ref: String,
    pub nexus_template_ref: String,
    pub validator_policy_ref: String,
    pub challenge_window_minutes: u32,
    pub settlement_dependency_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeEconomicReceipt {
    pub receipt_id: String,
    pub profile_id: String,
    pub template_id: String,
    pub product_ref: String,
    pub quote_posture: TassadarExactComputeQuotePosture,
    pub pricing_posture: TassadarExactComputePricingPosture,
    pub settlement_posture: TassadarExactComputeSettlementPosture,
    pub status: TassadarBroadGeneralComputeEconomicReceiptStatus,
    pub candidate_outcome_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_outcome_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeRefusedEnvelope {
    pub profile_id: String,
    pub refusal_reason: TassadarBroadGeneralComputeBridgeRefusalReason,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeEconomicBridgeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub validator_bridge_report_ref: String,
    pub validator_route_policy_report_ref: String,
    pub exact_compute_market_report_id: String,
    pub composite_accepted_outcome_report_id: String,
    pub templates: Vec<TassadarBroadGeneralComputeAcceptedOutcomeTemplate>,
    pub economic_receipts: Vec<TassadarBroadGeneralComputeEconomicReceipt>,
    pub refused_envelopes: Vec<TassadarBroadGeneralComputeRefusedEnvelope>,
    pub accepted_outcome_ready_receipt_count: u32,
    pub candidate_only_receipt_count: u32,
    pub refused_profile_count: u32,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub compute_market_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeEconomicBridgeReceipt {
    pub report_id: String,
    pub template_count: u32,
    pub economic_receipt_count: u32,
    pub accepted_outcome_ready_receipt_count: u32,
    pub candidate_only_receipt_count: u32,
    pub refused_profile_count: u32,
    pub detail: String,
}

impl TassadarBroadGeneralComputeEconomicBridgeReceipt {
    #[must_use]
    pub fn from_report(report: &TassadarBroadGeneralComputeEconomicBridgeReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            template_count: report.templates.len() as u32,
            economic_receipt_count: report.economic_receipts.len() as u32,
            accepted_outcome_ready_receipt_count: report.accepted_outcome_ready_receipt_count,
            candidate_only_receipt_count: report.candidate_only_receipt_count,
            refused_profile_count: report.refused_profile_count,
            detail: format!(
                "broad general-compute economic bridge `{}` carries templates={}, receipts={}, ready_receipts={}, candidate_only_receipts={}, refused_profiles={}",
                report.report_id,
                report.templates.len(),
                report.economic_receipts.len(),
                report.accepted_outcome_ready_receipt_count,
                report.candidate_only_receipt_count,
                report.refused_profile_count,
            ),
        }
    }
}

#[derive(Debug, Error)]
pub enum TassadarBroadGeneralComputeEconomicBridgeReportError {
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
}

pub fn build_tassadar_broad_general_compute_economic_bridge_report() -> Result<
    TassadarBroadGeneralComputeEconomicBridgeReport,
    TassadarBroadGeneralComputeEconomicBridgeReportError,
> {
    let validator_bridge_report: TassadarBroadGeneralComputeValidatorBridgeReport = read_json(
        repo_root().join(TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF),
    )?;
    let exact_compute_market_report: TassadarExactComputeMarketReport =
        read_json(repo_root().join(TASSADAR_EXACT_COMPUTE_MARKET_REPORT_REF))?;
    let composite_template_report: TassadarCompositeAcceptedOutcomeTemplateReport = read_json(
        repo_root().join(TASSADAR_COMPOSITE_ACCEPTED_OUTCOME_TEMPLATE_REPORT_REF),
    )?;

    let templates = validator_bridge_report
        .rows
        .iter()
        .filter(|row| row.economic_receipt_allowed)
        .map(template_for_row)
        .collect::<Vec<_>>();
    let economic_receipts = validator_bridge_report
        .rows
        .iter()
        .filter(|row| row.economic_receipt_allowed)
        .map(receipt_for_row)
        .collect::<Vec<_>>();
    let refused_envelopes = validator_bridge_report
        .rows
        .iter()
        .filter_map(|row| match row.bridge_status {
            TassadarBroadGeneralComputeValidatorBridgeStatus::SuppressedPendingPolicy => {
                Some(TassadarBroadGeneralComputeRefusedEnvelope {
                    profile_id: row.profile_id.clone(),
                    refusal_reason:
                        TassadarBroadGeneralComputeBridgeRefusalReason::PendingProfileSpecificPolicy,
                    note: String::from(
                        "profile stays refused for authority-facing and economic publication until explicit profile-specific mount and accepted-outcome templates exist",
                    ),
                })
            }
            TassadarBroadGeneralComputeValidatorBridgeStatus::RefusedPendingEvidence => {
                Some(TassadarBroadGeneralComputeRefusedEnvelope {
                    profile_id: row.profile_id.clone(),
                    refusal_reason:
                        TassadarBroadGeneralComputeBridgeRefusalReason::PendingPortabilityEvidence,
                    note: String::from(
                        "profile stays refused for authority-facing and economic publication until portability and validator evidence are green",
                    ),
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let accepted_outcome_ready_receipt_count = economic_receipts
        .iter()
        .filter(|receipt| {
            receipt.status == TassadarBroadGeneralComputeEconomicReceiptStatus::ReceiptReady
        })
        .count() as u32;
    let candidate_only_receipt_count = economic_receipts
        .iter()
        .filter(|receipt| {
            receipt.status
                == TassadarBroadGeneralComputeEconomicReceiptStatus::CandidateOnlyChallengeWindow
        })
        .count() as u32;
    let refused_profile_count = refused_envelopes.len() as u32;

    let mut report = TassadarBroadGeneralComputeEconomicBridgeReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_general_compute_economic_bridge.report.v1"),
        validator_bridge_report_ref: String::from(
            TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF,
        ),
        validator_route_policy_report_ref: String::from(
            psionic_router::TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF,
        ),
        exact_compute_market_report_id: exact_compute_market_report.report_id,
        composite_accepted_outcome_report_id: composite_template_report.report_id,
        templates,
        economic_receipts,
        refused_envelopes,
        accepted_outcome_ready_receipt_count,
        candidate_only_receipt_count,
        refused_profile_count,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical broad profile mount objects outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of canonical broad profile accepted-outcome template approval outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of canonical candidate-outcome, challenge-window, and accepted-outcome authority outside standalone psionic",
        ),
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical broad general-compute products, quotes, and market-wide economic posture outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this provider report turns the validator bridge into explicit accepted-outcome templates and economic receipts for named broad internal-compute profiles. It keeps ready, candidate-only, and refused profiles explicit and does not treat candidate receipts or runtime success as accepted-outcome or settlement authority.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad general-compute economic bridge now records templates={}, receipts={}, ready_receipts={}, candidate_only_receipts={}, refused_profiles={}.",
        report.templates.len(),
        report.economic_receipts.len(),
        report.accepted_outcome_ready_receipt_count,
        report.candidate_only_receipt_count,
        report.refused_profile_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_general_compute_economic_bridge_report|",
        &report,
    );
    Ok(report)
}

fn template_for_row(
    row: &psionic_eval::TassadarBroadGeneralComputeValidatorBridgeRow,
) -> TassadarBroadGeneralComputeAcceptedOutcomeTemplate {
    TassadarBroadGeneralComputeAcceptedOutcomeTemplate {
        template_id: format!("accepted_outcome.{}.v1", row.profile_id),
        profile_id: row.profile_id.clone(),
        world_mount_template_ref: format!("world-mounts.template.{}", row.profile_id),
        kernel_policy_template_ref: format!("kernel-policy.template.{}", row.profile_id),
        nexus_template_ref: format!("nexus.template.{}", row.profile_id),
        validator_policy_ref: row
            .validator_policy_ref
            .clone()
            .unwrap_or_else(|| format!("validator-policies.{}", row.profile_id)),
        challenge_window_minutes: row.challenge_window_minutes,
        settlement_dependency_refs: row.dependency_refs.clone(),
        note: format!(
            "accepted-outcome template for `{}` stays bounded to the named profile bridge posture",
            row.profile_id
        ),
    }
}

fn receipt_for_row(
    row: &psionic_eval::TassadarBroadGeneralComputeValidatorBridgeRow,
) -> TassadarBroadGeneralComputeEconomicReceipt {
    let (status, quote_posture, pricing_posture, candidate_outcome_id, accepted_outcome_id, note) =
        match row.bridge_status {
            TassadarBroadGeneralComputeValidatorBridgeStatus::AcceptedOutcomeReady => (
                TassadarBroadGeneralComputeEconomicReceiptStatus::ReceiptReady,
                TassadarExactComputeQuotePosture::ValidatorBound,
                TassadarExactComputePricingPosture::ValidatorPremium,
                format!("candidate.{}.ready.v1", row.profile_id),
                Some(format!("accepted_outcome.{}.ready.v1", row.profile_id)),
                String::from(
                    "profile is accepted-outcome-ready, so the economic receipt can bind the named profile directly under the current authority envelope",
                ),
            ),
            TassadarBroadGeneralComputeValidatorBridgeStatus::CandidateOnlyChallengeWindow => (
                TassadarBroadGeneralComputeEconomicReceiptStatus::CandidateOnlyChallengeWindow,
                TassadarExactComputeQuotePosture::ChallengeWindowRequired,
                TassadarExactComputePricingPosture::ChallengeWindowPremium,
                format!("candidate.{}.challenge_window.v1", row.profile_id),
                None,
                String::from(
                    "profile is economic-participating but candidate-only; challenge-window closure remains explicit before accepted-outcome issuance",
                ),
            ),
            _ => unreachable!("only economic-receipt-allowed rows are converted into receipts"),
        };

    TassadarBroadGeneralComputeEconomicReceipt {
        receipt_id: format!("receipt.{}.economic.v1", row.profile_id),
        profile_id: row.profile_id.clone(),
        template_id: format!("accepted_outcome.{}.v1", row.profile_id),
        product_ref: format!("compute-market.product.{}", row.profile_id),
        quote_posture,
        pricing_posture,
        settlement_posture: TassadarExactComputeSettlementPosture::AcceptedOutcomeDependent,
        status,
        candidate_outcome_id,
        accepted_outcome_id,
        note,
    }
}

#[must_use]
pub fn tassadar_broad_general_compute_economic_bridge_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_GENERAL_COMPUTE_ECONOMIC_BRIDGE_REPORT_REF)
}

pub fn write_tassadar_broad_general_compute_economic_bridge_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadGeneralComputeEconomicBridgeReport,
    TassadarBroadGeneralComputeEconomicBridgeReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadGeneralComputeEconomicBridgeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_general_compute_economic_bridge_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("broad general-compute economic bridge serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarBroadGeneralComputeEconomicBridgeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_broad_general_compute_economic_bridge_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarBroadGeneralComputeEconomicBridgeReport,
    TassadarBroadGeneralComputeEconomicBridgeReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadGeneralComputeEconomicBridgeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadGeneralComputeEconomicBridgeReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root")
}

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarBroadGeneralComputeEconomicBridgeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadGeneralComputeEconomicBridgeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadGeneralComputeEconomicBridgeReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
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
        build_tassadar_broad_general_compute_economic_bridge_report,
        read_json, tassadar_broad_general_compute_economic_bridge_report_path,
        TassadarBroadGeneralComputeEconomicBridgeReceipt,
        TassadarBroadGeneralComputeEconomicBridgeReport,
        TassadarBroadGeneralComputeEconomicReceiptStatus,
    };

    #[test]
    fn broad_general_compute_economic_bridge_keeps_ready_candidate_and_refused_profiles_distinct() {
        let report = build_tassadar_broad_general_compute_economic_bridge_report().expect("report");

        assert_eq!(report.accepted_outcome_ready_receipt_count, 1);
        assert_eq!(report.candidate_only_receipt_count, 2);
        assert!(report.economic_receipts.iter().any(|receipt| {
            receipt.profile_id == "tassadar.internal_compute.article_closeout.v1"
                && receipt.status == TassadarBroadGeneralComputeEconomicReceiptStatus::ReceiptReady
        }));
        assert!(report.economic_receipts.iter().any(|receipt| {
            receipt.profile_id == "tassadar.internal_compute.deterministic_import_subset.v1"
                && receipt.status
                    == TassadarBroadGeneralComputeEconomicReceiptStatus::CandidateOnlyChallengeWindow
        }));
        assert!(report.refused_envelopes.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.portable_broad_family.v1"
        }));

        let provider_receipt = TassadarBroadGeneralComputeEconomicBridgeReceipt::from_report(&report);
        assert_eq!(provider_receipt.economic_receipt_count, 3);
    }

    #[test]
    fn broad_general_compute_economic_bridge_matches_committed_truth() {
        let generated = build_tassadar_broad_general_compute_economic_bridge_report().expect("report");
        let committed: TassadarBroadGeneralComputeEconomicBridgeReport = read_json(
            tassadar_broad_general_compute_economic_bridge_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }
}
