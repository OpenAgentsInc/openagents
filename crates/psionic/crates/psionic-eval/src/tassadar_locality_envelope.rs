use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_models::{
    TASSADAR_LOCALITY_ENVELOPE_REPORT_REF, TassadarLocalityEnvelopePublication,
    tassadar_locality_envelope_publication,
};
use psionic_runtime::{
    TASSADAR_LOCALITY_ENVELOPE_RUNTIME_REPORT_REF, TassadarLocalityEnvelopePosture,
    TassadarLocalityEnvelopeReceipt, TassadarLocalityVariantFamily, TassadarLocalityWorkloadFamily,
    build_tassadar_locality_envelope_runtime_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Per-workload summary in the locality-envelope eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityEnvelopeWorkloadSummary {
    pub workload_family: TassadarLocalityWorkloadFamily,
    pub exact_variant_count: u32,
    pub degraded_variant_count: u32,
    pub refused_variant_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strongest_exact_variant: Option<TassadarLocalityVariantFamily>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lowest_lookback_exact_variant: Option<TassadarLocalityVariantFamily>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lowest_cost_non_refused_variant: Option<TassadarLocalityVariantFamily>,
    pub refusal_variants: Vec<TassadarLocalityVariantFamily>,
    pub average_non_refused_exactness_score_bps: u32,
    pub average_non_refused_cost_score_bps: u32,
    pub note: String,
}

/// Eval-facing report over the computational locality envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityEnvelopeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarLocalityEnvelopePublication,
    pub workload_summaries: Vec<TassadarLocalityEnvelopeWorkloadSummary>,
    pub exact_case_count: u32,
    pub degraded_case_count: u32,
    pub refused_case_count: u32,
    pub exact_winner_counts: BTreeMap<String, u32>,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical eval report for the locality-envelope lane.
#[must_use]
pub fn build_tassadar_locality_envelope_report() -> TassadarLocalityEnvelopeReport {
    let runtime_report = build_tassadar_locality_envelope_runtime_report();
    let publication = tassadar_locality_envelope_publication();
    let workload_summaries = publication
        .workload_families
        .iter()
        .map(|workload_family| build_workload_summary(&runtime_report.receipts, *workload_family))
        .collect::<Vec<_>>();
    let exact_winner_counts = count_exact_winners(&workload_summaries);
    let mut report = TassadarLocalityEnvelopeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.locality_envelope.report.v1"),
        publication,
        workload_summaries,
        exact_case_count: runtime_report.exact_case_count,
        degraded_case_count: runtime_report.degraded_case_count,
        refused_case_count: runtime_report.refused_case_count,
        exact_winner_counts,
        generated_from_refs: vec![String::from(TASSADAR_LOCALITY_ENVELOPE_RUNTIME_REPORT_REF)],
        claim_boundary: String::from(
            "this eval report keeps locality envelopes benchmark-bound and refusal-bounded. It compares dense, sparse, linear, recurrent, and scratchpadized variants on shared workloads without widening served capability, arbitrary Wasm closure, or broad learned exactness claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Locality-envelope eval covers {} workload families with exact winner counts {:?}, plus {} degraded and {} refused variant cells kept explicit.",
        report.workload_summaries.len(),
        report.exact_winner_counts,
        report.degraded_case_count,
        report.refused_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_locality_envelope_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_locality_envelope_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LOCALITY_ENVELOPE_REPORT_REF)
}

/// Writes the committed eval report.
pub fn write_tassadar_locality_envelope_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLocalityEnvelopeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_locality_envelope_report();
    let json = serde_json::to_string_pretty(&report).expect("locality-envelope report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_locality_envelope_report(
    path: impl AsRef<Path>,
) -> Result<TassadarLocalityEnvelopeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn build_workload_summary(
    receipts: &[TassadarLocalityEnvelopeReceipt],
    workload_family: TassadarLocalityWorkloadFamily,
) -> TassadarLocalityEnvelopeWorkloadSummary {
    let workload_receipts = receipts
        .iter()
        .filter(|receipt| receipt.workload_family == workload_family)
        .cloned()
        .collect::<Vec<_>>();
    let exact_receipts = workload_receipts
        .iter()
        .filter(|receipt| receipt.posture == TassadarLocalityEnvelopePosture::Exact)
        .collect::<Vec<_>>();
    let non_refused_receipts = workload_receipts
        .iter()
        .filter(|receipt| receipt.posture != TassadarLocalityEnvelopePosture::Refused)
        .collect::<Vec<_>>();
    TassadarLocalityEnvelopeWorkloadSummary {
        workload_family,
        exact_variant_count: exact_receipts.len() as u32,
        degraded_variant_count: workload_receipts
            .iter()
            .filter(|receipt| {
                receipt.posture == TassadarLocalityEnvelopePosture::DegradedButBounded
            })
            .count() as u32,
        refused_variant_count: workload_receipts
            .iter()
            .filter(|receipt| receipt.posture == TassadarLocalityEnvelopePosture::Refused)
            .count() as u32,
        strongest_exact_variant: exact_receipts
            .iter()
            .max_by_key(|receipt| {
                (
                    receipt.branch_locality_bps + receipt.call_frame_locality_bps,
                    u32::MAX - receipt.cost_score_bps,
                    u32::MAX - receipt.max_useful_lookback_tokens,
                )
            })
            .map(|receipt| receipt.variant_family),
        lowest_lookback_exact_variant: exact_receipts
            .iter()
            .min_by_key(|receipt| (receipt.max_useful_lookback_tokens, receipt.cost_score_bps))
            .map(|receipt| receipt.variant_family),
        lowest_cost_non_refused_variant: non_refused_receipts
            .iter()
            .min_by_key(|receipt| receipt.cost_score_bps)
            .map(|receipt| receipt.variant_family),
        refusal_variants: workload_receipts
            .iter()
            .filter(|receipt| receipt.posture == TassadarLocalityEnvelopePosture::Refused)
            .map(|receipt| receipt.variant_family)
            .collect(),
        average_non_refused_exactness_score_bps: average(&non_refused_receipts, |receipt| {
            receipt.exactness_score_bps
        }),
        average_non_refused_cost_score_bps: average(&non_refused_receipts, |receipt| {
            receipt.cost_score_bps
        }),
        note: String::from(match workload_family {
            TassadarLocalityWorkloadFamily::ArithmeticMultiOperand => {
                "arithmetic stays the cleanest locality win surface, with scratchpad and recurrent variants both remaining exact"
            }
            TassadarLocalityWorkloadFamily::ClrsShortestPath => {
                "CLRS remains exact only when frontier-state publication stays explicit; lower-cost variants still degrade under bounded bridge export"
            }
            TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch => {
                "search-heavy Sudoku is the clearest refusal-first workload family for linear and recurrent shortcuts"
            }
            TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop => {
                "module-scale Wasm still has one exact dense baseline, but every lower-footprint alternative either degrades or refuses today"
            }
        }),
    }
}

fn count_exact_winners(
    workload_summaries: &[TassadarLocalityEnvelopeWorkloadSummary],
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for summary in workload_summaries {
        if let Some(variant) = summary.strongest_exact_variant {
            *counts.entry(variant.as_str().to_string()).or_insert(0) += 1;
        }
    }
    counts
}

fn average(
    receipts: &[&TassadarLocalityEnvelopeReceipt],
    project: impl Fn(&TassadarLocalityEnvelopeReceipt) -> u32,
) -> u32 {
    if receipts.is_empty() {
        0
    } else {
        receipts.iter().map(|receipt| project(receipt)).sum::<u32>() / receipts.len() as u32
    }
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
        build_tassadar_locality_envelope_report, load_tassadar_locality_envelope_report,
        tassadar_locality_envelope_report_path,
    };
    use psionic_runtime::TassadarLocalityVariantFamily;

    #[test]
    fn locality_envelope_report_keeps_refusal_boundaries_explicit() {
        let report = build_tassadar_locality_envelope_report();

        assert_eq!(report.workload_summaries.len(), 4);
        assert_eq!(report.refused_case_count, 3);
        assert!(report.workload_summaries.iter().any(|summary| {
            summary
                .refusal_variants
                .contains(&TassadarLocalityVariantFamily::LinearAttentionProxy)
        }));
    }

    #[test]
    fn locality_envelope_report_matches_committed_truth() {
        let expected = build_tassadar_locality_envelope_report();
        let committed =
            load_tassadar_locality_envelope_report(tassadar_locality_envelope_report_path())
                .expect("committed locality-envelope report");

        assert_eq!(committed, expected);
    }
}
