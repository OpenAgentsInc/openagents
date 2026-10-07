use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_batch_accumulation_ablation_packet,
    builtin_executor_optimizer_ablation_packet,
    builtin_executor_scheduler_ablation_packet,
    builtin_executor_supervision_density_ablation_packet,
    builtin_executor_trace_family_weighting_ablation_packet,
    PsionExecutorBatchAccumulationAblationError, PsionExecutorOptimizerAblationError,
    PsionExecutorSchedulerAblationError, PsionExecutorSupervisionDensityAblationError,
    PsionExecutorTraceFamilyWeightingAblationError, PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_DOC_PATH,
    PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH,
    PSION_EXECUTOR_OPTIMIZER_ABLATION_DOC_PATH,
    PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
    PSION_EXECUTOR_SCHEDULER_ABLATION_DOC_PATH,
    PSION_EXECUTOR_SCHEDULER_ABLATION_FIXTURE_PATH,
    PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_DOC_PATH,
    PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
    PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_DOC_PATH,
    PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_SCHEMA_VERSION: &str =
    "psion.executor.tokenizer_architecture_gate.v1";
pub const PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_tokenizer_architecture_gate_v1.json";
pub const PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE.md";

const PACKET_ID: &str = "psion_executor_tokenizer_architecture_gate_v1";
const REQUIRED_SUCCESSFUL_ABLATION_RUNS: u32 = 5;
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";

#[derive(Debug, Error)]
pub enum PsionExecutorTokenizerArchitectureGateError {
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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Optimizer(#[from] PsionExecutorOptimizerAblationError),
    #[error(transparent)]
    Scheduler(#[from] PsionExecutorSchedulerAblationError),
    #[error(transparent)]
    BatchAccumulation(#[from] PsionExecutorBatchAccumulationAblationError),
    #[error(transparent)]
    TraceWeighting(#[from] PsionExecutorTraceFamilyWeightingAblationError),
    #[error(transparent)]
    SupervisionDensity(#[from] PsionExecutorSupervisionDensityAblationError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorGateEvidenceRow {
    pub issue_id: String,
    pub ablation_id: String,
    pub evidence_ref: String,
    pub evidence_digest: String,
    pub success_class: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTokenizerArchitectureGatePacket {
    pub schema_version: String,
    pub packet_id: String,
    pub required_successful_ablation_runs: u32,
    pub successful_same_baseline_run_count: u32,
    pub evidence_rows: Vec<PsionExecutorGateEvidenceRow>,
    pub compression_limit_evidence_present: bool,
    pub fit_limit_evidence_present: bool,
    pub tokenizer_issue_open_allowed: bool,
    pub architecture_issue_open_allowed: bool,
    pub tokenizer_block_reason: String,
    pub architecture_gate_reason: String,
    pub review_decision: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorGateEvidenceRow {
    fn validate(&self) -> Result<(), PsionExecutorTokenizerArchitectureGateError> {
        for (field, value) in [
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].issue_id",
                self.issue_id.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].ablation_id",
                self.ablation_id.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].evidence_ref",
                self.evidence_ref.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].evidence_digest",
                self.evidence_digest.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].success_class",
                self.success_class.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.evidence_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.success_class != "successful_same_baseline_run" {
            return Err(PsionExecutorTokenizerArchitectureGateError::InvalidValue {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.evidence_rows[].success_class",
                ),
                detail: String::from(
                    "gate evidence rows must stay tied to successful same-baseline runs",
                ),
            });
        }
        if stable_evidence_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTokenizerArchitectureGateError::DigestMismatch {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.evidence_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTokenizerArchitectureGatePacket {
    pub fn validate(&self) -> Result<(), PsionExecutorTokenizerArchitectureGateError> {
        if self.schema_version != PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_SCHEMA_VERSION {
            return Err(
                PsionExecutorTokenizerArchitectureGateError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_tokenizer_architecture_gate.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.tokenizer_block_reason",
                self.tokenizer_block_reason.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.architecture_gate_reason",
                self.architecture_gate_reason.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.review_decision",
                self.review_decision.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_tokenizer_architecture_gate.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.required_successful_ablation_runs != REQUIRED_SUCCESSFUL_ABLATION_RUNS
            || self.evidence_rows.len() != REQUIRED_SUCCESSFUL_ABLATION_RUNS as usize
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutorTokenizerArchitectureGateError::MissingField {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.required_rows",
                ),
            });
        }
        let mut seen_issues = BTreeSet::new();
        for row in &self.evidence_rows {
            row.validate()?;
            if !seen_issues.insert(row.issue_id.clone()) {
                return Err(PsionExecutorTokenizerArchitectureGateError::InvalidValue {
                    field: String::from(
                        "psion_executor_tokenizer_architecture_gate.evidence_rows",
                    ),
                    detail: String::from("each successful ablation issue must appear once"),
                });
            }
        }
        if self.successful_same_baseline_run_count != self.evidence_rows.len() as u32 {
            return Err(PsionExecutorTokenizerArchitectureGateError::InvalidValue {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.successful_same_baseline_run_count",
                ),
                detail: String::from("successful ablation count must match the evidence rows"),
            });
        }
        if self.tokenizer_issue_open_allowed
            != (self.compression_limit_evidence_present || self.fit_limit_evidence_present)
        {
            return Err(PsionExecutorTokenizerArchitectureGateError::InvalidValue {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.tokenizer_issue_open_allowed",
                ),
                detail: String::from(
                    "tokenizer work only opens when compression or fit limits are real",
                ),
            });
        }
        if self.architecture_issue_open_allowed
            != (self.successful_same_baseline_run_count >= self.required_successful_ablation_runs)
        {
            return Err(PsionExecutorTokenizerArchitectureGateError::InvalidValue {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.architecture_issue_open_allowed",
                ),
                detail: String::from(
                    "architecture work only opens after the full successful ablation tranche exists",
                ),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorTokenizerArchitectureGateError::DigestMismatch {
                field: String::from(
                    "psion_executor_tokenizer_architecture_gate.packet_digest",
                ),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_tokenizer_architecture_gate_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorTokenizerArchitectureGatePacket, PsionExecutorTokenizerArchitectureGateError>
{
    let optimizer = builtin_executor_optimizer_ablation_packet(workspace_root)?;
    let scheduler = builtin_executor_scheduler_ablation_packet(workspace_root)?;
    let batch = builtin_executor_batch_accumulation_ablation_packet(workspace_root)?;
    let trace = builtin_executor_trace_family_weighting_ablation_packet(workspace_root)?;
    let supervision = builtin_executor_supervision_density_ablation_packet(workspace_root)?;

    let evidence_rows = vec![
        build_evidence_row(
            "#776",
            "psion_executor_optimizer_ablation_v1",
            PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
            optimizer.packet_digest,
            "The optimizer ablation is retained as a successful same-baseline run because it repeated cleanly, stayed outside the frozen noise band, and kept zero regressions.",
        )?,
        build_evidence_row(
            "#777",
            "psion_executor_scheduler_ablation_v1",
            PSION_EXECUTOR_SCHEDULER_ABLATION_FIXTURE_PATH,
            scheduler.packet_digest,
            "The scheduler ablation counts as a successful same-baseline run because it stayed baseline-comparable, reviewable, and regression-free even though it remained below the promotion-noise threshold.",
        )?,
        build_evidence_row(
            "#778",
            "psion_executor_batch_accumulation_ablation_v1",
            PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_FIXTURE_PATH,
            batch.packet_digest,
            "The batch / accumulation ablation counts as a successful same-baseline run because it kept effective batch fixed, made the memory tradeoff explicit, and kept zero regressions.",
        )?,
        build_evidence_row(
            "#779",
            "psion_executor_trace_family_weighting_ablation_v1",
            PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
            trace.packet_digest,
            "The trace-family weighting ablation counts as a successful same-baseline run because the held-out rollback guard stayed inactive while the mixture shift remained inside one lever class.",
        )?,
        build_evidence_row(
            "#780",
            "psion_executor_supervision_density_ablation_v1",
            PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
            supervision.packet_digest,
            "The supervision-density ablation counts as a successful same-baseline run because exactness, held-out, throughput, and stability all stayed green together.",
        )?,
    ];

    let mut packet = PsionExecutorTokenizerArchitectureGatePacket {
        schema_version: String::from(PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        required_successful_ablation_runs: REQUIRED_SUCCESSFUL_ABLATION_RUNS,
        successful_same_baseline_run_count: REQUIRED_SUCCESSFUL_ABLATION_RUNS,
        evidence_rows,
        compression_limit_evidence_present: false,
        fit_limit_evidence_present: false,
        tokenizer_issue_open_allowed: false,
        architecture_issue_open_allowed: true,
        tokenizer_block_reason: String::from(
            "No retained executor packet shows a real compression or fit limit yet, so tokenizer work remains blocked behind evidence.",
        ),
        architecture_gate_reason: String::from(
            "Five successful same-baseline ablations now exist, so architecture work may open if it becomes necessary after the current trained-v1 candidate path is exhausted.",
        ),
        review_decision: String::from(
            "keep_tokenizer_blocked_allow_architecture_only_after_ablation_tranche",
        ),
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_OPTIMIZER_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_SCHEDULER_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_BATCH_ACCUMULATION_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one retained tokenizer/architecture evidence-gate packet. Tokenizer work stays blocked because no compression or fit limit is real yet, while architecture work becomes eligible only because the full five-run same-baseline ablation tranche now exists.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_tokenizer_architecture_gate_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorTokenizerArchitectureGatePacket, PsionExecutorTokenizerArchitectureGateError>
{
    let packet = builtin_executor_tokenizer_architecture_gate_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_evidence_row(
    issue_id: &str,
    ablation_id: &str,
    evidence_ref: &str,
    evidence_digest: String,
    detail: &str,
) -> Result<PsionExecutorGateEvidenceRow, PsionExecutorTokenizerArchitectureGateError> {
    let mut row = PsionExecutorGateEvidenceRow {
        issue_id: String::from(issue_id),
        ablation_id: String::from(ablation_id),
        evidence_ref: String::from(evidence_ref),
        evidence_digest,
        success_class: String::from("successful_same_baseline_run"),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_evidence_row_digest(&row);
    Ok(row)
}

fn stable_evidence_row_digest(row: &PsionExecutorGateEvidenceRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_tokenizer_architecture_gate_evidence_row", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorTokenizerArchitectureGatePacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_tokenizer_architecture_gate_packet", &clone)
}

fn stable_json_digest<T: Serialize>(label: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).expect("stable json"));
    hex::encode(hasher.finalize())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorTokenizerArchitectureGateError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorTokenizerArchitectureGateError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| {
        PsionExecutorTokenizerArchitectureGateError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorTokenizerArchitectureGateError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorTokenizerArchitectureGateError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorTokenizerArchitectureGateError::Parse {
            path: path.display().to_string(),
            error,
        }
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorTokenizerArchitectureGateError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorTokenizerArchitectureGateError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_tokenizer_architecture_gate_packet_is_valid(
    ) -> Result<(), PsionExecutorTokenizerArchitectureGateError> {
        let root = workspace_root();
        let packet = builtin_executor_tokenizer_architecture_gate_packet(root.as_path())?;
        packet.validate()?;
        assert_eq!(packet.successful_same_baseline_run_count, 5);
        assert!(!packet.tokenizer_issue_open_allowed);
        assert!(packet.architecture_issue_open_allowed);
        Ok(())
    }

    #[test]
    fn tokenizer_architecture_gate_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorTokenizerArchitectureGateError> {
        let root = workspace_root();
        let expected: PsionExecutorTokenizerArchitectureGatePacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_tokenizer_architecture_gate_packet(root.as_path())?;
        if expected.packet_digest != actual.packet_digest {
            return Err(PsionExecutorTokenizerArchitectureGateError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH),
            });
        }
        Ok(())
    }
}
