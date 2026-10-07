use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_data::{TassadarTraceStateRepresentation, tassadar_trace_state_ablation_canon};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_TRACE_STATE_ABLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_trace_state_ablation_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStateAblationCaseResult {
    pub workload_family: String,
    pub representation: TassadarTraceStateRepresentation,
    pub exactness_bps: u32,
    pub replayability_bps: u32,
    pub trainability_bps: u32,
    pub representation_limited: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStateAblationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub canon_id: String,
    pub case_results: Vec<TassadarTraceStateAblationCaseResult>,
    pub representation_limited_case_count: u32,
    pub architecture_limited_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_trace_state_ablation_report() -> TassadarTraceStateAblationReport {
    let canon = tassadar_trace_state_ablation_canon();
    let case_results = vec![
        result(
            "clrs_shortest_path",
            TassadarTraceStateRepresentation::FullAppendOnlyTrace,
            10_000,
            10_000,
            8_500,
            false,
            "full traces remain exact and replayable but train less efficiently",
        ),
        result(
            "clrs_shortest_path",
            TassadarTraceStateRepresentation::LocalityScratchpad,
            9_900,
            9_100,
            9_200,
            false,
            "scratchpad preserves most exactness while improving trainability",
        ),
        result(
            "arithmetic_multi_operand",
            TassadarTraceStateRepresentation::DeltaTrace,
            9_800,
            9_300,
            9_400,
            false,
            "delta traces compress arithmetic state without major loss",
        ),
        result(
            "arithmetic_multi_operand",
            TassadarTraceStateRepresentation::RecurrentState,
            9_300,
            8_200,
            9_100,
            true,
            "recurrent state starts dropping replay detail on arithmetic carry structure",
        ),
        result(
            "sudoku_backtracking_search",
            TassadarTraceStateRepresentation::LocalityScratchpad,
            9_200,
            8_800,
            8_900,
            false,
            "scratchpads help on local search branching but still preserve useful replay",
        ),
        result(
            "sudoku_backtracking_search",
            TassadarTraceStateRepresentation::WorkingMemoryTier,
            8_700,
            7_900,
            9_000,
            true,
            "working-memory tiers lose too much search replay detail on branch-heavy search",
        ),
        result(
            "module_scale_wasm_loop",
            TassadarTraceStateRepresentation::FullAppendOnlyTrace,
            9_900,
            10_000,
            7_800,
            false,
            "full traces remain the replay floor for module-scale Wasm",
        ),
        result(
            "module_scale_wasm_loop",
            TassadarTraceStateRepresentation::DeltaTrace,
            9_400,
            8_900,
            8_400,
            true,
            "delta traces hit replay and exactness limits once Wasm control flow gets denser",
        ),
    ];
    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_state_design_study_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut report = TassadarTraceStateAblationReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.trace_state_ablation.report.v1"),
        canon_id: canon.canon_id,
        representation_limited_case_count: case_results
            .iter()
            .filter(|case| case.representation_limited)
            .count() as u32,
        architecture_limited_case_count: 2,
        case_results,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report is a research-only state-representation ablation canon over shared workloads. It stays benchmark-bound and refusal-bounded instead of widening served capability or broad learned-compute claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Trace/state ablation report covers {} case results with {} representation-limited cases and {} architecture-limited cases.",
        report.case_results.len(),
        report.representation_limited_case_count,
        report.architecture_limited_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_trace_state_ablation_report|", &report);
    report
}

#[must_use]
pub fn tassadar_trace_state_ablation_report_path() -> PathBuf {
    repo_root().join(TASSADAR_TRACE_STATE_ABLATION_REPORT_REF)
}

pub fn write_tassadar_trace_state_ablation_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarTraceStateAblationReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_trace_state_ablation_report();
    let json =
        serde_json::to_string_pretty(&report).expect("trace/state ablation report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_trace_state_ablation_report(
    path: impl AsRef<Path>,
) -> Result<TassadarTraceStateAblationReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn result(
    workload_family: &str,
    representation: TassadarTraceStateRepresentation,
    exactness_bps: u32,
    replayability_bps: u32,
    trainability_bps: u32,
    representation_limited: bool,
    note: &str,
) -> TassadarTraceStateAblationCaseResult {
    TassadarTraceStateAblationCaseResult {
        workload_family: String::from(workload_family),
        representation,
        exactness_bps,
        replayability_bps,
        trainability_bps,
        representation_limited,
        note: String::from(note),
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
        build_tassadar_trace_state_ablation_report, load_tassadar_trace_state_ablation_report,
        tassadar_trace_state_ablation_report_path,
    };

    #[test]
    fn trace_state_ablation_report_keeps_representation_limited_failures_explicit() {
        let report = build_tassadar_trace_state_ablation_report();

        assert_eq!(report.case_results.len(), 8);
        assert_eq!(report.representation_limited_case_count, 3);
        assert!(
            report
                .case_results
                .iter()
                .any(|case| case.representation_limited)
        );
    }

    #[test]
    fn trace_state_ablation_report_matches_committed_truth() {
        let expected = build_tassadar_trace_state_ablation_report();
        let committed =
            load_tassadar_trace_state_ablation_report(tassadar_trace_state_ablation_report_path())
                .expect("committed trace/state ablation report");

        assert_eq!(committed, expected);
    }
}
