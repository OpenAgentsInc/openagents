use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Pylon legal training job spec schema.
pub const QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_training_job.v1";
/// Pylon legal training worker receipt schema.
pub const QWEN_LEGAL_PYLON_WORKER_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_worker_receipt.v1";
/// Pylon legal training payment decision receipt schema.
pub const QWEN_LEGAL_PYLON_PAYMENT_DECISION_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_payment_decision.v1";
/// Pylon legal training Treasury/Nexus payable batch schema.
pub const QWEN_LEGAL_PYLON_TREASURY_HANDOFF_BATCH_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_treasury_handoff_batch.v1";
/// Pylon legal training returned Bitcoin/Lightning settlement proof schema.
pub const QWEN_LEGAL_PYLON_BITCOIN_SETTLEMENT_PROOF_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_bitcoin_settlement_proof.v1";
/// Pylon legal training payment closeout schema.
pub const QWEN_LEGAL_PYLON_PAYMENT_CLOSEOUT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_payment_closeout.v1";
/// Local worker implementation id.
pub const QWEN_LEGAL_PYLON_LOCAL_WORKER_IMPL_ID: &str = "psionic.qwen_legal_pylon.local_worker.v1";

/// Pylon payment status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingPaymentStatus {
    /// Worker has submitted work, but validation has not settled payment.
    PendingValidation,
    /// Work is valid and payable, but no payment proof is attached yet.
    Payable,
    /// Payment was withheld under the declared payment rules.
    Withheld,
    /// Payment has been paid and a proof is attached.
    Paid,
}

/// Bitcoin/Lightning payout target type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingBitcoinPayoutTargetKind {
    Bolt11Invoice,
    Bolt12Offer,
    Bip353Address,
    LnurlPay,
    OnchainAddress,
}

/// Bitcoin/Lightning settlement state returned by Treasury or Nexus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingBitcoinSettlementStatus {
    Pending,
    Settled,
    Failed,
    DeferredByOperator,
}

/// Settlement execution mode used for local tests and operator-approved runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingSettlementExecutionMode {
    DryRunLocal,
    LiveSmallValueOperatorApproved,
}

/// Promotion payment gate applied to adapter promotion closeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingPromotionPaymentGateStatus {
    PaymentSettled,
    DeferredByOperator,
    Blocked,
}

/// Pylon worker receipt validation status for payment decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingPaymentValidationStatus {
    /// Worker receipt passed signature, digest, output, and integrity checks.
    Valid,
    /// Worker receipt was present but did not pass validation.
    Invalid,
    /// Worker receipt was missing.
    MissingReceipt,
}

/// Pylon legal training job kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingJobKind {
    /// Build one dataset shard manifest.
    DatasetShardBuild,
    /// Run SFT over one assigned shard.
    SftTrainShard,
    /// Run DPO over one assigned shard.
    DpoTrainShard,
    /// Sample a GRPO rollout group.
    GrpoSampleBatch,
    /// Train GRPO over one assigned shard.
    GrpoTrainShard,
    /// Evaluate one assigned shard.
    EvalShard,
    /// Merge adapter deltas or LoRA adapters.
    AdapterMerge,
    /// Verify artifact hashes and receipt shape.
    ArtifactVerify,
}

/// Stable artifact reference used in job specs and receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingArtifactRef {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Logical artifact type.
    pub artifact_type: String,
    /// Local path available to the worker.
    pub path: String,
    /// Expected SHA-256 hex digest.
    pub sha256: String,
}

/// Expected output artifact declaration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingExpectedOutputArtifact {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Logical artifact type.
    pub artifact_type: String,
    /// Local path the worker must write.
    pub path: String,
    /// Whether the output is required for job success.
    pub required: bool,
}

/// Output artifact retained in a worker receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingOutputArtifactRef {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Logical artifact type.
    pub artifact_type: String,
    /// Local artifact path.
    pub path: String,
    /// Observed SHA-256 hex digest.
    pub sha256: String,
    /// Observed byte length.
    pub byte_len: u64,
}

/// Shard assignment for one Pylon job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingShardAssignment {
    /// Stable assignment id.
    pub assignment_id: String,
    /// Stable shard id.
    pub shard_id: String,
    /// Zero-based shard index.
    pub shard_index: u32,
    /// Total shard count.
    pub shard_count: u32,
    /// Optional start row or task index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_index: Option<u64>,
    /// Optional exclusive end row or task index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_index: Option<u64>,
}

/// Hardware requirements declared by the job issuer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingHardwareRequirements {
    /// Minimum memory in bytes.
    pub min_memory_bytes: u64,
    /// Whether the job requires a GPU or accelerator.
    pub require_accelerator: bool,
    /// Accepted backend labels.
    pub accepted_backend_labels: Vec<String>,
}

/// Payment and budget metadata carried with the job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingPaymentBudget {
    /// Stable budget id.
    pub budget_id: String,
    /// Agreed fixed price for a valid worker receipt in micro-USD.
    pub agreed_price_microusd: u64,
    /// Max authorized runtime spend in micro-USD.
    pub max_cost_microusd: u64,
    /// Currency code for the amount fields.
    pub currency: String,
    /// Payment account or ledger reference.
    pub payment_account_ref: String,
    /// Bitcoin/Lightning payout target and authorization metadata.
    #[serde(default = "default_pylon_bitcoin_payout_target")]
    pub bitcoin_payout: PylonTrainingBitcoinPayoutTarget,
    /// Whether a failed but otherwise valid eval attempt can still be paid.
    pub pay_failed_but_valid_eval_attempts: bool,
}

/// Bitcoin/Lightning payout target retained in job specs and payment decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingBitcoinPayoutTarget {
    /// Stable payout target reference, not a raw wallet secret.
    pub payout_target_ref: String,
    /// Target type.
    pub target_kind: PylonTrainingBitcoinPayoutTargetKind,
    /// Payable amount in millisatoshis.
    pub amount_msat: u64,
    /// Currency code for the payable amount.
    pub currency: String,
    /// Expiry timestamp in milliseconds since epoch.
    pub expires_at_ms: u64,
    /// Fee policy reference.
    pub fee_policy: String,
    /// Payout authorization id generated by Psionic for Treasury/Nexus.
    pub payout_authorization_id: String,
}

/// One payable item handed from Psionic to Treasury/Nexus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingTreasuryHandoffItem {
    pub decision_id: String,
    pub job_id: String,
    pub parent_run_id: String,
    pub worker_id: String,
    pub worker_receipt_digest: String,
    pub payout: PylonTrainingBitcoinPayoutTarget,
    pub agreed_price_microusd: u64,
    pub budget_id: String,
    pub shard_key: String,
    pub payout_authorization_id: String,
}

/// Deterministic payable batch emitted to Treasury/Nexus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingTreasuryHandoffBatch {
    pub schema_version: String,
    pub batch_id: String,
    pub payable_items: Vec<PylonTrainingTreasuryHandoffItem>,
    pub withheld_decision_ids: Vec<String>,
    pub batch_digest: String,
}

impl PylonTrainingTreasuryHandoffBatch {
    /// Stable digest with digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.batch_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_treasury_handoff_batch|", &clone)
    }
}

/// Settlement proof returned from Treasury/Nexus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingBitcoinSettlementProof {
    pub schema_version: String,
    pub operation_id: String,
    pub payout_authorization_id: String,
    pub worker_receipt_digest: String,
    pub amount_msat: u64,
    pub settlement_method: PylonTrainingBitcoinPayoutTargetKind,
    pub settlement_mode: PylonTrainingSettlementExecutionMode,
    pub status: PylonTrainingBitcoinSettlementStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_preimage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_proof: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolt11_invoice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolt12_offer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bip353_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lnurl_pay_ref: Option<String>,
    pub fee_msat: u64,
    pub settled_at_ms: u64,
    pub reconciliation_digest: String,
    pub proof_digest: String,
}

impl PylonTrainingBitcoinSettlementProof {
    /// Stable digest with digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.proof_digest.clear();
        stable_json_digest(
            b"psionic_qwen_legal_pylon_bitcoin_settlement_proof|",
            &clone,
        )
    }
}

/// Explicit operator approval for deferred payments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingDeferredPaymentPolicy {
    pub policy_id: String,
    pub operator_id: String,
    pub reason: String,
    pub approved_at_ms: u64,
    pub expires_at_ms: u64,
}

/// Training run closeout after Treasury/Nexus settlement receipts are attached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingPaymentCloseout {
    pub schema_version: String,
    pub closeout_id: String,
    pub batch_id: String,
    pub accepted_work_count: u32,
    pub payable_payment_count: u32,
    pub pending_payment_count: u32,
    pub settled_payment_count: u32,
    pub deferred_payment_count: u32,
    pub failed_payment_count: u32,
    pub proofs: Vec<PylonTrainingBitcoinSettlementProof>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred_policy: Option<PylonTrainingDeferredPaymentPolicy>,
    pub promotion_payment_gate_status: PylonTrainingPromotionPaymentGateStatus,
    pub no_wallet_secrets_present: bool,
    pub closeout_digest: String,
}

impl PylonTrainingPaymentCloseout {
    /// Stable digest with digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.closeout_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_payment_closeout|", &clone)
    }
}

/// Receipt requirements requested by the job issuer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingReceiptRequirements {
    /// Whether the receipt must include an Ed25519 signature.
    pub require_signature: bool,
    /// Whether the receipt must include a logs hash.
    pub require_logs_hash: bool,
    /// Whether the receipt must include metrics.
    pub require_metrics: bool,
    /// Output artifact types that must be present on success.
    pub required_output_artifact_types: Vec<String>,
}

/// Full Pylon legal training job spec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingJobSpec {
    /// Schema version.
    pub schema_version: String,
    /// Stable job id.
    pub job_id: String,
    /// Parent run id.
    pub parent_run_id: String,
    /// Job kind.
    pub job_kind: PylonTrainingJobKind,
    /// Model id.
    pub model_id: String,
    /// Model hash.
    pub model_hash: String,
    /// Optional input adapter id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    /// Optional input adapter hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_hash: Option<String>,
    /// Dataset manifest hash.
    pub dataset_manifest_hash: String,
    /// Shard assignment.
    pub shard_assignment: PylonTrainingShardAssignment,
    /// Training config hash.
    pub training_config_hash: String,
    /// Expected input artifacts.
    pub expected_input_artifacts: Vec<PylonTrainingArtifactRef>,
    /// Expected output artifacts.
    pub expected_output_artifacts: Vec<PylonTrainingExpectedOutputArtifact>,
    /// Max runtime in milliseconds.
    pub max_runtime_ms: u64,
    /// Hardware requirements.
    pub hardware_requirements: PylonTrainingHardwareRequirements,
    /// Payment and budget metadata.
    pub payment_budget: PylonTrainingPaymentBudget,
    /// Receipt requirements.
    pub receipt_requirements: PylonTrainingReceiptRequirements,
    /// Worker output directory.
    pub output_dir: String,
    /// Receipt output path.
    pub receipt_path: String,
}

impl PylonTrainingJobSpec {
    /// Stable digest over the job spec.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_json_digest(b"psionic_qwen_legal_pylon_training_job|", self)
    }

    /// Validates required fields.
    pub fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        if self.schema_version != QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION {
            return invalid_job("schema version drifted");
        }
        require_nonempty(self.job_id.as_str(), "job_id")?;
        require_nonempty(self.parent_run_id.as_str(), "parent_run_id")?;
        require_nonempty(self.model_id.as_str(), "model_id")?;
        require_nonempty(self.model_hash.as_str(), "model_hash")?;
        require_nonempty(self.dataset_manifest_hash.as_str(), "dataset_manifest_hash")?;
        require_nonempty(self.training_config_hash.as_str(), "training_config_hash")?;
        require_nonempty(self.output_dir.as_str(), "output_dir")?;
        require_nonempty(self.receipt_path.as_str(), "receipt_path")?;
        self.shard_assignment.validate()?;
        if self.expected_input_artifacts.is_empty() {
            return invalid_job("expected_input_artifacts must not be empty");
        }
        if self.expected_output_artifacts.is_empty() {
            return invalid_job("expected_output_artifacts must not be empty");
        }
        for artifact in &self.expected_input_artifacts {
            artifact.validate()?;
        }
        for artifact in &self.expected_output_artifacts {
            artifact.validate()?;
        }
        if self.max_runtime_ms == 0 {
            return invalid_job("max_runtime_ms must be non-zero");
        }
        self.hardware_requirements.validate()?;
        self.payment_budget.validate()?;
        self.receipt_requirements.validate()?;
        Ok(())
    }
}

impl PylonTrainingArtifactRef {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        require_nonempty(self.artifact_id.as_str(), "artifact_id")?;
        require_nonempty(self.artifact_type.as_str(), "artifact_type")?;
        require_nonempty(self.path.as_str(), "path")?;
        require_nonempty(self.sha256.as_str(), "sha256")?;
        if self.sha256.len() != 64 || !self.sha256.chars().all(|value| value.is_ascii_hexdigit()) {
            return invalid_job("artifact sha256 must be 64 hex characters");
        }
        Ok(())
    }
}

impl PylonTrainingExpectedOutputArtifact {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        require_nonempty(self.artifact_id.as_str(), "output artifact_id")?;
        require_nonempty(self.artifact_type.as_str(), "output artifact_type")?;
        require_nonempty(self.path.as_str(), "output path")
    }
}

impl PylonTrainingShardAssignment {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        require_nonempty(self.assignment_id.as_str(), "assignment_id")?;
        require_nonempty(self.shard_id.as_str(), "shard_id")?;
        if self.shard_count == 0 || self.shard_index >= self.shard_count {
            return invalid_job("shard_index must be inside non-empty shard_count");
        }
        if self
            .start_index
            .zip(self.end_index)
            .is_some_and(|(start, end)| start >= end)
        {
            return invalid_job("start_index must be lower than end_index");
        }
        Ok(())
    }
}

impl PylonTrainingHardwareRequirements {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        if self.min_memory_bytes == 0 {
            return invalid_job("min_memory_bytes must be non-zero");
        }
        if self.accepted_backend_labels.is_empty() {
            return invalid_job("accepted_backend_labels must not be empty");
        }
        Ok(())
    }
}

impl PylonTrainingPaymentBudget {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        require_nonempty(self.budget_id.as_str(), "budget_id")?;
        require_nonempty(self.currency.as_str(), "currency")?;
        require_nonempty(self.payment_account_ref.as_str(), "payment_account_ref")?;
        if self.agreed_price_microusd > self.max_cost_microusd {
            return invalid_job("agreed_price_microusd must not exceed max_cost_microusd");
        }
        self.bitcoin_payout.validate()?;
        Ok(())
    }
}

impl PylonTrainingBitcoinPayoutTarget {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        require_nonempty(self.payout_target_ref.as_str(), "payout_target_ref")?;
        require_nonempty(self.currency.as_str(), "bitcoin payout currency")?;
        require_nonempty(self.fee_policy.as_str(), "fee_policy")?;
        require_nonempty(
            self.payout_authorization_id.as_str(),
            "payout_authorization_id",
        )?;
        if self.amount_msat == 0 {
            return invalid_job("bitcoin payout amount_msat must be non-zero");
        }
        if self.expires_at_ms == 0 {
            return invalid_job("bitcoin payout expires_at_ms must be non-zero");
        }
        if contains_wallet_secret_material(self.payout_target_ref.as_str())
            || contains_wallet_secret_material(self.fee_policy.as_str())
            || contains_wallet_secret_material(self.payout_authorization_id.as_str())
        {
            return invalid_job("bitcoin payout fields must not contain wallet secret material");
        }
        Ok(())
    }
}

impl PylonTrainingReceiptRequirements {
    fn validate(&self) -> Result<(), QwenLegalPylonTrainingJobError> {
        if !self.require_signature {
            return invalid_job("receipt signature is required for Pylon training jobs");
        }
        if self.required_output_artifact_types.is_empty() {
            return invalid_job("required_output_artifact_types must not be empty");
        }
        Ok(())
    }
}

/// Job status retained in worker receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingWorkerJobStatus {
    /// Job completed and all required outputs were present.
    Succeeded,
    /// Job failed validation or output checks.
    Failed,
}

/// Hardware observed by a local worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingWorkerHardwareSummary {
    /// Host id.
    pub host_id: String,
    /// OS family.
    pub os: String,
    /// Architecture.
    pub arch: String,
    /// Backend label used by this local worker.
    pub backend_label: String,
    /// Total memory estimate.
    pub total_memory_bytes: u64,
    /// Accelerator summary.
    pub accelerator: String,
}

/// Signed worker receipt for one Pylon legal training job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingWorkerReceipt {
    /// Schema version.
    pub schema_version: String,
    /// Worker implementation id.
    pub worker_impl_id: String,
    /// Worker id.
    pub worker_id: String,
    /// Worker Ed25519 public key.
    pub worker_pubkey: String,
    /// Job id.
    pub job_id: String,
    /// Parent run id.
    pub parent_run_id: String,
    /// Job kind.
    pub job_kind: PylonTrainingJobKind,
    /// Stable job spec digest.
    pub job_spec_digest: String,
    /// Budget metadata copied from the job spec.
    pub payment_budget: PylonTrainingPaymentBudget,
    /// Agreed fixed price copied from the job budget.
    pub agreed_price_microusd: u64,
    /// Input artifact hashes observed by the worker.
    pub input_hashes: Vec<PylonTrainingArtifactRef>,
    /// Output artifact hashes observed by the worker.
    pub output_hashes: Vec<PylonTrainingOutputArtifactRef>,
    /// Start time in milliseconds.
    pub started_at_ms: u64,
    /// End time in milliseconds.
    pub ended_at_ms: u64,
    /// Hardware summary.
    pub hardware_summary: PylonTrainingWorkerHardwareSummary,
    /// Psionic crate version.
    pub psionic_version: String,
    /// Git commit, if supplied by the packaged runtime.
    pub git_commit: String,
    /// Logs hash.
    pub logs_hash: String,
    /// Numeric worker metrics.
    pub metrics: BTreeMap<String, f64>,
    /// Job status.
    pub status: PylonTrainingWorkerJobStatus,
    /// Failure reason if status is failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Signature scheme.
    pub signature_scheme: String,
    /// Digest over the signable receipt payload.
    pub signed_payload_digest: String,
    /// Ed25519 signature over `signed_payload_digest`.
    pub signature_hex: String,
    /// Stable digest over the full receipt.
    pub receipt_digest: String,
    /// Payment status before settlement.
    pub payment_status: PylonTrainingPaymentStatus,
    /// Payment proof placeholder or actual payment proof after settlement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_proof: Option<String>,
    /// Reason payment was withheld, if settlement mutates or copies the receipt later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withheld_reason: Option<String>,
}

impl PylonTrainingWorkerReceipt {
    /// Stable receipt digest with the digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_worker_receipt|", &clone)
    }

    /// Stable digest over the signable payload.
    #[must_use]
    pub fn signable_payload_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        clone.signed_payload_digest.clear();
        clone.signature_hex.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_worker_receipt_signable|", &clone)
    }
}

/// Local worker options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PylonLocalWorkerRunOptions {
    /// Worker id.
    pub worker_id: String,
    /// Logical start time in milliseconds.
    pub started_at_ms: u64,
    /// Whether the local worker should materialize expected outputs.
    pub emit_outputs: bool,
}

impl Default for PylonLocalWorkerRunOptions {
    fn default() -> Self {
        Self {
            worker_id: String::from("pylon.local.qwen-legal.01"),
            started_at_ms: 10_000,
            emit_outputs: true,
        }
    }
}

/// Receipt verification result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingWorkerReceiptVerification {
    /// Receipt path.
    pub receipt_path: String,
    /// Job id.
    pub job_id: String,
    /// Worker id.
    pub worker_id: String,
    /// Job status.
    pub status: PylonTrainingWorkerJobStatus,
    /// Receipt digest.
    pub receipt_digest: String,
    /// Signed payload digest.
    pub signed_payload_digest: String,
    /// Whether the signature is valid.
    pub signature_valid: bool,
    /// Whether succeeded output files were rechecked.
    pub output_files_rechecked: bool,
}

/// Worker contribution and payment row for training reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingWorkerContributionPaymentRow {
    /// Job id.
    pub job_id: String,
    /// Worker id when a receipt exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    /// Worker Ed25519 public key when a receipt exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_pubkey: Option<String>,
    /// Work type.
    pub work_type: PylonTrainingJobKind,
    /// Stable shard key used for duplicate detection.
    pub shard_key: String,
    /// Agreed price.
    pub agreed_price_microusd: u64,
    /// Budget cap.
    pub budget_max_microusd: u64,
    /// Currency code.
    pub currency: String,
    /// Budget id.
    pub budget_id: String,
    /// Bitcoin/Lightning payout target reference.
    pub payout_target_ref: String,
    /// Payable amount in millisatoshis.
    pub amount_msat: u64,
    /// Payout authorization id.
    pub payout_authorization_id: String,
    /// Stable digest of observed or expected inputs.
    pub input_hash: String,
    /// Stable digest of observed outputs when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    /// Validation status.
    pub validation_status: PylonTrainingPaymentValidationStatus,
    /// Payment status.
    pub payment_status: PylonTrainingPaymentStatus,
    /// Payment proof placeholder or actual payment proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_proof: Option<String>,
    /// Reason payment was withheld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withheld_reason: Option<String>,
    /// Payment decision receipt path.
    pub decision_path: String,
    /// Payment decision digest.
    pub decision_digest: String,
}

/// Payment decision receipt for one worker job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PylonTrainingPaymentDecisionReceipt {
    /// Schema version.
    pub schema_version: String,
    /// Stable decision id.
    pub decision_id: String,
    /// Payment decision receipt path.
    pub decision_path: String,
    /// Job id.
    pub job_id: String,
    /// Parent run id.
    pub parent_run_id: String,
    /// Work type.
    pub work_type: PylonTrainingJobKind,
    /// Stable shard key used for duplicate detection.
    pub shard_key: String,
    /// Worker id when a receipt exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    /// Worker Ed25519 public key when a receipt exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_pubkey: Option<String>,
    /// Budget id.
    pub budget_id: String,
    /// Agreed price.
    pub agreed_price_microusd: u64,
    /// Budget cap.
    pub budget_max_microusd: u64,
    /// Currency code.
    pub currency: String,
    /// Payment account or ledger reference.
    pub payment_account_ref: String,
    /// Bitcoin/Lightning payout target and authorization metadata.
    pub bitcoin_payout: PylonTrainingBitcoinPayoutTarget,
    /// Stable job spec digest.
    pub job_spec_digest: String,
    /// Worker receipt digest when a receipt exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_receipt_digest: Option<String>,
    /// Signed payload digest when a receipt exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_payload_digest: Option<String>,
    /// Stable digest of observed or expected inputs.
    pub input_hash: String,
    /// Stable digest of observed outputs when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    /// Validation status.
    pub validation_status: PylonTrainingPaymentValidationStatus,
    /// Whether output files were rechecked.
    pub output_files_rechecked: bool,
    /// Whether another payable receipt already covers this shard.
    pub duplicate_shard: bool,
    /// Payment status.
    pub payment_status: PylonTrainingPaymentStatus,
    /// Payment proof placeholder or actual payment proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_proof: Option<String>,
    /// Reason payment was withheld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withheld_reason: Option<String>,
    /// Payment rules applied to this decision.
    pub payment_rules_applied: Vec<String>,
    /// Validation errors observed while settling.
    pub validation_errors: Vec<String>,
    /// Stable digest over the decision.
    pub decision_digest: String,
}

impl PylonTrainingPaymentDecisionReceipt {
    /// Stable digest with the digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.decision_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_payment_decision|", &clone)
    }

    /// Report row for this decision.
    #[must_use]
    pub fn contribution_payment_row(&self) -> PylonTrainingWorkerContributionPaymentRow {
        PylonTrainingWorkerContributionPaymentRow {
            job_id: self.job_id.clone(),
            worker_id: self.worker_id.clone(),
            worker_pubkey: self.worker_pubkey.clone(),
            work_type: self.work_type,
            shard_key: self.shard_key.clone(),
            agreed_price_microusd: self.agreed_price_microusd,
            budget_max_microusd: self.budget_max_microusd,
            currency: self.currency.clone(),
            budget_id: self.budget_id.clone(),
            payout_target_ref: self.bitcoin_payout.payout_target_ref.clone(),
            amount_msat: self.bitcoin_payout.amount_msat,
            payout_authorization_id: self.bitcoin_payout.payout_authorization_id.clone(),
            input_hash: self.input_hash.clone(),
            output_hash: self.output_hash.clone(),
            validation_status: self.validation_status,
            payment_status: self.payment_status,
            payment_proof: self.payment_proof.clone(),
            withheld_reason: self.withheld_reason.clone(),
            decision_path: self.decision_path.clone(),
            decision_digest: self.decision_digest.clone(),
        }
    }
}

/// Error for Pylon legal training job handling.
#[derive(Debug, Error)]
pub enum QwenLegalPylonTrainingJobError {
    /// Invalid job spec.
    #[error("invalid Pylon training job: {detail}")]
    InvalidJob { detail: String },
    /// Invalid worker receipt.
    #[error("invalid Pylon worker receipt: {detail}")]
    InvalidReceipt { detail: String },
    /// I/O failure.
    #[error("Pylon training job I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    /// JSON failure.
    #[error("Pylon training job JSON failed at `{path}`: {message}")]
    Json { path: String, message: String },
    /// Serialization failure.
    #[error("Pylon training job serialization failed: {message}")]
    Serialization { message: String },
}

/// Runs one Pylon legal worker job from a JSON file.
pub fn run_qwen_legal_pylon_worker_job_path(
    path: impl AsRef<Path>,
) -> Result<PylonTrainingWorkerReceipt, QwenLegalPylonTrainingJobError> {
    let path = path.as_ref();
    let job = read_json::<PylonTrainingJobSpec>(path)?;
    run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())
}

/// Runs one Pylon legal worker job.
pub fn run_qwen_legal_pylon_worker_job(
    job: &PylonTrainingJobSpec,
    options: &PylonLocalWorkerRunOptions,
) -> Result<PylonTrainingWorkerReceipt, QwenLegalPylonTrainingJobError> {
    job.validate()?;
    let output_dir = resolve_workspace_path(job.output_dir.as_str());
    fs::create_dir_all(&output_dir).map_err(|error| QwenLegalPylonTrainingJobError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;

    let signing_key =
        deterministic_worker_signing_key(options.worker_id.as_str(), job.job_id.as_str());
    let mut failure_reason = validate_inputs(job).err();
    if failure_reason.is_none() && options.emit_outputs {
        for output in &job.expected_output_artifacts {
            write_expected_output(job, output)?;
        }
    }
    let output_hashes = if failure_reason.is_none() {
        match collect_output_hashes(job) {
            Ok(outputs) => outputs,
            Err(error) => {
                failure_reason = Some(error);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let status = if failure_reason.is_some() {
        PylonTrainingWorkerJobStatus::Failed
    } else {
        PylonTrainingWorkerJobStatus::Succeeded
    };
    let input_hashes = observed_input_hashes(job);
    let ended_at_ms = options.started_at_ms.saturating_add(25);
    let logs = format!(
        "worker={} job={} status={:?} inputs={} outputs={}",
        options.worker_id,
        job.job_id,
        status,
        input_hashes.len(),
        output_hashes.len()
    );
    let mut metrics = BTreeMap::new();
    metrics.insert(
        String::from("input_artifact_count"),
        input_hashes.len() as f64,
    );
    metrics.insert(
        String::from("output_artifact_count"),
        output_hashes.len() as f64,
    );
    metrics.insert(
        String::from("runtime_ms"),
        ended_at_ms.saturating_sub(options.started_at_ms) as f64,
    );
    metrics.insert(
        String::from("required_output_artifact_count"),
        job.expected_output_artifacts.len() as f64,
    );
    metrics.insert(
        String::from("output_bytes_total"),
        output_hashes
            .iter()
            .map(|artifact| artifact.byte_len)
            .sum::<u64>() as f64,
    );
    metrics.insert(
        String::from("shard_index"),
        f64::from(job.shard_assignment.shard_index),
    );
    metrics.insert(
        String::from("shard_count"),
        f64::from(job.shard_assignment.shard_count),
    );
    metrics.insert(
        String::from("shard_start_index"),
        job.shard_assignment.start_index.unwrap_or_default() as f64,
    );
    metrics.insert(
        String::from("shard_end_index"),
        job.shard_assignment.end_index.unwrap_or_default() as f64,
    );
    metrics.insert(
        String::from("agreed_price_microusd"),
        job.payment_budget.agreed_price_microusd as f64,
    );
    metrics.insert(
        String::from("max_cost_microusd"),
        job.payment_budget.max_cost_microusd as f64,
    );
    metrics.insert(
        String::from("succeeded"),
        if status == PylonTrainingWorkerJobStatus::Succeeded {
            1.0
        } else {
            0.0
        },
    );
    let mut receipt = PylonTrainingWorkerReceipt {
        schema_version: String::from(QWEN_LEGAL_PYLON_WORKER_RECEIPT_SCHEMA_VERSION),
        worker_impl_id: String::from(QWEN_LEGAL_PYLON_LOCAL_WORKER_IMPL_ID),
        worker_id: options.worker_id.clone(),
        worker_pubkey: hex::encode(signing_key.verifying_key().to_bytes()),
        job_id: job.job_id.clone(),
        parent_run_id: job.parent_run_id.clone(),
        job_kind: job.job_kind,
        job_spec_digest: job.stable_digest(),
        payment_budget: job.payment_budget.clone(),
        agreed_price_microusd: job.payment_budget.agreed_price_microusd,
        input_hashes,
        output_hashes,
        started_at_ms: options.started_at_ms,
        ended_at_ms,
        hardware_summary: local_hardware_summary(),
        psionic_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit: option_env!("PSIONIC_GIT_COMMIT")
            .unwrap_or("unknown-local")
            .to_string(),
        logs_hash: sha256_hex(logs.as_bytes()),
        metrics,
        status,
        failure_reason,
        signature_scheme: String::from("ed25519_detached_sha256_payload_v1"),
        signed_payload_digest: String::new(),
        signature_hex: String::new(),
        receipt_digest: String::new(),
        payment_status: PylonTrainingPaymentStatus::PendingValidation,
        payment_proof: None,
        withheld_reason: None,
    };
    sign_receipt(&mut receipt, &signing_key);
    write_json(Path::new(&job.receipt_path), &receipt)?;
    Ok(receipt)
}

/// Verifies one Pylon legal worker receipt from a JSON file.
pub fn verify_qwen_legal_pylon_worker_receipt_path(
    path: impl AsRef<Path>,
) -> Result<PylonTrainingWorkerReceiptVerification, QwenLegalPylonTrainingJobError> {
    let path = path.as_ref();
    let receipt = read_json::<PylonTrainingWorkerReceipt>(path)?;
    verify_qwen_legal_pylon_worker_receipt(&receipt, path)
}

/// Verifies one Pylon legal worker receipt.
pub fn verify_qwen_legal_pylon_worker_receipt(
    receipt: &PylonTrainingWorkerReceipt,
    receipt_path: &Path,
) -> Result<PylonTrainingWorkerReceiptVerification, QwenLegalPylonTrainingJobError> {
    if receipt.schema_version != QWEN_LEGAL_PYLON_WORKER_RECEIPT_SCHEMA_VERSION {
        return invalid_receipt("schema version drifted");
    }
    if receipt.worker_impl_id != QWEN_LEGAL_PYLON_LOCAL_WORKER_IMPL_ID {
        return invalid_receipt("unknown worker implementation id");
    }
    if receipt.receipt_digest != receipt.stable_digest() {
        return invalid_receipt("receipt digest drifted");
    }
    let signable_digest = receipt.signable_payload_digest();
    if receipt.signed_payload_digest != signable_digest {
        return invalid_receipt("signed payload digest drifted");
    }
    let verifying_key = decode_verifying_key(receipt.worker_pubkey.as_str())?;
    let signature = decode_signature(receipt.signature_hex.as_str())?;
    verifying_key
        .verify(receipt.signed_payload_digest.as_bytes(), &signature)
        .map_err(|_| QwenLegalPylonTrainingJobError::InvalidReceipt {
            detail: String::from("signature verification failed"),
        })?;
    if receipt.ended_at_ms < receipt.started_at_ms {
        return invalid_receipt("ended_at_ms is earlier than started_at_ms");
    }
    if receipt.status == PylonTrainingWorkerJobStatus::Succeeded {
        if receipt.failure_reason.is_some() {
            return invalid_receipt("succeeded receipt must not carry failure_reason");
        }
        if receipt.output_hashes.is_empty() {
            return invalid_receipt("succeeded receipt must carry output hashes");
        }
        for output in &receipt.output_hashes {
            let observed = sha256_file(output.path.as_str())?;
            if observed != output.sha256 {
                return invalid_receipt(format!(
                    "output artifact `{}` hash drifted",
                    output.artifact_id
                ));
            }
        }
    }
    if receipt.status == PylonTrainingWorkerJobStatus::Failed && receipt.failure_reason.is_none() {
        return invalid_receipt("failed receipt must carry failure_reason");
    }
    Ok(PylonTrainingWorkerReceiptVerification {
        receipt_path: receipt_path.display().to_string(),
        job_id: receipt.job_id.clone(),
        worker_id: receipt.worker_id.clone(),
        status: receipt.status,
        receipt_digest: receipt.receipt_digest.clone(),
        signed_payload_digest: receipt.signed_payload_digest.clone(),
        signature_valid: true,
        output_files_rechecked: receipt.status == PylonTrainingWorkerJobStatus::Succeeded,
    })
}

/// Settles one canonical Pylon legal training job by id.
pub fn settle_qwen_legal_pylon_training_job(
    job_id: &str,
) -> Result<PylonTrainingPaymentDecisionReceipt, QwenLegalPylonTrainingJobError> {
    let job = canonical_qwen_legal_pylon_training_jobs()
        .into_iter()
        .find(|job| job.job_id == job_id)
        .ok_or_else(|| QwenLegalPylonTrainingJobError::InvalidJob {
            detail: format!("unknown canonical Pylon legal training job `{job_id}`"),
        })?;
    settle_qwen_legal_pylon_training_job_spec(&job)
}

/// Settles one Pylon legal training job spec.
pub fn settle_qwen_legal_pylon_training_job_spec(
    job: &PylonTrainingJobSpec,
) -> Result<PylonTrainingPaymentDecisionReceipt, QwenLegalPylonTrainingJobError> {
    job.validate()?;
    let receipt_path = resolve_workspace_path(job.receipt_path.as_str());
    let decision_path = payment_decision_path(job);
    let decision_path_string = payment_decision_receipt_path(job);
    let mut validation_errors = Vec::new();
    let mut receipt = None;
    let mut verification = None;
    let mut validation_status = PylonTrainingPaymentValidationStatus::MissingReceipt;

    if receipt_path.is_file() {
        match read_json::<PylonTrainingWorkerReceipt>(receipt_path.as_path()) {
            Ok(candidate_receipt) => {
                match verify_qwen_legal_pylon_worker_receipt(
                    &candidate_receipt,
                    receipt_path.as_path(),
                ) {
                    Ok(candidate_verification) => {
                        validation_status = PylonTrainingPaymentValidationStatus::Valid;
                        verification = Some(candidate_verification);
                    }
                    Err(error) => {
                        validation_status = PylonTrainingPaymentValidationStatus::Invalid;
                        validation_errors.push(error.to_string());
                    }
                }
                receipt = Some(candidate_receipt);
            }
            Err(error) => {
                validation_status = PylonTrainingPaymentValidationStatus::Invalid;
                validation_errors.push(error.to_string());
            }
        }
    } else {
        validation_errors.push(format!(
            "missing worker receipt at `{}`",
            receipt_path.display()
        ));
    }

    let duplicate_shard = receipt
        .as_ref()
        .is_some_and(|receipt| duplicate_payable_shard_exists(job, receipt, &decision_path));
    let mut decision = build_payment_decision(
        job,
        receipt.as_ref(),
        verification.as_ref(),
        validation_status,
        duplicate_shard,
        validation_errors,
        decision_path_string,
    );
    decision.decision_digest = decision.stable_digest();
    write_json(decision_path.as_path(), &decision)?;
    Ok(decision)
}

/// Builds payment table rows for the canonical Pylon legal jobs.
pub fn qwen_legal_pylon_worker_contribution_payment_table()
-> Result<Vec<PylonTrainingWorkerContributionPaymentRow>, QwenLegalPylonTrainingJobError> {
    canonical_qwen_legal_pylon_training_jobs()
        .iter()
        .map(settle_qwen_legal_pylon_training_job_spec)
        .map(|decision| decision.map(|decision| decision.contribution_payment_row()))
        .collect()
}

/// Builds the deterministic payable batch Psionic hands to Treasury/Nexus.
pub fn build_qwen_legal_pylon_treasury_handoff_batch(
    batch_id: impl Into<String>,
    decisions: &[PylonTrainingPaymentDecisionReceipt],
) -> Result<PylonTrainingTreasuryHandoffBatch, QwenLegalPylonTrainingJobError> {
    let mut seen_authorizations = BTreeSet::new();
    let mut payable_items = Vec::new();
    let mut withheld_decision_ids = Vec::new();
    for decision in decisions {
        if decision.payment_status == PylonTrainingPaymentStatus::Payable {
            let worker_id = decision.worker_id.clone().ok_or_else(|| {
                QwenLegalPylonTrainingJobError::InvalidJob {
                    detail: format!(
                        "payable decision `{}` is missing worker_id",
                        decision.decision_id
                    ),
                }
            })?;
            let worker_receipt_digest =
                decision.worker_receipt_digest.clone().ok_or_else(|| {
                    QwenLegalPylonTrainingJobError::InvalidJob {
                        detail: format!(
                            "payable decision `{}` is missing worker_receipt_digest",
                            decision.decision_id
                        ),
                    }
                })?;
            let payout_authorization_id = decision.bitcoin_payout.payout_authorization_id.clone();
            if !seen_authorizations.insert(payout_authorization_id.clone()) {
                return invalid_job(format!(
                    "duplicate payout authorization id `{payout_authorization_id}`"
                ));
            }
            payable_items.push(PylonTrainingTreasuryHandoffItem {
                decision_id: decision.decision_id.clone(),
                job_id: decision.job_id.clone(),
                parent_run_id: decision.parent_run_id.clone(),
                worker_id,
                worker_receipt_digest,
                payout: decision.bitcoin_payout.clone(),
                agreed_price_microusd: decision.agreed_price_microusd,
                budget_id: decision.budget_id.clone(),
                shard_key: decision.shard_key.clone(),
                payout_authorization_id,
            });
        } else {
            withheld_decision_ids.push(decision.decision_id.clone());
        }
    }
    payable_items.sort_by(|left, right| left.decision_id.cmp(&right.decision_id));
    withheld_decision_ids.sort();
    let mut batch = PylonTrainingTreasuryHandoffBatch {
        schema_version: String::from(QWEN_LEGAL_PYLON_TREASURY_HANDOFF_BATCH_SCHEMA_VERSION),
        batch_id: batch_id.into(),
        payable_items,
        withheld_decision_ids,
        batch_digest: String::new(),
    };
    batch.batch_digest = batch.stable_digest();
    Ok(batch)
}

/// Attaches returned Treasury/Nexus settlement proofs to the training closeout.
pub fn attach_qwen_legal_pylon_settlement_proofs(
    closeout_id: impl Into<String>,
    batch: &PylonTrainingTreasuryHandoffBatch,
    proofs: &[PylonTrainingBitcoinSettlementProof],
    deferred_policy: Option<PylonTrainingDeferredPaymentPolicy>,
) -> Result<PylonTrainingPaymentCloseout, QwenLegalPylonTrainingJobError> {
    if batch.schema_version != QWEN_LEGAL_PYLON_TREASURY_HANDOFF_BATCH_SCHEMA_VERSION {
        return invalid_job("treasury handoff batch schema version drifted");
    }
    if batch.batch_digest != batch.stable_digest() {
        return invalid_job("treasury handoff batch digest drifted");
    }
    if deferred_policy
        .as_ref()
        .is_some_and(|policy| !valid_deferred_payment_policy(policy))
    {
        return invalid_job("deferred payment policy is not operator-approved and current");
    }
    let mut proofs = proofs.to_vec();
    proofs.sort_by(|left, right| {
        left.payout_authorization_id
            .cmp(&right.payout_authorization_id)
    });
    let item_by_authorization = batch
        .payable_items
        .iter()
        .map(|item| (item.payout_authorization_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let mut seen_proofs = BTreeSet::new();
    for proof in &proofs {
        validate_settlement_proof(proof)?;
        if !seen_proofs.insert(proof.payout_authorization_id.as_str()) {
            return invalid_job(format!(
                "duplicate settlement proof for payout authorization `{}`",
                proof.payout_authorization_id
            ));
        }
        let Some(item) = item_by_authorization.get(proof.payout_authorization_id.as_str()) else {
            return invalid_job(format!(
                "settlement proof references unknown payout authorization `{}`",
                proof.payout_authorization_id
            ));
        };
        validate_settlement_proof_matches_handoff(proof, item)?;
    }
    let proof_by_authorization = proofs
        .iter()
        .map(|proof| (proof.payout_authorization_id.as_str(), proof))
        .collect::<BTreeMap<_, _>>();
    let accepted_work_count = u32::try_from(batch.payable_items.len()).unwrap_or(u32::MAX);
    let payable_payment_count = accepted_work_count;
    let mut pending_payment_count = 0_u32;
    let mut settled_payment_count = 0_u32;
    let mut failed_payment_count = 0_u32;
    let mut deferred_payment_count = 0_u32;
    for item in &batch.payable_items {
        match proof_by_authorization.get(item.payout_authorization_id.as_str()) {
            Some(proof) => match proof.status {
                PylonTrainingBitcoinSettlementStatus::Settled => {
                    settled_payment_count = settled_payment_count.saturating_add(1);
                }
                PylonTrainingBitcoinSettlementStatus::DeferredByOperator => {
                    deferred_payment_count = deferred_payment_count.saturating_add(1);
                }
                PylonTrainingBitcoinSettlementStatus::Failed => {
                    failed_payment_count = failed_payment_count.saturating_add(1);
                }
                PylonTrainingBitcoinSettlementStatus::Pending => {
                    pending_payment_count = pending_payment_count.saturating_add(1);
                }
            },
            None => {
                if deferred_policy.is_some() {
                    deferred_payment_count = deferred_payment_count.saturating_add(1);
                } else {
                    pending_payment_count = pending_payment_count.saturating_add(1);
                }
            }
        }
    }
    let promotion_payment_gate_status =
        if accepted_work_count > 0 && settled_payment_count == accepted_work_count {
            PylonTrainingPromotionPaymentGateStatus::PaymentSettled
        } else if failed_payment_count == 0
            && pending_payment_count == 0
            && deferred_policy
                .as_ref()
                .is_some_and(valid_deferred_payment_policy)
        {
            PylonTrainingPromotionPaymentGateStatus::DeferredByOperator
        } else {
            PylonTrainingPromotionPaymentGateStatus::Blocked
        };
    let no_wallet_secrets_present = !closeout_contains_wallet_secret_material(
        batch,
        proofs.as_slice(),
        deferred_policy.as_ref(),
    );
    let mut closeout = PylonTrainingPaymentCloseout {
        schema_version: String::from(QWEN_LEGAL_PYLON_PAYMENT_CLOSEOUT_SCHEMA_VERSION),
        closeout_id: closeout_id.into(),
        batch_id: batch.batch_id.clone(),
        accepted_work_count,
        payable_payment_count,
        pending_payment_count,
        settled_payment_count,
        deferred_payment_count,
        failed_payment_count,
        proofs,
        deferred_policy,
        promotion_payment_gate_status,
        no_wallet_secrets_present,
        closeout_digest: String::new(),
    };
    closeout.closeout_digest = closeout.stable_digest();
    Ok(closeout)
}

/// Builds a deterministic settled proof fixture for a payable handoff item.
#[must_use]
pub fn settled_qwen_legal_pylon_bitcoin_proof_fixture(
    item: &PylonTrainingTreasuryHandoffItem,
) -> PylonTrainingBitcoinSettlementProof {
    let mut proof = PylonTrainingBitcoinSettlementProof {
        schema_version: String::from(QWEN_LEGAL_PYLON_BITCOIN_SETTLEMENT_PROOF_SCHEMA_VERSION),
        operation_id: format!("nexus.lightning.settle.{}", item.payout_authorization_id),
        payout_authorization_id: item.payout_authorization_id.clone(),
        worker_receipt_digest: item.worker_receipt_digest.clone(),
        amount_msat: item.payout.amount_msat,
        settlement_method: item.payout.target_kind,
        settlement_mode: PylonTrainingSettlementExecutionMode::DryRunLocal,
        status: PylonTrainingBitcoinSettlementStatus::Settled,
        payment_hash: Some(sha256_hex(item.payout_authorization_id.as_bytes())),
        payment_preimage: Some(sha256_hex(
            format!("preimage:{}", item.decision_id).as_bytes(),
        )),
        transaction_proof: None,
        bolt11_invoice: Some(item.payout.payout_target_ref.clone()),
        bolt12_offer: None,
        bip353_address: None,
        lnurl_pay_ref: None,
        fee_msat: 21,
        settled_at_ms: item.payout.expires_at_ms.saturating_sub(1_000),
        reconciliation_digest: stable_json_digest(
            b"psionic_qwen_legal_pylon_reconciliation|",
            item,
        ),
        proof_digest: String::new(),
    };
    proof.proof_digest = proof.stable_digest();
    proof
}

/// Builds a deterministic failed proof fixture for a payable handoff item.
#[must_use]
pub fn failed_qwen_legal_pylon_bitcoin_proof_fixture(
    item: &PylonTrainingTreasuryHandoffItem,
) -> PylonTrainingBitcoinSettlementProof {
    let mut proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(item);
    proof.status = PylonTrainingBitcoinSettlementStatus::Failed;
    proof.payment_preimage = None;
    proof.reconciliation_digest =
        stable_json_digest(b"psionic_qwen_legal_pylon_failed_reconciliation|", item);
    proof.proof_digest = String::new();
    proof.proof_digest = proof.stable_digest();
    proof
}

fn build_payment_decision(
    job: &PylonTrainingJobSpec,
    receipt: Option<&PylonTrainingWorkerReceipt>,
    verification: Option<&PylonTrainingWorkerReceiptVerification>,
    validation_status: PylonTrainingPaymentValidationStatus,
    duplicate_shard: bool,
    mut validation_errors: Vec<String>,
    decision_path: String,
) -> PylonTrainingPaymentDecisionReceipt {
    let worker_id = receipt.map(|receipt| receipt.worker_id.clone());
    let worker_pubkey = receipt.map(|receipt| receipt.worker_pubkey.clone());
    let input_hash = receipt
        .map(|receipt| stable_json_digest(b"psionic_pylon_observed_inputs|", &receipt.input_hashes))
        .unwrap_or_else(|| {
            stable_json_digest(
                b"psionic_pylon_expected_inputs|",
                &job.expected_input_artifacts,
            )
        });
    let output_hash = receipt.and_then(|receipt| {
        (!receipt.output_hashes.is_empty())
            .then(|| stable_json_digest(b"psionic_pylon_observed_outputs|", &receipt.output_hashes))
    });
    let mut payment_status = PylonTrainingPaymentStatus::Withheld;
    let mut payment_proof = None;
    let mut withheld_reason = None;
    if duplicate_shard {
        validation_errors.push(String::from(
            "duplicate shard already has a payable decision",
        ));
    }
    if validation_status == PylonTrainingPaymentValidationStatus::MissingReceipt {
        withheld_reason = Some(String::from("missing_worker_receipt"));
    } else if validation_status == PylonTrainingPaymentValidationStatus::Invalid {
        withheld_reason = Some(classify_invalid_receipt_reason(
            validation_errors.as_slice(),
        ));
    } else if duplicate_shard {
        withheld_reason = Some(String::from("duplicate_shard"));
    } else if let Some(receipt) = receipt {
        match receipt.status {
            PylonTrainingWorkerJobStatus::Succeeded => {
                payment_status = PylonTrainingPaymentStatus::Payable;
                payment_proof = Some(format!(
                    "pending_payment_proof:{}:{}:{}",
                    job.payment_budget.payment_account_ref,
                    job.payment_budget.budget_id,
                    job.job_id
                ));
            }
            PylonTrainingWorkerJobStatus::Failed
                if job.job_kind == PylonTrainingJobKind::EvalShard
                    && job.payment_budget.pay_failed_but_valid_eval_attempts =>
            {
                payment_status = PylonTrainingPaymentStatus::Payable;
                payment_proof = Some(format!(
                    "pending_failed_eval_attempt_payment:{}:{}:{}",
                    job.payment_budget.payment_account_ref,
                    job.payment_budget.budget_id,
                    job.job_id
                ));
            }
            PylonTrainingWorkerJobStatus::Failed => {
                withheld_reason = Some(classify_worker_failure_reason(
                    receipt.failure_reason.as_deref().unwrap_or_default(),
                ));
            }
        }
    }
    PylonTrainingPaymentDecisionReceipt {
        schema_version: String::from(QWEN_LEGAL_PYLON_PAYMENT_DECISION_SCHEMA_VERSION),
        decision_id: format!("decision.{}.payment", job.job_id),
        decision_path,
        job_id: job.job_id.clone(),
        parent_run_id: job.parent_run_id.clone(),
        work_type: job.job_kind,
        shard_key: pylon_payment_shard_key(job),
        worker_id,
        worker_pubkey,
        budget_id: job.payment_budget.budget_id.clone(),
        agreed_price_microusd: job.payment_budget.agreed_price_microusd,
        budget_max_microusd: job.payment_budget.max_cost_microusd,
        currency: job.payment_budget.currency.clone(),
        payment_account_ref: job.payment_budget.payment_account_ref.clone(),
        bitcoin_payout: job.payment_budget.bitcoin_payout.clone(),
        job_spec_digest: job.stable_digest(),
        worker_receipt_digest: receipt.map(|receipt| receipt.receipt_digest.clone()),
        signed_payload_digest: receipt.map(|receipt| receipt.signed_payload_digest.clone()),
        input_hash,
        output_hash,
        validation_status,
        output_files_rechecked: verification
            .is_some_and(|verification| verification.output_files_rechecked),
        duplicate_shard,
        payment_status,
        payment_proof,
        withheld_reason,
        payment_rules_applied: vec![
            String::from("withhold_wrong_input_hash"),
            String::from("withhold_missing_output"),
            String::from("withhold_invalid_receipt"),
            String::from("withhold_corrupted_artifact"),
            String::from("withhold_duplicate_shard"),
            String::from("withhold_failed_integrity_validation"),
            String::from("pay_failed_valid_eval_attempts_only_when_job_budget_allows"),
        ],
        validation_errors,
        decision_digest: String::new(),
    }
}

fn validate_settlement_proof(
    proof: &PylonTrainingBitcoinSettlementProof,
) -> Result<(), QwenLegalPylonTrainingJobError> {
    if proof.schema_version != QWEN_LEGAL_PYLON_BITCOIN_SETTLEMENT_PROOF_SCHEMA_VERSION {
        return invalid_job("settlement proof schema version drifted");
    }
    require_nonempty(proof.operation_id.as_str(), "settlement operation_id")?;
    require_nonempty(
        proof.payout_authorization_id.as_str(),
        "settlement payout_authorization_id",
    )?;
    require_nonempty(
        proof.worker_receipt_digest.as_str(),
        "settlement worker_receipt_digest",
    )?;
    require_nonempty(
        proof.reconciliation_digest.as_str(),
        "settlement reconciliation_digest",
    )?;
    if proof.amount_msat == 0 {
        return invalid_job("settlement amount_msat must be greater than zero");
    }
    if proof.proof_digest != proof.stable_digest() {
        return invalid_job("settlement proof digest drifted");
    }
    if proof.settlement_mode == PylonTrainingSettlementExecutionMode::LiveSmallValueOperatorApproved
        && proof.status == PylonTrainingBitcoinSettlementStatus::Settled
    {
        if proof.settled_at_ms == 0 {
            return invalid_job("live settled proof must carry a settlement time");
        }
        if proof.payment_hash.is_none() && proof.transaction_proof.is_none() {
            return invalid_job(
                "live settled proof must carry a payment hash or transaction proof",
            );
        }
    }
    if proof.status == PylonTrainingBitcoinSettlementStatus::Settled
        && proof.payment_hash.is_none()
        && proof.transaction_proof.is_none()
        && proof.bolt11_invoice.is_none()
        && proof.bolt12_offer.is_none()
        && proof.bip353_address.is_none()
        && proof.lnurl_pay_ref.is_none()
    {
        return invalid_job("settled proof must carry a payment hash or method-specific proof");
    }
    if settlement_proof_contains_wallet_secret_material(proof) {
        return invalid_job("settlement proof must not contain wallet secret material");
    }
    Ok(())
}

fn validate_settlement_proof_matches_handoff(
    proof: &PylonTrainingBitcoinSettlementProof,
    item: &PylonTrainingTreasuryHandoffItem,
) -> Result<(), QwenLegalPylonTrainingJobError> {
    if proof.worker_receipt_digest != item.worker_receipt_digest {
        return invalid_job(format!(
            "settlement proof worker receipt digest mismatch for payout authorization `{}`",
            proof.payout_authorization_id
        ));
    }
    if proof.amount_msat != item.payout.amount_msat {
        return invalid_job(format!(
            "settlement proof amount mismatch for payout authorization `{}`",
            proof.payout_authorization_id
        ));
    }
    if proof.settlement_method != item.payout.target_kind {
        return invalid_job(format!(
            "settlement proof method mismatch for payout authorization `{}`",
            proof.payout_authorization_id
        ));
    }
    Ok(())
}

fn valid_deferred_payment_policy(policy: &PylonTrainingDeferredPaymentPolicy) -> bool {
    !policy.policy_id.trim().is_empty()
        && !policy.operator_id.trim().is_empty()
        && !policy.reason.trim().is_empty()
        && policy.expires_at_ms > policy.approved_at_ms
        && !contains_wallet_secret_material(policy.policy_id.as_str())
        && !contains_wallet_secret_material(policy.operator_id.as_str())
        && !contains_wallet_secret_material(policy.reason.as_str())
}

fn closeout_contains_wallet_secret_material(
    batch: &PylonTrainingTreasuryHandoffBatch,
    proofs: &[PylonTrainingBitcoinSettlementProof],
    deferred_policy: Option<&PylonTrainingDeferredPaymentPolicy>,
) -> bool {
    batch.payable_items.iter().any(|item| {
        contains_wallet_secret_material(item.payout.payout_target_ref.as_str())
            || contains_wallet_secret_material(item.payout.fee_policy.as_str())
            || contains_wallet_secret_material(item.payout.payout_authorization_id.as_str())
    }) || proofs
        .iter()
        .any(settlement_proof_contains_wallet_secret_material)
        || deferred_policy.is_some_and(|policy| !valid_deferred_payment_policy(policy))
}

fn settlement_proof_contains_wallet_secret_material(
    proof: &PylonTrainingBitcoinSettlementProof,
) -> bool {
    [
        Some(proof.operation_id.as_str()),
        Some(proof.payout_authorization_id.as_str()),
        proof.payment_hash.as_deref(),
        proof.payment_preimage.as_deref(),
        proof.transaction_proof.as_deref(),
        proof.bolt11_invoice.as_deref(),
        proof.bolt12_offer.as_deref(),
        proof.bip353_address.as_deref(),
        proof.lnurl_pay_ref.as_deref(),
        Some(proof.reconciliation_digest.as_str()),
    ]
    .into_iter()
    .flatten()
    .any(contains_wallet_secret_material)
}

fn payment_decision_path(job: &PylonTrainingJobSpec) -> PathBuf {
    resolve_workspace_path(payment_decision_receipt_path(job))
}

fn payment_decision_receipt_path(job: &PylonTrainingJobSpec) -> String {
    Path::new(job.output_dir.as_str())
        .join("settlements")
        .join(format!("{}.payment_decision.json", job.job_id))
        .display()
        .to_string()
}

fn duplicate_payable_shard_exists(
    job: &PylonTrainingJobSpec,
    receipt: &PylonTrainingWorkerReceipt,
    decision_path: &Path,
) -> bool {
    let Some(dir) = decision_path.parent() else {
        return false;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    let shard_key = pylon_payment_shard_key(job);
    for entry in entries.flatten() {
        let path = entry.path();
        if path == decision_path || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(decision) = read_json::<PylonTrainingPaymentDecisionReceipt>(path.as_path()) else {
            continue;
        };
        if decision.shard_key == shard_key
            && decision.payment_status == PylonTrainingPaymentStatus::Payable
            && decision.worker_receipt_digest.as_deref() != Some(receipt.receipt_digest.as_str())
        {
            return true;
        }
    }
    false
}

fn pylon_payment_shard_key(job: &PylonTrainingJobSpec) -> String {
    format!(
        "{}:{:?}:{}:{}",
        job.parent_run_id,
        job.job_kind,
        job.shard_assignment.shard_id,
        job.shard_assignment.shard_index
    )
}

fn classify_invalid_receipt_reason(errors: &[String]) -> String {
    let joined = errors.join(" | ");
    if joined.contains("hash drifted") {
        String::from("corrupted_artifact")
    } else if joined.contains("signature") || joined.contains("digest drifted") {
        String::from("invalid_receipt")
    } else if joined.contains("missing worker receipt") {
        String::from("missing_worker_receipt")
    } else {
        String::from("failed_integrity_validation")
    }
}

fn classify_worker_failure_reason(reason: &str) -> String {
    if reason.contains("hash mismatch") {
        String::from("wrong_input_hash")
    } else if reason.contains("required output artifact") {
        String::from("missing_output")
    } else {
        String::from("failed_integrity_validation")
    }
}

fn validate_inputs(job: &PylonTrainingJobSpec) -> Result<(), String> {
    for input in &job.expected_input_artifacts {
        let observed = sha256_file(input.path.as_str()).map_err(|error| error.to_string())?;
        if observed != input.sha256 {
            return Err(format!(
                "input artifact `{}` hash mismatch: expected {}, observed {}",
                input.artifact_id, input.sha256, observed
            ));
        }
    }
    Ok(())
}

fn observed_input_hashes(job: &PylonTrainingJobSpec) -> Vec<PylonTrainingArtifactRef> {
    job.expected_input_artifacts
        .iter()
        .map(|input| {
            let sha256 = sha256_file(input.path.as_str()).unwrap_or_else(|_| String::new());
            PylonTrainingArtifactRef {
                artifact_id: input.artifact_id.clone(),
                artifact_type: input.artifact_type.clone(),
                path: input.path.clone(),
                sha256,
            }
        })
        .collect()
}

fn collect_output_hashes(
    job: &PylonTrainingJobSpec,
) -> Result<Vec<PylonTrainingOutputArtifactRef>, String> {
    let mut output_hashes = Vec::new();
    for expected in &job.expected_output_artifacts {
        let path = resolve_workspace_path(expected.path.as_str());
        if !path.is_file() {
            if expected.required {
                return Err(format!(
                    "required output artifact `{}` was not written",
                    expected.artifact_id
                ));
            }
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        output_hashes.push(PylonTrainingOutputArtifactRef {
            artifact_id: expected.artifact_id.clone(),
            artifact_type: expected.artifact_type.clone(),
            path: expected.path.clone(),
            sha256: sha256_hex(bytes.as_slice()),
            byte_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        });
    }
    for required_type in &job.receipt_requirements.required_output_artifact_types {
        if !output_hashes
            .iter()
            .any(|output| output.artifact_type == *required_type)
        {
            return Err(format!(
                "required output artifact type `{required_type}` missing"
            ));
        }
    }
    Ok(output_hashes)
}

fn write_expected_output(
    job: &PylonTrainingJobSpec,
    expected: &PylonTrainingExpectedOutputArtifact,
) -> Result<(), QwenLegalPylonTrainingJobError> {
    let path = resolve_workspace_path(expected.path.as_str());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| QwenLegalPylonTrainingJobError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    let output = serde_json::json!({
        "schema_version": "psionic.qwen_legal_pylon_worker_output.v1",
        "producer": QWEN_LEGAL_PYLON_LOCAL_WORKER_IMPL_ID,
        "job_id": job.job_id,
        "parent_run_id": job.parent_run_id,
        "job_kind": job.job_kind,
        "artifact_id": expected.artifact_id,
        "artifact_type": expected.artifact_type,
        "model_id": job.model_id,
        "model_hash": job.model_hash,
        "dataset_manifest_hash": job.dataset_manifest_hash,
        "training_config_hash": job.training_config_hash,
        "shard_assignment": job.shard_assignment,
        "claim_boundary": "Local Pylon worker protocol smoke output. This proves job intake, hash checks, output materialization, and receipt signing; it does not claim live distributed Qwen training.",
    });
    let bytes = serde_json::to_vec_pretty(&output).map_err(|error| {
        QwenLegalPylonTrainingJobError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(&path, bytes).map_err(|error| QwenLegalPylonTrainingJobError::Io {
        path: expected.path.clone(),
        message: error.to_string(),
    })
}

fn sign_receipt(receipt: &mut PylonTrainingWorkerReceipt, signing_key: &SigningKey) {
    receipt.signed_payload_digest = receipt.signable_payload_digest();
    let signature = signing_key.sign(receipt.signed_payload_digest.as_bytes());
    receipt.signature_hex = hex::encode(signature.to_bytes());
    receipt.receipt_digest = receipt.stable_digest();
}

fn local_hardware_summary() -> PylonTrainingWorkerHardwareSummary {
    PylonTrainingWorkerHardwareSummary {
        host_id: std::env::var("HOSTNAME").unwrap_or_else(|_| String::from("localhost")),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        backend_label: String::from("local_protocol_smoke"),
        total_memory_bytes: 8 * 1024 * 1024 * 1024,
        accelerator: String::from("not_required_for_protocol_smoke"),
    }
}

fn deterministic_worker_signing_key(worker_id: &str, job_id: &str) -> SigningKey {
    let digest = Sha256::digest(format!("qwen-legal-pylon-worker|{worker_id}|{job_id}").as_bytes());
    let mut secret = [0_u8; 32];
    secret.copy_from_slice(&digest[..32]);
    SigningKey::from_bytes(&secret)
}

fn decode_verifying_key(
    public_key_hex: &str,
) -> Result<VerifyingKey, QwenLegalPylonTrainingJobError> {
    let bytes = hex::decode(public_key_hex).map_err(|error| {
        QwenLegalPylonTrainingJobError::InvalidReceipt {
            detail: format!("invalid worker public key hex: {error}"),
        }
    })?;
    let bytes: [u8; 32] =
        bytes
            .try_into()
            .map_err(|_| QwenLegalPylonTrainingJobError::InvalidReceipt {
                detail: String::from("worker public key must be 32 bytes"),
            })?;
    VerifyingKey::from_bytes(&bytes).map_err(|error| {
        QwenLegalPylonTrainingJobError::InvalidReceipt {
            detail: format!("invalid worker public key: {error}"),
        }
    })
}

fn decode_signature(signature_hex: &str) -> Result<Signature, QwenLegalPylonTrainingJobError> {
    let bytes = hex::decode(signature_hex).map_err(|error| {
        QwenLegalPylonTrainingJobError::InvalidReceipt {
            detail: format!("invalid signature hex: {error}"),
        }
    })?;
    let bytes: [u8; 64] =
        bytes
            .try_into()
            .map_err(|_| QwenLegalPylonTrainingJobError::InvalidReceipt {
                detail: String::from("signature must be 64 bytes"),
            })?;
    Ok(Signature::from_bytes(&bytes))
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, QwenLegalPylonTrainingJobError> {
    let resolved = resolve_workspace_path(path);
    let bytes = fs::read(&resolved).map_err(|error| QwenLegalPylonTrainingJobError::Io {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|error| QwenLegalPylonTrainingJobError::Json {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), QwenLegalPylonTrainingJobError> {
    let resolved = resolve_workspace_path(path);
    if let Some(parent) = resolved.parent() {
        fs::create_dir_all(parent).map_err(|error| QwenLegalPylonTrainingJobError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        QwenLegalPylonTrainingJobError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(&resolved, bytes).map_err(|error| QwenLegalPylonTrainingJobError::Io {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })
}

fn sha256_file(path: &str) -> Result<String, QwenLegalPylonTrainingJobError> {
    let resolved = resolve_workspace_path(path);
    let bytes = fs::read(&resolved).map_err(|error| QwenLegalPylonTrainingJobError::Io {
        path: resolved.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn resolve_workspace_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        return path.to_path_buf();
    }
    workspace_root().join(path)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_json_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn default_pylon_bitcoin_payout_target() -> PylonTrainingBitcoinPayoutTarget {
    PylonTrainingBitcoinPayoutTarget {
        payout_target_ref: String::from("bolt11://qwen-legal-pylon-local-smoke"),
        target_kind: PylonTrainingBitcoinPayoutTargetKind::Bolt11Invoice,
        amount_msat: 25_000,
        currency: String::from("msat"),
        expires_at_ms: 4_102_444_800_000,
        fee_policy: String::from("fee_policy.qwen_legal.max_250msat"),
        payout_authorization_id: String::from("auth.qwen-legal-pylon.local-smoke"),
    }
}

/// Builds a deterministic non-secret Bitcoin/Lightning payout target for one job.
#[must_use]
pub fn pylon_bitcoin_payout_target_for_job(
    job_id: &str,
    amount_msat: u64,
) -> PylonTrainingBitcoinPayoutTarget {
    let digest = sha256_hex(job_id.as_bytes());
    let short_digest = &digest[..16];
    PylonTrainingBitcoinPayoutTarget {
        payout_target_ref: format!("bolt11://qwen-legal-pylon/{short_digest}"),
        target_kind: PylonTrainingBitcoinPayoutTargetKind::Bolt11Invoice,
        amount_msat,
        currency: String::from("msat"),
        expires_at_ms: 4_102_444_800_000,
        fee_policy: String::from("fee_policy.qwen_legal.max_250msat"),
        payout_authorization_id: format!("auth.qwen-legal-pylon.{short_digest}"),
    }
}

fn contains_wallet_secret_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "seed",
        "xprv",
        "mnemonic",
        "channel_monitor",
        "private_key",
        "wallet_secret",
        "node_secret",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn require_nonempty(value: &str, field: &str) -> Result<(), QwenLegalPylonTrainingJobError> {
    if value.trim().is_empty() {
        return invalid_job(format!("{field} must be present"));
    }
    Ok(())
}

fn invalid_job<T>(detail: impl Into<String>) -> Result<T, QwenLegalPylonTrainingJobError> {
    Err(QwenLegalPylonTrainingJobError::InvalidJob {
        detail: detail.into(),
    })
}

fn invalid_receipt<T>(detail: impl Into<String>) -> Result<T, QwenLegalPylonTrainingJobError> {
    Err(QwenLegalPylonTrainingJobError::InvalidReceipt {
        detail: detail.into(),
    })
}

/// Builds the canonical dataset-shard protocol fixture.
#[must_use]
pub fn canonical_qwen_legal_dataset_shard_job() -> PylonTrainingJobSpec {
    canonical_job(
        "job.qwen-legal.dataset-shard.000001",
        PylonTrainingJobKind::DatasetShardBuild,
        "dataset_shard_manifest",
        "target/legal/pylon_jobs/dataset_shard_manifest.json",
        vec![PylonTrainingArtifactRef {
            artifact_id: String::from("qwen36-grpo-config"),
            artifact_type: String::from("training_config"),
            path: String::from("configs/legal/qwen36_grpo_smoke.json"),
            sha256: String::from(
                "a1e896ad711ccce93bcf9e3efa4b49d3ef461e397f3b02348debc5740b165a02",
            ),
        }],
    )
}

/// Builds the canonical eval-shard protocol fixture.
#[must_use]
pub fn canonical_qwen_legal_eval_shard_job() -> PylonTrainingJobSpec {
    canonical_job(
        "job.qwen-legal.eval-shard.000001",
        PylonTrainingJobKind::EvalShard,
        "eval_shard_report",
        "target/legal/pylon_jobs/eval_shard_report.json",
        vec![PylonTrainingArtifactRef {
            artifact_id: String::from("harvey-public-three-suite"),
            artifact_type: String::from("eval_suite"),
            path: String::from("suites/harvey_public_three.json"),
            sha256: String::from(
                "d029bb19d1033acf192a9993025838c0d65478d4595e2ee6985c2be919cf2181",
            ),
        }],
    )
}

/// Builds the canonical Pylon legal job set.
#[must_use]
pub fn canonical_qwen_legal_pylon_training_jobs() -> Vec<PylonTrainingJobSpec> {
    vec![
        canonical_qwen_legal_dataset_shard_job(),
        canonical_qwen_legal_eval_shard_job(),
    ]
}

fn canonical_job(
    job_id: &str,
    job_kind: PylonTrainingJobKind,
    output_artifact_type: &str,
    output_path: &str,
    expected_input_artifacts: Vec<PylonTrainingArtifactRef>,
) -> PylonTrainingJobSpec {
    PylonTrainingJobSpec {
        schema_version: String::from(QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION),
        job_id: String::from(job_id),
        parent_run_id: String::from("run.qwen-legal.pylon.protocol.000001"),
        job_kind,
        model_id: String::from("Qwen/Qwen3.6-27B"),
        model_hash: String::from("sha256:synthetic-qwen36-legal-smoke"),
        adapter_id: Some(String::from("qwen36-27b-legal-grpo-smoke")),
        adapter_hash: Some(String::from(
            "825b2d81aeae56d395a4fee7608eead91adf25ac24bae9ff995959df2b95732f",
        )),
        dataset_manifest_hash: String::from(
            "sha256:dataset-manifest-qwen-legal-pylon-protocol-smoke",
        ),
        shard_assignment: PylonTrainingShardAssignment {
            assignment_id: format!("assignment.{job_id}"),
            shard_id: String::from("shard.qwen-legal.public-smoke.000001"),
            shard_index: 0,
            shard_count: 1,
            start_index: Some(0),
            end_index: Some(3),
        },
        training_config_hash: String::from(
            "a1e896ad711ccce93bcf9e3efa4b49d3ef461e397f3b02348debc5740b165a02",
        ),
        expected_input_artifacts,
        expected_output_artifacts: vec![PylonTrainingExpectedOutputArtifact {
            artifact_id: format!("artifact.{job_id}.output"),
            artifact_type: String::from(output_artifact_type),
            path: String::from(output_path),
            required: true,
        }],
        max_runtime_ms: 60_000,
        hardware_requirements: PylonTrainingHardwareRequirements {
            min_memory_bytes: 512 * 1024 * 1024,
            require_accelerator: false,
            accepted_backend_labels: vec![String::from("local_protocol_smoke")],
        },
        payment_budget: PylonTrainingPaymentBudget {
            budget_id: String::from("budget.qwen-legal.pylon.protocol.000001"),
            agreed_price_microusd: 2_500,
            max_cost_microusd: 2_500,
            currency: String::from("USD"),
            payment_account_ref: String::from("ledger://local-smoke/qwen-legal-pylon"),
            bitcoin_payout: default_pylon_bitcoin_payout_target(),
            pay_failed_but_valid_eval_attempts: false,
        },
        receipt_requirements: PylonTrainingReceiptRequirements {
            require_signature: true,
            require_logs_hash: true,
            require_metrics: true,
            required_output_artifact_types: vec![String::from(output_artifact_type)],
        },
        output_dir: String::from("target/legal/pylon_jobs"),
        receipt_path: format!("target/legal/pylon_jobs/{job_id}.receipt.json"),
    }
}

/// Writes canonical Pylon legal job fixtures.
pub fn write_canonical_qwen_legal_pylon_training_job_fixtures(
    output_dir: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, QwenLegalPylonTrainingJobError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| QwenLegalPylonTrainingJobError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;
    let jobs = [
        (
            output_dir.join("dataset_shard_job_v1.json"),
            canonical_qwen_legal_dataset_shard_job(),
        ),
        (
            output_dir.join("eval_shard_job_v1.json"),
            canonical_qwen_legal_eval_shard_job(),
        ),
    ];
    let mut paths = Vec::new();
    for (path, job) in jobs {
        write_json(path.as_path(), &job)?;
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payable_decision_fixture()
    -> Result<(tempfile::TempDir, PylonTrainingPaymentDecisionReceipt), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();

        run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        let decision = settle_qwen_legal_pylon_training_job_spec(&job)?;
        assert_eq!(decision.payment_status, PylonTrainingPaymentStatus::Payable);
        Ok((temp, decision))
    }

    #[test]
    fn pylon_worker_accepts_dataset_shard_and_verifies_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();

        let receipt =
            run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        assert_eq!(receipt.status, PylonTrainingWorkerJobStatus::Succeeded);
        assert_eq!(receipt.job_kind, PylonTrainingJobKind::DatasetShardBuild);
        assert_eq!(receipt.output_hashes.len(), 1);
        let verification = verify_qwen_legal_pylon_worker_receipt_path(&job.receipt_path)?;
        assert!(verification.signature_valid);
        assert!(verification.output_files_rechecked);
        Ok(())
    }

    #[test]
    fn pylon_worker_accepts_eval_shard_and_verifies_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_eval_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("eval_report.json").display().to_string();

        let receipt =
            run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        assert_eq!(receipt.status, PylonTrainingWorkerJobStatus::Succeeded);
        assert_eq!(receipt.job_kind, PylonTrainingJobKind::EvalShard);
        let verification = verify_qwen_legal_pylon_worker_receipt_path(&job.receipt_path)?;
        assert!(verification.signature_valid);
        Ok(())
    }

    #[test]
    fn pylon_worker_fails_wrong_input_hash() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();
        job.expected_input_artifacts[0].sha256 =
            String::from("0000000000000000000000000000000000000000000000000000000000000000");

        let receipt =
            run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        assert_eq!(receipt.status, PylonTrainingWorkerJobStatus::Failed);
        assert!(
            receipt
                .failure_reason
                .as_deref()
                .unwrap_or_default()
                .contains("hash mismatch")
        );
        verify_qwen_legal_pylon_worker_receipt_path(&job.receipt_path)?;
        Ok(())
    }

    #[test]
    fn pylon_worker_fails_missing_output_artifact() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_eval_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path = temp
            .path()
            .join("missing_eval_report.json")
            .display()
            .to_string();
        let options = PylonLocalWorkerRunOptions {
            emit_outputs: false,
            ..PylonLocalWorkerRunOptions::default()
        };

        let receipt = run_qwen_legal_pylon_worker_job(&job, &options)?;
        assert_eq!(receipt.status, PylonTrainingWorkerJobStatus::Failed);
        assert!(
            receipt
                .failure_reason
                .as_deref()
                .unwrap_or_default()
                .contains("required output artifact")
        );
        verify_qwen_legal_pylon_worker_receipt_path(&job.receipt_path)?;
        Ok(())
    }

    #[test]
    fn pylon_payment_settlement_marks_valid_receipt_payable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();

        run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        let decision = settle_qwen_legal_pylon_training_job_spec(&job)?;
        assert_eq!(
            decision.validation_status,
            PylonTrainingPaymentValidationStatus::Valid
        );
        assert_eq!(decision.payment_status, PylonTrainingPaymentStatus::Payable);
        assert_eq!(decision.agreed_price_microusd, 2_500);
        assert!(decision.payment_proof.is_some());
        assert!(Path::new(&decision.decision_path).is_file());
        Ok(())
    }

    #[test]
    fn pylon_treasury_handoff_batch_emits_payable_item() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;

        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;

        assert_eq!(
            batch.schema_version,
            QWEN_LEGAL_PYLON_TREASURY_HANDOFF_BATCH_SCHEMA_VERSION
        );
        assert_eq!(batch.payable_items.len(), 1);
        assert!(batch.withheld_decision_ids.is_empty());
        assert_eq!(batch.batch_digest, batch.stable_digest());
        let item = &batch.payable_items[0];
        assert_eq!(item.decision_id, decision.decision_id);
        assert_eq!(item.payout.amount_msat, decision.bitcoin_payout.amount_msat);
        assert_eq!(
            item.payout_authorization_id,
            decision.bitcoin_payout.payout_authorization_id
        );
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_attaches_settled_proof_and_opens_promotion_gate()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);

        let closeout = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )?;

        assert_eq!(closeout.accepted_work_count, 1);
        assert_eq!(closeout.payable_payment_count, 1);
        assert_eq!(closeout.pending_payment_count, 0);
        assert_eq!(closeout.settled_payment_count, 1);
        assert_eq!(closeout.failed_payment_count, 0);
        assert_eq!(
            closeout.promotion_payment_gate_status,
            PylonTrainingPromotionPaymentGateStatus::PaymentSettled
        );
        assert!(closeout.no_wallet_secrets_present);
        assert_eq!(closeout.closeout_digest, closeout.stable_digest());
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_allows_operator_deferred_policy()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let policy = PylonTrainingDeferredPaymentPolicy {
            policy_id: String::from("defer.qwen-legal.operator-approved.001"),
            operator_id: String::from("operator.legal-training"),
            reason: String::from("treasury window opens after adapter verification"),
            approved_at_ms: 1_800_000_000_000,
            expires_at_ms: 1_800_086_400_000,
        };

        let closeout = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[],
            Some(policy),
        )?;

        assert_eq!(closeout.accepted_work_count, 1);
        assert_eq!(closeout.payable_payment_count, 1);
        assert_eq!(closeout.pending_payment_count, 0);
        assert_eq!(closeout.deferred_payment_count, 1);
        assert_eq!(
            closeout.promotion_payment_gate_status,
            PylonTrainingPromotionPaymentGateStatus::DeferredByOperator
        );
        assert!(closeout.no_wallet_secrets_present);
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_blocks_failed_payment() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let proof = failed_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);

        let closeout = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )?;

        assert_eq!(closeout.failed_payment_count, 1);
        assert_eq!(closeout.pending_payment_count, 0);
        assert_eq!(
            closeout.promotion_payment_gate_status,
            PylonTrainingPromotionPaymentGateStatus::Blocked
        );
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_accepts_operator_approved_live_proof()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let mut proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);
        proof.settlement_mode =
            PylonTrainingSettlementExecutionMode::LiveSmallValueOperatorApproved;
        proof.transaction_proof = Some(format!(
            "treasury.txproof.{}",
            proof.payment_hash.as_deref().unwrap_or_default()
        ));
        proof.proof_digest = String::new();
        proof.proof_digest = proof.stable_digest();

        let closeout = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )?;

        assert_eq!(closeout.payable_payment_count, 1);
        assert_eq!(closeout.settled_payment_count, 1);
        assert_eq!(
            closeout.promotion_payment_gate_status,
            PylonTrainingPromotionPaymentGateStatus::PaymentSettled
        );
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_rejects_duplicate_payment_proof()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);

        let err = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof.clone(), proof],
            None,
        )
        .expect_err("duplicate proof should be rejected");

        assert!(err.to_string().contains("duplicate settlement proof"));
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_rejects_bad_payment_proof_digest()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let mut proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);
        proof.fee_msat = proof.fee_msat.saturating_add(1);

        let err = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )
        .expect_err("digest drift should be rejected");

        assert!(err.to_string().contains("settlement proof digest drifted"));
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_rejects_unknown_payment_authorization()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let mut proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);
        proof.payout_authorization_id = String::from("auth.qwen-legal-pylon.unknown");
        proof.proof_digest = String::new();
        proof.proof_digest = proof.stable_digest();

        let err = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )
        .expect_err("unknown authorization should be rejected");

        assert!(err.to_string().contains("unknown payout authorization"));
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_rejects_secret_looking_payment_proof_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let mut proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);
        proof.transaction_proof = Some(String::from("wallet_secret=do-not-store"));
        proof.proof_digest = String::new();
        proof.proof_digest = proof.stable_digest();

        let err = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )
        .expect_err("secret-looking proof fields should be rejected");

        assert!(err.to_string().contains("wallet secret material"));
        Ok(())
    }

    #[test]
    fn pylon_treasury_closeout_rejects_mismatched_proof_amount()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, decision) = payable_decision_fixture()?;
        let batch = build_qwen_legal_pylon_treasury_handoff_batch(
            "treasury.qwen-legal.batch.001",
            std::slice::from_ref(&decision),
        )?;
        let mut proof = settled_qwen_legal_pylon_bitcoin_proof_fixture(&batch.payable_items[0]);
        proof.amount_msat = proof.amount_msat.saturating_add(1);
        proof.proof_digest = String::new();
        proof.proof_digest = proof.stable_digest();

        let err = attach_qwen_legal_pylon_settlement_proofs(
            "closeout.qwen-legal.batch.001",
            &batch,
            &[proof],
            None,
        )
        .expect_err("mismatched amount should be rejected");

        assert!(err.to_string().contains("amount mismatch"));
        Ok(())
    }

    #[test]
    fn pylon_payment_settlement_withholds_invalid_receipt() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();
        job.expected_input_artifacts[0].sha256 =
            String::from("0000000000000000000000000000000000000000000000000000000000000000");

        run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        let decision = settle_qwen_legal_pylon_training_job_spec(&job)?;
        assert_eq!(
            decision.validation_status,
            PylonTrainingPaymentValidationStatus::Valid
        );
        assert_eq!(
            decision.payment_status,
            PylonTrainingPaymentStatus::Withheld
        );
        assert_eq!(
            decision.withheld_reason.as_deref(),
            Some("wrong_input_hash")
        );
        Ok(())
    }

    #[test]
    fn pylon_payment_settlement_withholds_missing_receipt() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp
            .path()
            .join("missing_receipt.json")
            .display()
            .to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();

        let decision = settle_qwen_legal_pylon_training_job_spec(&job)?;
        assert_eq!(
            decision.validation_status,
            PylonTrainingPaymentValidationStatus::MissingReceipt
        );
        assert_eq!(
            decision.payment_status,
            PylonTrainingPaymentStatus::Withheld
        );
        assert_eq!(
            decision.withheld_reason.as_deref(),
            Some("missing_worker_receipt")
        );
        Ok(())
    }

    #[test]
    fn pylon_payment_settlement_withholds_duplicate_shard() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let mut first = canonical_qwen_legal_dataset_shard_job();
        first.output_dir = temp.path().display().to_string();
        first.receipt_path = temp.path().join("first_receipt.json").display().to_string();
        first.expected_output_artifacts[0].path = temp
            .path()
            .join("first_dataset_shard.json")
            .display()
            .to_string();
        run_qwen_legal_pylon_worker_job(&first, &PylonLocalWorkerRunOptions::default())?;
        let first_decision = settle_qwen_legal_pylon_training_job_spec(&first)?;
        assert_eq!(
            first_decision.payment_status,
            PylonTrainingPaymentStatus::Payable
        );

        let mut duplicate = first.clone();
        duplicate.job_id = String::from("job.qwen-legal.dataset-shard.duplicate");
        duplicate.receipt_path = temp
            .path()
            .join("second_receipt.json")
            .display()
            .to_string();
        duplicate.expected_output_artifacts[0].artifact_id =
            String::from("artifact.job.qwen-legal.dataset-shard.duplicate.output");
        duplicate.expected_output_artifacts[0].path = temp
            .path()
            .join("second_dataset_shard.json")
            .display()
            .to_string();
        let options = PylonLocalWorkerRunOptions {
            worker_id: String::from("pylon.local.qwen-legal.duplicate"),
            ..PylonLocalWorkerRunOptions::default()
        };
        run_qwen_legal_pylon_worker_job(&duplicate, &options)?;
        let duplicate_decision = settle_qwen_legal_pylon_training_job_spec(&duplicate)?;
        assert!(duplicate_decision.duplicate_shard);
        assert_eq!(
            duplicate_decision.payment_status,
            PylonTrainingPaymentStatus::Withheld
        );
        assert_eq!(
            duplicate_decision.withheld_reason.as_deref(),
            Some("duplicate_shard")
        );
        Ok(())
    }

    #[test]
    fn pylon_receipt_rejects_bad_signature() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let mut job = canonical_qwen_legal_dataset_shard_job();
        job.output_dir = temp.path().display().to_string();
        job.receipt_path = temp.path().join("receipt.json").display().to_string();
        job.expected_output_artifacts[0].path =
            temp.path().join("dataset_shard.json").display().to_string();
        let mut receipt =
            run_qwen_legal_pylon_worker_job(&job, &PylonLocalWorkerRunOptions::default())?;
        receipt.signature_hex = "00".repeat(64);
        receipt.receipt_digest = receipt.stable_digest();

        assert!(
            verify_qwen_legal_pylon_worker_receipt(&receipt, Path::new("receipt.json")).is_err()
        );
        Ok(())
    }
}
