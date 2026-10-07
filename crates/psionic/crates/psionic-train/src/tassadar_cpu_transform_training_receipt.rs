//! Tassadar Percepta CPU-transform training receipt machinery.
//!
//! This module owns the Psionic-side execution-evidence boundary for the
//! `models.tassadar_percepta_executor.v1` product promise (EPIC DE-5, openagents
//! issue #5528). It runs a real, deterministic, CPU-only Tassadar executor
//! transformer training rehearsal over a frozen verified-trace corpus, replays
//! the selected checkpoint against CPU-reference truth, and emits one
//! dereferenceable receipt packet that records:
//!
//! - the frozen training manifest digest and dataset identity (verified work,
//!   not a claim);
//! - the trained model descriptor + weight digests (the trained-artifact
//!   digest);
//! - the exact-replay verifier verdict (per-case `exact_trace_match`,
//!   `final_output_match`, `halt_match`, first-divergence, and reference vs
//!   predicted output digests);
//! - the explicit gate state, separating the locally provable gates
//!   (training-completed, exact-replay-verified, trained-artifact-digest) from
//!   the compute/owner-gated gates that this CPU rehearsal deliberately does
//!   NOT satisfy (real Pylon assignment, real settlement where money moved, and
//!   the public green transition).
//!
//! The receipt is the dereferenceable proof shape. It does not claim a trained
//! product model, accepted Pylon work, real settlement, model promotion,
//! inference, CPU replacement, or a green product promise. Those remain the
//! explicit owner/compute gate.

use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    train_tassadar_executor_transformer, TassadarExecutorTrainingConfig,
    TassadarExecutorTrainingError, TassadarExecutorTrainingReport,
};

/// Stable schema version for the CPU-transform training receipt packet.
pub const TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_SCHEMA_VERSION: &str =
    "openagents.models.tassadar_percepta_executor.cpu_transform_training_receipt.v1";

/// Committed fixture path for the canonical CPU-transform training receipt.
pub const TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/tassadar/operator/tassadar_percepta_cpu_transform_training_receipt_v1.json";

/// Repo-local documentation path for this receipt lane.
pub const TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_DOC_PATH: &str =
    "docs/TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT.md";

/// Stable assignment reference for the local CPU-transform rehearsal.
///
/// This is a deterministic *local rehearsal* assignment reference, not a real
/// Pylon assignment record. It is shaped to slot into the openagents public
/// projection's
/// `receipt.models.tassadar_percepta_executor.cpu_transform_training.{assignmentRef}`
/// pattern while staying explicitly local-proof.
pub const TASSADAR_CPU_TRANSFORM_TRAINING_LOCAL_ASSIGNMENT_REF: &str =
    "local_cpu_transform_rehearsal_v1";

/// Public-safe receipt reference emitted by this lane.
pub const TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_REF: &str =
    "receipt.models.tassadar_percepta_executor.cpu_transform_training.local_cpu_transform_rehearsal_v1";

/// Architecture-receipt input reference (already public-safe and live).
pub const TASSADAR_PERCEPTA_ARCHITECTURE_RECEIPT_REF: &str =
    "receipt.models.tassadar_percepta_executor.architecture.bundle.v1";

/// Artanis distillation-dataset input reference (already public-safe and live).
pub const ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF: &str =
    "receipt.training.tassadar_distillation_dataset.artanis_admin_verified_trace_refs.v1";

/// The product-promise blocker this receipt lane informs but does NOT clear.
pub const TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_BLOCKER: &str =
    "blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing";

const RECEIPT_ID: &str = "tassadar_percepta_cpu_transform_training_receipt_v1";
const PROMISE_REF: &str = "promise:models.tassadar_percepta_executor.v1";
const VERIFIER_CLASS: &str = "exact_trace_replay";

/// Error surface for CPU-transform training receipt generation and validation.
#[derive(Debug, Error)]
pub enum TassadarCpuTransformTrainingReceiptError {
    /// The underlying deterministic CPU training rehearsal failed.
    #[error(transparent)]
    Training(#[from] TassadarExecutorTrainingError),
    /// A required string field was empty.
    #[error("missing required field `{field}`")]
    MissingField {
        /// The empty field path.
        field: String,
    },
    /// A value was outside its admitted set.
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue {
        /// The offending field path.
        field: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Schema version mismatch on a deserialized packet.
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch {
        /// The expected schema version.
        expected: String,
        /// The found schema version.
        actual: String,
    },
    /// The packet digest did not match its recomputed value.
    #[error("digest mismatch for `{field}`")]
    DigestMismatch {
        /// The digest field path.
        field: String,
    },
    /// The committed fixture drifted from the canonical generator output.
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift {
        /// The drifted fixture path.
        path: String,
    },
    /// Reading a fixture failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// The path that failed.
        path: String,
        /// The underlying io error.
        error: std::io::Error,
    },
    /// Writing a fixture failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// The path that failed.
        path: String,
        /// The underlying io error.
        error: std::io::Error,
    },
    /// Creating a fixture directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// The path that failed.
        path: String,
        /// The underlying io error.
        error: std::io::Error,
    },
    /// JSON (de)serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// One named gate in the CPU-transform training receipt.
///
/// `satisfied` records whether the gate is locally provable from this receipt.
/// `compute_or_owner_gated` records whether the gate stays held behind the
/// real-GPU / real-money / owner-sign-off boundary regardless of any local
/// proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCpuTransformTrainingGate {
    /// Stable gate identifier (mirrors the openagents projection gate keys).
    pub gate_id: String,
    /// Whether this gate is satisfied by the local CPU rehearsal evidence.
    pub satisfied: bool,
    /// Whether this gate is held behind the compute/owner boundary.
    pub compute_or_owner_gated: bool,
    /// Human-readable status detail.
    pub detail: String,
}

/// Per-case exact-replay verdict row carried by the receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCpuTransformReplayCaseVerdict {
    /// Stable validation sequence identifier.
    pub sequence_id: String,
    /// Stable validation case identifier.
    pub case_id: String,
    /// Whether the trace replayed exactly against CPU-reference truth.
    pub exact_trace_match: bool,
    /// Whether the final output matched CPU-reference truth.
    pub final_output_match: bool,
    /// Whether the halt decision matched CPU-reference truth.
    pub halt_match: bool,
    /// Zero-based first divergence index, when the trace diverged.
    pub first_divergence_index: Option<u32>,
    /// Reference (CPU-truth) target token digest for the case.
    pub reference_target_digest: String,
    /// Predicted target token digest for the case.
    pub predicted_target_digest: String,
}

/// Aggregate exact-replay verifier verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCpuTransformReplayVerdict {
    /// Named verification class (matches the spec's `exact_trace_replay`).
    pub verifier_class: String,
    /// Frozen dataset storage key the verdict was computed over.
    pub dataset_storage_key: String,
    /// Frozen dataset digest the verdict was computed over.
    pub dataset_digest: String,
    /// Validation split the verdict was computed over.
    pub split: String,
    /// Honest model claim boundary at verdict time.
    pub claim_boundary: String,
    /// Total exact-replay validation cases.
    pub case_count: u32,
    /// Cases that replayed the full trace exactly.
    pub exact_trace_case_count: u32,
    /// Cases whose final output matched CPU-reference truth.
    pub final_output_exact_case_count: u32,
    /// Cases whose halt decision matched CPU-reference truth.
    pub halt_exact_case_count: u32,
    /// Aggregate target-token exactness in basis points.
    pub aggregate_target_token_exactness_bps: u32,
    /// Per-case verdict rows.
    pub case_verdicts: Vec<TassadarCpuTransformReplayCaseVerdict>,
}

/// The dereferenceable CPU-transform training receipt packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCpuTransformTrainingReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Public-safe receipt reference.
    pub receipt_ref: String,
    /// Product-promise reference this receipt informs.
    pub promise_ref: String,
    /// Promise state this receipt is consistent with (stays `planned`).
    pub promise_state: String,
    /// Local rehearsal assignment reference (NOT a real Pylon assignment).
    pub local_assignment_ref: String,
    /// Whether this assignment reference is a real Pylon assignment record.
    pub is_real_pylon_assignment: bool,
    /// Frozen training run identifier.
    pub training_run_id: String,
    /// Frozen training manifest digest (verified-work anchor).
    pub training_manifest_digest: String,
    /// Trained model descriptor digest (trained-artifact digest).
    pub trained_model_descriptor_digest: String,
    /// Trained model weight digest (trained-artifact digest).
    pub trained_weight_digest: String,
    /// Stable digest over the underlying training report.
    pub training_report_digest: String,
    /// Selected checkpoint identifier.
    pub best_checkpoint_id: String,
    /// Checkpoint selection basis.
    pub checkpoint_selection_basis: String,
    /// Architecture-receipt input reference.
    pub architecture_receipt_input_ref: String,
    /// Artanis distillation-dataset input reference.
    pub distillation_dataset_input_ref: String,
    /// Exact-replay verifier verdict.
    pub replay_verdict: TassadarCpuTransformReplayVerdict,
    /// Named gate state.
    pub gates: Vec<TassadarCpuTransformTrainingGate>,
    /// Whether the public green gate is satisfied (always false here).
    pub green_gate_satisfied: bool,
    /// Product-promise blocker this receipt informs but does not clear.
    pub informs_blocker_ref: String,
    /// What this receipt can now legitimately claim.
    pub can_now_claim: Vec<String>,
    /// What stays explicitly out of scope behind compute/owner gates.
    pub still_out_of_scope: Vec<String>,
    /// The single exact remaining compute/owner action to reach green.
    pub remaining_compute_owner_gate: String,
    /// Explicit claim boundary copy.
    pub claim_boundary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl TassadarCpuTransformTrainingReceipt {
    /// Validates the receipt's internal consistency and honesty invariants.
    pub fn validate(&self) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
        if self.schema_version != TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_SCHEMA_VERSION {
            return Err(
                TassadarCpuTransformTrainingReceiptError::SchemaVersionMismatch {
                    expected: String::from(TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            ("receipt_id", self.receipt_id.as_str()),
            ("receipt_ref", self.receipt_ref.as_str()),
            ("promise_ref", self.promise_ref.as_str()),
            ("promise_state", self.promise_state.as_str()),
            ("local_assignment_ref", self.local_assignment_ref.as_str()),
            ("training_run_id", self.training_run_id.as_str()),
            (
                "training_manifest_digest",
                self.training_manifest_digest.as_str(),
            ),
            (
                "trained_model_descriptor_digest",
                self.trained_model_descriptor_digest.as_str(),
            ),
            ("trained_weight_digest", self.trained_weight_digest.as_str()),
            (
                "training_report_digest",
                self.training_report_digest.as_str(),
            ),
            ("best_checkpoint_id", self.best_checkpoint_id.as_str()),
            (
                "architecture_receipt_input_ref",
                self.architecture_receipt_input_ref.as_str(),
            ),
            (
                "distillation_dataset_input_ref",
                self.distillation_dataset_input_ref.as_str(),
            ),
            ("informs_blocker_ref", self.informs_blocker_ref.as_str()),
            (
                "remaining_compute_owner_gate",
                self.remaining_compute_owner_gate.as_str(),
            ),
            ("claim_boundary", self.claim_boundary.as_str()),
            ("receipt_digest", self.receipt_digest.as_str()),
        ] {
            ensure_nonempty(value, field)?;
        }

        if self.promise_ref != PROMISE_REF {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("promise_ref"),
                detail: format!("promise ref must stay `{PROMISE_REF}`"),
            });
        }
        if self.promise_state != "planned" {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("promise_state"),
                detail: String::from(
                    "CPU-transform training receipt must keep the promise `planned`; it cannot green the promise",
                ),
            });
        }
        // Honesty invariant: this receipt must never describe a real Pylon
        // assignment, and must never satisfy the public green gate.
        if self.is_real_pylon_assignment {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("is_real_pylon_assignment"),
                detail: String::from(
                    "local CPU-transform rehearsal must not claim a real Pylon assignment",
                ),
            });
        }
        if self.green_gate_satisfied {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("green_gate_satisfied"),
                detail: String::from(
                    "CPU-transform training receipt must not satisfy the public green gate",
                ),
            });
        }
        if self.informs_blocker_ref != TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_BLOCKER {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("informs_blocker_ref"),
                detail: String::from("blocker ref must stay the CPU-transform training blocker"),
            });
        }

        // The compute/owner-gated gates must stay unsatisfied. Any local proof
        // must NOT flip them, because real GPU training, accepted Pylon work,
        // real settlement, and owner sign-off cannot be produced locally.
        for gate in &self.gates {
            ensure_nonempty(gate.gate_id.as_str(), "gates[].gate_id")?;
            ensure_nonempty(gate.detail.as_str(), "gates[].detail")?;
            if gate.compute_or_owner_gated && gate.satisfied {
                return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                    field: format!("gates[{}].satisfied", gate.gate_id),
                    detail: String::from(
                        "a compute/owner-gated gate cannot be satisfied by a local CPU rehearsal",
                    ),
                });
            }
        }
        let satisfied_local = self
            .gates
            .iter()
            .filter(|gate| !gate.compute_or_owner_gated && gate.satisfied)
            .count();
        if satisfied_local == 0 {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("gates"),
                detail: String::from(
                    "receipt must record at least one locally provable satisfied gate",
                ),
            });
        }
        if self.can_now_claim.is_empty() || self.still_out_of_scope.is_empty() {
            return Err(TassadarCpuTransformTrainingReceiptError::MissingField {
                field: String::from("claim_boundary_lists"),
            });
        }

        // Verifier verdict consistency.
        let verdict = &self.replay_verdict;
        if verdict.verifier_class != VERIFIER_CLASS {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("replay_verdict.verifier_class"),
                detail: format!("verifier class must stay `{VERIFIER_CLASS}`"),
            });
        }
        for (field, value) in [
            (
                "replay_verdict.dataset_storage_key",
                verdict.dataset_storage_key.as_str(),
            ),
            (
                "replay_verdict.dataset_digest",
                verdict.dataset_digest.as_str(),
            ),
            ("replay_verdict.split", verdict.split.as_str()),
            (
                "replay_verdict.claim_boundary",
                verdict.claim_boundary.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if verdict.case_count == 0 || verdict.case_verdicts.len() as u32 != verdict.case_count {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("replay_verdict.case_count"),
                detail: String::from(
                    "verifier verdict must carry one verdict row per validation case",
                ),
            });
        }
        let exact_rows = verdict
            .case_verdicts
            .iter()
            .filter(|row| row.exact_trace_match)
            .count() as u32;
        if exact_rows != verdict.exact_trace_case_count {
            return Err(TassadarCpuTransformTrainingReceiptError::InvalidValue {
                field: String::from("replay_verdict.exact_trace_case_count"),
                detail: String::from("exact-trace case count must agree with the case rows"),
            });
        }

        if stable_receipt_digest(self) != self.receipt_digest {
            return Err(TassadarCpuTransformTrainingReceiptError::DigestMismatch {
                field: String::from("receipt_digest"),
            });
        }
        Ok(())
    }
}

/// Builds the canonical CPU-transform training receipt by running a real,
/// deterministic, CPU-only Tassadar executor transformer training rehearsal and
/// replaying the selected checkpoint against CPU-reference truth.
pub fn builtin_cpu_transform_training_receipt(
) -> Result<TassadarCpuTransformTrainingReceipt, TassadarCpuTransformTrainingReceiptError> {
    let config = TassadarExecutorTrainingConfig::reference();
    let outcome = train_tassadar_executor_transformer(&config)?;
    Ok(receipt_from_training_report(&config, &outcome.report))
}

/// Constructs a receipt from a frozen config and a completed training report.
///
/// Split out from [`builtin_cpu_transform_training_receipt`] so the receipt
/// shape can be unit-tested without re-running training.
pub fn receipt_from_training_report(
    config: &TassadarExecutorTrainingConfig,
    report: &TassadarExecutorTrainingReport,
) -> TassadarCpuTransformTrainingReceipt {
    let eval = &report.evaluation;
    let case_verdicts = eval
        .case_reports
        .iter()
        .map(|case| TassadarCpuTransformReplayCaseVerdict {
            sequence_id: case.sequence_id.clone(),
            case_id: case.case_id.clone(),
            exact_trace_match: case.exact_trace_match,
            final_output_match: case.final_output_match,
            halt_match: case.halt_match,
            first_divergence_index: case.first_divergence_index,
            reference_target_digest: case.reference_target_digest.clone(),
            predicted_target_digest: case.predicted_target_digest.clone(),
        })
        .collect::<Vec<_>>();

    let replay_verdict = TassadarCpuTransformReplayVerdict {
        verifier_class: String::from(VERIFIER_CLASS),
        dataset_storage_key: eval.dataset_storage_key.clone(),
        dataset_digest: eval.dataset_digest.clone(),
        split: format!("{:?}", eval.split),
        claim_boundary: format!("{:?}", eval.claim_boundary),
        case_count: eval.case_reports.len() as u32,
        exact_trace_case_count: eval.exact_trace_case_count,
        final_output_exact_case_count: eval.final_output_exact_case_count,
        halt_exact_case_count: eval.halt_exact_case_count,
        aggregate_target_token_exactness_bps: eval.aggregate_target_token_exactness_bps,
        case_verdicts,
    };

    let gates = vec![
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("cpu_transform_training_completed"),
            satisfied: true,
            compute_or_owner_gated: false,
            detail: String::from(
                "A deterministic CPU-only Tassadar executor transformer training run completed over the frozen verified-trace manifest.",
            ),
        },
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("exact_replay_verifier_verdict_local"),
            satisfied: true,
            compute_or_owner_gated: false,
            detail: String::from(
                "The selected checkpoint was replayed against CPU-reference truth, producing a per-case exact-trace / final-output / halt verdict by first divergence.",
            ),
        },
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("trained_artifact_digest_present"),
            satisfied: true,
            compute_or_owner_gated: false,
            detail: String::from(
                "The trained model carries stable descriptor and weight digests usable as a public-safe trained-artifact digest.",
            ),
        },
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("pylon_assignment_receipt"),
            satisfied: false,
            compute_or_owner_gated: true,
            detail: String::from(
                "No real Pylon assignment record exists; this is a local rehearsal, not dispatched contributor work.",
            ),
        },
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("accepted_work_receipt"),
            satisfied: false,
            compute_or_owner_gated: true,
            detail: String::from(
                "No accepted-work closeout exists; acceptance requires real dispatched Pylon work.",
            ),
        },
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("real_settlement_receipt"),
            satisfied: false,
            compute_or_owner_gated: true,
            detail: String::from(
                "No real settlement receipt exists; no money moved for this local rehearsal.",
            ),
        },
        TassadarCpuTransformTrainingGate {
            gate_id: String::from("green_promise_transition"),
            satisfied: false,
            compute_or_owner_gated: true,
            detail: String::from(
                "Green requires owner sign-off under proof.claim_upgrade_receipts.v1 over real dispatched, accepted, verified, and settled work.",
            ),
        },
    ];

    let mut receipt = TassadarCpuTransformTrainingReceipt {
        schema_version: String::from(TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_SCHEMA_VERSION),
        receipt_id: String::from(RECEIPT_ID),
        receipt_ref: String::from(TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_REF),
        promise_ref: String::from(PROMISE_REF),
        promise_state: String::from("planned"),
        local_assignment_ref: String::from(TASSADAR_CPU_TRANSFORM_TRAINING_LOCAL_ASSIGNMENT_REF),
        is_real_pylon_assignment: false,
        training_run_id: config.run_id.clone(),
        training_manifest_digest: report.training_manifest_digest.clone(),
        trained_model_descriptor_digest: report.trained_model_descriptor_digest.clone(),
        trained_weight_digest: report.trained_weight_digest.clone(),
        training_report_digest: report.report_digest.clone(),
        best_checkpoint_id: report.best_checkpoint_id.clone(),
        checkpoint_selection_basis: report.checkpoint_selection_basis.clone(),
        architecture_receipt_input_ref: String::from(TASSADAR_PERCEPTA_ARCHITECTURE_RECEIPT_REF),
        distillation_dataset_input_ref: String::from(
            ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
        ),
        replay_verdict,
        gates,
        green_gate_satisfied: false,
        informs_blocker_ref: String::from(TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_BLOCKER),
        can_now_claim: vec![
            String::from(
                "a deterministic CPU-only Tassadar executor transformer training run completed over a frozen verified-trace manifest",
            ),
            String::from(
                "an exact-replay verifier verdict (trace / final-output / halt by first divergence) was produced against CPU-reference truth",
            ),
            String::from(
                "a stable trained-artifact digest (model descriptor + weights) is available as public-safe evidence",
            ),
            String::from(
                "this receipt is a dereferenceable proof shape that the openagents CPU-transform status route can cite",
            ),
        ],
        still_out_of_scope: vec![
            String::from("a real Pylon CPU-transform training assignment with accepted work"),
            String::from("a real settlement receipt where money actually moved"),
            String::from("model promotion, hosted inference, or a trained product model"),
            String::from("CPU replacement, CPU outperformance, or general-model claims"),
            String::from("a green transition for models.tassadar_percepta_executor.v1"),
        ],
        remaining_compute_owner_gate: String::from(
            "Dispatch this CPU-transform training as a real Pylon assignment (owner arms compute/spend), let it produce accepted-work + exact-replay verdict + real settlement receipts where money moves, then take the receipt-first green upgrade under proof.claim_upgrade_receipts.v1 with owner sign-off.",
        ),
        claim_boundary: String::from(
            "This is a local, deterministic, CPU-only training rehearsal receipt for models.tassadar_percepta_executor.v1. It proves training completed and replays exactly against CPU-reference truth, and it carries a trained-artifact digest. It does NOT claim a trained product model, accepted Pylon work, verifier-accepted paid work, real settlement, model promotion, inference, CPU replacement, or a green product promise. The promise stays planned.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_receipt_digest(&receipt);
    receipt
}

/// Generates the canonical receipt and writes it to the committed fixture path.
pub fn write_builtin_cpu_transform_training_receipt(
    workspace_root: &Path,
) -> Result<TassadarCpuTransformTrainingReceipt, TassadarCpuTransformTrainingReceiptError> {
    let receipt = builtin_cpu_transform_training_receipt()?;
    write_json_fixture(
        workspace_root,
        TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH,
        &receipt,
    )?;
    Ok(receipt)
}

fn stable_receipt_digest(receipt: &TassadarCpuTransformTrainingReceipt) -> String {
    let mut clone = receipt.clone();
    clone.receipt_digest.clear();
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_percepta_cpu_transform_training_receipt|");
    hasher.update(serde_json::to_vec(&clone).unwrap_or_default());
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
    if value.trim().is_empty() {
        return Err(TassadarCpuTransformTrainingReceiptError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, TassadarCpuTransformTrainingReceiptError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarCpuTransformTrainingReceiptError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(TassadarCpuTransformTrainingReceiptError::Json)
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCpuTransformTrainingReceiptError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| TassadarCpuTransformTrainingReceiptError::Write {
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
    fn builtin_cpu_transform_training_receipt_is_valid(
    ) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
        let receipt = builtin_cpu_transform_training_receipt()?;
        receipt.validate()?;
        // Honesty: the compute/owner-gated gates must all stay unsatisfied.
        assert!(!receipt.green_gate_satisfied);
        assert!(!receipt.is_real_pylon_assignment);
        assert!(receipt
            .gates
            .iter()
            .filter(|gate| gate.compute_or_owner_gated)
            .all(|gate| !gate.satisfied));
        // Local-proof: at least training-completed, replay-verdict, and
        // artifact-digest gates are satisfied.
        assert!(receipt
            .gates
            .iter()
            .filter(|gate| !gate.compute_or_owner_gated)
            .all(|gate| gate.satisfied));
        // The exact-replay verdict carries real per-case digests.
        assert!(receipt.replay_verdict.case_count > 0);
        assert!(receipt
            .replay_verdict
            .case_verdicts
            .iter()
            .all(|row| !row.reference_target_digest.is_empty()
                && !row.predicted_target_digest.is_empty()));
        // The trained-artifact digests are present.
        assert!(!receipt.trained_model_descriptor_digest.is_empty());
        assert!(!receipt.trained_weight_digest.is_empty());
        Ok(())
    }

    #[test]
    fn cpu_transform_training_receipt_is_deterministic(
    ) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
        let first = builtin_cpu_transform_training_receipt()?;
        let second = builtin_cpu_transform_training_receipt()?;
        assert_eq!(first, second);
        assert_eq!(first.receipt_digest, second.receipt_digest);
        Ok(())
    }

    #[test]
    fn cpu_transform_training_receipt_fixture_matches_committed_truth(
    ) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
        let root = workspace_root();
        let expected: TassadarCpuTransformTrainingReceipt =
            read_json(root.as_path(), TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH)?;
        let actual = builtin_cpu_transform_training_receipt()?;
        if expected != actual {
            return Err(TassadarCpuTransformTrainingReceiptError::FixtureDrift {
                path: String::from(TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_cpu_transform_training_receipt_persists_current_truth(
    ) -> Result<(), TassadarCpuTransformTrainingReceiptError> {
        let root = workspace_root();
        let receipt = write_builtin_cpu_transform_training_receipt(root.as_path())?;
        let persisted: TassadarCpuTransformTrainingReceipt =
            read_json(root.as_path(), TASSADAR_CPU_TRANSFORM_TRAINING_RECEIPT_FIXTURE_PATH)?;
        assert_eq!(receipt, persisted);
        Ok(())
    }
}
