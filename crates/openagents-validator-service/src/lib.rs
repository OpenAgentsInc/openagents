//! Validator challenge execution substrate for OpenAgents compute.

use std::collections::{BTreeMap, VecDeque};

use psionic_runtime::ExecutionProofBundle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable protocol identifier for the first GPU challenge protocol.
pub const GPU_FREIVALDS_MERKLE_PROTOCOL_ID: &str = "openagents.validator.gpu_freivalds_merkle.v1";

/// Default finite field modulus used by the first challenge protocol.
pub const GPU_FREIVALDS_FIELD_MODULUS: i64 = 2_147_483_647;

/// Service-level failure for challenge queueing and lease execution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidatorServiceError {
    /// The caller attempted to enqueue a duplicate challenge.
    #[error("validator challenge `{0}` already exists")]
    DuplicateChallenge(String),
    /// The caller referenced a challenge the service does not know about.
    #[error("validator challenge `{0}` was not found")]
    UnknownChallenge(String),
    /// The caller supplied a lease that is no longer valid.
    #[error("validator challenge lease `{0}` is invalid")]
    InvalidLease(String),
    /// The caller supplied malformed witness data.
    #[error("validator challenge witness matrix `{0}` is invalid")]
    InvalidMatrix(String),
}

/// Public lifecycle state for one validator challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorChallengeStatus {
    /// The challenge is waiting for a validator lease.
    Queued,
    /// The challenge is currently leased to one validator worker.
    Leased,
    /// The challenge was requeued after a retryable interruption.
    Retrying,
    /// The challenge verified successfully.
    Verified,
    /// The challenge rejected the claimed computation.
    Rejected,
    /// The challenge exhausted its lease or retry window.
    TimedOut,
}

/// Final or interim verdict produced by one validator action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorChallengeVerdict {
    /// The validator confirmed the claimed computation.
    Verified,
    /// The validator rejected the claimed computation.
    Rejected,
    /// The challenge was requeued for another attempt.
    RetryScheduled,
    /// The challenge exhausted its retry or lease window.
    TimedOut,
}

/// Machine-legible reason codes for validator challenge failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorChallengeFailureCode {
    /// Left/right/result matrix dimensions are inconsistent.
    DimensionMismatch,
    /// Field moduli do not agree across the committed matrices.
    FieldMismatch,
    /// At least one expected row opening was not supplied.
    RowOpeningMissing,
    /// One row opening does not verify against its Merkle root.
    MerkleProofInvalid,
    /// Freivalds verification rejected the claimed result.
    FreivaldsMismatch,
    /// The current validator lease expired before adjudication completed.
    LeaseExpired,
    /// The service exhausted the retry budget after repeated interruptions.
    RetryBudgetExhausted,
}

/// Supported validator challenge protocols.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorChallengeProtocolKind {
    /// Freivalds verification over Merkle-committed matrices.
    GpuFreivaldsMerkleV1,
}

impl ValidatorChallengeProtocolKind {
    /// Returns the stable protocol identifier.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GpuFreivaldsMerkleV1 => GPU_FREIVALDS_MERKLE_PROTOCOL_ID,
        }
    }
}

/// Request metadata required for one validator challenge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeContext {
    /// Stable challenge identifier.
    pub challenge_id: String,
    /// Stable digest of the proof bundle under challenge.
    pub proof_bundle_digest: String,
    /// Stable request digest the challenged execution belongs to.
    pub request_digest: String,
    /// Stable compute product identifier.
    pub product_id: String,
    /// Runtime backend that produced the challenged work.
    pub runtime_backend: String,
    /// Stable model identifier when the challenged work belongs to a served model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Validator pool reference requested by the market policy when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_pool_ref: Option<String>,
    /// Creation time for the challenge.
    pub created_at_ms: u64,
    /// Maximum validator attempts allowed before the challenge times out.
    pub max_attempts: u32,
    /// Lease timeout for one validator attempt.
    pub lease_timeout_ms: u64,
}

impl ValidatorChallengeContext {
    /// Creates a validator challenge context with conservative defaults.
    #[must_use]
    pub fn new(
        challenge_id: impl Into<String>,
        proof_bundle_digest: impl Into<String>,
        request_digest: impl Into<String>,
        product_id: impl Into<String>,
        runtime_backend: impl Into<String>,
        created_at_ms: u64,
    ) -> Self {
        Self {
            challenge_id: challenge_id.into(),
            proof_bundle_digest: proof_bundle_digest.into(),
            request_digest: request_digest.into(),
            product_id: product_id.into(),
            runtime_backend: runtime_backend.into(),
            model_id: None,
            validator_pool_ref: None,
            created_at_ms,
            max_attempts: 3,
            lease_timeout_ms: 30_000,
        }
    }

    /// Attaches a model identifier.
    #[must_use]
    pub fn with_model_id(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    /// Attaches the validator pool reference.
    #[must_use]
    pub fn with_validator_pool_ref(mut self, validator_pool_ref: impl Into<String>) -> Self {
        self.validator_pool_ref = Some(validator_pool_ref.into());
        self
    }

    /// Overrides the maximum attempt count.
    #[must_use]
    pub const fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Overrides the validator lease timeout.
    #[must_use]
    pub const fn with_lease_timeout_ms(mut self, lease_timeout_ms: u64) -> Self {
        self.lease_timeout_ms = lease_timeout_ms;
        self
    }
}

/// Commitment metadata for one matrix in the Merkle-committed challenge protocol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleCommittedMatrix {
    /// Stable logical matrix identifier.
    pub matrix_id: String,
    /// Number of rows in the committed matrix.
    pub row_count: u32,
    /// Number of columns in the committed matrix.
    pub column_count: u32,
    /// Merkle root over all committed row leaves.
    pub row_root: String,
    /// Finite field modulus used for verification.
    pub field_modulus: i64,
}

/// One Merkle-authenticated row opening for a committed matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleRowOpening {
    /// Row index inside the committed matrix.
    pub row_index: u32,
    /// Signed row values opened for verification.
    pub values: Vec<i64>,
    /// Merkle sibling hashes from the row leaf to the root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sibling_hashes: Vec<String>,
}

/// Full witness for one Merkle-committed matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleMatrixWitness {
    /// Commitment metadata for the matrix.
    pub commitment: MerkleCommittedMatrix,
    /// Opened rows supplied for verification.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_openings: Vec<MerkleRowOpening>,
}

impl MerkleMatrixWitness {
    /// Builds a fully opened Merkle witness from row-major values.
    pub fn from_rows(
        matrix_id: impl Into<String>,
        rows: &[Vec<i64>],
        field_modulus: i64,
    ) -> Result<Self, ValidatorServiceError> {
        let matrix_id = matrix_id.into();
        let column_count = validate_matrix_rows(&matrix_id, rows)?;
        let leaf_digests = rows
            .iter()
            .enumerate()
            .map(|(row_index, values)| {
                row_leaf_digest(&matrix_id, row_index, values, field_modulus)
            })
            .collect::<Vec<_>>();
        let levels = merkle_levels(leaf_digests.as_slice());
        let row_root = levels
            .last()
            .and_then(|level| level.first())
            .cloned()
            .ok_or_else(|| ValidatorServiceError::InvalidMatrix(matrix_id.clone()))?;
        let row_openings = rows
            .iter()
            .enumerate()
            .map(|(row_index, values)| MerkleRowOpening {
                row_index: row_index as u32,
                values: values.clone(),
                sibling_hashes: merkle_path(levels.as_slice(), row_index),
            })
            .collect::<Vec<_>>();
        Ok(Self {
            commitment: MerkleCommittedMatrix {
                matrix_id,
                row_count: rows.len() as u32,
                column_count: column_count as u32,
                row_root,
                field_modulus,
            },
            row_openings,
        })
    }
}

/// Witness carried by the first GPU challenge protocol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuFreivaldsMerkleWitness {
    /// Left matrix commitment and openings.
    pub left: MerkleMatrixWitness,
    /// Right matrix commitment and openings.
    pub right: MerkleMatrixWitness,
    /// Claimed result commitment and openings.
    pub claimed_result: MerkleMatrixWitness,
}

impl GpuFreivaldsMerkleWitness {
    /// Builds a fully opened witness from row-major matrices.
    pub fn from_matrices(
        left: &[Vec<i64>],
        right: &[Vec<i64>],
        claimed_result: &[Vec<i64>],
    ) -> Result<Self, ValidatorServiceError> {
        Ok(Self {
            left: MerkleMatrixWitness::from_rows("left", left, GPU_FREIVALDS_FIELD_MODULUS)?,
            right: MerkleMatrixWitness::from_rows("right", right, GPU_FREIVALDS_FIELD_MODULUS)?,
            claimed_result: MerkleMatrixWitness::from_rows(
                "claimed_result",
                claimed_result,
                GPU_FREIVALDS_FIELD_MODULUS,
            )?,
        })
    }
}

/// One queued validator challenge request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeRequest {
    /// Contextual metadata for the challenge.
    pub context: ValidatorChallengeContext,
    /// Protocol to run for this challenge.
    pub protocol: ValidatorChallengeProtocolKind,
    /// Witness bytes and commitments for the protocol.
    pub witness: GpuFreivaldsMerkleWitness,
}

impl ValidatorChallengeRequest {
    /// Creates a challenge request for the Freivalds-plus-Merkle protocol.
    #[must_use]
    pub fn new(context: ValidatorChallengeContext, witness: GpuFreivaldsMerkleWitness) -> Self {
        Self {
            context,
            protocol: ValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1,
            witness,
        }
    }
}

/// Temporary lease handed to one validator worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeLease {
    /// Challenge currently leased.
    pub challenge_id: String,
    /// Current attempt number.
    pub attempt: u32,
    /// Worker or validator identity holding the lease.
    pub validator_id: String,
    /// Lease acquisition time.
    pub leased_at_ms: u64,
    /// Lease expiry time.
    pub expires_at_ms: u64,
}

/// Machine-legible result for one validator action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeResult {
    /// Stable challenge identifier.
    pub challenge_id: String,
    /// Stable proof-bundle digest under challenge.
    pub proof_bundle_digest: String,
    /// Stable protocol identifier.
    pub protocol_id: String,
    /// Attempt number the result belongs to.
    pub attempt: u32,
    /// Lifecycle state after this result.
    pub status: ValidatorChallengeStatus,
    /// Verdict associated with the result.
    pub verdict: ValidatorChallengeVerdict,
    /// Failure code when the result is not a clean verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<ValidatorChallengeFailureCode>,
    /// Plain-language diagnostic.
    pub detail: String,
    /// Creation time inherited from the original challenge.
    pub created_at_ms: u64,
    /// Finalization or retry time for this result.
    pub finalized_at_ms: u64,
    /// Stable challenge-seed digest when protocol verification ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenge_seed_digest: Option<String>,
    /// Number of Merkle-authenticated rows verified by the protocol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_row_count: Option<u32>,
    /// Stable digest for this result record.
    pub result_digest: String,
    /// Stable string reference that proof bundles and later kernel receipts can store.
    pub challenge_result_ref: String,
}

impl ValidatorChallengeResult {
    fn new(
        context: &ValidatorChallengeContext,
        protocol: ValidatorChallengeProtocolKind,
        attempt: u32,
        status: ValidatorChallengeStatus,
        verdict: ValidatorChallengeVerdict,
        reason_code: Option<ValidatorChallengeFailureCode>,
        detail: impl Into<String>,
        finalized_at_ms: u64,
        challenge_seed_digest: Option<String>,
        verified_row_count: Option<u32>,
    ) -> Self {
        let detail = detail.into();
        let result_digest = challenge_result_digest(
            &context.challenge_id,
            &context.proof_bundle_digest,
            protocol.label(),
            attempt,
            status,
            verdict,
            reason_code,
            detail.as_str(),
            context.created_at_ms,
            finalized_at_ms,
            challenge_seed_digest.as_deref(),
            verified_row_count,
        );
        Self {
            challenge_id: context.challenge_id.clone(),
            proof_bundle_digest: context.proof_bundle_digest.clone(),
            protocol_id: protocol.label().to_string(),
            attempt,
            status,
            verdict,
            reason_code,
            detail,
            created_at_ms: context.created_at_ms,
            finalized_at_ms,
            challenge_seed_digest,
            verified_row_count,
            challenge_result_ref: format!("validator_challenge_result:{result_digest}"),
            result_digest,
        }
    }
}

/// Trait for validator challenge protocols.
pub trait ValidatorChallengeProtocol {
    /// Stable protocol kind.
    fn kind(&self) -> ValidatorChallengeProtocolKind;

    /// Adjudicates one challenge request.
    fn adjudicate(
        &self,
        request: &ValidatorChallengeRequest,
        attempt: u32,
        finalized_at_ms: u64,
    ) -> ValidatorChallengeResult;
}

/// Freivalds verification over Merkle-committed matrices.
#[derive(Clone, Debug, Default)]
pub struct GpuFreivaldsMerkleProtocol;

impl ValidatorChallengeProtocol for GpuFreivaldsMerkleProtocol {
    fn kind(&self) -> ValidatorChallengeProtocolKind {
        ValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1
    }

    fn adjudicate(
        &self,
        request: &ValidatorChallengeRequest,
        attempt: u32,
        finalized_at_ms: u64,
    ) -> ValidatorChallengeResult {
        let context = &request.context;
        let left = &request.witness.left.commitment;
        let right = &request.witness.right.commitment;
        let claimed = &request.witness.claimed_result.commitment;
        if left.column_count != right.row_count
            || left.row_count != claimed.row_count
            || right.column_count != claimed.column_count
        {
            return ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Rejected,
                ValidatorChallengeVerdict::Rejected,
                Some(ValidatorChallengeFailureCode::DimensionMismatch),
                "validator rejected inconsistent matrix dimensions",
                finalized_at_ms,
                None,
                None,
            );
        }
        if left.field_modulus != right.field_modulus || left.field_modulus != claimed.field_modulus
        {
            return ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Rejected,
                ValidatorChallengeVerdict::Rejected,
                Some(ValidatorChallengeFailureCode::FieldMismatch),
                "validator rejected inconsistent field moduli",
                finalized_at_ms,
                None,
                None,
            );
        }
        let Ok(left_rows) = materialize_matrix(&request.witness.left) else {
            return ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Rejected,
                ValidatorChallengeVerdict::Rejected,
                Some(ValidatorChallengeFailureCode::MerkleProofInvalid),
                "validator rejected invalid left-matrix opening",
                finalized_at_ms,
                None,
                None,
            );
        };
        let Ok(right_rows) = materialize_matrix(&request.witness.right) else {
            return ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Rejected,
                ValidatorChallengeVerdict::Rejected,
                Some(ValidatorChallengeFailureCode::MerkleProofInvalid),
                "validator rejected invalid right-matrix opening",
                finalized_at_ms,
                None,
                None,
            );
        };
        let Ok(claimed_rows) = materialize_matrix(&request.witness.claimed_result) else {
            return ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Rejected,
                ValidatorChallengeVerdict::Rejected,
                Some(ValidatorChallengeFailureCode::MerkleProofInvalid),
                "validator rejected invalid claimed-result opening",
                finalized_at_ms,
                None,
                None,
            );
        };
        let challenge_seed_digest = challenge_seed_digest(context, &request.witness);
        let random_vector = challenge_vector(
            challenge_seed_digest.as_str(),
            right.column_count as usize,
            left.field_modulus,
        );
        let right_projection = matrix_vector_product_mod(
            right_rows.as_slice(),
            random_vector.as_slice(),
            left.field_modulus,
        );
        let left_projection = matrix_vector_product_mod(
            left_rows.as_slice(),
            right_projection.as_slice(),
            left.field_modulus,
        );
        let claimed_projection = matrix_vector_product_mod(
            claimed_rows.as_slice(),
            random_vector.as_slice(),
            left.field_modulus,
        );
        let verified_row_count = left.row_count + right.row_count + claimed.row_count;
        if left_projection == claimed_projection {
            ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Verified,
                ValidatorChallengeVerdict::Verified,
                None,
                "validator verified the claimed matrix product",
                finalized_at_ms,
                Some(challenge_seed_digest),
                Some(verified_row_count),
            )
        } else {
            ValidatorChallengeResult::new(
                context,
                self.kind(),
                attempt,
                ValidatorChallengeStatus::Rejected,
                ValidatorChallengeVerdict::Rejected,
                Some(ValidatorChallengeFailureCode::FreivaldsMismatch),
                "validator rejected the claimed matrix product",
                finalized_at_ms,
                Some(challenge_seed_digest),
                Some(verified_row_count),
            )
        }
    }
}

#[derive(Clone, Debug)]
struct ValidatorChallengeRecord {
    request: ValidatorChallengeRequest,
    attempts_used: u32,
    active_lease: Option<ValidatorChallengeLease>,
    final_result: Option<ValidatorChallengeResult>,
}

/// In-memory validator challenge queue and execution service.
#[derive(Default)]
pub struct ValidatorChallengeService {
    records: BTreeMap<String, ValidatorChallengeRecord>,
    queue: VecDeque<String>,
}

impl ValidatorChallengeService {
    /// Enqueues one validator challenge.
    pub fn enqueue(
        &mut self,
        request: ValidatorChallengeRequest,
    ) -> Result<(), ValidatorServiceError> {
        if self
            .records
            .contains_key(request.context.challenge_id.as_str())
        {
            return Err(ValidatorServiceError::DuplicateChallenge(
                request.context.challenge_id,
            ));
        }
        self.queue.push_back(request.context.challenge_id.clone());
        self.records.insert(
            request.context.challenge_id.clone(),
            ValidatorChallengeRecord {
                request,
                attempts_used: 0,
                active_lease: None,
                final_result: None,
            },
        );
        Ok(())
    }

    /// Returns the visible lifecycle state for one challenge.
    #[must_use]
    pub fn status(&self, challenge_id: &str) -> Option<ValidatorChallengeStatus> {
        let record = self.records.get(challenge_id)?;
        if let Some(result) = record.final_result.as_ref() {
            return Some(result.status);
        }
        if record.active_lease.is_some() {
            return Some(ValidatorChallengeStatus::Leased);
        }
        if record.attempts_used > 0 {
            return Some(ValidatorChallengeStatus::Retrying);
        }
        Some(ValidatorChallengeStatus::Queued)
    }

    /// Leases the next available challenge to one validator worker.
    pub fn lease_next(
        &mut self,
        validator_id: impl Into<String>,
        now_ms: u64,
    ) -> Option<ValidatorChallengeLease> {
        let validator_id = validator_id.into();
        let queue_len = self.queue.len();
        for _ in 0..queue_len {
            let challenge_id = self.queue.pop_front()?;
            let Some(record) = self.records.get_mut(challenge_id.as_str()) else {
                continue;
            };
            if record.final_result.is_some() || record.active_lease.is_some() {
                continue;
            }
            let attempt = record.attempts_used.saturating_add(1);
            let lease = ValidatorChallengeLease {
                challenge_id: challenge_id.clone(),
                attempt,
                validator_id: validator_id.clone(),
                leased_at_ms: now_ms,
                expires_at_ms: now_ms.saturating_add(record.request.context.lease_timeout_ms),
            };
            record.active_lease = Some(lease.clone());
            return Some(lease);
        }
        None
    }

    /// Executes one active lease and finalizes the challenge result.
    pub fn execute_lease(
        &mut self,
        lease: &ValidatorChallengeLease,
        finalized_at_ms: u64,
    ) -> Result<ValidatorChallengeResult, ValidatorServiceError> {
        let record = self
            .records
            .get_mut(lease.challenge_id.as_str())
            .ok_or_else(|| ValidatorServiceError::UnknownChallenge(lease.challenge_id.clone()))?;
        if record.active_lease.as_ref() != Some(lease) || finalized_at_ms > lease.expires_at_ms {
            return Err(ValidatorServiceError::InvalidLease(
                lease.challenge_id.clone(),
            ));
        }
        let result = match record.request.protocol {
            ValidatorChallengeProtocolKind::GpuFreivaldsMerkleV1 => GpuFreivaldsMerkleProtocol
                .adjudicate(&record.request, lease.attempt, finalized_at_ms),
        };
        record.attempts_used = lease.attempt;
        record.active_lease = None;
        record.final_result = Some(result.clone());
        Ok(result)
    }

    /// Requeues one active lease after a retryable interruption.
    pub fn retry_lease(
        &mut self,
        lease: &ValidatorChallengeLease,
        finalized_at_ms: u64,
        detail: impl Into<String>,
    ) -> Result<ValidatorChallengeResult, ValidatorServiceError> {
        let record = self
            .records
            .get_mut(lease.challenge_id.as_str())
            .ok_or_else(|| ValidatorServiceError::UnknownChallenge(lease.challenge_id.clone()))?;
        if record.active_lease.as_ref() != Some(lease) || finalized_at_ms > lease.expires_at_ms {
            return Err(ValidatorServiceError::InvalidLease(
                lease.challenge_id.clone(),
            ));
        }
        record.attempts_used = lease.attempt;
        record.active_lease = None;
        if record.attempts_used >= record.request.context.max_attempts {
            let result = ValidatorChallengeResult::new(
                &record.request.context,
                record.request.protocol,
                lease.attempt,
                ValidatorChallengeStatus::TimedOut,
                ValidatorChallengeVerdict::TimedOut,
                Some(ValidatorChallengeFailureCode::RetryBudgetExhausted),
                detail,
                finalized_at_ms,
                None,
                None,
            );
            record.final_result = Some(result.clone());
            return Ok(result);
        }
        self.queue.push_back(lease.challenge_id.clone());
        Ok(ValidatorChallengeResult::new(
            &record.request.context,
            record.request.protocol,
            lease.attempt,
            ValidatorChallengeStatus::Retrying,
            ValidatorChallengeVerdict::RetryScheduled,
            None,
            detail,
            finalized_at_ms,
            None,
            None,
        ))
    }

    /// Requeues expired leases or finalizes them when the retry budget is exhausted.
    pub fn expire_leases(&mut self, now_ms: u64) -> Vec<ValidatorChallengeResult> {
        let mut expired = Vec::new();
        for record in self.records.values_mut() {
            let Some(lease) = record.active_lease.clone() else {
                continue;
            };
            if lease.expires_at_ms > now_ms {
                continue;
            }
            record.attempts_used = lease.attempt;
            record.active_lease = None;
            if record.attempts_used >= record.request.context.max_attempts {
                let result = ValidatorChallengeResult::new(
                    &record.request.context,
                    record.request.protocol,
                    lease.attempt,
                    ValidatorChallengeStatus::TimedOut,
                    ValidatorChallengeVerdict::TimedOut,
                    Some(ValidatorChallengeFailureCode::LeaseExpired),
                    "validator lease expired before challenge completed",
                    now_ms,
                    None,
                    None,
                );
                record.final_result = Some(result.clone());
                expired.push(result);
            } else {
                self.queue
                    .push_back(record.request.context.challenge_id.clone());
            }
        }
        expired
    }

    /// Returns the final result for one challenge when available.
    #[must_use]
    pub fn result(&self, challenge_id: &str) -> Option<&ValidatorChallengeResult> {
        self.records
            .get(challenge_id)
            .and_then(|record| record.final_result.as_ref())
    }
}

/// Appends one validator challenge result reference to a proof bundle.
#[must_use]
pub fn attach_challenge_result_ref(
    bundle: ExecutionProofBundle,
    result: &ValidatorChallengeResult,
) -> ExecutionProofBundle {
    bundle.with_challenge_result_ref(result.challenge_result_ref.clone())
}

fn validate_matrix_rows(
    matrix_id: &str,
    rows: &[Vec<i64>],
) -> Result<usize, ValidatorServiceError> {
    let Some(first_row) = rows.first() else {
        return Err(ValidatorServiceError::InvalidMatrix(matrix_id.to_string()));
    };
    let column_count = first_row.len();
    if column_count == 0 || rows.iter().any(|row| row.len() != column_count) {
        return Err(ValidatorServiceError::InvalidMatrix(matrix_id.to_string()));
    }
    Ok(column_count)
}

fn materialize_matrix(
    witness: &MerkleMatrixWitness,
) -> Result<Vec<Vec<i64>>, ValidatorChallengeFailureCode> {
    let row_count = witness.commitment.row_count as usize;
    if witness.row_openings.len() != row_count {
        return Err(ValidatorChallengeFailureCode::RowOpeningMissing);
    }
    let mut rows = vec![Vec::new(); row_count];
    for opening in &witness.row_openings {
        let row_index = opening.row_index as usize;
        if row_index >= row_count
            || opening.values.len() != witness.commitment.column_count as usize
        {
            return Err(ValidatorChallengeFailureCode::DimensionMismatch);
        }
        let leaf_digest = row_leaf_digest(
            witness.commitment.matrix_id.as_str(),
            row_index,
            opening.values.as_slice(),
            witness.commitment.field_modulus,
        );
        if !verify_merkle_path(
            leaf_digest.as_str(),
            row_index,
            opening.sibling_hashes.as_slice(),
            witness.commitment.row_root.as_str(),
        ) {
            return Err(ValidatorChallengeFailureCode::MerkleProofInvalid);
        }
        rows[row_index] = opening.values.clone();
    }
    if rows.iter().any(Vec::is_empty) {
        return Err(ValidatorChallengeFailureCode::RowOpeningMissing);
    }
    Ok(rows)
}

fn row_leaf_digest(
    matrix_id: &str,
    row_index: usize,
    values: &[i64],
    field_modulus: i64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(matrix_id.as_bytes());
    hasher.update(b"|");
    hasher.update(row_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(field_modulus.to_string().as_bytes());
    for value in values {
        hasher.update(b"|");
        hasher.update(value.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn merkle_levels(leaves: &[String]) -> Vec<Vec<String>> {
    let mut levels = vec![leaves.to_vec()];
    while levels.last().map_or(0, Vec::len) > 1 {
        let previous = levels.last().cloned().unwrap_or_default();
        let next = previous
            .chunks(2)
            .map(|pair| {
                let left = &pair[0];
                let right = pair.get(1).unwrap_or(left);
                hash_pair(left.as_str(), right.as_str())
            })
            .collect::<Vec<_>>();
        levels.push(next);
    }
    levels
}

fn merkle_path(levels: &[Vec<String>], mut index: usize) -> Vec<String> {
    let mut path = Vec::new();
    for level in levels.iter().take(levels.len().saturating_sub(1)) {
        let sibling_index = if index % 2 == 0 {
            index.saturating_add(1)
        } else {
            index.saturating_sub(1)
        };
        let sibling = level
            .get(sibling_index)
            .cloned()
            .unwrap_or_else(|| level[index].clone());
        path.push(sibling);
        index /= 2;
    }
    path
}

fn verify_merkle_path(
    leaf_digest: &str,
    mut index: usize,
    sibling_hashes: &[String],
    expected_root: &str,
) -> bool {
    let mut current = leaf_digest.to_string();
    for sibling in sibling_hashes {
        current = if index % 2 == 0 {
            hash_pair(current.as_str(), sibling.as_str())
        } else {
            hash_pair(sibling.as_str(), current.as_str())
        };
        index /= 2;
    }
    current == expected_root
}

fn hash_pair(left: &str, right: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(left.as_bytes());
    hasher.update(b"|");
    hasher.update(right.as_bytes());
    hex::encode(hasher.finalize())
}

fn challenge_seed_digest(
    context: &ValidatorChallengeContext,
    witness: &GpuFreivaldsMerkleWitness,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(context.proof_bundle_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(witness.left.commitment.row_root.as_bytes());
    hasher.update(b"|");
    hasher.update(witness.right.commitment.row_root.as_bytes());
    hasher.update(b"|");
    hasher.update(witness.claimed_result.commitment.row_root.as_bytes());
    hex::encode(hasher.finalize())
}

fn challenge_vector(seed_digest: &str, len: usize, field_modulus: i64) -> Vec<i64> {
    (0..len)
        .map(|index| {
            let mut hasher = Sha256::new();
            hasher.update(seed_digest.as_bytes());
            hasher.update(b"|");
            hasher.update(index.to_string().as_bytes());
            let digest = hex::encode(hasher.finalize());
            let sample = u64::from_str_radix(&digest[..16], 16).unwrap_or(1);
            ((sample % (field_modulus as u64 - 1)) + 1) as i64
        })
        .collect()
}

fn matrix_vector_product_mod(matrix: &[Vec<i64>], vector: &[i64], field_modulus: i64) -> Vec<i64> {
    let modulus = i128::from(field_modulus);
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(vector.iter())
                .fold(0_i128, |accumulator, (left, right)| {
                    let left = modulo_normalize(*left, field_modulus);
                    let right = modulo_normalize(*right, field_modulus);
                    (accumulator + (left * right)) % modulus
                }) as i64
        })
        .collect()
}

fn modulo_normalize(value: i64, field_modulus: i64) -> i128 {
    let modulus = i128::from(field_modulus);
    let normalized = i128::from(value) % modulus;
    if normalized < 0 {
        normalized + modulus
    } else {
        normalized
    }
}

fn challenge_result_digest(
    challenge_id: &str,
    proof_bundle_digest: &str,
    protocol_id: &str,
    attempt: u32,
    status: ValidatorChallengeStatus,
    verdict: ValidatorChallengeVerdict,
    reason_code: Option<ValidatorChallengeFailureCode>,
    detail: &str,
    created_at_ms: u64,
    finalized_at_ms: u64,
    challenge_seed_digest: Option<&str>,
    verified_row_count: Option<u32>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(challenge_id.as_bytes());
    hasher.update(b"|");
    hasher.update(proof_bundle_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(protocol_id.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{attempt:?}|{status:?}|{verdict:?}").as_bytes());
    hasher.update(b"|");
    if let Some(reason_code) = reason_code {
        hasher.update(format!("{reason_code:?}").as_bytes());
    }
    hasher.update(b"|");
    hasher.update(detail.as_bytes());
    hasher.update(b"|");
    hasher.update(created_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(finalized_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    if let Some(challenge_seed_digest) = challenge_seed_digest {
        hasher.update(challenge_seed_digest.as_bytes());
    }
    hasher.update(b"|");
    if let Some(verified_row_count) = verified_row_count {
        hasher.update(verified_row_count.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{
        BackendProbeState, BackendToolchainIdentity, ExecutionProofBundle,
        ExecutionProofBundleKind, ExecutionProofBundleStatus, ExecutionProofRuntimeIdentity,
    };

    use super::{
        GpuFreivaldsMerkleWitness, ValidatorChallengeContext, ValidatorChallengeFailureCode,
        ValidatorChallengeRequest, ValidatorChallengeService, ValidatorChallengeStatus,
        ValidatorChallengeVerdict, attach_challenge_result_ref,
    };

    fn sample_context(challenge_id: &str) -> ValidatorChallengeContext {
        ValidatorChallengeContext::new(
            challenge_id,
            "proof-bundle-123",
            "request-digest-123",
            "psionic.clustered_text_generation",
            "cuda",
            10,
        )
        .with_model_id("gpt-oss-20b")
        .with_validator_pool_ref("validators.alpha")
        .with_max_attempts(2)
        .with_lease_timeout_ms(50)
    }

    fn sample_bundle() -> ExecutionProofBundle {
        ExecutionProofBundle::new(
            ExecutionProofBundleKind::Clustered,
            ExecutionProofBundleStatus::Succeeded,
            "req-1",
            "request-digest-123",
            "psionic.clustered_text_generation",
            ExecutionProofRuntimeIdentity::new(
                "cuda",
                BackendToolchainIdentity::new("cuda", "cuda@1.0.0", Vec::new()).with_probe(
                    BackendProbeState::CompiledAndProbed,
                    vec!["sm90".to_string()],
                ),
            ),
        )
    }

    #[test]
    fn freivalds_merkle_protocol_verifies_consistent_witness()
    -> Result<(), Box<dyn std::error::Error>> {
        let witness = GpuFreivaldsMerkleWitness::from_matrices(
            &[vec![1, 2], vec![3, 4]],
            &[vec![5, 6], vec![7, 8]],
            &[vec![19, 22], vec![43, 50]],
        )?;
        let request = ValidatorChallengeRequest::new(sample_context("challenge-ok"), witness);
        let mut service = ValidatorChallengeService::default();
        service.enqueue(request)?;
        let lease = service
            .lease_next("validator-a", 20)
            .ok_or("expected one lease")?;
        let result = service.execute_lease(&lease, 30)?;
        assert_eq!(result.status, ValidatorChallengeStatus::Verified);
        assert_eq!(result.verdict, ValidatorChallengeVerdict::Verified);
        assert!(result.reason_code.is_none());
        assert!(result.challenge_seed_digest.is_some());
        assert_eq!(result.verified_row_count, Some(6));
        let bundle = attach_challenge_result_ref(sample_bundle(), &result);
        assert_eq!(
            bundle.challenge_result_refs,
            vec![result.challenge_result_ref.clone()]
        );
        Ok(())
    }

    #[test]
    fn freivalds_merkle_protocol_rejects_mismatched_claimed_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let witness = GpuFreivaldsMerkleWitness::from_matrices(
            &[vec![1, 2], vec![3, 4]],
            &[vec![5, 6], vec![7, 8]],
            &[vec![19, 21], vec![43, 51]],
        )?;
        let request = ValidatorChallengeRequest::new(sample_context("challenge-bad"), witness);
        let mut service = ValidatorChallengeService::default();
        service.enqueue(request)?;
        let lease = service
            .lease_next("validator-a", 20)
            .ok_or("expected one lease")?;
        let result = service.execute_lease(&lease, 30)?;
        assert_eq!(result.status, ValidatorChallengeStatus::Rejected);
        assert_eq!(
            result.reason_code,
            Some(ValidatorChallengeFailureCode::FreivaldsMismatch)
        );
        Ok(())
    }

    #[test]
    fn service_retries_then_times_out_expired_leases() -> Result<(), Box<dyn std::error::Error>> {
        let witness = GpuFreivaldsMerkleWitness::from_matrices(
            &[vec![1, 2], vec![3, 4]],
            &[vec![5, 6], vec![7, 8]],
            &[vec![19, 22], vec![43, 50]],
        )?;
        let request = ValidatorChallengeRequest::new(sample_context("challenge-timeout"), witness);
        let mut service = ValidatorChallengeService::default();
        service.enqueue(request)?;

        let first_lease = service
            .lease_next("validator-a", 20)
            .ok_or("expected first lease")?;
        let expired = service.expire_leases(100);
        assert!(expired.is_empty(), "first expiry should requeue");
        assert_eq!(
            service.status("challenge-timeout"),
            Some(ValidatorChallengeStatus::Retrying)
        );

        let second_lease = service
            .lease_next("validator-b", 110)
            .ok_or("expected second lease")?;
        assert_eq!(second_lease.attempt, first_lease.attempt + 1);
        let expired = service.expire_leases(200);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].status, ValidatorChallengeStatus::TimedOut);
        assert_eq!(expired[0].verdict, ValidatorChallengeVerdict::TimedOut);
        assert_eq!(
            expired[0].reason_code,
            Some(ValidatorChallengeFailureCode::LeaseExpired)
        );
        Ok(())
    }
}
