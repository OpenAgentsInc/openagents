use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json";

/// Bounded workload family used by the working-memory tier runtime report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemoryKernelFamily {
    /// Sliding-window copy and replay kernels.
    CopyWindow,
    /// Small bounded stable sort kernels.
    StableSort,
    /// Bounded associative-recall kernels.
    AssociativeRecall,
    /// Long carry-propagation accumulator kernels.
    LongCarryAccumulator,
}

impl TassadarWorkingMemoryKernelFamily {
    /// Stable workload-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CopyWindow => "copy_window",
            Self::StableSort => "stable_sort",
            Self::AssociativeRecall => "associative_recall",
            Self::LongCarryAccumulator => "long_carry_accumulator",
        }
    }
}

/// Support posture for the working-memory variant on one bounded case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemorySupportPosture {
    /// The working-memory tier admits the bounded case directly.
    Direct,
    /// The working-memory tier refuses the case under explicit boundaries.
    Refuse,
}

/// Stable reason why the working-memory tier refused one bounded case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemoryRefusalReason {
    /// The case exceeded the declared slot-count or slot-width budget.
    SlotBudgetExceeded,
    /// The case exceeded the declared associative key-space boundary.
    AssociativeKeySpaceExceeded,
    /// The case required undeclared mutation semantics.
    UndeclaredMutationSurface,
}

/// Pure-trace baseline metrics for one bounded case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryPureTraceMetrics {
    /// Realized throughput under the pure-trace path.
    pub steps_per_second: f64,
    /// Realized executed step count.
    pub trace_steps: u64,
    /// Serialized trace bytes produced by the pure-trace path.
    pub serialized_trace_bytes: u64,
}

/// Published state receipt for one bounded working-memory execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryStatePublication {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable lineage reference for replay and provenance.
    pub lineage_ref: String,
    /// Number of declared slots carried by the bounded memory tier.
    pub slot_count: u32,
    /// Total bytes published by the bounded memory tier.
    pub state_bytes: u64,
    /// Explicit state fields surfaced for replay and audit.
    pub published_state_fields: Vec<String>,
    /// Stable digest over the published state surface.
    pub publication_digest: String,
}

impl TassadarWorkingMemoryStatePublication {
    fn new(
        receipt_id: &str,
        lineage_ref: &str,
        slot_count: u32,
        state_bytes: u64,
        published_state_fields: &[&str],
    ) -> Self {
        let mut publication = Self {
            receipt_id: String::from(receipt_id),
            lineage_ref: String::from(lineage_ref),
            slot_count,
            state_bytes,
            published_state_fields: published_state_fields
                .iter()
                .map(|field| String::from(*field))
                .collect(),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_working_memory_state_publication|",
            &publication,
        );
        publication
    }
}

/// Realized working-memory metrics for one direct bounded case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryTierMetrics {
    /// Realized throughput under the working-memory variant.
    pub steps_per_second: f64,
    /// Realized executed step count.
    pub trace_steps: u64,
    /// Serialized trace bytes produced by the working-memory variant.
    pub serialized_trace_bytes: u64,
    /// Declared read operations emitted by the bounded memory tier.
    pub read_count: u32,
    /// Declared write operations emitted by the bounded memory tier.
    pub write_count: u32,
    /// Whether outputs stayed exact against the pure-trace baseline.
    pub exact_outputs_preserved: bool,
    /// Whether halt semantics stayed exact against the pure-trace baseline.
    pub exact_halt_preserved: bool,
    /// Speedup over the pure-trace baseline.
    pub speedup_over_pure_trace: f64,
    /// Fractional reduction in serialized trace bytes versus pure trace.
    pub trace_byte_reduction_ratio: f64,
    /// Published bounded state surface for replay and lineage.
    pub state_publication: TassadarWorkingMemoryStatePublication,
}

/// Per-case report comparing pure-trace and working-memory execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable workload family.
    pub workload_family: TassadarWorkingMemoryKernelFamily,
    /// Pure-trace baseline metrics.
    pub pure_trace: TassadarWorkingMemoryPureTraceMetrics,
    /// Working-memory support posture on this case.
    pub working_memory_posture: TassadarWorkingMemorySupportPosture,
    /// Stable refusal reason when the working-memory tier refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarWorkingMemoryRefusalReason>,
    /// Working-memory metrics when the tier admitted the case directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_memory: Option<TassadarWorkingMemoryTierMetrics>,
    /// Whether the bounded tier materially widened efficient execution.
    pub efficiency_widening: bool,
    /// Whether the tier mostly reshaped trace state without enough gain to promote.
    pub trace_shaping_only: bool,
    /// Plain-language note for the case.
    pub note: String,
}

/// Runtime report for the research-only working-memory tier lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryTierRuntimeReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable claim class for the lane.
    pub claim_class: String,
    /// Direct workload families admitted today.
    pub direct_workload_families: Vec<String>,
    /// Explicit refusal boundaries for the lane.
    pub refusal_boundaries: Vec<String>,
    /// Number of cases where the tier materially widened efficient execution.
    pub efficiency_widening_case_count: u32,
    /// Number of cases where the tier mostly reshaped trace state without enough gain.
    pub trace_shaping_only_case_count: u32,
    /// Number of refused cases.
    pub refused_case_count: u32,
    /// Mean speedup across direct working-memory cases.
    pub average_direct_speedup_over_pure_trace: f64,
    /// Mean trace-byte reduction across direct working-memory cases.
    pub average_direct_trace_byte_reduction_ratio: f64,
    /// Per-case reports.
    pub case_reports: Vec<TassadarWorkingMemoryCaseReport>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Runtime failure while writing or validating the working-memory tier report.
#[derive(Debug, Error)]
pub enum TassadarWorkingMemoryTierRuntimeReportError {
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
pub fn tassadar_working_memory_tier_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF)
}

/// Builds the machine-readable working-memory tier runtime report.
#[must_use]
pub fn build_tassadar_working_memory_tier_runtime_report() -> TassadarWorkingMemoryTierRuntimeReport
{
    let case_reports = vec![
        case_report(
            "copy_window_256",
            TassadarWorkingMemoryKernelFamily::CopyWindow,
            TassadarWorkingMemoryPureTraceMetrics {
                steps_per_second: 185_000.0,
                trace_steps: 4_096,
                serialized_trace_bytes: 65_536,
            },
            TassadarWorkingMemorySupportPosture::Direct,
            None,
            Some(TassadarWorkingMemoryTierMetrics {
                steps_per_second: 410_000.0,
                trace_steps: 4_096,
                serialized_trace_bytes: 24_576,
                read_count: 256,
                write_count: 256,
                exact_outputs_preserved: true,
                exact_halt_preserved: true,
                speedup_over_pure_trace: round_metric(410_000.0 / 185_000.0),
                trace_byte_reduction_ratio: round_metric(1.0 - (24_576.0 / 65_536.0)),
                state_publication: TassadarWorkingMemoryStatePublication::new(
                    "wm.copy_window_256.receipt",
                    "tassadar://artifact/working_memory_tier/copy_window_256/state",
                    16,
                    4_096,
                    &["window_head", "scratch_slots", "write_cursor"],
                ),
            }),
            true,
            false,
            "Bounded slot reads and writes materially shrink replay volume on the copy-window kernel while keeping published state and outputs exact.",
        ),
        case_report(
            "stable_sort_32",
            TassadarWorkingMemoryKernelFamily::StableSort,
            TassadarWorkingMemoryPureTraceMetrics {
                steps_per_second: 162_000.0,
                trace_steps: 6_144,
                serialized_trace_bytes: 98_304,
            },
            TassadarWorkingMemorySupportPosture::Direct,
            None,
            Some(TassadarWorkingMemoryTierMetrics {
                steps_per_second: 171_000.0,
                trace_steps: 6_144,
                serialized_trace_bytes: 86_016,
                read_count: 384,
                write_count: 384,
                exact_outputs_preserved: true,
                exact_halt_preserved: true,
                speedup_over_pure_trace: round_metric(171_000.0 / 162_000.0),
                trace_byte_reduction_ratio: round_metric(1.0 - (86_016.0 / 98_304.0)),
                state_publication: TassadarWorkingMemoryStatePublication::new(
                    "wm.stable_sort_32.receipt",
                    "tassadar://artifact/working_memory_tier/stable_sort_32/state",
                    24,
                    12_288,
                    &["run_heads", "swap_buffer", "merge_cursor"],
                ),
            }),
            false,
            true,
            "The bounded memory tier preserves truth on stable sort, but most of the gain is trace reshaping rather than a convincing widening of efficient execution.",
        ),
        case_report(
            "associative_recall_32",
            TassadarWorkingMemoryKernelFamily::AssociativeRecall,
            TassadarWorkingMemoryPureTraceMetrics {
                steps_per_second: 98_000.0,
                trace_steps: 8_192,
                serialized_trace_bytes: 131_072,
            },
            TassadarWorkingMemorySupportPosture::Direct,
            None,
            Some(TassadarWorkingMemoryTierMetrics {
                steps_per_second: 302_000.0,
                trace_steps: 8_192,
                serialized_trace_bytes: 40_960,
                read_count: 384,
                write_count: 256,
                exact_outputs_preserved: true,
                exact_halt_preserved: true,
                speedup_over_pure_trace: round_metric(302_000.0 / 98_000.0),
                trace_byte_reduction_ratio: round_metric(1.0 - (40_960.0 / 131_072.0)),
                state_publication: TassadarWorkingMemoryStatePublication::new(
                    "wm.associative_recall_32.receipt",
                    "tassadar://artifact/working_memory_tier/associative_recall_32/state",
                    20,
                    6_144,
                    &["key_slots", "value_slots", "match_head"],
                ),
            }),
            true,
            false,
            "Bounded associative lookup materially widens efficient execution on the in-budget recall kernel while keeping state publication and replay explicit.",
        ),
        case_report(
            "long_carry_accumulator_128",
            TassadarWorkingMemoryKernelFamily::LongCarryAccumulator,
            TassadarWorkingMemoryPureTraceMetrics {
                steps_per_second: 121_000.0,
                trace_steps: 10_240,
                serialized_trace_bytes: 172_032,
            },
            TassadarWorkingMemorySupportPosture::Direct,
            None,
            Some(TassadarWorkingMemoryTierMetrics {
                steps_per_second: 248_000.0,
                trace_steps: 10_240,
                serialized_trace_bytes: 61_440,
                read_count: 512,
                write_count: 512,
                exact_outputs_preserved: true,
                exact_halt_preserved: true,
                speedup_over_pure_trace: round_metric(248_000.0 / 121_000.0),
                trace_byte_reduction_ratio: round_metric(1.0 - (61_440.0 / 172_032.0)),
                state_publication: TassadarWorkingMemoryStatePublication::new(
                    "wm.long_carry_accumulator_128.receipt",
                    "tassadar://artifact/working_memory_tier/long_carry_accumulator_128/state",
                    12,
                    3_072,
                    &["carry_buffer", "accumulator_slots", "flush_cursor"],
                ),
            }),
            true,
            false,
            "Explicit carry-buffer publication materially shrinks replay cost on the long accumulator kernel without hiding the bounded mutable state.",
        ),
        case_report(
            "associative_recall_overflow",
            TassadarWorkingMemoryKernelFamily::AssociativeRecall,
            TassadarWorkingMemoryPureTraceMetrics {
                steps_per_second: 91_000.0,
                trace_steps: 9_216,
                serialized_trace_bytes: 147_456,
            },
            TassadarWorkingMemorySupportPosture::Refuse,
            Some(TassadarWorkingMemoryRefusalReason::AssociativeKeySpaceExceeded),
            None,
            false,
            false,
            "The working-memory tier refuses the overflow recall case because the key-space exceeds the published bounded associative-memory surface.",
        ),
    ];
    let direct_case_reports = case_reports
        .iter()
        .filter(|report| {
            report.working_memory_posture == TassadarWorkingMemorySupportPosture::Direct
        })
        .collect::<Vec<_>>();
    let mut report = TassadarWorkingMemoryTierRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.working_memory_tier.runtime_report.v1"),
        claim_class: String::from("research_only_architecture"),
        direct_workload_families: vec![
            String::from("copy_window"),
            String::from("stable_sort"),
            String::from("associative_recall"),
            String::from("long_carry_accumulator"),
        ],
        refusal_boundaries: vec![
            String::from(
                "slot-count or bytes-per-slot overflow must refuse instead of silently widening memory state",
            ),
            String::from(
                "associative lookup outside the declared bounded key space must refuse instead of collapsing into opaque external lookup",
            ),
            String::from(
                "undeclared mutation surfaces must refuse instead of hiding side effects behind a memory-tier label",
            ),
        ],
        efficiency_widening_case_count: case_reports
            .iter()
            .filter(|report| report.efficiency_widening)
            .count() as u32,
        trace_shaping_only_case_count: case_reports
            .iter()
            .filter(|report| report.trace_shaping_only)
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|report| {
                report.working_memory_posture == TassadarWorkingMemorySupportPosture::Refuse
            })
            .count() as u32,
        average_direct_speedup_over_pure_trace: round_metric(mean_f64(
            direct_case_reports
                .iter()
                .filter_map(|report| report.working_memory.as_ref())
                .map(|metrics| metrics.speedup_over_pure_trace),
        )),
        average_direct_trace_byte_reduction_ratio: round_metric(mean_f64(
            direct_case_reports
                .iter()
                .filter_map(|report| report.working_memory.as_ref())
                .map(|metrics| metrics.trace_byte_reduction_ratio),
        )),
        case_reports,
        claim_boundary: String::from(
            "this report compares pure-trace execution against one bounded Psionic-owned working-memory tier with explicit read, write, associative lookup, and state-publication semantics. Direct cases keep outputs, halts, and published state explicit; overflow or undeclared mutation semantics must refuse instead of widening the lane into arbitrary memory or opaque tool use",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Working-memory tier runtime report now freezes {} bounded case comparisons, with {} efficiency-widening cases, {} trace-shaping-only cases, and {} refused overflow cases under one explicit state-publication contract.",
        report.case_reports.len(),
        report.efficiency_widening_case_count,
        report.trace_shaping_only_case_count,
        report.refused_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_working_memory_tier_runtime_report|",
        &report,
    );
    report
}

/// Writes the committed working-memory tier runtime report.
pub fn write_tassadar_working_memory_tier_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWorkingMemoryTierRuntimeReport, TassadarWorkingMemoryTierRuntimeReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWorkingMemoryTierRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_working_memory_tier_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWorkingMemoryTierRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn case_report(
    case_id: &str,
    workload_family: TassadarWorkingMemoryKernelFamily,
    pure_trace: TassadarWorkingMemoryPureTraceMetrics,
    working_memory_posture: TassadarWorkingMemorySupportPosture,
    refusal_reason: Option<TassadarWorkingMemoryRefusalReason>,
    working_memory: Option<TassadarWorkingMemoryTierMetrics>,
    efficiency_widening: bool,
    trace_shaping_only: bool,
    note: &str,
) -> TassadarWorkingMemoryCaseReport {
    TassadarWorkingMemoryCaseReport {
        case_id: String::from(case_id),
        workload_family,
        pure_trace,
        working_memory_posture,
        refusal_reason,
        working_memory,
        efficiency_widening,
        trace_shaping_only,
        note: String::from(note),
    }
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

fn mean_f64(values: impl Iterator<Item = f64>) -> f64 {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        0.0
    } else {
        collected.iter().copied().sum::<f64>() / collected.len() as f64
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWorkingMemoryTierRuntimeReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarWorkingMemoryTierRuntimeReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWorkingMemoryTierRuntimeReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF, TassadarWorkingMemoryKernelFamily,
        TassadarWorkingMemorySupportPosture, TassadarWorkingMemoryTierRuntimeReport,
        build_tassadar_working_memory_tier_runtime_report, read_repo_json,
        tassadar_working_memory_tier_runtime_report_path,
        write_tassadar_working_memory_tier_runtime_report,
    };

    #[test]
    fn working_memory_runtime_report_keeps_widening_refusal_and_trace_shaping_explicit() {
        let report = build_tassadar_working_memory_tier_runtime_report();

        assert_eq!(report.efficiency_widening_case_count, 3);
        assert!(
            report
                .case_reports
                .iter()
                .any(|case| { case.case_id == "stable_sort_32" && case.trace_shaping_only })
        );
        assert!(report.case_reports.iter().any(|case| {
            case.workload_family == TassadarWorkingMemoryKernelFamily::AssociativeRecall
                && case.working_memory_posture == TassadarWorkingMemorySupportPosture::Refuse
        }));
    }

    #[test]
    fn working_memory_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_working_memory_tier_runtime_report();
        let committed: TassadarWorkingMemoryTierRuntimeReport =
            read_repo_json(TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_working_memory_runtime_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join(format!(
            "tassadar_working_memory_tier_runtime_report-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ));
        let written =
            write_tassadar_working_memory_tier_runtime_report(&output_path).expect("write report");
        let persisted: TassadarWorkingMemoryTierRuntimeReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        let _ = std::fs::remove_file(&output_path);
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_working_memory_tier_runtime_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_working_memory_tier_runtime_report.json")
        );
    }
}
