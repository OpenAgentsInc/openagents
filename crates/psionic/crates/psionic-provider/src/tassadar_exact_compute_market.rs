use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_router::build_tassadar_composite_routing_report;
use psionic_serve::build_tassadar_execution_unit_registration_report;

use crate::{
    TassadarCompositeRoutingReceipt, TassadarExecutionUnitRegistrationReceipt,
    build_tassadar_accepted_outcome_binding_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_EXACT_COMPUTE_MARKET_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exact_compute_market_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExactComputeEvidenceClass {
    ExecutionBound,
    AcceptedOutcomeReady,
    ValidatorBoundAcceptedOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExactComputeQuotePosture {
    Indicative,
    ValidatorBound,
    ChallengeWindowRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExactComputePricingPosture {
    BenchmarkCalibratedIndicative,
    ValidatorPremium,
    ChallengeWindowPremium,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExactComputeSettlementPosture {
    ExecutionOnly,
    AcceptedOutcomeDependent,
    SettlementNotPublished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExactComputeEnvelopeRefusalReason {
    UnsupportedWorkloadFamily,
    UnsupportedEvidenceClass,
    ValidatorBindingUnavailable,
    SettlementPostureUnsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactComputeCapabilityEnvelope {
    pub envelope_id: String,
    pub execution_unit_receipt_id: String,
    pub accepted_outcome_binding_report_id: String,
    pub composite_routing_report_id: String,
    pub supported_workload_families: Vec<String>,
    pub validator_binding_required: bool,
    pub evidence_class: TassadarExactComputeEvidenceClass,
    pub accepted_outcome_dependency_refs: Vec<String>,
    pub settlement_posture: TassadarExactComputeSettlementPosture,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactComputeMarketProduct {
    pub product_id: String,
    pub product_family: String,
    pub capability_envelope: TassadarExactComputeCapabilityEnvelope,
    pub quote_posture: TassadarExactComputeQuotePosture,
    pub pricing_posture: TassadarExactComputePricingPosture,
    pub quoted_price_milliunits: u32,
    pub validator_premium_milliunits: u32,
    pub evidence_premium_milliunits: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactComputeQuote {
    pub quote_id: String,
    pub product_id: String,
    pub workload_family: String,
    pub execution_unit_receipt_id: String,
    pub accepted_outcome_binding_report_id: String,
    pub quote_posture: TassadarExactComputeQuotePosture,
    pub quoted_price_milliunits: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactComputeMarketReceipt {
    pub receipt_id: String,
    pub product_id: String,
    pub quote_id: String,
    pub execution_unit_receipt_id: String,
    pub candidate_outcome_id: String,
    pub accepted_outcome_dependency_ref: String,
    pub settlement_posture: TassadarExactComputeSettlementPosture,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUnsupportedExactComputeEnvelope {
    pub simulation_id: String,
    pub requested_product_id: String,
    pub requested_workload_family: String,
    pub requested_evidence_class: TassadarExactComputeEvidenceClass,
    pub requested_settlement_posture: TassadarExactComputeSettlementPosture,
    pub refusal_reason: TassadarExactComputeEnvelopeRefusalReason,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactComputeMarketReport {
    pub schema_version: u16,
    pub report_id: String,
    pub execution_unit_receipt: TassadarExecutionUnitRegistrationReceipt,
    pub composite_routing_receipt: TassadarCompositeRoutingReceipt,
    pub accepted_outcome_binding_report_id: String,
    pub products: Vec<TassadarExactComputeMarketProduct>,
    pub quotes: Vec<TassadarExactComputeQuote>,
    pub receipts: Vec<TassadarExactComputeMarketReceipt>,
    pub refused_envelopes: Vec<TassadarUnsupportedExactComputeEnvelope>,
    pub compute_market_dependency_marker: String,
    pub kernel_objects_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_exact_compute_market_report() -> TassadarExactComputeMarketReport {
    let execution_unit_report =
        build_tassadar_execution_unit_registration_report().expect("execution unit report");
    let accepted_outcome_binding_report = build_tassadar_accepted_outcome_binding_report();
    let composite_routing_report = build_tassadar_composite_routing_report();

    let execution_unit_receipt =
        TassadarExecutionUnitRegistrationReceipt::from_report(&execution_unit_report);
    let composite_routing_receipt =
        TassadarCompositeRoutingReceipt::from_report(&composite_routing_report);
    let accepted_outcome_binding_report_id = accepted_outcome_binding_report.report_id.clone();

    let products = seeded_products(&execution_unit_receipt, &accepted_outcome_binding_report_id);
    let quotes = seeded_quotes(
        &products,
        &execution_unit_receipt,
        &accepted_outcome_binding_report_id,
    );
    let receipts = seeded_market_receipts(&quotes, &execution_unit_receipt);
    let refused_envelopes = seeded_refused_envelopes();

    let mut report = TassadarExactComputeMarketReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.exact_compute_market.report.v1"),
        execution_unit_receipt,
        composite_routing_receipt,
        accepted_outcome_binding_report_id,
        products,
        quotes,
        receipts,
        refused_envelopes,
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical quote publication, buyer matching, and market-wide pricing policy outside standalone psionic",
        ),
        kernel_objects_dependency_marker: String::from(
            "kernel-objects remain the owner of canonical product and receipt object identities outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of accepted-outcome and settlement-qualified product closure outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this provider report defines a machine-legible exact-compute product family over execution-unit identity, accepted-outcome dependencies, validator posture, and quote economics. It does not treat quotes, receipts, or runtime success as settlement-qualified authority",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Exact-compute market report now freezes {} products, {} quotes, {} receipts, and {} refused product-envelope simulations.",
        report.products.len(),
        report.quotes.len(),
        report.receipts.len(),
        report.refused_envelopes.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_exact_compute_market_report|", &report);
    report
}

#[must_use]
pub fn tassadar_exact_compute_market_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXACT_COMPUTE_MARKET_REPORT_REF)
}

pub fn write_tassadar_exact_compute_market_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExactComputeMarketReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_exact_compute_market_report();
    let json =
        serde_json::to_string_pretty(&report).expect("exact-compute market report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_exact_compute_market_report(
    path: impl AsRef<Path>,
) -> Result<TassadarExactComputeMarketReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn seeded_products(
    execution_unit_receipt: &TassadarExecutionUnitRegistrationReceipt,
    accepted_outcome_binding_report_id: &str,
) -> Vec<TassadarExactComputeMarketProduct> {
    vec![
        TassadarExactComputeMarketProduct {
            product_id: String::from("product.exact_compute.patch_apply.validator_bound"),
            product_family: String::from("exact_compute_patch"),
            capability_envelope: TassadarExactComputeCapabilityEnvelope {
                envelope_id: String::from("envelope.patch_apply.validator_bound"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                accepted_outcome_binding_report_id: String::from(
                    accepted_outcome_binding_report_id,
                ),
                composite_routing_report_id: String::from("tassadar.composite_routing.report.v1"),
                supported_workload_families: vec![String::from("patch_apply_internal_exact")],
                validator_binding_required: true,
                evidence_class: TassadarExactComputeEvidenceClass::ValidatorBoundAcceptedOutcome,
                accepted_outcome_dependency_refs: vec![
                    String::from("kernel-policy.accepted-outcome.patch_apply.v1"),
                    String::from("nexus.accepted-outcome.patch_apply.v1"),
                ],
                settlement_posture: TassadarExactComputeSettlementPosture::AcceptedOutcomeDependent,
                note: String::from(
                    "premium exact-compute patch product with validator-bound accepted-outcome dependencies",
                ),
            },
            quote_posture: TassadarExactComputeQuotePosture::ValidatorBound,
            pricing_posture: TassadarExactComputePricingPosture::ValidatorPremium,
            quoted_price_milliunits: 4200,
            validator_premium_milliunits: 900,
            evidence_premium_milliunits: 500,
            note: String::from(
                "pricing stays tied to validator-bound evidence and accepted-outcome dependencies rather than raw hardware",
            ),
        },
        TassadarExactComputeMarketProduct {
            product_id: String::from("product.exact_compute.long_loop.challenge_window"),
            product_family: String::from("exact_compute_long_loop"),
            capability_envelope: TassadarExactComputeCapabilityEnvelope {
                envelope_id: String::from("envelope.long_loop.challenge_window"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                accepted_outcome_binding_report_id: String::from(
                    accepted_outcome_binding_report_id,
                ),
                composite_routing_report_id: String::from("tassadar.composite_routing.report.v1"),
                supported_workload_families: vec![String::from("long_loop_validator_heavy")],
                validator_binding_required: true,
                evidence_class: TassadarExactComputeEvidenceClass::AcceptedOutcomeReady,
                accepted_outcome_dependency_refs: vec![String::from(
                    "kernel-policy.accepted-outcome.long_loop.v1",
                )],
                settlement_posture: TassadarExactComputeSettlementPosture::AcceptedOutcomeDependent,
                note: String::from(
                    "hybrid long-loop product requires a challenge window before any authority-facing closure",
                ),
            },
            quote_posture: TassadarExactComputeQuotePosture::ChallengeWindowRequired,
            pricing_posture: TassadarExactComputePricingPosture::ChallengeWindowPremium,
            quoted_price_milliunits: 6900,
            validator_premium_milliunits: 1100,
            evidence_premium_milliunits: 700,
            note: String::from(
                "pricing reflects hybrid fallback cost and challenge-window burden instead of pretending the lane is unconditional",
            ),
        },
        TassadarExactComputeMarketProduct {
            product_id: String::from("product.exact_compute.parity_short"),
            product_family: String::from("exact_compute_bounded_parity"),
            capability_envelope: TassadarExactComputeCapabilityEnvelope {
                envelope_id: String::from("envelope.parity_short.indicative"),
                execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
                accepted_outcome_binding_report_id: String::from(
                    accepted_outcome_binding_report_id,
                ),
                composite_routing_report_id: String::from("tassadar.composite_routing.report.v1"),
                supported_workload_families: vec![String::from("parity_short_bounded")],
                validator_binding_required: false,
                evidence_class: TassadarExactComputeEvidenceClass::ExecutionBound,
                accepted_outcome_dependency_refs: vec![String::from(
                    "kernel-policy.accepted-outcome.parity.short.v1",
                )],
                settlement_posture: TassadarExactComputeSettlementPosture::ExecutionOnly,
                note: String::from(
                    "bounded short-parity product stays execution-bound and does not market validator-backed settlement posture",
                ),
            },
            quote_posture: TassadarExactComputeQuotePosture::Indicative,
            pricing_posture: TassadarExactComputePricingPosture::BenchmarkCalibratedIndicative,
            quoted_price_milliunits: 1200,
            validator_premium_milliunits: 0,
            evidence_premium_milliunits: 150,
            note: String::from(
                "bounded parity stays a cheap indicative exact-compute product without overclaiming broader outcome closure",
            ),
        },
    ]
}

fn seeded_quotes(
    products: &[TassadarExactComputeMarketProduct],
    execution_unit_receipt: &TassadarExecutionUnitRegistrationReceipt,
    accepted_outcome_binding_report_id: &str,
) -> Vec<TassadarExactComputeQuote> {
    products
        .iter()
        .map(|product| TassadarExactComputeQuote {
            quote_id: format!("quote.{}", product.product_id.replace("product.", "")),
            product_id: product.product_id.clone(),
            workload_family: product.capability_envelope.supported_workload_families[0].clone(),
            execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
            accepted_outcome_binding_report_id: String::from(accepted_outcome_binding_report_id),
            quote_posture: product.quote_posture,
            quoted_price_milliunits: product.quoted_price_milliunits,
            note: format!(
                "quote references execution-unit `{}` and accepted-outcome binding `{}` instead of raw token or hardware abstractions",
                execution_unit_receipt.report_id, accepted_outcome_binding_report_id
            ),
        })
        .collect()
}

fn seeded_market_receipts(
    quotes: &[TassadarExactComputeQuote],
    execution_unit_receipt: &TassadarExecutionUnitRegistrationReceipt,
) -> Vec<TassadarExactComputeMarketReceipt> {
    vec![
        TassadarExactComputeMarketReceipt {
            receipt_id: String::from("receipt.exact_compute.patch_apply.validator_bound"),
            product_id: quotes[0].product_id.clone(),
            quote_id: quotes[0].quote_id.clone(),
            execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
            candidate_outcome_id: String::from("candidate.patch_apply.validator_bound.v1"),
            accepted_outcome_dependency_ref: String::from("nexus.accepted-outcome.patch_apply.v1"),
            settlement_posture: TassadarExactComputeSettlementPosture::AcceptedOutcomeDependent,
            note: String::from(
                "receipt identity stays distinct from both quote identity and accepted-outcome authority",
            ),
        },
        TassadarExactComputeMarketReceipt {
            receipt_id: String::from("receipt.exact_compute.long_loop.challenge_window"),
            product_id: quotes[1].product_id.clone(),
            quote_id: quotes[1].quote_id.clone(),
            execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
            candidate_outcome_id: String::from("candidate.long_loop.validator_missing.v1"),
            accepted_outcome_dependency_ref: String::from(
                "kernel-policy.accepted-outcome.long_loop.v1",
            ),
            settlement_posture: TassadarExactComputeSettlementPosture::AcceptedOutcomeDependent,
            note: String::from(
                "receipt stays publishable even though outcome authority still depends on challenge-window and validator closure",
            ),
        },
        TassadarExactComputeMarketReceipt {
            receipt_id: String::from("receipt.exact_compute.parity_short"),
            product_id: quotes[2].product_id.clone(),
            quote_id: quotes[2].quote_id.clone(),
            execution_unit_receipt_id: execution_unit_receipt.report_id.clone(),
            candidate_outcome_id: String::from("candidate.parity.short.v1"),
            accepted_outcome_dependency_ref: String::from(
                "kernel-policy.accepted-outcome.parity.short.v1",
            ),
            settlement_posture: TassadarExactComputeSettlementPosture::ExecutionOnly,
            note: String::from(
                "bounded parity receipt stays execution-facing and does not advertise broader settlement eligibility",
            ),
        },
    ]
}

fn seeded_refused_envelopes() -> Vec<TassadarUnsupportedExactComputeEnvelope> {
    vec![
        TassadarUnsupportedExactComputeEnvelope {
            simulation_id: String::from("refusal.research_search_product"),
            requested_product_id: String::from("product.exact_compute.search.research_only"),
            requested_workload_family: String::from("served_search_validator_mount"),
            requested_evidence_class:
                TassadarExactComputeEvidenceClass::ValidatorBoundAcceptedOutcome,
            requested_settlement_posture:
                TassadarExactComputeSettlementPosture::AcceptedOutcomeDependent,
            refusal_reason: TassadarExactComputeEnvelopeRefusalReason::UnsupportedWorkloadFamily,
            note: String::from(
                "research-only search remains outside the current marketable exact-compute product family",
            ),
        },
        TassadarUnsupportedExactComputeEnvelope {
            simulation_id: String::from("refusal.settlement_eligible_exact_compute"),
            requested_product_id: String::from(
                "product.exact_compute.patch_apply.settlement_ready",
            ),
            requested_workload_family: String::from("patch_apply_internal_exact"),
            requested_evidence_class:
                TassadarExactComputeEvidenceClass::ValidatorBoundAcceptedOutcome,
            requested_settlement_posture:
                TassadarExactComputeSettlementPosture::SettlementNotPublished,
            refusal_reason: TassadarExactComputeEnvelopeRefusalReason::SettlementPostureUnsupported,
            note: String::from(
                "the current execution-unit registration is not settlement-eligible, so the product family must refuse a settlement-published envelope",
            ),
        },
    ]
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        TassadarExactComputeEnvelopeRefusalReason, TassadarExactComputeSettlementPosture,
        build_tassadar_exact_compute_market_report, load_tassadar_exact_compute_market_report,
        tassadar_exact_compute_market_report_path,
    };

    #[test]
    fn exact_compute_market_report_keeps_product_quote_and_receipt_identities_distinct() {
        let report = build_tassadar_exact_compute_market_report();

        assert_eq!(report.products.len(), 3);
        assert_eq!(report.quotes.len(), 3);
        assert_eq!(report.receipts.len(), 3);
        assert_eq!(report.refused_envelopes.len(), 2);
        assert!(report.products.iter().all(|product| {
            report
                .quotes
                .iter()
                .any(|quote| quote.product_id == product.product_id)
        }));
        assert!(report.receipts.iter().all(|receipt| {
            report
                .quotes
                .iter()
                .any(|quote| quote.quote_id == receipt.quote_id)
                && receipt.receipt_id != receipt.quote_id
        }));
        assert!(report.refused_envelopes.iter().any(|refusal| {
            refusal.refusal_reason
                == TassadarExactComputeEnvelopeRefusalReason::SettlementPostureUnsupported
        }));
        assert!(report.products.iter().any(|product| {
            product.capability_envelope.settlement_posture
                == TassadarExactComputeSettlementPosture::ExecutionOnly
        }));
    }

    #[test]
    fn exact_compute_market_report_matches_committed_truth() {
        let expected = build_tassadar_exact_compute_market_report();
        let committed =
            load_tassadar_exact_compute_market_report(tassadar_exact_compute_market_report_path())
                .expect("committed exact-compute market report");

        assert_eq!(committed, expected);
    }
}
