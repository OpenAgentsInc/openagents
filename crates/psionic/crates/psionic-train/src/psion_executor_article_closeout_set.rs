use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_baseline_truth_record, builtin_executor_eval_pack_catalog,
    PsionExecutorBaselineTruthError, PsionExecutorEvalPackCatalog, PsionExecutorEvalPackError,
    PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH, PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_SCHEMA_VERSION: &str =
    "psion.executor.article_closeout_set.v1";
pub const PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_article_closeout_set_v1.json";
pub const PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET.md";

const PACKET_ID: &str = "psion_executor_article_closeout_set_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_BASELINE_DOC_PATH: &str = "docs/PSION_EXECUTOR_BASELINE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD.md";
const PSION_EXECUTOR_UNIFIED_THROUGHPUT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING.md";

const CLOSEOUT_WORKLOAD_IDS: [&str; 3] =
    ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"];

#[derive(Debug, Error)]
pub enum PsionExecutorArticleCloseoutSetError {
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
    #[error("closeout workload `{workload_id}` is missing from the frozen exactness suites")]
    MissingWorkload { workload_id: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    BaselineTruth(#[from] PsionExecutorBaselineTruthError),
    #[error(transparent)]
    EvalPack(#[from] PsionExecutorEvalPackError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorArticleCloseoutWorkloadRow {
    pub workload_id: String,
    pub workload_family: String,
    pub promotion_pack_id: String,
    pub frequent_pack_id: String,
    pub required_for_promotion: bool,
    pub required_for_local_cluster_validation: bool,
    pub local_cluster_visibility_ref: String,
    pub promotion_visibility_ref: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorArticleCloseoutSetPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub baseline_truth_ref: String,
    pub baseline_truth_digest: String,
    pub eval_pack_catalog_ref: String,
    pub eval_pack_catalog_digest: String,
    pub workload_rows: Vec<PsionExecutorArticleCloseoutWorkloadRow>,
    pub promotion_validation_refs: Vec<String>,
    pub local_cluster_validation_refs: Vec<String>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorArticleCloseoutWorkloadRow {
    fn validate(&self) -> Result<(), PsionExecutorArticleCloseoutSetError> {
        for (field, value) in [
            (
                "psion_executor_article_closeout_set.workload_rows[].workload_id",
                self.workload_id.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].workload_family",
                self.workload_family.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].promotion_pack_id",
                self.promotion_pack_id.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].frequent_pack_id",
                self.frequent_pack_id.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].local_cluster_visibility_ref",
                self.local_cluster_visibility_ref.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].promotion_visibility_ref",
                self.promotion_visibility_ref.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.workload_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if !self.required_for_promotion || !self.required_for_local_cluster_validation {
            return Err(PsionExecutorArticleCloseoutSetError::InvalidValue {
                field: String::from(
                    "psion_executor_article_closeout_set.workload_rows[].required_flags",
                ),
                detail: String::from("all closeout workloads must stay required for both lanes"),
            });
        }
        if stable_workload_row_digest(self) != self.row_digest {
            return Err(PsionExecutorArticleCloseoutSetError::DigestMismatch {
                field: String::from(
                    "psion_executor_article_closeout_set.workload_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorArticleCloseoutSetPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorArticleCloseoutSetError> {
        if self.schema_version != PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_SCHEMA_VERSION {
            return Err(
                PsionExecutorArticleCloseoutSetError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_article_closeout_set.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.baseline_truth_ref",
                self.baseline_truth_ref.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.eval_pack_catalog_ref",
                self.eval_pack_catalog_ref.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.eval_pack_catalog_digest",
                self.eval_pack_catalog_digest.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_article_closeout_set.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.workload_rows.len() != CLOSEOUT_WORKLOAD_IDS.len() {
            return Err(PsionExecutorArticleCloseoutSetError::InvalidValue {
                field: String::from("psion_executor_article_closeout_set.workload_rows"),
                detail: String::from("closeout set must stay fixed to three workload rows"),
            });
        }
        let mut seen = BTreeSet::new();
        for row in &self.workload_rows {
            row.validate()?;
            seen.insert(row.workload_id.as_str());
        }
        for workload_id in CLOSEOUT_WORKLOAD_IDS {
            if !seen.contains(workload_id) {
                return Err(PsionExecutorArticleCloseoutSetError::MissingWorkload {
                    workload_id: String::from(workload_id),
                });
            }
        }
        if self.promotion_validation_refs.is_empty()
            || self.local_cluster_validation_refs.is_empty()
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutorArticleCloseoutSetError::MissingField {
                field: String::from("psion_executor_article_closeout_set.required_refs"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorArticleCloseoutSetError::DigestMismatch {
                field: String::from("psion_executor_article_closeout_set.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_article_closeout_set_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorArticleCloseoutSetPacket, PsionExecutorArticleCloseoutSetError> {
    let catalog = builtin_executor_eval_pack_catalog(workspace_root)?;
    let baseline = builtin_executor_baseline_truth_record(workspace_root)?;

    let exactness_workloads = collect_exactness_workloads(&catalog);
    for workload_id in CLOSEOUT_WORKLOAD_IDS {
        if !exactness_workloads.contains(workload_id) {
            return Err(PsionExecutorArticleCloseoutSetError::MissingWorkload {
                workload_id: String::from(workload_id),
            });
        }
    }
    let baseline_workloads = collect_baseline_workloads(&baseline);
    for workload_id in CLOSEOUT_WORKLOAD_IDS {
        if !baseline_workloads.contains(workload_id) {
            return Err(PsionExecutorArticleCloseoutSetError::MissingWorkload {
                workload_id: String::from(workload_id),
            });
        }
    }

    let workload_rows = CLOSEOUT_WORKLOAD_IDS
        .iter()
        .map(|workload_id| {
            let detail = format!(
                "Closeout workload `{}` stays frozen for both promotion review and local-cluster validation on the admitted executor lane.",
                workload_id
            );
            let mut row = PsionExecutorArticleCloseoutWorkloadRow {
                workload_id: String::from(*workload_id),
                workload_family: String::from("bounded_article_executor"),
                promotion_pack_id: String::from("tassadar.eval.promotion.v0"),
                frequent_pack_id: String::from("tassadar.eval.frequent.v0"),
                required_for_promotion: true,
                required_for_local_cluster_validation: true,
                local_cluster_visibility_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
                promotion_visibility_ref: String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
                detail,
                row_digest: String::new(),
            };
            row.row_digest = stable_workload_row_digest(&row);
            row
        })
        .collect::<Vec<_>>();

    let mut packet = PsionExecutorArticleCloseoutSetPacket {
        schema_version: String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline.record_digest,
        eval_pack_catalog_ref: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
        eval_pack_catalog_digest: catalog.catalog_digest,
        workload_rows,
        promotion_validation_refs: vec![
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
        ],
        local_cluster_validation_refs: vec![
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_DOC_PATH),
        ],
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now freezes one bounded article-workload closeout set across promotion review and local-cluster validation: long_loop_kernel, sudoku_v0_test_a, and hungarian_matching.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    Ok(packet)
}

pub fn write_builtin_executor_article_closeout_set_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorArticleCloseoutSetPacket, PsionExecutorArticleCloseoutSetError> {
    let packet = builtin_executor_article_closeout_set_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn collect_exactness_workloads(catalog: &PsionExecutorEvalPackCatalog) -> BTreeSet<String> {
    catalog
        .packs
        .iter()
        .flat_map(|pack| pack.suite_refs.iter())
        .filter(|suite| {
            suite.suite_id == "frequent_exactness_cases_v0"
                || suite.suite_id == "promotion_exactness_suite_v0"
        })
        .flat_map(|suite| suite.case_ids.iter().cloned())
        .collect()
}

fn collect_baseline_workloads(
    baseline: &crate::PsionExecutorBaselineTruthRecord,
) -> BTreeSet<String> {
    baseline
        .suite_truths
        .iter()
        .filter(|suite| {
            suite.suite_id == "frequent_exactness_cases_v0"
                || suite.suite_id == "promotion_exactness_suite_v0"
        })
        .flat_map(|suite| suite.case_rows.iter())
        .map(|row| row.case_id.clone())
        .collect()
}

fn stable_workload_row_digest(row: &PsionExecutorArticleCloseoutWorkloadRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_digest(b"psion_executor_article_closeout_workload_row|", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorArticleCloseoutSetPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_digest(b"psion_executor_article_closeout_set_packet|", &clone)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec_pretty(value).expect("serialize packet"));
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorArticleCloseoutSetError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorArticleCloseoutSetError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorArticleCloseoutSetError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorArticleCloseoutSetError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorArticleCloseoutSetError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorArticleCloseoutSetError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorArticleCloseoutSetError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorArticleCloseoutSetError::Write {
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
    fn builtin_executor_article_closeout_set_packet_is_valid(
    ) -> Result<(), PsionExecutorArticleCloseoutSetError> {
        let root = workspace_root();
        let packet = builtin_executor_article_closeout_set_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_article_closeout_set_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorArticleCloseoutSetError> {
        let root = workspace_root();
        let expected: PsionExecutorArticleCloseoutSetPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_article_closeout_set_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorArticleCloseoutSetError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_executor_article_closeout_set_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorArticleCloseoutSetError> {
        let root = workspace_root();
        let packet = write_builtin_executor_article_closeout_set_packet(root.as_path())?;
        let persisted: PsionExecutorArticleCloseoutSetPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
        )?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
