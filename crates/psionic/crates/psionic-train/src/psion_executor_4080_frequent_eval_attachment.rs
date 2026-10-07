use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutor4080DurableCheckpointPacket,
    PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_DOC_PATH,
    PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
};

/// Stable schema version for the admitted 4080 frequent-eval attachment packet.
pub const PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_SCHEMA_VERSION: &str =
    "psion.executor.4080_frequent_eval_attachment.v1";
/// Canonical fixture path for the admitted 4080 frequent-eval attachment packet.
pub const PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_4080_frequent_eval_attachment_v1.json";
/// Canonical doc path for the admitted 4080 frequent-eval attachment packet.
pub const PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT.md";

const LOCAL_4080_PROFILE_ID: &str = "local_4080_cuda_tailnet_x86_64";
const LOCAL_TAILNET_CONTROL_PROFILE_ID: &str = "local_tailnet_cluster_control_plane";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_eval_packs_v1.json";
const TAILNET_RUN_BUNDLE_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260328k/first_swarm_real_run_bundle.json";
const FREQUENT_PACK_ID: &str = "tassadar.eval.frequent.v0";
const EXPECTED_REQUIRED_OPERATOR_CASE_IDS: &[&str] = &[
    "artifact_packet_complete",
    "checkpoint_restore_rehearsal_green",
    "export_smoke_green",
    "local_cluster_roundtrip_green",
];

#[derive(Clone, Debug, Deserialize)]
struct PsionExecutorEvalPackCatalog {
    catalog_digest: String,
    packs: Vec<PsionExecutorEvalPack>,
}

#[derive(Clone, Debug, Deserialize)]
struct PsionExecutorEvalPack {
    pack_id: String,
    admitted_profile_ids: Vec<String>,
    suite_refs: Vec<PsionExecutorEvalSuiteRef>,
    pack_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PsionExecutorEvalSuiteRef {
    suite_id: String,
    required_for_green: bool,
    case_ids: Vec<String>,
}

/// One suite-level result retained by the checkpoint eval ledger row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080FrequentEvalSuiteResult {
    /// Stable suite id.
    pub suite_id: String,
    /// Final suite status.
    pub status: String,
    /// Whether this suite is required for a green frequent-pack decision.
    pub required_for_green: bool,
    /// Honest detail.
    pub detail: String,
}

/// One case-level result retained by the checkpoint eval ledger row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutor4080FrequentEvalCaseResult {
    /// Parent suite id.
    pub suite_id: String,
    /// Stable case id.
    pub case_id: String,
    /// Final case status.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// One per-checkpoint ledger row for the admitted 4080 frequent-pack attachment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080CheckpointEvalLedgerRow {
    /// Stable ledger row id.
    pub ledger_row_id: String,
    /// Pack id attached to the checkpoint.
    pub pack_id: String,
    /// Checkpoint family under review.
    pub checkpoint_family: String,
    /// Checkpoint pointer digest under review.
    pub checkpoint_pointer_digest: String,
    /// Stable checkpoint ref under review.
    pub checkpoint_ref: String,
    /// Checkpoint step under review.
    pub checkpoint_step: u64,
    /// Suite-level result rows.
    pub suite_results: Vec<PsionExecutor4080FrequentEvalSuiteResult>,
    /// Case-level result rows.
    pub case_results: Vec<PsionExecutor4080FrequentEvalCaseResult>,
    /// Hard blockers that keep promotion closed when frequent-pack coverage is missing.
    pub promotion_blocker_ids: Vec<String>,
    /// Whether promotion is blocked due to missing or unscored frequent-pack coverage.
    pub promotion_blocked: bool,
    /// Stable ledger row digest.
    pub ledger_row_digest: String,
}

/// Typed packet recording automatic checkpoint-time frequent-pack attachment for 4080 runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutor4080FrequentEvalAttachmentPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted 4080 worker profile id.
    pub worker_profile_id: String,
    /// Admitted Tailnet control-plane profile id.
    pub control_plane_profile_id: String,
    /// Prerequisite durable-checkpoint packet reference.
    pub durable_checkpoint_packet_ref: String,
    /// Stable SHA256 over the durable-checkpoint packet bytes.
    pub durable_checkpoint_packet_sha256: String,
    /// Frozen eval-pack catalog reference.
    pub eval_pack_catalog_ref: String,
    /// Stable SHA256 over the eval-pack catalog bytes.
    pub eval_pack_catalog_sha256: String,
    /// Stable catalog digest embedded by the eval-pack catalog.
    pub eval_pack_catalog_digest: String,
    /// Stable digest embedded by the frequent pack.
    pub frequent_pack_digest: String,
    /// Retained run bundle reference.
    pub retained_run_bundle_ref: String,
    /// Stable SHA256 over the retained run bundle bytes.
    pub retained_run_bundle_sha256: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable pack id.
    pub pack_id: String,
    /// Whether missing frequent-pack coverage blocks later promotion.
    pub missing_eval_blocks_promotion: bool,
    /// Honest blocker summary.
    pub promotion_blocker_summary: String,
    /// One per-checkpoint ledger row for the retained admitted rerun.
    pub checkpoint_eval_row: PsionExecutor4080CheckpointEvalLedgerRow,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutor4080CheckpointEvalLedgerRow {
    fn validate(&self) -> Result<(), PsionExecutor4080FrequentEvalAttachmentError> {
        for (field, value) in [
            (
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.ledger_row_id",
                self.ledger_row_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.pack_id",
                self.pack_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.checkpoint_family",
                self.checkpoint_family.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.checkpoint_pointer_digest",
                self.checkpoint_pointer_digest.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.checkpoint_ref",
                self.checkpoint_ref.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.ledger_row_digest",
                self.ledger_row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.suite_results.is_empty() {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::MissingField {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.suite_results",
                ),
            });
        }
        if self.case_results.is_empty() {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::MissingField {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.case_results",
                ),
            });
        }
        for row in &self.suite_results {
            ensure_nonempty(
                row.suite_id.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.suite_results[].suite_id",
            )?;
            ensure_nonempty(
                row.status.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.suite_results[].status",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.suite_results[].detail",
            )?;
        }
        for row in &self.case_results {
            ensure_nonempty(
                row.suite_id.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.case_results[].suite_id",
            )?;
            ensure_nonempty(
                row.case_id.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.case_results[].case_id",
            )?;
            ensure_nonempty(
                row.status.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.case_results[].status",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.case_results[].detail",
            )?;
        }
        if self.ledger_row_digest != stable_checkpoint_eval_row_digest(self) {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::DigestMismatch {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.checkpoint_eval_row.ledger_row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutor4080FrequentEvalAttachmentPacket {
    /// Validate the retained frequent-eval attachment packet.
    pub fn validate(&self) -> Result<(), PsionExecutor4080FrequentEvalAttachmentError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_4080_frequent_eval_attachment.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_SCHEMA_VERSION {
            return Err(
                PsionExecutor4080FrequentEvalAttachmentError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_4080_frequent_eval_attachment.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.worker_profile_id",
                self.worker_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.control_plane_profile_id",
                self.control_plane_profile_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.durable_checkpoint_packet_ref",
                self.durable_checkpoint_packet_ref.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.durable_checkpoint_packet_sha256",
                self.durable_checkpoint_packet_sha256.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.eval_pack_catalog_ref",
                self.eval_pack_catalog_ref.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.eval_pack_catalog_sha256",
                self.eval_pack_catalog_sha256.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.eval_pack_catalog_digest",
                self.eval_pack_catalog_digest.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.frequent_pack_digest",
                self.frequent_pack_digest.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.retained_run_bundle_ref",
                self.retained_run_bundle_ref.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.retained_run_bundle_sha256",
                self.retained_run_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.pack_id",
                self.pack_id.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.promotion_blocker_summary",
                self.promotion_blocker_summary.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_4080_frequent_eval_attachment.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.worker_profile_id != LOCAL_4080_PROFILE_ID {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.worker_profile_id",
                ),
                detail: String::from("worker profile id drifted"),
            });
        }
        if self.control_plane_profile_id != LOCAL_TAILNET_CONTROL_PROFILE_ID {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.control_plane_profile_id",
                ),
                detail: String::from("control-plane profile id drifted"),
            });
        }
        if !self.missing_eval_blocks_promotion {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::InvalidValue {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.missing_eval_blocks_promotion",
                ),
                detail: String::from("missing eval must stay a hard promotion blocker"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::MissingField {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.support_refs",
                ),
            });
        }
        self.checkpoint_eval_row.validate()?;
        if self.packet_digest != stable_frequent_eval_attachment_packet_digest(self) {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::DigestMismatch {
                field: String::from(
                    "psion_executor_4080_frequent_eval_attachment.packet_digest",
                ),
            });
        }
        Ok(())
    }
}

/// Errors emitted by the retained 4080 frequent-eval attachment packet.
#[derive(Debug, Error)]
pub enum PsionExecutor4080FrequentEvalAttachmentError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("frozen eval pack `{pack_id}` missing suite `{suite_id}`")]
    MissingSuite { pack_id: String, suite_id: String },
    #[error("frozen eval suite `{suite_id}` missing case `{case_id}`")]
    MissingCase { suite_id: String, case_id: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        #[source]
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to encode packet: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Build the retained 4080 frequent-eval attachment packet.
pub fn builtin_executor_4080_frequent_eval_attachment_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080FrequentEvalAttachmentPacket, PsionExecutor4080FrequentEvalAttachmentError>
{
    let durable_packet_path =
        workspace_root.join(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH);
    let durable_packet_bytes = fs::read(&durable_packet_path).map_err(|error| {
        PsionExecutor4080FrequentEvalAttachmentError::Read {
            path: durable_packet_path.display().to_string(),
            error,
        }
    })?;
    let durable_packet: PsionExecutor4080DurableCheckpointPacket =
        serde_json::from_slice(&durable_packet_bytes).map_err(|error| {
            PsionExecutor4080FrequentEvalAttachmentError::Decode {
                path: durable_packet_path.display().to_string(),
                error,
            }
        })?;
    let eval_pack_path = workspace_root.join(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH);
    let eval_pack_bytes = fs::read(&eval_pack_path).map_err(|error| {
        PsionExecutor4080FrequentEvalAttachmentError::Read {
            path: eval_pack_path.display().to_string(),
            error,
        }
    })?;
    let eval_catalog: PsionExecutorEvalPackCatalog = serde_json::from_slice(&eval_pack_bytes)
        .map_err(|error| PsionExecutor4080FrequentEvalAttachmentError::Decode {
            path: eval_pack_path.display().to_string(),
            error,
        })?;
    let run_bundle_path = workspace_root.join(TAILNET_RUN_BUNDLE_PATH);
    let run_bundle_bytes = fs::read(&run_bundle_path).map_err(|error| {
        PsionExecutor4080FrequentEvalAttachmentError::Read {
            path: run_bundle_path.display().to_string(),
            error,
        }
    })?;

    let frequent_pack = eval_catalog
        .packs
        .iter()
        .find(|pack| pack.pack_id == FREQUENT_PACK_ID)
        .ok_or_else(|| PsionExecutor4080FrequentEvalAttachmentError::MissingSuite {
            pack_id: String::from(FREQUENT_PACK_ID),
            suite_id: String::from("<pack>"),
        })?;
    if !frequent_pack
        .admitted_profile_ids
        .iter()
        .any(|profile_id| profile_id == LOCAL_4080_PROFILE_ID)
    {
        return Err(PsionExecutor4080FrequentEvalAttachmentError::InvalidValue {
            field: String::from(
                "psion_executor_4080_frequent_eval_attachment.worker_profile_id",
            ),
            detail: String::from("frequent pack drifted off the admitted 4080 worker profile"),
        });
    }
    let operator_suite = frequent_pack
        .suite_refs
        .iter()
        .find(|suite| suite.suite_id == "frequent_operator_review_cases_v0")
        .ok_or_else(|| PsionExecutor4080FrequentEvalAttachmentError::MissingSuite {
            pack_id: String::from(FREQUENT_PACK_ID),
            suite_id: String::from("frequent_operator_review_cases_v0"),
        })?;
    for case_id in EXPECTED_REQUIRED_OPERATOR_CASE_IDS {
        if !operator_suite.case_ids.iter().any(|candidate| candidate == case_id) {
            return Err(PsionExecutor4080FrequentEvalAttachmentError::MissingCase {
                suite_id: String::from("frequent_operator_review_cases_v0"),
                case_id: String::from(*case_id),
            });
        }
    }
    let suite_required = |suite_id: &str| -> Result<bool, PsionExecutor4080FrequentEvalAttachmentError> {
        frequent_pack
            .suite_refs
            .iter()
            .find(|suite| suite.suite_id == suite_id)
            .map(|suite| suite.required_for_green)
            .ok_or_else(|| PsionExecutor4080FrequentEvalAttachmentError::MissingSuite {
                pack_id: String::from(FREQUENT_PACK_ID),
                suite_id: String::from(suite_id),
            })
    };

    let suite_results = vec![
        PsionExecutor4080FrequentEvalSuiteResult {
            suite_id: String::from("frequent_exactness_cases_v0"),
            status: String::from("blocked_missing_executor_outputs"),
            required_for_green: suite_required("frequent_exactness_cases_v0")?,
            detail: String::from(
                "The retained 2026-03-28 admitted rerun is an open-adapter infrastructure run, not an executor candidate output run, so exactness cases stay explicitly unscored and therefore block promotion instead of disappearing from review.",
            ),
        },
        PsionExecutor4080FrequentEvalSuiteResult {
            suite_id: String::from("frequent_held_out_exclusions_v0"),
            status: String::from("blocked_missing_executor_outputs"),
            required_for_green: suite_required("frequent_held_out_exclusions_v0")?,
            detail: String::from(
                "Held-out exclusion review stays attached to the checkpoint ledger row, but this retained infrastructure run does not emit executor outputs that could honestly score the held-out suite.",
            ),
        },
        PsionExecutor4080FrequentEvalSuiteResult {
            suite_id: String::from("frequent_operator_review_cases_v0"),
            status: String::from("green"),
            required_for_green: suite_required("frequent_operator_review_cases_v0")?,
            detail: String::from(
                "The admitted rerun can already score the operator-review slice because artifact completeness, checkpoint-path readback, export smoke, and Mac -> 4080 -> Mac roundtrip evidence are all retained explicitly.",
            ),
        },
        PsionExecutor4080FrequentEvalSuiteResult {
            suite_id: String::from("frequent_throughput_blockers_v0"),
            status: String::from("blocked_missing_executor_metrics"),
            required_for_green: suite_required("frequent_throughput_blockers_v0")?,
            detail: String::from(
                "Throughput blockers remain attached as required review surfaces, but the retained open-adapter infrastructure run does not emit the admitted executor fast-route metrics and therefore stays non-promotable until later decision-grade packets score them honestly.",
            ),
        },
    ];

    let case_results = vec![
        PsionExecutor4080FrequentEvalCaseResult {
            suite_id: String::from("frequent_operator_review_cases_v0"),
            case_id: String::from("artifact_packet_complete"),
            status: String::from("green"),
            detail: String::from(
                "The admitted 4080 lane now keeps the remote-launch packet, durable-checkpoint packet, retained rerun bundle, and merged portable bundle together as one reviewable artifact packet.",
            ),
        },
        PsionExecutor4080FrequentEvalCaseResult {
            suite_id: String::from("frequent_operator_review_cases_v0"),
            case_id: String::from("checkpoint_restore_rehearsal_green"),
            status: String::from("green"),
            detail: String::from(
                "The durable-checkpoint packet already proves explicit pointer receipts plus control-plane portable-bundle readback on the retained rerun; the later live interruption rehearsal stays separate in `PSION-0305`.",
            ),
        },
        PsionExecutor4080FrequentEvalCaseResult {
            suite_id: String::from("frequent_operator_review_cases_v0"),
            case_id: String::from("export_smoke_green"),
            status: String::from("green"),
            detail: String::from(
                "The merged portable bundle imports cleanly through the shipped model-IO surface, so export smoke is already explicit on the retained rerun.",
            ),
        },
        PsionExecutor4080FrequentEvalCaseResult {
            suite_id: String::from("frequent_operator_review_cases_v0"),
            case_id: String::from("local_cluster_roundtrip_green"),
            status: String::from("green"),
            detail: String::from(
                "The retained rerun already completed the Mac -> 4080 -> Mac roundtrip and returned the merged artifacts to the controller-owned bundle root.",
            ),
        },
    ];

    let mut checkpoint_eval_row = PsionExecutor4080CheckpointEvalLedgerRow {
        ledger_row_id: format!(
            "psion.executor.4080.frequent_eval_row:{}:{}",
            durable_packet.run_id, durable_packet.checkpoint_pointer_digest
        ),
        pack_id: String::from(FREQUENT_PACK_ID),
        checkpoint_family: durable_packet.checkpoint_family.clone(),
        checkpoint_pointer_digest: durable_packet.checkpoint_pointer_digest.clone(),
        checkpoint_ref: durable_packet.checkpoint_ref.clone(),
        checkpoint_step: durable_packet.checkpoint_step,
        suite_results,
        case_results,
        promotion_blocker_ids: vec![
            String::from("frequent_exactness_cases_v0"),
            String::from("frequent_held_out_exclusions_v0"),
            String::from("frequent_throughput_blockers_v0"),
        ],
        promotion_blocked: true,
        ledger_row_digest: String::new(),
    };
    checkpoint_eval_row.ledger_row_digest = stable_checkpoint_eval_row_digest(&checkpoint_eval_row);
    checkpoint_eval_row.validate()?;

    let mut packet = PsionExecutor4080FrequentEvalAttachmentPacket {
        schema_version: String::from(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_4080_frequent_eval_attachment_v1"),
        worker_profile_id: String::from(LOCAL_4080_PROFILE_ID),
        control_plane_profile_id: String::from(LOCAL_TAILNET_CONTROL_PROFILE_ID),
        durable_checkpoint_packet_ref: String::from(
            PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_FIXTURE_PATH,
        ),
        durable_checkpoint_packet_sha256: hex::encode(Sha256::digest(&durable_packet_bytes)),
        eval_pack_catalog_ref: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
        eval_pack_catalog_sha256: hex::encode(Sha256::digest(&eval_pack_bytes)),
        eval_pack_catalog_digest: eval_catalog.catalog_digest,
        frequent_pack_digest: frequent_pack.pack_digest.clone(),
        retained_run_bundle_ref: String::from(TAILNET_RUN_BUNDLE_PATH),
        retained_run_bundle_sha256: hex::encode(Sha256::digest(&run_bundle_bytes)),
        run_id: durable_packet.run_id.clone(),
        pack_id: String::from(FREQUENT_PACK_ID),
        missing_eval_blocks_promotion: true,
        promotion_blocker_summary: String::from(
            "Any admitted 4080 checkpoint without an attached frequent-pack ledger row is non-promotable. On the retained 2026-03-28 rerun, exactness, held-out, and throughput suites stay explicitly unscored and therefore continue blocking promotion even though the operator-review suite is green.",
        ),
        checkpoint_eval_row,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DURABLE_CHECKPOINT_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
            String::from(TAILNET_RUN_BUNDLE_PATH),
        ],
        summary: format!(
            "The admitted 4080 lane now has one automatic frequent-pack attachment packet. The retained rerun at checkpoint pointer digest `{}` stores one per-checkpoint ledger row keyed to `{}`, keeps the operator-review suite green, and makes missing or unscored frequent-pack coverage a hard promotion blocker instead of a silent omission.",
            durable_packet.checkpoint_pointer_digest,
            FREQUENT_PACK_ID,
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_frequent_eval_attachment_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained 4080 frequent-eval attachment packet.
pub fn write_builtin_executor_4080_frequent_eval_attachment_packet(
    workspace_root: &Path,
) -> Result<PsionExecutor4080FrequentEvalAttachmentPacket, PsionExecutor4080FrequentEvalAttachmentError>
{
    let packet = builtin_executor_4080_frequent_eval_attachment_packet(workspace_root)?;
    let path = workspace_root.join(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutor4080FrequentEvalAttachmentError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutor4080FrequentEvalAttachmentError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn stable_checkpoint_eval_row_digest(row: &PsionExecutor4080CheckpointEvalLedgerRow) -> String {
    let mut canonical = row.clone();
    canonical.ledger_row_digest.clear();
    stable_digest(b"psion_executor_4080_checkpoint_eval_row|", &canonical)
}

fn stable_frequent_eval_attachment_packet_digest(
    packet: &PsionExecutor4080FrequentEvalAttachmentPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_4080_frequent_eval_attachment|", &canonical)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutor4080FrequentEvalAttachmentError> {
    if value.trim().is_empty() {
        return Err(PsionExecutor4080FrequentEvalAttachmentError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_executor_4080_frequent_eval_attachment_packet_is_valid() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet =
            builtin_executor_4080_frequent_eval_attachment_packet(workspace_root.as_path())
                .expect("build frequent eval attachment packet");
        packet
            .validate()
            .expect("validate frequent eval attachment packet");
        assert_eq!(packet.run_id, "tailrun-home-admitted-20260328k");
        assert_eq!(packet.pack_id, FREQUENT_PACK_ID);
        assert!(packet.missing_eval_blocks_promotion);
        assert_eq!(packet.checkpoint_eval_row.case_results.len(), 4);
    }

    #[test]
    fn executor_4080_frequent_eval_attachment_fixture_matches_committed_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let generated =
            builtin_executor_4080_frequent_eval_attachment_packet(workspace_root.as_path())
                .expect("build frequent eval attachment packet");
        let fixture_path =
            workspace_root.join(PSION_EXECUTOR_4080_FREQUENT_EVAL_ATTACHMENT_FIXTURE_PATH);
        let fixture_bytes = fs::read(&fixture_path).expect("read frequent eval attachment fixture");
        let committed: PsionExecutor4080FrequentEvalAttachmentPacket =
            serde_json::from_slice(&fixture_bytes)
                .expect("decode frequent eval attachment fixture");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_executor_4080_frequent_eval_attachment_packet_persists_current_truth() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let packet =
            write_builtin_executor_4080_frequent_eval_attachment_packet(workspace_root.as_path())
                .expect("write frequent eval attachment packet");
        packet
            .validate()
            .expect("validate written frequent eval attachment packet");
    }
}
