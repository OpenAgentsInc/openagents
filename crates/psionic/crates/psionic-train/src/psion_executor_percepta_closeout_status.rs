use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutorArticleCloseoutSetPacket, PsionExecutorHullCacheBenchmarkPacket,
    PsionExecutorMacExportInspectionPacket, PsionExecutorResearchBranchPacket,
    PsionExecutorTraceNativeMetricsPacket, PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
    PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH, PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH,
    PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_SCHEMA_VERSION: &str =
    "psion.executor.percepta_closeout_status.v1";
pub const PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_percepta_closeout_status_v1.json";
pub const PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md";

const PACKET_ID: &str = "psion_executor_percepta_closeout_status_v1";
const FAST_ROUTE_LEGITIMACY_SUMMARY_PATH: &str =
    "fixtures/tassadar/reports/tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary.json";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET.md";
const PSION_EXECUTOR_HULL_CACHE_BENCHMARK_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_HULL_CACHE_BENCHMARK.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";
const PSION_EXECUTOR_RESEARCH_BRANCH_DOC_PATH: &str = "docs/PSION_EXECUTOR_RESEARCH_BRANCH.md";
const RESEARCH_CLOSEOUT_WORKLOAD_IDS: [&str; 3] =
    ["hungarian_matching", "long_loop_kernel", "sudoku_v0_test_a"];

#[derive(Debug, Error)]
pub enum PsionExecutorPerceptaCloseoutStatusError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error("closeout workload `{workload_id}` is missing from the retained packet set")]
    MissingCloseoutWorkload { workload_id: String },
    #[error("model mismatch between retained route-replacement surfaces")]
    ModelMismatch,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteLegitimacySummary {
    canonical_model_id: String,
    canonical_route_id: String,
    contract_status: String,
    carrier_binding_complete: bool,
    fast_route_legitimacy_complete: bool,
    summary_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorPerceptaCloseoutStatusPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub closeout_set_ref: String,
    pub closeout_set_digest: String,
    pub trace_native_metrics_ref: String,
    pub trace_native_metrics_digest: String,
    pub hull_cache_benchmark_ref: String,
    pub hull_cache_benchmark_digest: String,
    pub research_branch_ref: String,
    pub research_branch_digest: String,
    pub mac_export_inspection_ref: String,
    pub mac_export_inspection_digest: String,
    pub fast_route_legitimacy_summary_ref: String,
    pub fast_route_legitimacy_summary_digest: String,
    pub percepta_closeout_status: String,
    pub workload_truth_status: String,
    pub fast_path_truth_status: String,
    pub route_replacement_truth_status: String,
    pub research_branch_status: String,
    pub broad_claim_posture: String,
    pub closeout_workload_count: u64,
    pub candidate_row_count: u64,
    pub min_hull_cache_speedup_over_reference_linear: f64,
    pub max_hull_cache_remaining_gap_vs_cpu_reference: f64,
    pub route_checklist_row_count: u64,
    pub remaining_limitations: Vec<String>,
    pub next_epic_id: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorPerceptaCloseoutStatusPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
        if self.schema_version != PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_SCHEMA_VERSION {
            return Err(
                PsionExecutorPerceptaCloseoutStatusError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_percepta_closeout_status.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.canonical_model_id",
                self.canonical_model_id.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.canonical_route_id",
                self.canonical_route_id.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.closeout_set_ref",
                self.closeout_set_ref.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.closeout_set_digest",
                self.closeout_set_digest.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.trace_native_metrics_ref",
                self.trace_native_metrics_ref.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.trace_native_metrics_digest",
                self.trace_native_metrics_digest.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.hull_cache_benchmark_ref",
                self.hull_cache_benchmark_ref.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.hull_cache_benchmark_digest",
                self.hull_cache_benchmark_digest.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.research_branch_ref",
                self.research_branch_ref.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.research_branch_digest",
                self.research_branch_digest.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.mac_export_inspection_ref",
                self.mac_export_inspection_ref.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.mac_export_inspection_digest",
                self.mac_export_inspection_digest.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.fast_route_legitimacy_summary_ref",
                self.fast_route_legitimacy_summary_ref.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.fast_route_legitimacy_summary_digest",
                self.fast_route_legitimacy_summary_digest.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.percepta_closeout_status",
                self.percepta_closeout_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.workload_truth_status",
                self.workload_truth_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.fast_path_truth_status",
                self.fast_path_truth_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.route_replacement_truth_status",
                self.route_replacement_truth_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.research_branch_status",
                self.research_branch_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.broad_claim_posture",
                self.broad_claim_posture.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.next_epic_id",
                self.next_epic_id.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_allowed_status(
            self.percepta_closeout_status.as_str(),
            &["red", "partial", "green_bounded"],
            "psion_executor_percepta_closeout_status.percepta_closeout_status",
        )?;
        for (field, value) in [
            (
                "psion_executor_percepta_closeout_status.workload_truth_status",
                self.workload_truth_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.fast_path_truth_status",
                self.fast_path_truth_status.as_str(),
            ),
            (
                "psion_executor_percepta_closeout_status.route_replacement_truth_status",
                self.route_replacement_truth_status.as_str(),
            ),
        ] {
            ensure_allowed_status(value, &["red", "partial", "green"], field)?;
        }
        if self.research_branch_status != "research_only" {
            return Err(PsionExecutorPerceptaCloseoutStatusError::InvalidValue {
                field: String::from(
                    "psion_executor_percepta_closeout_status.research_branch_status",
                ),
                detail: String::from("bounded research branch must stay `research_only`"),
            });
        }
        if self.broad_claim_posture != "not_claimed_outside_bounded_executor_closeout" {
            return Err(PsionExecutorPerceptaCloseoutStatusError::InvalidValue {
                field: String::from("psion_executor_percepta_closeout_status.broad_claim_posture"),
                detail: String::from("broad claim posture must stay explicitly bounded"),
            });
        }
        if self.closeout_workload_count != RESEARCH_CLOSEOUT_WORKLOAD_IDS.len() as u64 {
            return Err(PsionExecutorPerceptaCloseoutStatusError::InvalidValue {
                field: String::from(
                    "psion_executor_percepta_closeout_status.closeout_workload_count",
                ),
                detail: String::from("closeout status must stay bound to the frozen workload trio"),
            });
        }
        if self.candidate_row_count == 0 || self.route_checklist_row_count == 0 {
            return Err(PsionExecutorPerceptaCloseoutStatusError::InvalidValue {
                field: String::from("psion_executor_percepta_closeout_status.row_counts"),
                detail: String::from(
                    "status packet must keep retained candidate and checklist rows visible",
                ),
            });
        }
        if self.min_hull_cache_speedup_over_reference_linear <= 0.0
            || self.max_hull_cache_remaining_gap_vs_cpu_reference <= 0.0
        {
            return Err(PsionExecutorPerceptaCloseoutStatusError::InvalidValue {
                field: String::from("psion_executor_percepta_closeout_status.fast_path_metrics"),
                detail: String::from("fast-path metrics must stay positive"),
            });
        }
        if self.remaining_limitations.is_empty() || self.support_refs.is_empty() {
            return Err(PsionExecutorPerceptaCloseoutStatusError::MissingField {
                field: String::from("psion_executor_percepta_closeout_status.required_lists"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorPerceptaCloseoutStatusError::DigestMismatch {
                field: String::from("psion_executor_percepta_closeout_status.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_percepta_closeout_status_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorPerceptaCloseoutStatusPacket, PsionExecutorPerceptaCloseoutStatusError> {
    let closeout: PsionExecutorArticleCloseoutSetPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
    )?;
    let trace_native: PsionExecutorTraceNativeMetricsPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH,
    )?;
    let benchmark: PsionExecutorHullCacheBenchmarkPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH,
    )?;
    let research: PsionExecutorResearchBranchPacket =
        read_json(workspace_root, PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH)?;
    let mac_export: PsionExecutorMacExportInspectionPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
    )?;
    let legitimacy: FastRouteLegitimacySummary =
        read_json(workspace_root, FAST_ROUTE_LEGITIMACY_SUMMARY_PATH)?;

    if legitimacy.canonical_model_id != mac_export.transformer_model_id
        || legitimacy.canonical_model_id != research.canonical_model_id
    {
        return Err(PsionExecutorPerceptaCloseoutStatusError::ModelMismatch);
    }

    let closeout_ids = closeout_id_set(&closeout)?;
    let workload_truth_green = trace_native
        .candidate_rows
        .iter()
        .all(|candidate| candidate_workload_truth_green(candidate, &closeout_ids));
    let workload_truth_status = if workload_truth_green {
        "green"
    } else if !trace_native.candidate_rows.is_empty() {
        "partial"
    } else {
        "red"
    };

    let fast_path_truth_green = benchmark.all_serving_truth_green
        && !benchmark.promotion_blocked
        && research.route_truth_guard_green
        && legitimacy.contract_status == "green"
        && legitimacy.fast_route_legitimacy_complete;
    let fast_path_truth_status = if fast_path_truth_green {
        "green"
    } else if !benchmark.candidate_rows.is_empty() {
        "partial"
    } else {
        "red"
    };

    let route_replacement_truth_green = mac_export
        .checklist_rows
        .iter()
        .all(|row| row.status == "green")
        && mac_export
            .checklist_rows
            .iter()
            .any(|row| row.checklist_id == "replacement_publication_green")
        && research.export_truth_guard_green
        && legitimacy.contract_status == "green"
        && legitimacy.carrier_binding_complete;
    let route_replacement_truth_status = if route_replacement_truth_green {
        "green"
    } else if !mac_export.checklist_rows.is_empty() {
        "partial"
    } else {
        "red"
    };

    let percepta_closeout_status = if workload_truth_green
        && fast_path_truth_green
        && route_replacement_truth_green
        && research.branch_status == "research_only"
        && research.widening_claim_blocked
    {
        "green_bounded"
    } else if workload_truth_status != "red"
        || fast_path_truth_status != "red"
        || route_replacement_truth_status != "red"
    {
        "partial"
    } else {
        "red"
    };

    let mut packet = PsionExecutorPerceptaCloseoutStatusPacket {
        schema_version: String::from(PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        canonical_model_id: legitimacy.canonical_model_id.clone(),
        canonical_route_id: legitimacy.canonical_route_id.clone(),
        closeout_set_ref: String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH),
        closeout_set_digest: closeout.packet_digest.clone(),
        trace_native_metrics_ref: String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH),
        trace_native_metrics_digest: trace_native.packet_digest.clone(),
        hull_cache_benchmark_ref: String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH),
        hull_cache_benchmark_digest: benchmark.packet_digest.clone(),
        research_branch_ref: String::from(PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH),
        research_branch_digest: research.packet_digest.clone(),
        mac_export_inspection_ref: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH),
        mac_export_inspection_digest: mac_export.packet_digest.clone(),
        fast_route_legitimacy_summary_ref: String::from(FAST_ROUTE_LEGITIMACY_SUMMARY_PATH),
        fast_route_legitimacy_summary_digest: legitimacy.summary_digest.clone(),
        percepta_closeout_status: String::from(percepta_closeout_status),
        workload_truth_status: String::from(workload_truth_status),
        fast_path_truth_status: String::from(fast_path_truth_status),
        route_replacement_truth_status: String::from(route_replacement_truth_status),
        research_branch_status: research.branch_status.clone(),
        broad_claim_posture: String::from("not_claimed_outside_bounded_executor_closeout"),
        closeout_workload_count: closeout_ids.len() as u64,
        candidate_row_count: trace_native.candidate_rows.len() as u64,
        min_hull_cache_speedup_over_reference_linear: benchmark.min_speedup_over_reference_linear,
        max_hull_cache_remaining_gap_vs_cpu_reference: benchmark.max_remaining_gap_vs_cpu_reference,
        route_checklist_row_count: mac_export.checklist_rows.len() as u64,
        remaining_limitations: vec![
            String::from("arbitrary_c_or_wasm_not_claimed"),
            String::from("research_branch_remains_research_only"),
            String::from("trained_v1_candidate_promotion_moves_to_psion_epic_8"),
        ],
        next_epic_id: String::from("PSION-EPIC-8"),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_DOC_PATH),
            String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_RESEARCH_BRANCH_DOC_PATH),
            String::from(FAST_ROUTE_LEGITIMACY_SUMMARY_PATH),
        ],
        summary: String::from(
            "Bounded Percepta / Tassadar-computation closeout is now `green_bounded`: the frozen closeout trio stays green, the retained HullKVCache fast-path packet stays green on that admitted family, route-replacement continuity remains explicit through the Mac export inspection and carrier-binding contract, and the executor-style research branch stays research-only instead of widening the claim boundary.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    Ok(packet)
}

pub fn write_builtin_executor_percepta_closeout_status_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorPerceptaCloseoutStatusPacket, PsionExecutorPerceptaCloseoutStatusError> {
    let packet = builtin_executor_percepta_closeout_status_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn closeout_id_set(
    closeout: &PsionExecutorArticleCloseoutSetPacket,
) -> Result<BTreeSet<String>, PsionExecutorPerceptaCloseoutStatusError> {
    let ids = closeout
        .workload_rows
        .iter()
        .map(|row| row.workload_id.clone())
        .collect::<BTreeSet<_>>();
    for workload_id in RESEARCH_CLOSEOUT_WORKLOAD_IDS {
        if !ids.contains(workload_id) {
            return Err(
                PsionExecutorPerceptaCloseoutStatusError::MissingCloseoutWorkload {
                    workload_id: String::from(workload_id),
                },
            );
        }
    }
    Ok(ids)
}

fn candidate_workload_truth_green(
    candidate: &crate::PsionExecutorTraceNativeCandidateRow,
    closeout_ids: &BTreeSet<String>,
) -> bool {
    let candidate_ids = candidate
        .workload_metrics
        .iter()
        .map(|row| row.closeout_workload_id.clone())
        .collect::<BTreeSet<_>>();
    if &candidate_ids != closeout_ids {
        return false;
    }
    candidate.workload_metrics.iter().all(|row| {
        row.final_output_exactness_bps == 10_000
            && row.step_exactness_bps == 10_000
            && row.halt_exactness_bps == 10_000
            && row.trace_digest_equal
            && row.trace_digest_equal_bps == 10_000
    })
}

fn stable_packet_digest(packet: &PsionExecutorPerceptaCloseoutStatusPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_digest(b"psion_executor_percepta_closeout_status_packet|", &clone)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec_pretty(value).expect("serialize packet"));
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorPerceptaCloseoutStatusError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_allowed_status(
    value: &str,
    allowed: &[&str],
    field: &str,
) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
    if allowed.iter().any(|candidate| value == *candidate) {
        return Ok(());
    }
    Err(PsionExecutorPerceptaCloseoutStatusError::InvalidValue {
        field: String::from(field),
        detail: format!("status `{}` is outside {:?}", value, allowed),
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorPerceptaCloseoutStatusError> {
    let path = workspace_root.join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| PsionExecutorPerceptaCloseoutStatusError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorPerceptaCloseoutStatusError::Parse {
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorPerceptaCloseoutStatusError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorPerceptaCloseoutStatusError::Write {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_percepta_closeout_status_packet_is_valid(
    ) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
        let root = workspace_root();
        let packet = builtin_executor_percepta_closeout_status_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_percepta_closeout_status_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
        let root = workspace_root();
        let expected: PsionExecutorPerceptaCloseoutStatusPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_percepta_closeout_status_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorPerceptaCloseoutStatusError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_executor_percepta_closeout_status_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorPerceptaCloseoutStatusError> {
        let root = workspace_root();
        let packet = write_builtin_executor_percepta_closeout_status_packet(root.as_path())?;
        let persisted: PsionExecutorPerceptaCloseoutStatusPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH,
        )?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
