use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use psionic_eval::{build_tassadar_clrs_wasm_bridge_report, TassadarClrsWasmBridgeReportError};
use psionic_models::{
    TassadarExecutorSubroutineWorkloadFamily, TassadarScratchpadWorkloadFamily,
    TassadarSequenceTraceFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_executor_no_hint_signal_proxy,
    build_tassadar_scratchpad_framework_comparison_report,
    execute_tassadar_trace_family_comparison, TassadarExecutorHintRegime,
    TassadarExecutorNoHintSignalProxyConfig, TassadarTraceFamilyComparisonError,
    TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_REF, TASSADAR_TRACE_FAMILY_COMPARISON_REPORT_REF,
};

pub const TASSADAR_LEARNABILITY_GAP_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_learnability_gap_v1";
pub const TASSADAR_LEARNABILITY_GAP_REPORT_FILE: &str = "learnability_gap_evidence_bundle.json";
pub const TASSADAR_LEARNABILITY_GAP_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_learnability_gap_v1/learnability_gap_evidence_bundle.json";

const TASSADAR_NO_HINT_SELF_SUPERVISED_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_no_hint_self_supervised_report.json";
const TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json";
const REPORT_SCHEMA_VERSION: u16 = 1;

/// Stable workload family audited by the learnability-gap bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityGapWorkloadFamily {
    KernelArithmetic,
    SudokuSearch,
    HungarianMatching,
    ClrsWasmShortestPath,
}

/// Stable architecture family carried by one learnability-gap case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityArchitectureFamily {
    SequenceFormattingProxy,
    LearnedProxy,
    ParallelTraceTarget,
    CompiledBridgeReference,
}

/// Stable trace-format family referenced by one learnability-gap case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityTraceFormat {
    FlatTrace,
    DelimitedScratchpad,
    SequentialCpuReference,
    HungarianAssignmentFrontier,
    CompiledWasmBridge,
}

/// Stable positional scheme referenced by one learnability-gap case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityPositionScheme {
    AbsoluteMonotonic,
    SegmentReset,
    NotApplicable,
}

/// Stable supervision density referenced by one learnability-gap case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilitySupervisionDensity {
    FullHintTrace,
    SubroutineHints,
    FinalOutputsOnly,
    CompiledReference,
}

/// Plausible learnability failure class surfaced by the bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnabilityGapClass {
    PositionalSchemeMiss,
    SupervisionMiss,
    TraceFormatMiss,
    ArchitectureCapacityLimit,
}

/// One train-side case in the learnability-gap evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapEvidenceCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload family being audited.
    pub workload_family: TassadarLearnabilityGapWorkloadFamily,
    /// Architecture family whose learnability is being discussed.
    pub architecture_family: TassadarLearnabilityArchitectureFamily,
    /// Baseline trace format.
    pub baseline_trace_format: TassadarLearnabilityTraceFormat,
    /// Improved trace format when an alternative exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub improved_trace_format: Option<TassadarLearnabilityTraceFormat>,
    /// Baseline positional scheme.
    pub baseline_position_scheme: TassadarLearnabilityPositionScheme,
    /// Improved positional scheme when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub improved_position_scheme: Option<TassadarLearnabilityPositionScheme>,
    /// Baseline supervision density.
    pub baseline_supervision_density: TassadarLearnabilitySupervisionDensity,
    /// Improved supervision density when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub improved_supervision_density: Option<TassadarLearnabilitySupervisionDensity>,
    /// Corpus coverage over the currently seeded family.
    pub corpus_coverage_bps: u32,
    /// Stable baseline metric identifier.
    pub baseline_metric_id: String,
    /// Baseline metric value in basis points.
    pub baseline_metric_bps: u32,
    /// Stable improved metric identifier.
    pub improved_metric_id: String,
    /// Improved metric value in basis points.
    pub improved_metric_bps: u32,
    /// Primary learnability gap the case points to.
    pub primary_gap: TassadarLearnabilityGapClass,
    /// Additional plausible contributing gaps.
    pub secondary_gaps: Vec<TassadarLearnabilityGapClass>,
    /// Stable evidence refs anchoring the case.
    pub evidence_refs: Vec<String>,
    /// Plain-language case detail.
    pub detail: String,
}

/// Train-side learnability-gap evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapEvidenceBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Ordered learnability-gap cases.
    pub cases: Vec<TassadarLearnabilityGapEvidenceCase>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl TassadarLearnabilityGapEvidenceBundle {
    fn new(cases: Vec<TassadarLearnabilityGapEvidenceCase>) -> Self {
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.learnability_gap.evidence_bundle.v1"),
            cases,
            claim_boundary: String::from(
                "this bundle records bounded learnability-gap evidence for seeded kernel, Sudoku, Hungarian, and CLRS-to-Wasm families only; it ties trace format, positional scheme, supervision density, and corpus coverage facts to plausible failure classes without promoting any served capability or claiming that a gap is closed",
            ),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_learnability_gap_evidence_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Train-side learnability-gap build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarLearnabilityGapError {
    /// CLRS bridge build failed.
    #[error(transparent)]
    ClrsBridge(#[from] TassadarClrsWasmBridgeReportError),
    /// Trace-family comparison failed.
    #[error(transparent)]
    TraceFamily(#[from] TassadarTraceFamilyComparisonError),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the bundle.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to clean up a temporary directory.
    #[error("failed to remove temporary directory `{path}`: {error}")]
    Cleanup { path: String, error: std::io::Error },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the canonical learnability-gap evidence bundle.
pub fn build_tassadar_learnability_gap_evidence_bundle(
) -> Result<TassadarLearnabilityGapEvidenceBundle, TassadarLearnabilityGapError> {
    Ok(TassadarLearnabilityGapEvidenceBundle::new(vec![
        build_kernel_case(),
        build_sudoku_case(),
        build_hungarian_case()?,
        build_clrs_case()?,
    ]))
}

/// Writes the canonical learnability-gap evidence bundle.
pub fn write_tassadar_learnability_gap_evidence_bundle(
    output_dir: impl AsRef<Path>,
) -> Result<TassadarLearnabilityGapEvidenceBundle, TassadarLearnabilityGapError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| TassadarLearnabilityGapError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;
    let bundle = build_tassadar_learnability_gap_evidence_bundle()?;
    let output_path = output_dir.join(TASSADAR_LEARNABILITY_GAP_REPORT_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarLearnabilityGapError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_kernel_case() -> TassadarLearnabilityGapEvidenceCase {
    let report = build_tassadar_scratchpad_framework_comparison_report();
    let arithmetic = report
        .variants
        .iter()
        .find(|variant| variant.workload_family == TassadarScratchpadWorkloadFamily::Arithmetic)
        .expect("arithmetic scratchpad variant should exist");
    let baseline_max = arithmetic
        .case_reports
        .iter()
        .map(|case| case.baseline_max_output_local_position_index)
        .max()
        .unwrap_or(0);
    let improved_metric_bps = arithmetic.mean_locality_gain_bps;
    TassadarLearnabilityGapEvidenceCase {
        case_id: String::from("kernel_arithmetic_position_gap"),
        workload_family: TassadarLearnabilityGapWorkloadFamily::KernelArithmetic,
        architecture_family: TassadarLearnabilityArchitectureFamily::SequenceFormattingProxy,
        baseline_trace_format: TassadarLearnabilityTraceFormat::FlatTrace,
        improved_trace_format: Some(TassadarLearnabilityTraceFormat::DelimitedScratchpad),
        baseline_position_scheme: TassadarLearnabilityPositionScheme::AbsoluteMonotonic,
        improved_position_scheme: Some(TassadarLearnabilityPositionScheme::SegmentReset),
        baseline_supervision_density: TassadarLearnabilitySupervisionDensity::FullHintTrace,
        improved_supervision_density: Some(TassadarLearnabilitySupervisionDensity::FullHintTrace),
        corpus_coverage_bps: 10_000,
        baseline_metric_id: String::from("locality_gain_bps"),
        baseline_metric_bps: 0,
        improved_metric_id: String::from("locality_gain_bps"),
        improved_metric_bps,
        primary_gap: TassadarLearnabilityGapClass::PositionalSchemeMiss,
        secondary_gaps: vec![TassadarLearnabilityGapClass::TraceFormatMiss],
        evidence_refs: vec![String::from(TASSADAR_SCRATCHPAD_FRAMEWORK_REPORT_REF)],
        detail: format!(
            "The seeded arithmetic kernel family keeps final outputs exact, but the absolute-position flat trace leaves a max output local position of {} before the segment-reset scratchpad cut. The report therefore tags the current learnability gap as primarily positional-scheme-sensitive rather than a claim that the kernel family is solved in one representation.",
            baseline_max
        ),
    }
}

fn build_sudoku_case() -> TassadarLearnabilityGapEvidenceCase {
    let baseline =
        build_tassadar_executor_no_hint_signal_proxy(&TassadarExecutorNoHintSignalProxyConfig {
            supervision_regime: TassadarExecutorHintRegime::FullHintTrace,
            held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
        });
    let subroutine =
        build_tassadar_executor_no_hint_signal_proxy(&TassadarExecutorNoHintSignalProxyConfig {
            supervision_regime: TassadarExecutorHintRegime::SubroutineHints,
            held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
        });
    TassadarLearnabilityGapEvidenceCase {
        case_id: String::from("sudoku_supervision_gap"),
        workload_family: TassadarLearnabilityGapWorkloadFamily::SudokuSearch,
        architecture_family: TassadarLearnabilityArchitectureFamily::LearnedProxy,
        baseline_trace_format: TassadarLearnabilityTraceFormat::SequentialCpuReference,
        improved_trace_format: None,
        baseline_position_scheme: TassadarLearnabilityPositionScheme::NotApplicable,
        improved_position_scheme: None,
        baseline_supervision_density: TassadarLearnabilitySupervisionDensity::FullHintTrace,
        improved_supervision_density: Some(TassadarLearnabilitySupervisionDensity::SubroutineHints),
        corpus_coverage_bps: 10_000,
        baseline_metric_id: String::from("reusable_signal_bps"),
        baseline_metric_bps: baseline.reusable_signal_bps,
        improved_metric_id: String::from("reusable_signal_bps"),
        improved_metric_bps: subroutine.reusable_signal_bps,
        primary_gap: TassadarLearnabilityGapClass::SupervisionMiss,
        secondary_gaps: Vec::new(),
        evidence_refs: vec![String::from(TASSADAR_NO_HINT_SELF_SUPERVISED_REPORT_REF)],
        detail: format!(
            "Held-out Sudoku-style reusable signal rises from {} bps under full-hint traces to {} bps when the seeded corpus switches to reusable subroutine hints, so the current learnability gap is better explained by supervision structure than by an immediate claim of architecture closure.",
            baseline.reusable_signal_bps,
            subroutine.reusable_signal_bps
        ),
    }
}

fn build_hungarian_case(
) -> Result<TassadarLearnabilityGapEvidenceCase, TassadarLearnabilityGapError> {
    let temp_dir = create_temp_dir()?;
    let comparison = execute_tassadar_trace_family_comparison(&temp_dir)?;
    fs::remove_dir_all(&temp_dir).map_err(|error| TassadarLearnabilityGapError::Cleanup {
        path: temp_dir.display().to_string(),
        error,
    })?;
    let matching = comparison
        .workload_comparisons
        .iter()
        .filter(|workload| {
            workload.alternate_trace_family.trace_family
                == TassadarSequenceTraceFamily::HungarianAssignmentFrontier
        })
        .collect::<Vec<_>>();
    let total_examples = matching
        .iter()
        .map(|workload| workload.sequential_cpu_reference.example_count)
        .sum::<u32>();
    let baseline_fit_cases = matching
        .iter()
        .map(|workload| {
            workload
                .sequential_cpu_reference
                .fits_current_model_context_case_count
        })
        .sum::<u32>();
    let improved_fit_cases = matching
        .iter()
        .map(|workload| {
            workload
                .alternate_trace_family
                .fits_current_model_context_case_count
        })
        .sum::<u32>();
    Ok(TassadarLearnabilityGapEvidenceCase {
        case_id: String::from("hungarian_trace_capacity_gap"),
        workload_family: TassadarLearnabilityGapWorkloadFamily::HungarianMatching,
        architecture_family: TassadarLearnabilityArchitectureFamily::ParallelTraceTarget,
        baseline_trace_format: TassadarLearnabilityTraceFormat::SequentialCpuReference,
        improved_trace_format: Some(TassadarLearnabilityTraceFormat::HungarianAssignmentFrontier),
        baseline_position_scheme: TassadarLearnabilityPositionScheme::NotApplicable,
        improved_position_scheme: None,
        baseline_supervision_density: TassadarLearnabilitySupervisionDensity::FullHintTrace,
        improved_supervision_density: Some(TassadarLearnabilitySupervisionDensity::FinalOutputsOnly),
        corpus_coverage_bps: 10_000,
        baseline_metric_id: String::from("context_fit_bps"),
        baseline_metric_bps: ratio_bps(baseline_fit_cases, total_examples),
        improved_metric_id: String::from("context_fit_bps"),
        improved_metric_bps: ratio_bps(improved_fit_cases, total_examples),
        primary_gap: TassadarLearnabilityGapClass::ArchitectureCapacityLimit,
        secondary_gaps: vec![TassadarLearnabilityGapClass::TraceFormatMiss],
        evidence_refs: vec![String::from(TASSADAR_TRACE_FAMILY_COMPARISON_REPORT_REF)],
        detail: format!(
            "The Hungarian matching family improves context fit from {} bps to {} bps when the target family moves from sequential CPU authority to the research-only assignment frontier, but the improved frontier still does not erase the fit ceiling across the seeded workloads. The bundle therefore marks a remaining architecture-capacity limit with a trace-format miss as secondary.",
            ratio_bps(baseline_fit_cases, total_examples),
            ratio_bps(improved_fit_cases, total_examples)
        ),
    })
}

fn build_clrs_case() -> Result<TassadarLearnabilityGapEvidenceCase, TassadarLearnabilityGapError> {
    let bridge_report = build_tassadar_clrs_wasm_bridge_report()?;
    let baseline =
        build_tassadar_executor_no_hint_signal_proxy(&TassadarExecutorNoHintSignalProxyConfig {
            supervision_regime: TassadarExecutorHintRegime::FullHintTrace,
            held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
        });
    let improved =
        build_tassadar_executor_no_hint_signal_proxy(&TassadarExecutorNoHintSignalProxyConfig {
            supervision_regime: TassadarExecutorHintRegime::SubroutineHints,
            held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
        });
    let bridge_exactness_bps = if bridge_report.length_generalization_matrix.is_empty() {
        0
    } else {
        bridge_report
            .length_generalization_matrix
            .iter()
            .map(|cell| u64::from(cell.exactness_bps))
            .sum::<u64>() as u32
            / bridge_report.length_generalization_matrix.len() as u32
    };
    Ok(TassadarLearnabilityGapEvidenceCase {
        case_id: String::from("clrs_wasm_supervision_gap"),
        workload_family: TassadarLearnabilityGapWorkloadFamily::ClrsWasmShortestPath,
        architecture_family: TassadarLearnabilityArchitectureFamily::CompiledBridgeReference,
        baseline_trace_format: TassadarLearnabilityTraceFormat::CompiledWasmBridge,
        improved_trace_format: None,
        baseline_position_scheme: TassadarLearnabilityPositionScheme::NotApplicable,
        improved_position_scheme: None,
        baseline_supervision_density: TassadarLearnabilitySupervisionDensity::FullHintTrace,
        improved_supervision_density: Some(TassadarLearnabilitySupervisionDensity::SubroutineHints),
        corpus_coverage_bps: 10_000,
        baseline_metric_id: String::from("reusable_signal_bps"),
        baseline_metric_bps: baseline.reusable_signal_bps,
        improved_metric_id: String::from("reusable_signal_bps"),
        improved_metric_bps: improved.reusable_signal_bps,
        primary_gap: TassadarLearnabilityGapClass::SupervisionMiss,
        secondary_gaps: vec![TassadarLearnabilityGapClass::ArchitectureCapacityLimit],
        evidence_refs: vec![
            String::from(TASSADAR_NO_HINT_SELF_SUPERVISED_REPORT_REF),
            String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF),
        ],
        detail: format!(
            "The compiled CLRS-to-Wasm bridge stays exact at {} bps across the seeded tiny and small shortest-path witnesses, but the learned proxy only moves from {} bps reusable signal under full-hint traces to {} bps under reusable subroutine hints. The current learnability gap therefore reads primarily as a supervision miss and secondarily as remaining architecture distance from compiled truth.",
            bridge_exactness_bps,
            baseline.reusable_signal_bps,
            improved.reusable_signal_bps
        ),
    })
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        ((u64::from(numerator) * 10_000) / u64::from(denominator)) as u32
    }
}

fn create_temp_dir() -> Result<PathBuf, TassadarLearnabilityGapError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "psionic-tassadar-learnability-gap-{}-{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&path).map_err(|error| TassadarLearnabilityGapError::CreateDir {
        path: path.display().to_string(),
        error,
    })?;
    Ok(path)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use serde::de::DeserializeOwned;

    use super::{
        build_tassadar_learnability_gap_evidence_bundle,
        write_tassadar_learnability_gap_evidence_bundle, TassadarLearnabilityGapClass,
        TassadarLearnabilityGapEvidenceBundle, TassadarLearnabilityGapWorkloadFamily,
        TASSADAR_LEARNABILITY_GAP_REPORT_FILE, TASSADAR_LEARNABILITY_GAP_REPORT_REF,
    };

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn learnability_gap_evidence_bundle_covers_seeded_workload_families(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_learnability_gap_evidence_bundle()?;
        let families = bundle
            .cases
            .iter()
            .map(|case| case.workload_family)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(bundle.cases.len(), 4);
        assert_eq!(
            families.into_iter().collect::<Vec<_>>(),
            vec![
                TassadarLearnabilityGapWorkloadFamily::KernelArithmetic,
                TassadarLearnabilityGapWorkloadFamily::SudokuSearch,
                TassadarLearnabilityGapWorkloadFamily::HungarianMatching,
                TassadarLearnabilityGapWorkloadFamily::ClrsWasmShortestPath,
            ]
        );
        assert!(bundle
            .cases
            .iter()
            .any(|case| case.primary_gap == TassadarLearnabilityGapClass::SupervisionMiss));
        Ok(())
    }

    #[test]
    fn learnability_gap_evidence_bundle_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_learnability_gap_evidence_bundle()?;
        let committed: TassadarLearnabilityGapEvidenceBundle =
            read_repo_json(TASSADAR_LEARNABILITY_GAP_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn learnability_gap_evidence_bundle_writes_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let bundle = write_tassadar_learnability_gap_evidence_bundle(temp_dir.path())?;
        let bytes = std::fs::read(temp_dir.path().join(TASSADAR_LEARNABILITY_GAP_REPORT_FILE))?;
        let roundtrip: TassadarLearnabilityGapEvidenceBundle = serde_json::from_slice(&bytes)?;
        assert_eq!(bundle, roundtrip);
        Ok(())
    }
}
