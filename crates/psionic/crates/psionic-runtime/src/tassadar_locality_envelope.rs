use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LOCALITY_ENVELOPE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_locality_envelope_runtime_report.json";

/// Shared workload families used by the locality-envelope lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityWorkloadFamily {
    ArithmeticMultiOperand,
    ClrsShortestPath,
    SudokuBacktrackingSearch,
    ModuleScaleWasmLoop,
}

impl TassadarLocalityWorkloadFamily {
    /// Returns the stable workload label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArithmeticMultiOperand => "arithmetic_multi_operand",
            Self::ClrsShortestPath => "clrs_shortest_path",
            Self::SudokuBacktrackingSearch => "sudoku_backtracking_search",
            Self::ModuleScaleWasmLoop => "module_scale_wasm_loop",
        }
    }
}

/// Variant families compared by the locality-envelope lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityVariantFamily {
    DenseReferenceLinear,
    SparseTopKValidated,
    LinearAttentionProxy,
    RecurrentStateRuntime,
    LocalityScratchpadized,
}

impl TassadarLocalityVariantFamily {
    /// Returns the stable variant label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenseReferenceLinear => "dense_reference_linear",
            Self::SparseTopKValidated => "sparse_top_k_validated",
            Self::LinearAttentionProxy => "linear_attention_proxy",
            Self::RecurrentStateRuntime => "recurrent_state_runtime",
            Self::LocalityScratchpadized => "locality_scratchpadized",
        }
    }
}

/// Observed support posture for one locality-envelope receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityEnvelopePosture {
    Exact,
    DegradedButBounded,
    Refused,
}

/// Stable reason one locality-envelope row refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityEnvelopeRefusalReason {
    UsefulLookbackBudgetExceeded,
    BranchDispersionExceeded,
    CallFrameSurfaceInsufficient,
    MutableMemoryAliasRisk,
}

/// One workload-family and variant-family locality-envelope receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityEnvelopeReceipt {
    pub receipt_id: String,
    pub workload_family: TassadarLocalityWorkloadFamily,
    pub variant_family: TassadarLocalityVariantFamily,
    pub max_useful_lookback_tokens: u32,
    pub active_memory_footprint_tokens: u32,
    pub branch_locality_bps: u32,
    pub call_frame_locality_bps: u32,
    pub exactness_score_bps: u32,
    pub cost_score_bps: u32,
    pub posture: TassadarLocalityEnvelopePosture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarLocalityEnvelopeRefusalReason>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Runtime-owned report over the current computational locality envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityEnvelopeRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub claim_class: String,
    pub exact_case_count: u32,
    pub degraded_case_count: u32,
    pub refused_case_count: u32,
    pub average_non_refused_useful_lookback_tokens: u32,
    pub average_non_refused_active_memory_tokens: u32,
    pub average_non_refused_exactness_score_bps: u32,
    pub average_non_refused_cost_score_bps: u32,
    pub receipts: Vec<TassadarLocalityEnvelopeReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical runtime report for the locality-envelope lane.
#[must_use]
pub fn build_tassadar_locality_envelope_runtime_report() -> TassadarLocalityEnvelopeRuntimeReport {
    let receipts = locality_receipts();
    let exact_case_count = receipts
        .iter()
        .filter(|receipt| receipt.posture == TassadarLocalityEnvelopePosture::Exact)
        .count() as u32;
    let degraded_case_count = receipts
        .iter()
        .filter(|receipt| receipt.posture == TassadarLocalityEnvelopePosture::DegradedButBounded)
        .count() as u32;
    let refused_case_count = receipts
        .iter()
        .filter(|receipt| receipt.posture == TassadarLocalityEnvelopePosture::Refused)
        .count() as u32;
    let average_non_refused_useful_lookback_tokens =
        average_non_refused(&receipts, |receipt| receipt.max_useful_lookback_tokens);
    let average_non_refused_active_memory_tokens =
        average_non_refused(&receipts, |receipt| receipt.active_memory_footprint_tokens);
    let average_non_refused_exactness_score_bps =
        average_non_refused(&receipts, |receipt| receipt.exactness_score_bps);
    let average_non_refused_cost_score_bps =
        average_non_refused(&receipts, |receipt| receipt.cost_score_bps);
    let mut report = TassadarLocalityEnvelopeRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.locality_envelope.runtime_report.v1"),
        claim_class: String::from("research_only_fast_path_substrate"),
        exact_case_count,
        degraded_case_count,
        refused_case_count,
        average_non_refused_useful_lookback_tokens,
        average_non_refused_active_memory_tokens,
        average_non_refused_exactness_score_bps,
        average_non_refused_cost_score_bps,
        receipts,
        claim_boundary: String::from(
            "this runtime report is a benchmark-bound locality analysis over seeded workload families and dense, sparse, linear, recurrent, and scratchpadized variants. It keeps exact, degraded-but-bounded, and refused posture explicit instead of widening served capability, arbitrary Wasm closure, or broad learned exactness claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Locality-envelope runtime report covers {} receipts with {} exact, {} degraded, and {} refused cases; non-refused averages are {} useful-lookback tokens, {} active-memory tokens, {} bps exactness, and {} bps cost.",
        report.receipts.len(),
        report.exact_case_count,
        report.degraded_case_count,
        report.refused_case_count,
        report.average_non_refused_useful_lookback_tokens,
        report.average_non_refused_active_memory_tokens,
        report.average_non_refused_exactness_score_bps,
        report.average_non_refused_cost_score_bps,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_locality_envelope_runtime_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_locality_envelope_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LOCALITY_ENVELOPE_RUNTIME_REPORT_REF)
}

/// Writes the committed runtime report.
pub fn write_tassadar_locality_envelope_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLocalityEnvelopeRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_locality_envelope_runtime_report();
    let json = serde_json::to_string_pretty(&report).expect("locality-envelope report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_locality_envelope_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarLocalityEnvelopeRuntimeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn locality_receipts() -> Vec<TassadarLocalityEnvelopeReceipt> {
    vec![
        exact_case(
            TassadarLocalityWorkloadFamily::ArithmeticMultiOperand,
            TassadarLocalityVariantFamily::DenseReferenceLinear,
            52,
            40,
            9_600,
            9_800,
            8_400,
            &[
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_runtime_report.json",
            ],
            "dense replay remains exact on arithmetic but pays a visibly wider lookback and cost envelope than more local variants",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::ArithmeticMultiOperand,
            TassadarLocalityVariantFamily::SparseTopKValidated,
            36,
            28,
            9_700,
            9_800,
            6_400,
            &[
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_runtime_report.json",
            ],
            "validated sparse attention narrows the arithmetic envelope while preserving exactness on the seeded local workload family",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ArithmeticMultiOperand,
            TassadarLocalityVariantFamily::LinearAttentionProxy,
            34,
            24,
            9_100,
            9_000,
            9_100,
            5_600,
            &["fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json"],
            "linear proxy stays cheap on arithmetic but still loses enough intermediate precision to remain degraded rather than exact",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::ArithmeticMultiOperand,
            TassadarLocalityVariantFamily::RecurrentStateRuntime,
            22,
            18,
            9_600,
            9_400,
            4_700,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "recurrent carry is currently exact on the bounded arithmetic family because the carried state stays small and stable",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::ArithmeticMultiOperand,
            TassadarLocalityVariantFamily::LocalityScratchpadized,
            18,
            20,
            9_900,
            9_950,
            4_100,
            &[
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
                "fixtures/tassadar/reports/tassadar_state_design_runtime_report.json",
            ],
            "scratchpadization makes arithmetic the clearest bounded-local problem in the current workload pack",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::ClrsShortestPath,
            TassadarLocalityVariantFamily::DenseReferenceLinear,
            88,
            72,
            8_500,
            7_900,
            8_900,
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "dense replay stays exact on the seeded CLRS bridge family but keeps both lookback and memory footprints materially high",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ClrsShortestPath,
            TassadarLocalityVariantFamily::SparseTopKValidated,
            54,
            46,
            7_600,
            6_800,
            8_400,
            6_500,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_runtime_report.json",
            ],
            "sparse routing cuts the CLRS footprint but still drops too much frontier state to claim exact closure",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ClrsShortestPath,
            TassadarLocalityVariantFamily::LinearAttentionProxy,
            48,
            38,
            7_100,
            6_500,
            8_100,
            5_700,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json",
            ],
            "linear proxy keeps the CLRS bridge cheaper than dense replay, but path-relaxation state still degrades under the current bounded export",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ClrsShortestPath,
            TassadarLocalityVariantFamily::RecurrentStateRuntime,
            42,
            30,
            6_900,
            6_200,
            7_600,
            4_800,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json",
            ],
            "recurrent carry reduces the CLRS state footprint but still smooths over enough graph-frontier structure to stay degraded",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::ClrsShortestPath,
            TassadarLocalityVariantFamily::LocalityScratchpadized,
            40,
            34,
            9_200,
            8_800,
            5_100,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
            ],
            "scratchpadized CLRS traces preserve the frontier state while lowering the useful lookback enough to stay exact on the seeded bridge family",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch,
            TassadarLocalityVariantFamily::DenseReferenceLinear,
            128,
            84,
            8_200,
            7_600,
            9_300,
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "dense replay remains the safe baseline for bounded Sudoku search, but it is both high-lookback and high-cost",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch,
            TassadarLocalityVariantFamily::SparseTopKValidated,
            80,
            56,
            6_700,
            6_000,
            7_400,
            6_100,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_runtime_report.json",
            ],
            "sparse routing helps cost on Sudoku search but still drops contradictory-branch state often enough to remain degraded",
        ),
        refused_case(
            TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch,
            TassadarLocalityVariantFamily::LinearAttentionProxy,
            58,
            44,
            4_100,
            3_900,
            4_700,
            TassadarLocalityEnvelopeRefusalReason::BranchDispersionExceeded,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json",
            ],
            "the current linear proxy does not preserve enough branch discrimination for verifier-guided Sudoku backtracking and must refuse",
        ),
        refused_case(
            TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch,
            TassadarLocalityVariantFamily::RecurrentStateRuntime,
            46,
            32,
            4_800,
            3_600,
            4_300,
            TassadarLocalityEnvelopeRefusalReason::CallFrameSurfaceInsufficient,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json",
            ],
            "bounded recurrent carry cannot yet expose the explicit guess, contradiction, and backtrack frames required by the search trace family",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch,
            TassadarLocalityVariantFamily::LocalityScratchpadized,
            62,
            44,
            9_100,
            9_000,
            5_400,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
            ],
            "scratchpadized search traces preserve explicit search events while materially shrinking the useful-lookback window",
        ),
        exact_case(
            TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop,
            TassadarLocalityVariantFamily::DenseReferenceLinear,
            144,
            108,
            7_800,
            7_200,
            9_600,
            &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "dense replay stays exact on the bounded module-scale suite, but it preserves the widest active footprint in the comparison",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop,
            TassadarLocalityVariantFamily::SparseTopKValidated,
            96,
            72,
            7_000,
            6_500,
            7_700,
            6_800,
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_runtime_report.json",
            ],
            "sparse routing lowers module-scale cost but still drops enough memory-update context to stay degraded",
        ),
        refused_case(
            TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop,
            TassadarLocalityVariantFamily::LinearAttentionProxy,
            70,
            52,
            4_900,
            4_300,
            4_900,
            TassadarLocalityEnvelopeRefusalReason::MutableMemoryAliasRisk,
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json",
            ],
            "the current linear proxy cannot keep byte-addressed mutation and alias boundaries honest on the module-scale Wasm loop family",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop,
            TassadarLocalityVariantFamily::RecurrentStateRuntime,
            74,
            58,
            6_600,
            6_100,
            7_000,
            5_300,
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json",
            ],
            "recurrent carry lowers module-scale memory cost but still smooths over too much mutable-address state to remain exact",
        ),
        degraded_case(
            TassadarLocalityWorkloadFamily::ModuleScaleWasmLoop,
            TassadarLocalityVariantFamily::LocalityScratchpadized,
            90,
            60,
            8_600,
            8_400,
            8_300,
            5_600,
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
                "fixtures/tassadar/reports/tassadar_state_design_runtime_report.json",
            ],
            "scratchpadization narrows the module-scale lookback envelope but still remains degraded while mutable memory and call-frame state stay coupled",
        ),
    ]
}

fn exact_case(
    workload_family: TassadarLocalityWorkloadFamily,
    variant_family: TassadarLocalityVariantFamily,
    max_useful_lookback_tokens: u32,
    active_memory_footprint_tokens: u32,
    branch_locality_bps: u32,
    call_frame_locality_bps: u32,
    cost_score_bps: u32,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarLocalityEnvelopeReceipt {
    receipt(
        workload_family,
        variant_family,
        max_useful_lookback_tokens,
        active_memory_footprint_tokens,
        branch_locality_bps,
        call_frame_locality_bps,
        10_000,
        cost_score_bps,
        TassadarLocalityEnvelopePosture::Exact,
        None,
        benchmark_refs,
        note,
    )
}

fn degraded_case(
    workload_family: TassadarLocalityWorkloadFamily,
    variant_family: TassadarLocalityVariantFamily,
    max_useful_lookback_tokens: u32,
    active_memory_footprint_tokens: u32,
    branch_locality_bps: u32,
    call_frame_locality_bps: u32,
    exactness_score_bps: u32,
    cost_score_bps: u32,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarLocalityEnvelopeReceipt {
    receipt(
        workload_family,
        variant_family,
        max_useful_lookback_tokens,
        active_memory_footprint_tokens,
        branch_locality_bps,
        call_frame_locality_bps,
        exactness_score_bps,
        cost_score_bps,
        TassadarLocalityEnvelopePosture::DegradedButBounded,
        None,
        benchmark_refs,
        note,
    )
}

fn refused_case(
    workload_family: TassadarLocalityWorkloadFamily,
    variant_family: TassadarLocalityVariantFamily,
    max_useful_lookback_tokens: u32,
    active_memory_footprint_tokens: u32,
    branch_locality_bps: u32,
    call_frame_locality_bps: u32,
    cost_score_bps: u32,
    refusal_reason: TassadarLocalityEnvelopeRefusalReason,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarLocalityEnvelopeReceipt {
    receipt(
        workload_family,
        variant_family,
        max_useful_lookback_tokens,
        active_memory_footprint_tokens,
        branch_locality_bps,
        call_frame_locality_bps,
        0,
        cost_score_bps,
        TassadarLocalityEnvelopePosture::Refused,
        Some(refusal_reason),
        benchmark_refs,
        note,
    )
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    workload_family: TassadarLocalityWorkloadFamily,
    variant_family: TassadarLocalityVariantFamily,
    max_useful_lookback_tokens: u32,
    active_memory_footprint_tokens: u32,
    branch_locality_bps: u32,
    call_frame_locality_bps: u32,
    exactness_score_bps: u32,
    cost_score_bps: u32,
    posture: TassadarLocalityEnvelopePosture,
    refusal_reason: Option<TassadarLocalityEnvelopeRefusalReason>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarLocalityEnvelopeReceipt {
    TassadarLocalityEnvelopeReceipt {
        receipt_id: format!(
            "tassadar.locality_envelope.{}.{}.v1",
            workload_family.as_str(),
            variant_family.as_str()
        ),
        workload_family,
        variant_family,
        max_useful_lookback_tokens,
        active_memory_footprint_tokens,
        branch_locality_bps,
        call_frame_locality_bps,
        exactness_score_bps,
        cost_score_bps,
        posture,
        refusal_reason,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn average_non_refused(
    receipts: &[TassadarLocalityEnvelopeReceipt],
    project: impl Fn(&TassadarLocalityEnvelopeReceipt) -> u32,
) -> u32 {
    let values = receipts
        .iter()
        .filter(|receipt| receipt.posture != TassadarLocalityEnvelopePosture::Refused)
        .map(project)
        .collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u32>() / values.len() as u32
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
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
        TassadarLocalityEnvelopePosture, TassadarLocalityVariantFamily,
        TassadarLocalityWorkloadFamily, build_tassadar_locality_envelope_runtime_report,
        load_tassadar_locality_envelope_runtime_report,
        tassadar_locality_envelope_runtime_report_path,
        write_tassadar_locality_envelope_runtime_report,
    };

    #[test]
    fn locality_envelope_runtime_report_keeps_exact_degraded_and_refused_posture_explicit() {
        let report = build_tassadar_locality_envelope_runtime_report();

        assert!(report.receipts.iter().any(|receipt| {
            receipt.workload_family == TassadarLocalityWorkloadFamily::ArithmeticMultiOperand
                && receipt.variant_family == TassadarLocalityVariantFamily::LocalityScratchpadized
                && receipt.posture == TassadarLocalityEnvelopePosture::Exact
        }));
        assert!(report.receipts.iter().any(|receipt| {
            receipt.workload_family == TassadarLocalityWorkloadFamily::ClrsShortestPath
                && receipt.variant_family == TassadarLocalityVariantFamily::LinearAttentionProxy
                && receipt.posture == TassadarLocalityEnvelopePosture::DegradedButBounded
        }));
        assert!(report.receipts.iter().any(|receipt| {
            receipt.workload_family == TassadarLocalityWorkloadFamily::SudokuBacktrackingSearch
                && receipt.variant_family == TassadarLocalityVariantFamily::RecurrentStateRuntime
                && receipt.posture == TassadarLocalityEnvelopePosture::Refused
        }));
        assert_eq!(report.exact_case_count, 9);
        assert_eq!(report.refused_case_count, 3);
    }

    #[test]
    fn locality_envelope_runtime_report_matches_committed_truth() {
        let expected = build_tassadar_locality_envelope_runtime_report();
        let committed = load_tassadar_locality_envelope_runtime_report(
            tassadar_locality_envelope_runtime_report_path(),
        )
        .expect("committed locality-envelope runtime report");

        assert_eq!(committed, expected);
    }

    #[test]
    fn write_locality_envelope_runtime_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_locality_envelope_runtime_report.json");
        let written = write_tassadar_locality_envelope_runtime_report(&output_path)
            .expect("write locality-envelope runtime report");
        let persisted = load_tassadar_locality_envelope_runtime_report(&output_path)
            .expect("persisted locality-envelope runtime report");

        assert_eq!(written, persisted);
    }
}
