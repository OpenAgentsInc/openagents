use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutorArticleCloseoutSetPacket, PsionExecutorHullCacheBenchmarkPacket,
    PsionExecutorMacExportInspectionPacket, PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
    PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_RESEARCH_BRANCH_SCHEMA_VERSION: &str = "psion.executor.research_branch.v1";
pub const PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_research_branch_v1.json";
pub const PSION_EXECUTOR_RESEARCH_BRANCH_DOC_PATH: &str = "docs/PSION_EXECUTOR_RESEARCH_BRANCH.md";

const PACKET_ID: &str = "psion_executor_research_branch_v1";
const BRANCH_ID: &str = "psion.executor.research_branch.executor_style.v1";
const FAST_ROUTE_LEGITIMACY_SUMMARY_PATH: &str =
    "fixtures/tassadar/reports/tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary.json";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_HULL_CACHE_BENCHMARK_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_HULL_CACHE_BENCHMARK.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";
const TASSADAR_STACK_BOUNDARY_DOC_PATH: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const RESEARCH_CLOSEOUT_WORKLOAD_IDS: [&str; 3] =
    ["hungarian_matching", "long_loop_kernel", "sudoku_v0_test_a"];

#[derive(Debug, Error)]
pub enum PsionExecutorResearchBranchError {
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
    #[error(
        "closeout workload `{workload_id}` is missing from the research branch evaluation set"
    )]
    MissingCloseoutWorkload { workload_id: String },
    #[error("model mismatch between retained export truth and route truth")]
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
    unproven_fast_routes_quarantined: bool,
    resumable_family_not_presented_as_direct_machine: bool,
    served_or_plugin_machine_overclaim_refused: bool,
    fast_route_legitimacy_complete: bool,
    summary_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorResearchExperimentRow {
    pub experiment_id: String,
    pub experiment_family: String,
    pub research_posture: String,
    pub evaluated_closeout_workload_ids: Vec<String>,
    pub route_truth_guard_ids: Vec<String>,
    pub export_truth_guard_ids: Vec<String>,
    pub direct_replacement_allowed: bool,
    pub widening_claim_allowed: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorResearchBranchPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub branch_id: String,
    pub branch_status: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub closeout_set_ref: String,
    pub closeout_set_digest: String,
    pub hull_cache_benchmark_ref: String,
    pub hull_cache_benchmark_digest: String,
    pub mac_export_inspection_ref: String,
    pub mac_export_inspection_digest: String,
    pub fast_route_legitimacy_summary_ref: String,
    pub fast_route_legitimacy_summary_digest: String,
    pub closeout_evaluation_complete: bool,
    pub route_truth_guard_green: bool,
    pub export_truth_guard_green: bool,
    pub carrier_binding_guard_green: bool,
    pub widening_claim_blocked: bool,
    pub experiments: Vec<PsionExecutorResearchExperimentRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorResearchExperimentRow {
    fn validate(&self) -> Result<(), PsionExecutorResearchBranchError> {
        for (field, value) in [
            (
                "psion_executor_research_branch.experiments[].experiment_id",
                self.experiment_id.as_str(),
            ),
            (
                "psion_executor_research_branch.experiments[].experiment_family",
                self.experiment_family.as_str(),
            ),
            (
                "psion_executor_research_branch.experiments[].research_posture",
                self.research_posture.as_str(),
            ),
            (
                "psion_executor_research_branch.experiments[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_research_branch.experiments[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        ensure_expected_closeout_ids(
            &self.evaluated_closeout_workload_ids,
            "psion_executor_research_branch.experiments[].evaluated_closeout_workload_ids",
        )?;
        if self.route_truth_guard_ids.is_empty() || self.export_truth_guard_ids.is_empty() {
            return Err(PsionExecutorResearchBranchError::MissingField {
                field: String::from("psion_executor_research_branch.experiments[].guard_ids"),
            });
        }
        if self.direct_replacement_allowed || self.widening_claim_allowed {
            return Err(PsionExecutorResearchBranchError::InvalidValue {
                field: String::from(
                    "psion_executor_research_branch.experiments[].replacement_or_widening",
                ),
                detail: String::from(
                    "bounded research experiments must stay research-only and non-widening",
                ),
            });
        }
        if stable_experiment_row_digest(self) != self.row_digest {
            return Err(PsionExecutorResearchBranchError::DigestMismatch {
                field: String::from("psion_executor_research_branch.experiments[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorResearchBranchPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorResearchBranchError> {
        if self.schema_version != PSION_EXECUTOR_RESEARCH_BRANCH_SCHEMA_VERSION {
            return Err(PsionExecutorResearchBranchError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_RESEARCH_BRANCH_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_research_branch.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_research_branch.branch_id",
                self.branch_id.as_str(),
            ),
            (
                "psion_executor_research_branch.branch_status",
                self.branch_status.as_str(),
            ),
            (
                "psion_executor_research_branch.canonical_model_id",
                self.canonical_model_id.as_str(),
            ),
            (
                "psion_executor_research_branch.canonical_route_id",
                self.canonical_route_id.as_str(),
            ),
            (
                "psion_executor_research_branch.closeout_set_ref",
                self.closeout_set_ref.as_str(),
            ),
            (
                "psion_executor_research_branch.closeout_set_digest",
                self.closeout_set_digest.as_str(),
            ),
            (
                "psion_executor_research_branch.hull_cache_benchmark_ref",
                self.hull_cache_benchmark_ref.as_str(),
            ),
            (
                "psion_executor_research_branch.hull_cache_benchmark_digest",
                self.hull_cache_benchmark_digest.as_str(),
            ),
            (
                "psion_executor_research_branch.mac_export_inspection_ref",
                self.mac_export_inspection_ref.as_str(),
            ),
            (
                "psion_executor_research_branch.mac_export_inspection_digest",
                self.mac_export_inspection_digest.as_str(),
            ),
            (
                "psion_executor_research_branch.fast_route_legitimacy_summary_ref",
                self.fast_route_legitimacy_summary_ref.as_str(),
            ),
            (
                "psion_executor_research_branch.fast_route_legitimacy_summary_digest",
                self.fast_route_legitimacy_summary_digest.as_str(),
            ),
            (
                "psion_executor_research_branch.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_research_branch.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.branch_status != "research_only" {
            return Err(PsionExecutorResearchBranchError::InvalidValue {
                field: String::from("psion_executor_research_branch.branch_status"),
                detail: String::from("research branch must stay `research_only`"),
            });
        }
        if !self.closeout_evaluation_complete
            || !self.route_truth_guard_green
            || !self.export_truth_guard_green
            || !self.carrier_binding_guard_green
            || !self.widening_claim_blocked
        {
            return Err(PsionExecutorResearchBranchError::InvalidValue {
                field: String::from("psion_executor_research_branch.aggregate_guards"),
                detail: String::from(
                    "bounded research branch must keep closeout, route, export, and carrier-binding guards green while blocking widening claims",
                ),
            });
        }
        if self.experiments.len() != 2 {
            return Err(PsionExecutorResearchBranchError::InvalidValue {
                field: String::from("psion_executor_research_branch.experiments"),
                detail: String::from(
                    "research branch must keep the retained two-experiment bounded lane",
                ),
            });
        }
        let mut seen = BTreeSet::new();
        for experiment in &self.experiments {
            experiment.validate()?;
            seen.insert(experiment.experiment_id.clone());
        }
        if seen.len() != self.experiments.len() {
            return Err(PsionExecutorResearchBranchError::InvalidValue {
                field: String::from("psion_executor_research_branch.experiments[].experiment_id"),
                detail: String::from("experiment ids must stay unique"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorResearchBranchError::DigestMismatch {
                field: String::from("psion_executor_research_branch.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_research_branch_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorResearchBranchPacket, PsionExecutorResearchBranchError> {
    let closeout: PsionExecutorArticleCloseoutSetPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
    )?;
    let benchmark: PsionExecutorHullCacheBenchmarkPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH,
    )?;
    let mac_export: PsionExecutorMacExportInspectionPacket = read_json(
        workspace_root,
        PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
    )?;
    let legitimacy: FastRouteLegitimacySummary =
        read_json(workspace_root, FAST_ROUTE_LEGITIMACY_SUMMARY_PATH)?;

    if mac_export.transformer_model_id != legitimacy.canonical_model_id {
        return Err(PsionExecutorResearchBranchError::ModelMismatch);
    }

    let closeout_ids = closeout_id_set(&closeout)?;
    let closeout_evaluation_complete = benchmark
        .candidate_rows
        .iter()
        .all(|candidate| workload_row_ids(&candidate.workload_rows) == closeout_ids);
    let route_truth_guard_green = benchmark.all_serving_truth_green && !benchmark.promotion_blocked;
    let export_truth_guard_green = mac_export
        .checklist_rows
        .iter()
        .all(|row| row.status == "green")
        && mac_export
            .checklist_rows
            .iter()
            .any(|row| row.checklist_id == "replacement_publication_green")
        && mac_export
            .checklist_rows
            .iter()
            .any(|row| row.checklist_id == "reference_linear_anchor_green");
    let carrier_binding_guard_green = legitimacy.contract_status == "green"
        && legitimacy.carrier_binding_complete
        && legitimacy.unproven_fast_routes_quarantined
        && legitimacy.resumable_family_not_presented_as_direct_machine
        && legitimacy.served_or_plugin_machine_overclaim_refused
        && legitimacy.fast_route_legitimacy_complete;

    let evaluated_closeout_workload_ids = closeout_ids.iter().cloned().collect::<Vec<_>>();
    let experiments = vec![
        build_experiment_row(
            "two_d_head_hard_max_candidate",
            "two_d_head_executor_style",
            &evaluated_closeout_workload_ids,
            "Research-only 2D-head executor-style candidate. It stays bound to the frozen closeout trio, inherits the `reference_linear` truth anchor, and cannot bypass export or replacement truth.",
        ),
        build_experiment_row(
            "executor_style_hierarchical_hull_candidate",
            "hierarchical_hull_executor_style",
            &evaluated_closeout_workload_ids,
            "Research-only hierarchical-hull executor-style candidate. It stays measured only against the admitted closeout trio and remains outside direct carrier or replacement authority until later explicit promotion.",
        ),
    ];

    let mut packet = PsionExecutorResearchBranchPacket {
        schema_version: String::from(PSION_EXECUTOR_RESEARCH_BRANCH_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        branch_id: String::from(BRANCH_ID),
        branch_status: String::from("research_only"),
        canonical_model_id: legitimacy.canonical_model_id.clone(),
        canonical_route_id: legitimacy.canonical_route_id.clone(),
        closeout_set_ref: String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH),
        closeout_set_digest: closeout.packet_digest.clone(),
        hull_cache_benchmark_ref: String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH),
        hull_cache_benchmark_digest: benchmark.packet_digest.clone(),
        mac_export_inspection_ref: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH),
        mac_export_inspection_digest: mac_export.packet_digest.clone(),
        fast_route_legitimacy_summary_ref: String::from(FAST_ROUTE_LEGITIMACY_SUMMARY_PATH),
        fast_route_legitimacy_summary_digest: legitimacy.summary_digest.clone(),
        closeout_evaluation_complete,
        route_truth_guard_green,
        export_truth_guard_green,
        carrier_binding_guard_green,
        widening_claim_blocked: true,
        experiments,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(TASSADAR_STACK_BOUNDARY_DOC_PATH),
            String::from(FAST_ROUTE_LEGITIMACY_SUMMARY_PATH),
        ],
        summary: String::from(
            "The executor lane now keeps one explicit research-only branch for 2D-head and executor-style fast-path experiments. That branch is bound to the frozen closeout trio, the retained HullKVCache benchmark truth, the Mac export inspection receipt, and the post-article carrier-binding contract instead of being treated as direct replacement authority.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    Ok(packet)
}

pub fn write_builtin_executor_research_branch_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorResearchBranchPacket, PsionExecutorResearchBranchError> {
    let packet = builtin_executor_research_branch_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_experiment_row(
    experiment_id: &str,
    experiment_family: &str,
    evaluated_closeout_workload_ids: &[String],
    detail: &str,
) -> PsionExecutorResearchExperimentRow {
    let mut row = PsionExecutorResearchExperimentRow {
        experiment_id: String::from(experiment_id),
        experiment_family: String::from(experiment_family),
        research_posture: String::from("research_only_not_direct_carrier"),
        evaluated_closeout_workload_ids: evaluated_closeout_workload_ids.to_vec(),
        route_truth_guard_ids: vec![
            String::from("reference_linear_truth_anchor_required"),
            String::from("hull_cache_serving_truth_required"),
            String::from("canonical_fast_route_legitimacy_required"),
        ],
        export_truth_guard_ids: vec![
            String::from("portable_bundle_roundtrip_required"),
            String::from("cpu_route_validation_required"),
            String::from("replacement_publication_required"),
        ],
        direct_replacement_allowed: false,
        widening_claim_allowed: false,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_experiment_row_digest(&row);
    row
}

fn closeout_id_set(
    closeout: &PsionExecutorArticleCloseoutSetPacket,
) -> Result<BTreeSet<String>, PsionExecutorResearchBranchError> {
    let ids = closeout
        .workload_rows
        .iter()
        .map(|row| row.workload_id.clone())
        .collect::<BTreeSet<_>>();
    for workload_id in RESEARCH_CLOSEOUT_WORKLOAD_IDS {
        if !ids.contains(workload_id) {
            return Err(PsionExecutorResearchBranchError::MissingCloseoutWorkload {
                workload_id: String::from(workload_id),
            });
        }
    }
    Ok(ids)
}

fn workload_row_ids<T>(rows: &[T]) -> BTreeSet<String>
where
    T: ResearchWorkloadRow,
{
    rows.iter().map(|row| row.closeout_workload_id()).collect()
}

trait ResearchWorkloadRow {
    fn closeout_workload_id(&self) -> String;
}

impl ResearchWorkloadRow for crate::PsionExecutorHullCacheBenchmarkWorkloadRow {
    fn closeout_workload_id(&self) -> String {
        self.closeout_workload_id.clone()
    }
}

fn stable_experiment_row_digest(row: &PsionExecutorResearchExperimentRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_digest(b"psion_executor_research_branch_experiment_row|", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorResearchBranchPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_digest(b"psion_executor_research_branch_packet|", &clone)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec_pretty(value).expect("serialize packet"));
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorResearchBranchError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorResearchBranchError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_expected_closeout_ids(
    ids: &[String],
    field: &str,
) -> Result<(), PsionExecutorResearchBranchError> {
    if ids.len() != RESEARCH_CLOSEOUT_WORKLOAD_IDS.len() {
        return Err(PsionExecutorResearchBranchError::InvalidValue {
            field: String::from(field),
            detail: String::from("research branch must stay bound to the frozen closeout trio"),
        });
    }
    let actual = ids.iter().cloned().collect::<BTreeSet<_>>();
    for workload_id in RESEARCH_CLOSEOUT_WORKLOAD_IDS {
        if !actual.contains(workload_id) {
            return Err(PsionExecutorResearchBranchError::MissingCloseoutWorkload {
                workload_id: String::from(workload_id),
            });
        }
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorResearchBranchError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorResearchBranchError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorResearchBranchError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorResearchBranchError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorResearchBranchError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorResearchBranchError::Write {
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
    fn builtin_executor_research_branch_packet_is_valid(
    ) -> Result<(), PsionExecutorResearchBranchError> {
        let root = workspace_root();
        let packet = builtin_executor_research_branch_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_research_branch_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorResearchBranchError> {
        let root = workspace_root();
        let expected: PsionExecutorResearchBranchPacket =
            read_json(root.as_path(), PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH)?;
        let actual = builtin_executor_research_branch_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorResearchBranchError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_executor_research_branch_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorResearchBranchError> {
        let root = workspace_root();
        let packet = write_builtin_executor_research_branch_packet(root.as_path())?;
        let persisted: PsionExecutorResearchBranchPacket =
            read_json(root.as_path(), PSION_EXECUTOR_RESEARCH_BRANCH_FIXTURE_PATH)?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
