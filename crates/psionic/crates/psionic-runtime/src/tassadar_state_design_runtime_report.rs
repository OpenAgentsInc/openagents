use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{TassadarStateDesignFamily, TassadarStateDesignReplayPosture};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_state_design_runtime_report.json";

/// Same-workload family compared across state designs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStateDesignWorkloadFamily {
    /// Module-call and frame-transition traces.
    ModuleCallTrace,
    /// Symbolic sequences dominated by locality-sensitive formatting.
    SymbolicLocality,
    /// Associative lookup and recall workloads.
    AssociativeRecall,
    /// Long-horizon control workloads with extended carried context.
    LongHorizonControl,
    /// Byte-addressed mutation and loop workloads.
    ByteMemoryLoop,
}

impl TassadarStateDesignWorkloadFamily {
    /// Returns the stable workload label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModuleCallTrace => "module_call_trace",
            Self::SymbolicLocality => "symbolic_locality",
            Self::AssociativeRecall => "associative_recall",
            Self::LongHorizonControl => "long_horizon_control",
            Self::ByteMemoryLoop => "byte_memory_loop",
        }
    }
}

/// Stable reason one state-design family refused a workload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStateDesignRefusalReason {
    /// The workload required replay-complete intermediate trace state.
    ReplaySurfaceInsufficient,
    /// The workload exceeded the declared bounded-state budget.
    DeclaredStateBudgetExceeded,
    /// The workload required mutable-address semantics the design did not expose.
    UnsupportedMutableAddressing,
    /// The workload would alias semantic state behind compressed tokens.
    SemanticAliasRisk,
}

/// Per-case runtime report for one state-design and workload-family pair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Compared workload family.
    pub workload_family: TassadarStateDesignWorkloadFamily,
    /// Compared state-design family.
    pub design_family: TassadarStateDesignFamily,
    /// Realized replay posture on this workload/design pair.
    pub replay_posture: TassadarStateDesignReplayPosture,
    /// Whether exact outputs stayed preserved.
    pub exact_output_preserved: bool,
    /// Whether halt semantics stayed preserved.
    pub exact_halt_preserved: bool,
    /// Locality score in basis points where higher is better.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locality_score_bps: Option<u32>,
    /// Edit-cost score in basis points where lower is better.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_cost_bps: Option<u32>,
    /// Serialized trace bytes for this state design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_bytes: Option<u64>,
    /// Published or carried state bytes for this design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_bytes: Option<u64>,
    /// Explicit refusal reason when the pair is out of scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarStateDesignRefusalReason>,
    /// Artifact refs anchoring the case.
    pub benchmark_refs: Vec<String>,
    /// Plain-language case note.
    pub note: String,
}

/// Runtime-owned report over the same-workload state-design study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignRuntimeReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable claim class.
    pub claim_class: String,
    /// Exact replay case count.
    pub exact_replay_case_count: u32,
    /// Reconstructable replay case count.
    pub reconstructable_replay_case_count: u32,
    /// Bounded state-publication case count.
    pub bounded_state_publication_case_count: u32,
    /// Refused case count.
    pub refused_case_count: u32,
    /// Mean locality over all non-refused cases.
    pub average_non_refused_locality_score_bps: u32,
    /// Mean edit cost over all non-refused cases.
    pub average_non_refused_edit_cost_bps: u32,
    /// Per-case reports.
    pub case_reports: Vec<TassadarStateDesignCaseReport>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Runtime failure while building or writing the study report.
#[derive(Debug, Error)]
pub enum TassadarStateDesignRuntimeReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_state_design_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF)
}

/// Builds the machine-readable state-design runtime report.
#[must_use]
pub fn build_tassadar_state_design_runtime_report() -> TassadarStateDesignRuntimeReport {
    let case_reports = vec![
        supported_case(
            TassadarStateDesignWorkloadFamily::ModuleCallTrace,
            TassadarStateDesignFamily::FullAppendOnlyTrace,
            TassadarStateDesignReplayPosture::ExactReplay,
            4_900,
            9_200,
            96_000,
            96_000,
            &[
                "fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json",
                "fixtures/tassadar/reports/tassadar_trace_abi_decision_report.json",
            ],
            "append-only trace keeps frame transitions replay-complete but pays the highest edit cost on module-call workloads",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ModuleCallTrace,
            TassadarStateDesignFamily::DeltaTrace,
            TassadarStateDesignReplayPosture::ReconstructableReplay,
            7_300,
            4_300,
            44_000,
            11_000,
            &["fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json"],
            "delta trace keeps module-call truth reconstructable while cutting the edit surface sharply",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ModuleCallTrace,
            TassadarStateDesignFamily::LocalityScratchpad,
            TassadarStateDesignReplayPosture::ExactReplay,
            8_400,
            3_000,
            58_000,
            58_000,
            &[
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
                "fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json",
            ],
            "scratchpad formatting preserves recovered token truth and wins locality on frame-heavy traces",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::ModuleCallTrace,
            TassadarStateDesignFamily::RecurrentState,
            TassadarStateDesignRefusalReason::ReplaySurfaceInsufficient,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "current recurrent publication cannot expose every intermediate frame transition required for module-call replay",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::ModuleCallTrace,
            TassadarStateDesignFamily::WorkingMemoryTier,
            TassadarStateDesignRefusalReason::UnsupportedMutableAddressing,
            &["fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json"],
            "bounded working-memory slots are not a substitute for exact module-call frame publication",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::SymbolicLocality,
            TassadarStateDesignFamily::FullAppendOnlyTrace,
            TassadarStateDesignReplayPosture::ExactReplay,
            4_600,
            8_800,
            78_000,
            78_000,
            &["fixtures/tassadar/reports/tassadar_scratchpad_framework_comparison_report.json"],
            "full traces remain replay-complete but leave positional span and edit cost unnecessarily high",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::SymbolicLocality,
            TassadarStateDesignFamily::DeltaTrace,
            TassadarStateDesignReplayPosture::ReconstructableReplay,
            7_100,
            4_100,
            35_000,
            9_000,
            &["fixtures/tassadar/reports/tassadar_scratchpad_framework_comparison_report.json"],
            "delta traces improve symbolic locality but still require reconstruction to recover the full reasoning surface",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::SymbolicLocality,
            TassadarStateDesignFamily::LocalityScratchpad,
            TassadarStateDesignReplayPosture::ExactReplay,
            9_300,
            2_500,
            52_000,
            52_000,
            &[
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
                "fixtures/tassadar/reports/tassadar_scratchpad_framework_comparison_report.json",
            ],
            "scratchpad formatting is the current best trace-preserving representation for locality-sensitive symbolic workloads",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::SymbolicLocality,
            TassadarStateDesignFamily::RecurrentState,
            TassadarStateDesignRefusalReason::SemanticAliasRisk,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "compressing symbolic intermediate state into a carried recurrent vector would hide reconstruction semantics the study keeps explicit",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::SymbolicLocality,
            TassadarStateDesignFamily::WorkingMemoryTier,
            TassadarStateDesignRefusalReason::SemanticAliasRisk,
            &["fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json"],
            "working-memory publication helps semantic memory tasks, not symbolic trace-locality studies that still require token-level replay",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::AssociativeRecall,
            TassadarStateDesignFamily::FullAppendOnlyTrace,
            TassadarStateDesignReplayPosture::ExactReplay,
            4_000,
            9_400,
            91_000,
            91_000,
            &["fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json"],
            "full traces stay replay-complete on associative recall but expand far beyond the bounded memory state they are simulating",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::AssociativeRecall,
            TassadarStateDesignFamily::DeltaTrace,
            TassadarStateDesignRefusalReason::SemanticAliasRisk,
            &["fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json"],
            "delta-only publication would hide lookup semantics unless the state surface is made explicit",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::AssociativeRecall,
            TassadarStateDesignFamily::LocalityScratchpad,
            TassadarStateDesignRefusalReason::SemanticAliasRisk,
            &["fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json"],
            "scratchpad layout changes locality but does not expose the semantic lookup state associative recall requires",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::AssociativeRecall,
            TassadarStateDesignFamily::RecurrentState,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            7_800,
            3_200,
            24_000,
            7_200,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "bounded recurrent carry can preserve the current associative workload family when the published state receipt stays explicit",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::AssociativeRecall,
            TassadarStateDesignFamily::WorkingMemoryTier,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            9_100,
            2_100,
            18_000,
            6_400,
            &[
                "fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json",
                "fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json",
            ],
            "working-memory publication is the strongest current bounded state design for associative recall because the slot and lookup semantics remain explicit",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::LongHorizonControl,
            TassadarStateDesignFamily::FullAppendOnlyTrace,
            TassadarStateDesignReplayPosture::ExactReplay,
            3_700,
            9_600,
            120_000,
            120_000,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "full traces remain honest on long-horizon control but become the most expensive representation to edit or route",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::LongHorizonControl,
            TassadarStateDesignFamily::DeltaTrace,
            TassadarStateDesignRefusalReason::DeclaredStateBudgetExceeded,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "current delta reconstruction budget does not survive the carried-state churn in the seeded long-horizon control family",
        ),
        refused_case(
            TassadarStateDesignWorkloadFamily::LongHorizonControl,
            TassadarStateDesignFamily::LocalityScratchpad,
            TassadarStateDesignRefusalReason::ReplaySurfaceInsufficient,
            &["fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json"],
            "scratchpad formatting improves locality but does not supply the semantic state compression long-horizon control now needs",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::LongHorizonControl,
            TassadarStateDesignFamily::RecurrentState,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            8_900,
            2_900,
            30_000,
            8_100,
            &[
                "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json",
                "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json",
            ],
            "bounded recurrent state is the current best fit for long-horizon control because it keeps carried context explicit while avoiding full-trace blowup",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::LongHorizonControl,
            TassadarStateDesignFamily::WorkingMemoryTier,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            7_400,
            3_700,
            33_000,
            10_200,
            &["fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json"],
            "working memory helps on bounded control loops, but the seeded family still favors recurrent carry over explicit slot mutation",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ByteMemoryLoop,
            TassadarStateDesignFamily::FullAppendOnlyTrace,
            TassadarStateDesignReplayPosture::ExactReplay,
            4_300,
            9_000,
            108_000,
            108_000,
            &["fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json"],
            "append-only trace preserves every byte mutation but carries the full cost of serializing each intermediate write",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ByteMemoryLoop,
            TassadarStateDesignFamily::DeltaTrace,
            TassadarStateDesignReplayPosture::ReconstructableReplay,
            8_000,
            3_900,
            42_000,
            12_600,
            &["fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json"],
            "delta traces are strong on byte-memory loops because the changed addresses stay explicit and reconstructable",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ByteMemoryLoop,
            TassadarStateDesignFamily::LocalityScratchpad,
            TassadarStateDesignReplayPosture::ExactReplay,
            7_700,
            4_500,
            61_000,
            61_000,
            &["fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json"],
            "scratchpad formatting helps byte-loop locality, but it still pays more serialized cost than explicit delta or state publication",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ByteMemoryLoop,
            TassadarStateDesignFamily::RecurrentState,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            7_500,
            3_800,
            28_000,
            8_400,
            &["fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json"],
            "bounded recurrent carry survives the current byte-loop family when the carried terminal state is published explicitly",
        ),
        supported_case(
            TassadarStateDesignWorkloadFamily::ByteMemoryLoop,
            TassadarStateDesignFamily::WorkingMemoryTier,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            8_600,
            2_700,
            20_000,
            7_000,
            &[
                "fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json",
                "fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json",
            ],
            "working-memory publication performs best on the seeded byte-loop family because declared slot mutation matches the workload semantics directly",
        ),
    ];
    let exact_replay_case_count = case_reports
        .iter()
        .filter(|case| case.replay_posture == TassadarStateDesignReplayPosture::ExactReplay)
        .count() as u32;
    let reconstructable_replay_case_count = case_reports
        .iter()
        .filter(|case| {
            case.replay_posture == TassadarStateDesignReplayPosture::ReconstructableReplay
        })
        .count() as u32;
    let bounded_state_publication_case_count = case_reports
        .iter()
        .filter(|case| {
            case.replay_posture == TassadarStateDesignReplayPosture::BoundedStatePublication
        })
        .count() as u32;
    let refused_case_count = case_reports
        .iter()
        .filter(|case| case.replay_posture == TassadarStateDesignReplayPosture::Refused)
        .count() as u32;
    let non_refused = case_reports
        .iter()
        .filter(|case| case.replay_posture != TassadarStateDesignReplayPosture::Refused)
        .collect::<Vec<_>>();
    let average_non_refused_locality_score_bps = rounded_mean(
        non_refused
            .iter()
            .filter_map(|case| case.locality_score_bps.map(u64::from)),
    );
    let average_non_refused_edit_cost_bps = rounded_mean(
        non_refused
            .iter()
            .filter_map(|case| case.edit_cost_bps.map(u64::from)),
    );

    let mut report = TassadarStateDesignRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.state_design.runtime_report.v1"),
        claim_class: String::from("research_only_architecture"),
        exact_replay_case_count,
        reconstructable_replay_case_count,
        bounded_state_publication_case_count,
        refused_case_count,
        average_non_refused_locality_score_bps,
        average_non_refused_edit_cost_bps,
        case_reports,
        claim_boundary: String::from(
            "this runtime report compares representation surfaces on the same workload families while keeping replay posture, bounded-state publication, and refusal thresholds explicit. It does not promote one representation into a broad executor or served capability claim",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "State-design runtime report now freezes {} exact-replay, {} reconstructable-replay, {} bounded-state-publication, and {} refused workload/design pairs across five shared workload families.",
        report.exact_replay_case_count,
        report.reconstructable_replay_case_count,
        report.bounded_state_publication_case_count,
        report.refused_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_state_design_runtime_report|", &report);
    report
}

/// Writes the committed state-design runtime report.
pub fn write_tassadar_state_design_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarStateDesignRuntimeReport, TassadarStateDesignRuntimeReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarStateDesignRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_state_design_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarStateDesignRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn supported_case(
    workload_family: TassadarStateDesignWorkloadFamily,
    design_family: TassadarStateDesignFamily,
    replay_posture: TassadarStateDesignReplayPosture,
    locality_score_bps: u32,
    edit_cost_bps: u32,
    trace_bytes: u64,
    state_bytes: u64,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarStateDesignCaseReport {
    TassadarStateDesignCaseReport {
        case_id: format!("{}.{}", workload_family.as_str(), design_family.as_str()),
        workload_family,
        design_family,
        replay_posture,
        exact_output_preserved: true,
        exact_halt_preserved: true,
        locality_score_bps: Some(locality_score_bps),
        edit_cost_bps: Some(edit_cost_bps),
        trace_bytes: Some(trace_bytes),
        state_bytes: Some(state_bytes),
        refusal_reason: None,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        note: String::from(note),
    }
}

fn refused_case(
    workload_family: TassadarStateDesignWorkloadFamily,
    design_family: TassadarStateDesignFamily,
    refusal_reason: TassadarStateDesignRefusalReason,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarStateDesignCaseReport {
    TassadarStateDesignCaseReport {
        case_id: format!("{}.{}", workload_family.as_str(), design_family.as_str()),
        workload_family,
        design_family,
        replay_posture: TassadarStateDesignReplayPosture::Refused,
        exact_output_preserved: false,
        exact_halt_preserved: false,
        locality_score_bps: None,
        edit_cost_bps: None,
        trace_bytes: None,
        state_bytes: None,
        refusal_reason: Some(refusal_reason),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        note: String::from(note),
    }
}

fn rounded_mean(values: impl IntoIterator<Item = u64>) -> u32 {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    let sum = values.iter().sum::<u64>();
    ((sum + (values.len() as u64 / 2)) / values.len() as u64) as u32
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarStateDesignRuntimeReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarStateDesignRuntimeReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarStateDesignRuntimeReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF, TassadarStateDesignRuntimeReport,
        TassadarStateDesignWorkloadFamily, build_tassadar_state_design_runtime_report,
        read_repo_json, tassadar_state_design_runtime_report_path,
        write_tassadar_state_design_runtime_report,
    };
    use psionic_ir::{TassadarStateDesignFamily, TassadarStateDesignReplayPosture};

    #[test]
    fn state_design_runtime_report_tracks_replay_and_refusal_boundaries() {
        let report = build_tassadar_state_design_runtime_report();

        let symbolic_scratchpad = report
            .case_reports
            .iter()
            .find(|case| {
                case.workload_family == TassadarStateDesignWorkloadFamily::SymbolicLocality
                    && case.design_family == TassadarStateDesignFamily::LocalityScratchpad
            })
            .expect("symbolic scratchpad case");
        assert_eq!(
            symbolic_scratchpad.replay_posture,
            TassadarStateDesignReplayPosture::ExactReplay
        );

        let module_recurrent = report
            .case_reports
            .iter()
            .find(|case| {
                case.workload_family == TassadarStateDesignWorkloadFamily::ModuleCallTrace
                    && case.design_family == TassadarStateDesignFamily::RecurrentState
            })
            .expect("module recurrent case");
        assert_eq!(
            module_recurrent.replay_posture,
            TassadarStateDesignReplayPosture::Refused
        );
        assert!(module_recurrent.refusal_reason.is_some());
    }

    #[test]
    fn state_design_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_state_design_runtime_report();
        let committed: TassadarStateDesignRuntimeReport =
            read_repo_json(TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF)
                .expect("committed state-design runtime report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_state_design_runtime_report_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_state_design_runtime_report.json");
        let report =
            write_tassadar_state_design_runtime_report(&output_path).expect("write runtime report");
        let written =
            std::fs::read_to_string(&output_path).expect("written runtime report should exist");
        let reparsed: TassadarStateDesignRuntimeReport =
            serde_json::from_str(&written).expect("written runtime report should parse");

        assert_eq!(report, reparsed);
        assert_eq!(
            tassadar_state_design_runtime_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_state_design_runtime_report.json")
        );
    }
}
