use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_environments::{
    EnvironmentContractError, EnvironmentPackageContract, EnvironmentPackageKey,
    EnvironmentPolicyKind,
};

use crate::{RolloutArtifact, RolloutProofKind, RolloutWorkerIdentity, RolloutWorkerTrustClass};

/// Error returned by the train security posture layer.
#[derive(Debug, Error)]
pub enum TrainSecurityError {
    /// One environment package contract failed validation.
    #[error(transparent)]
    EnvironmentContract(#[from] EnvironmentContractError),
    /// The trust root repeated one signer identifier.
    #[error("artifact trust root repeated signer `{signer_id}`")]
    DuplicateTrustSigner {
        /// Repeated signer identifier.
        signer_id: String,
    },
    /// The trust root requires at least one valid signature.
    #[error("artifact trust root `{trust_root_id}` must require at least one signature")]
    InvalidMinimumSignatures {
        /// Stable trust-root identifier.
        trust_root_id: String,
    },
    /// One trust-root signer key could not be decoded.
    #[error("artifact trust root signer `{signer_id}` has invalid verifying key")]
    InvalidSignerKey {
        /// Stable signer identifier.
        signer_id: String,
    },
}

/// Security scope admitted by artifact attestations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSigningScope {
    /// Attestation over a rollout artifact digest.
    RolloutArtifact,
}

impl ArtifactSigningScope {
    fn label(self) -> &'static [u8] {
        match self {
            Self::RolloutArtifact => b"rollout_artifact",
        }
    }
}

/// Trusted signer inside one train-security trust root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSigner {
    /// Stable signer identifier.
    pub signer_id: String,
    /// Hex-encoded Ed25519 verifying key.
    pub public_key_hex: String,
}

impl ArtifactSigner {
    /// Creates one trusted artifact signer.
    #[must_use]
    pub fn new(signer_id: impl Into<String>, public_key_hex: impl Into<String>) -> Self {
        Self {
            signer_id: signer_id.into(),
            public_key_hex: public_key_hex.into(),
        }
    }
}

/// Trust root for artifact attestations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTrustRoot {
    /// Stable trust-root identifier.
    pub trust_root_id: String,
    /// Minimum number of valid signatures required.
    pub minimum_signatures: u8,
    /// Trusted signers admitted by the trust root.
    pub signers: Vec<ArtifactSigner>,
}

impl ArtifactTrustRoot {
    /// Creates an artifact trust root.
    #[must_use]
    pub fn new(
        trust_root_id: impl Into<String>,
        minimum_signatures: u8,
        signers: Vec<ArtifactSigner>,
    ) -> Self {
        Self {
            trust_root_id: trust_root_id.into(),
            minimum_signatures,
            signers,
        }
    }

    fn validate(&self) -> Result<(), TrainSecurityError> {
        if self.minimum_signatures == 0 {
            return Err(TrainSecurityError::InvalidMinimumSignatures {
                trust_root_id: self.trust_root_id.clone(),
            });
        }
        let mut signer_ids = BTreeSet::new();
        for signer in &self.signers {
            if !signer_ids.insert(signer.signer_id.clone()) {
                return Err(TrainSecurityError::DuplicateTrustSigner {
                    signer_id: signer.signer_id.clone(),
                });
            }
            decode_verifying_key(signer.public_key_hex.as_str()).map_err(|_| {
                TrainSecurityError::InvalidSignerKey {
                    signer_id: signer.signer_id.clone(),
                }
            })?;
        }
        Ok(())
    }
}

/// Signed attestation over one artifact subject digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedArtifactAttestation {
    /// Stable signer identifier.
    pub signer_id: String,
    /// Attestation scope.
    pub scope: ArtifactSigningScope,
    /// Stable digest of the signed subject.
    pub subject_digest: String,
    /// Stable trust-root identifier that scoped the signature payload.
    pub trust_root_id: String,
    /// Hex-encoded Ed25519 signature.
    pub signature_hex: String,
}

impl SignedArtifactAttestation {
    /// Signs one rollout-artifact digest using the provided signing key.
    #[must_use]
    pub fn sign_rollout_artifact(
        signer_id: impl Into<String>,
        trust_root_id: impl Into<String>,
        artifact_digest: impl Into<String>,
        signing_key: &SigningKey,
    ) -> Self {
        let signer_id = signer_id.into();
        let trust_root_id = trust_root_id.into();
        let subject_digest = artifact_digest.into();
        let signature = signing_key.sign(&artifact_attestation_payload(
            ArtifactSigningScope::RolloutArtifact,
            subject_digest.as_str(),
            trust_root_id.as_str(),
        ));
        Self {
            signer_id,
            scope: ArtifactSigningScope::RolloutArtifact,
            subject_digest,
            trust_root_id,
            signature_hex: hex::encode(signature.to_bytes()),
        }
    }
}

/// Environment verification posture admitted by the train security controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentVerificationPolicy {
    /// Expected environment package identity.
    pub package_key: EnvironmentPackageKey,
    /// Expected package digest.
    pub expected_package_digest: String,
    /// Required artifact verification references sourced from the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_artifact_verification_refs: Vec<String>,
    /// Required package policy references sourced from the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_policy_refs: Vec<String>,
}

impl EnvironmentVerificationPolicy {
    /// Builds the verification policy directly from one environment package.
    #[must_use]
    pub fn from_package(package: &EnvironmentPackageContract) -> Self {
        Self {
            package_key: package.key.clone(),
            expected_package_digest: package.stable_digest(),
            required_artifact_verification_refs: package
                .expected_artifacts
                .iter()
                .filter(|artifact| artifact.required)
                .filter_map(|artifact| artifact.verification_policy_ref.clone())
                .collect(),
            required_policy_refs: package
                .policy_references
                .iter()
                .filter(|policy| policy.required)
                .map(|policy| policy.policy_ref.clone())
                .collect(),
        }
    }
}

/// Admission and rate-limit policy for untrusted rollout workers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UntrustedWorkerAdmissionPolicy {
    /// Sliding-window length for worker submission accounting.
    pub window_ms: u64,
    /// Maximum submissions admitted from one untrusted worker inside the window.
    pub max_submissions_per_window: u32,
    /// Minimum spacing between untrusted submissions.
    pub minimum_interarrival_ms: u64,
    /// Whether untrusted workers must always carry an execution proof.
    pub require_execution_proof: bool,
}

impl Default for UntrustedWorkerAdmissionPolicy {
    fn default() -> Self {
        Self {
            window_ms: 60_000,
            max_submissions_per_window: 2,
            minimum_interarrival_ms: 1_000,
            require_execution_proof: true,
        }
    }
}

/// Spam and poisoning controls for rollout submissions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutPoisoningControls {
    /// Whether duplicate response signatures should quarantine the submission.
    pub quarantine_on_duplicate_signature: bool,
    /// Whether duplicate artifact digests should reject the submission.
    pub reject_on_duplicate_artifact: bool,
    /// Maximum duplicate response signatures tolerated inside the current window.
    pub max_duplicate_signatures_per_window: u32,
}

impl Default for RolloutPoisoningControls {
    fn default() -> Self {
        Self {
            quarantine_on_duplicate_signature: true,
            reject_on_duplicate_artifact: true,
            max_duplicate_signatures_per_window: 1,
        }
    }
}

/// Full security posture for train rollout submission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainSecurityPolicy {
    /// Validator policy identifier paired with the security posture.
    pub validator_policy_id: String,
    /// Environment verification posture.
    pub environment_verification: EnvironmentVerificationPolicy,
    /// Artifact trust root.
    pub artifact_trust_root: ArtifactTrustRoot,
    /// Untrusted-worker admission policy.
    pub untrusted_worker_admission: UntrustedWorkerAdmissionPolicy,
    /// Spam and poisoning controls.
    pub poisoning_controls: RolloutPoisoningControls,
}

impl TrainSecurityPolicy {
    /// Creates a train security policy from one environment package and trust root.
    #[must_use]
    pub fn for_environment(
        validator_policy_id: impl Into<String>,
        package: &EnvironmentPackageContract,
        artifact_trust_root: ArtifactTrustRoot,
    ) -> Self {
        Self {
            validator_policy_id: validator_policy_id.into(),
            environment_verification: EnvironmentVerificationPolicy::from_package(package),
            artifact_trust_root,
            untrusted_worker_admission: UntrustedWorkerAdmissionPolicy::default(),
            poisoning_controls: RolloutPoisoningControls::default(),
        }
    }
}

/// Final security disposition for one rollout submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainSecurityDisposition {
    /// Submission is admissible under the current security posture.
    Accepted,
    /// Submission is retained but quarantined for later review.
    Quarantined,
    /// Submission is rejected immediately.
    Rejected,
}

/// Security reason code surfaced by the controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainSecurityReasonCode {
    /// Environment key mismatched the policy.
    EnvironmentKeyMismatch,
    /// Environment digest mismatched the policy.
    EnvironmentDigestMismatch,
    /// One required artifact-verification ref was missing.
    MissingArtifactVerificationPolicy,
    /// One required package policy ref was missing.
    MissingRequiredEnvironmentPolicy,
    /// No attestations were provided when signatures were required.
    ArtifactSignatureMissing,
    /// One or more provided signatures were malformed or invalid.
    ArtifactSignatureInvalid,
    /// One or more signatures came from an unknown signer.
    ArtifactSignerUntrusted,
    /// An untrusted worker omitted the required execution proof.
    ExecutionProofMissingForUntrustedWorker,
    /// Untrusted worker exceeded the per-window submission budget.
    UntrustedWorkerRateLimited,
    /// Untrusted worker submitted too quickly after its previous submission.
    UntrustedWorkerBurstLimited,
    /// Artifact digest was already seen previously.
    DuplicateArtifactDigest,
    /// Response signature matched an already-seen output pattern.
    DuplicateResponseSignature,
}

/// Security receipt for one rollout submission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainSecurityReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Validator policy paired with this security posture.
    pub validator_policy_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Worker trust posture.
    pub trust_class: RolloutWorkerTrustClass,
    /// Artifact digest under review.
    pub artifact_digest: String,
    /// Stable response-signature digest used for duplicate detection.
    pub response_signature_digest: String,
    /// Final security disposition.
    pub disposition: TrainSecurityDisposition,
    /// Number of valid signatures admitted by the trust root.
    pub valid_signature_count: u16,
    /// Machine-readable reason codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<TrainSecurityReasonCode>,
    /// Observation time used for rate-limit accounting.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Persistent submission record retained for rate limits and poisoning checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct WorkerSubmissionRecord {
    worker_id: String,
    trust_class: RolloutWorkerTrustClass,
    observed_at_ms: u64,
    artifact_digest: String,
    response_signature_digest: String,
}

/// Stateful controller over train security posture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainSecurityController {
    /// Active train security policy.
    pub policy: TrainSecurityPolicy,
    /// Historical submission state retained for rate limits and poisoning checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    submission_history: Vec<WorkerSubmissionRecord>,
    /// Historical receipt log.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<TrainSecurityReceipt>,
}

impl TrainSecurityController {
    /// Creates a security controller with a validated policy.
    pub fn new(policy: TrainSecurityPolicy) -> Result<Self, TrainSecurityError> {
        policy.artifact_trust_root.validate()?;
        Ok(Self {
            policy,
            submission_history: Vec::new(),
            receipts: Vec::new(),
        })
    }

    /// Evaluates one rollout submission against the active security posture.
    pub fn assess_rollout_submission(
        &mut self,
        worker: &RolloutWorkerIdentity,
        package: &EnvironmentPackageContract,
        artifact: &RolloutArtifact,
        attestations: &[SignedArtifactAttestation],
        observed_at_ms: u64,
    ) -> Result<TrainSecurityReceipt, TrainSecurityError> {
        package.validate()?;

        let mut reason_codes = Vec::new();
        let response_signature_digest = stable_response_signature_digest(artifact);

        if package.key != self.policy.environment_verification.package_key {
            reason_codes.push(TrainSecurityReasonCode::EnvironmentKeyMismatch);
        }
        if package.stable_digest() != self.policy.environment_verification.expected_package_digest {
            reason_codes.push(TrainSecurityReasonCode::EnvironmentDigestMismatch);
        }

        let artifact_verification_refs = package
            .expected_artifacts
            .iter()
            .filter_map(|artifact| artifact.verification_policy_ref.as_deref())
            .collect::<BTreeSet<_>>();
        for required_ref in &self.policy.environment_verification.required_artifact_verification_refs
        {
            if !artifact_verification_refs.contains(required_ref.as_str()) {
                reason_codes.push(TrainSecurityReasonCode::MissingArtifactVerificationPolicy);
            }
        }

        let policy_refs = package
            .policy_references
            .iter()
            .filter(|policy| {
                matches!(
                    policy.kind,
                    EnvironmentPolicyKind::Verification | EnvironmentPolicyKind::Safety
                )
            })
            .map(|policy| policy.policy_ref.as_str())
            .collect::<BTreeSet<_>>();
        for required_ref in &self.policy.environment_verification.required_policy_refs {
            if !policy_refs.contains(required_ref.as_str()) {
                reason_codes.push(TrainSecurityReasonCode::MissingRequiredEnvironmentPolicy);
            }
        }

        let signature_assessment = assess_artifact_signatures(
            &self.policy.artifact_trust_root,
            artifact.artifact_digest.as_str(),
            attestations,
        );
        if signature_assessment.valid_signature_count < self.policy.artifact_trust_root.minimum_signatures
            as u16
        {
            if attestations.is_empty() {
                reason_codes.push(TrainSecurityReasonCode::ArtifactSignatureMissing);
            }
            if signature_assessment.invalid_signature_count > 0 {
                reason_codes.push(TrainSecurityReasonCode::ArtifactSignatureInvalid);
            }
            if signature_assessment.untrusted_signer_count > 0 {
                reason_codes.push(TrainSecurityReasonCode::ArtifactSignerUntrusted);
            }
        }

        if worker.trust_class == RolloutWorkerTrustClass::UntrustedWorker {
            if self.policy.untrusted_worker_admission.require_execution_proof
                && !artifact
                    .proof_references
                    .iter()
                    .any(|proof| proof.kind == RolloutProofKind::ExecutionProof)
            {
                reason_codes.push(TrainSecurityReasonCode::ExecutionProofMissingForUntrustedWorker);
            }

            let recent_submissions = self
                .submission_history
                .iter()
                .filter(|record| {
                    record.worker_id == worker.worker_id
                        && observed_at_ms.saturating_sub(record.observed_at_ms)
                            <= self.policy.untrusted_worker_admission.window_ms
                })
                .collect::<Vec<_>>();
            if recent_submissions.len()
                >= usize::try_from(
                    self.policy.untrusted_worker_admission.max_submissions_per_window,
                )
                .unwrap_or(usize::MAX)
            {
                reason_codes.push(TrainSecurityReasonCode::UntrustedWorkerRateLimited);
            }
            if recent_submissions.iter().any(|record| {
                observed_at_ms.saturating_sub(record.observed_at_ms)
                    < self.policy.untrusted_worker_admission.minimum_interarrival_ms
            }) {
                reason_codes.push(TrainSecurityReasonCode::UntrustedWorkerBurstLimited);
            }
        }

        if self
            .submission_history
            .iter()
            .any(|record| record.artifact_digest == artifact.artifact_digest)
        {
            reason_codes.push(TrainSecurityReasonCode::DuplicateArtifactDigest);
        }

        let duplicate_signature_count = self
            .submission_history
            .iter()
            .filter(|record| {
                record.response_signature_digest == response_signature_digest
                    && observed_at_ms.saturating_sub(record.observed_at_ms)
                        <= self.policy.untrusted_worker_admission.window_ms
            })
            .count();
        if duplicate_signature_count
            >= usize::try_from(self.policy.poisoning_controls.max_duplicate_signatures_per_window)
                .unwrap_or(usize::MAX)
        {
            reason_codes.push(TrainSecurityReasonCode::DuplicateResponseSignature);
        }

        let disposition = security_disposition(&self.policy, reason_codes.as_slice());
        let receipt_id = format!("security-{}-{}", worker.worker_id, self.receipts.len() + 1);
        let receipt_digest = stable_security_receipt_digest(
            receipt_id.as_str(),
            self.policy.validator_policy_id.as_str(),
            worker.worker_id.as_str(),
            worker.trust_class,
            artifact.artifact_digest.as_str(),
            response_signature_digest.as_str(),
            disposition,
            signature_assessment.valid_signature_count,
            reason_codes.as_slice(),
            observed_at_ms,
        );
        let receipt = TrainSecurityReceipt {
            receipt_id,
            validator_policy_id: self.policy.validator_policy_id.clone(),
            worker_id: worker.worker_id.clone(),
            trust_class: worker.trust_class,
            artifact_digest: artifact.artifact_digest.clone(),
            response_signature_digest: response_signature_digest.clone(),
            disposition,
            valid_signature_count: signature_assessment.valid_signature_count,
            reason_codes,
            observed_at_ms,
            receipt_digest,
        };

        self.submission_history.push(WorkerSubmissionRecord {
            worker_id: worker.worker_id.clone(),
            trust_class: worker.trust_class,
            observed_at_ms,
            artifact_digest: artifact.artifact_digest.clone(),
            response_signature_digest,
        });
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArtifactSignatureAssessment {
    valid_signature_count: u16,
    invalid_signature_count: u16,
    untrusted_signer_count: u16,
}

fn assess_artifact_signatures(
    trust_root: &ArtifactTrustRoot,
    artifact_digest: &str,
    attestations: &[SignedArtifactAttestation],
) -> ArtifactSignatureAssessment {
    let trusted_signers = trust_root
        .signers
        .iter()
        .map(|signer| (signer.signer_id.as_str(), signer))
        .collect::<BTreeMap<_, _>>();

    let mut valid_signature_count = 0_u16;
    let mut invalid_signature_count = 0_u16;
    let mut untrusted_signer_count = 0_u16;
    for attestation in attestations {
        if attestation.scope != ArtifactSigningScope::RolloutArtifact
            || attestation.subject_digest != artifact_digest
            || attestation.trust_root_id != trust_root.trust_root_id
        {
            invalid_signature_count = invalid_signature_count.saturating_add(1);
            continue;
        }
        let Some(signer) = trusted_signers.get(attestation.signer_id.as_str()) else {
            untrusted_signer_count = untrusted_signer_count.saturating_add(1);
            continue;
        };
        let Ok(verifying_key) = decode_verifying_key(signer.public_key_hex.as_str()) else {
            invalid_signature_count = invalid_signature_count.saturating_add(1);
            continue;
        };
        let Ok(signature) = decode_signature(attestation.signature_hex.as_str()) else {
            invalid_signature_count = invalid_signature_count.saturating_add(1);
            continue;
        };
        if verifying_key
            .verify(
                &artifact_attestation_payload(
                    attestation.scope,
                    attestation.subject_digest.as_str(),
                    attestation.trust_root_id.as_str(),
                ),
                &signature,
            )
            .is_ok()
        {
            valid_signature_count = valid_signature_count.saturating_add(1);
        } else {
            invalid_signature_count = invalid_signature_count.saturating_add(1);
        }
    }
    ArtifactSignatureAssessment {
        valid_signature_count,
        invalid_signature_count,
        untrusted_signer_count,
    }
}

fn security_disposition(
    policy: &TrainSecurityPolicy,
    reason_codes: &[TrainSecurityReasonCode],
) -> TrainSecurityDisposition {
    let hard_reject = reason_codes.iter().any(|reason_code| {
        matches!(
            reason_code,
            TrainSecurityReasonCode::EnvironmentKeyMismatch
                | TrainSecurityReasonCode::EnvironmentDigestMismatch
                | TrainSecurityReasonCode::MissingArtifactVerificationPolicy
                | TrainSecurityReasonCode::MissingRequiredEnvironmentPolicy
                | TrainSecurityReasonCode::ArtifactSignatureMissing
                | TrainSecurityReasonCode::ArtifactSignatureInvalid
                | TrainSecurityReasonCode::ArtifactSignerUntrusted
                | TrainSecurityReasonCode::ExecutionProofMissingForUntrustedWorker
                | TrainSecurityReasonCode::UntrustedWorkerRateLimited
                | TrainSecurityReasonCode::UntrustedWorkerBurstLimited
        ) || (*reason_code == TrainSecurityReasonCode::DuplicateArtifactDigest
            && policy.poisoning_controls.reject_on_duplicate_artifact)
    });
    if hard_reject {
        return TrainSecurityDisposition::Rejected;
    }
    if policy.poisoning_controls.quarantine_on_duplicate_signature
        && reason_codes.contains(&TrainSecurityReasonCode::DuplicateResponseSignature)
    {
        TrainSecurityDisposition::Quarantined
    } else {
        TrainSecurityDisposition::Accepted
    }
}

fn artifact_attestation_payload(
    scope: ArtifactSigningScope,
    subject_digest: &str,
    trust_root_id: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"psionic_train_artifact_attestation|");
    payload.extend_from_slice(scope.label());
    payload.extend_from_slice(b"|");
    payload.extend_from_slice(subject_digest.as_bytes());
    payload.extend_from_slice(b"|");
    payload.extend_from_slice(trust_root_id.as_bytes());
    payload
}

fn decode_verifying_key(public_key_hex: &str) -> Result<VerifyingKey, ()> {
    let bytes = hex::decode(public_key_hex).map_err(|_| ())?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| ())?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| ())
}

fn decode_signature(signature_hex: &str) -> Result<Signature, ()> {
    let bytes = hex::decode(signature_hex).map_err(|_| ())?;
    let bytes: [u8; 64] = bytes.try_into().map_err(|_| ())?;
    Ok(Signature::from_bytes(&bytes))
}

fn stable_response_signature_digest(artifact: &RolloutArtifact) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_response_signature|");
    hasher.update(artifact.task_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.source_policy_revision.policy_family.as_bytes());
    for sample in &artifact.samples {
        hasher.update(b"|sample|");
        hasher.update(sample.token_id.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(sample.logprob.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(sample.reward.to_bits().to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_security_receipt_digest(
    receipt_id: &str,
    validator_policy_id: &str,
    worker_id: &str,
    trust_class: RolloutWorkerTrustClass,
    artifact_digest: &str,
    response_signature_digest: &str,
    disposition: TrainSecurityDisposition,
    valid_signature_count: u16,
    reason_codes: &[TrainSecurityReasonCode],
    observed_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_security_receipt|");
    hasher.update(receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(validator_policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(rollout_worker_trust_class_label(trust_class));
    hasher.update(b"|");
    hasher.update(artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(response_signature_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(train_security_disposition_label(disposition));
    hasher.update(b"|");
    hasher.update(valid_signature_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(observed_at_ms.to_string().as_bytes());
    for reason_code in reason_codes {
        hasher.update(b"|reason|");
        hasher.update(train_security_reason_code_label(*reason_code));
    }
    hex::encode(hasher.finalize())
}

fn rollout_worker_trust_class_label(trust_class: RolloutWorkerTrustClass) -> &'static [u8] {
    match trust_class {
        RolloutWorkerTrustClass::TrustedTrainer => b"trusted_trainer",
        RolloutWorkerTrustClass::SemiTrustedWorker => b"semi_trusted_worker",
        RolloutWorkerTrustClass::UntrustedWorker => b"untrusted_worker",
    }
}

fn train_security_disposition_label(disposition: TrainSecurityDisposition) -> &'static [u8] {
    match disposition {
        TrainSecurityDisposition::Accepted => b"accepted",
        TrainSecurityDisposition::Quarantined => b"quarantined",
        TrainSecurityDisposition::Rejected => b"rejected",
    }
}

fn train_security_reason_code_label(reason_code: TrainSecurityReasonCode) -> &'static [u8] {
    match reason_code {
        TrainSecurityReasonCode::EnvironmentKeyMismatch => b"environment_key_mismatch",
        TrainSecurityReasonCode::EnvironmentDigestMismatch => b"environment_digest_mismatch",
        TrainSecurityReasonCode::MissingArtifactVerificationPolicy => {
            b"missing_artifact_verification_policy"
        }
        TrainSecurityReasonCode::MissingRequiredEnvironmentPolicy => {
            b"missing_required_environment_policy"
        }
        TrainSecurityReasonCode::ArtifactSignatureMissing => b"artifact_signature_missing",
        TrainSecurityReasonCode::ArtifactSignatureInvalid => b"artifact_signature_invalid",
        TrainSecurityReasonCode::ArtifactSignerUntrusted => b"artifact_signer_untrusted",
        TrainSecurityReasonCode::ExecutionProofMissingForUntrustedWorker => {
            b"execution_proof_missing_for_untrusted_worker"
        }
        TrainSecurityReasonCode::UntrustedWorkerRateLimited => {
            b"untrusted_worker_rate_limited"
        }
        TrainSecurityReasonCode::UntrustedWorkerBurstLimited => {
            b"untrusted_worker_burst_limited"
        }
        TrainSecurityReasonCode::DuplicateArtifactDigest => b"duplicate_artifact_digest",
        TrainSecurityReasonCode::DuplicateResponseSignature => b"duplicate_response_signature",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use psionic_environments::{
        EnvironmentArtifactExpectation, EnvironmentExecutionEntrypoint, EnvironmentPackageFamily,
        EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentStateMode,
        EnvironmentToolContract, EnvironmentToolInterface,
    };

    use crate::{
        PolicyRevision, RolloutProofReference, RolloutSample, RolloutTerminationReason,
    };

    use super::*;

    #[test]
    fn train_security_controller_accepts_signed_verified_untrusted_submission()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let package = sample_environment_package();
        let trust_root = sample_trust_root(&signing_key);
        let mut controller = TrainSecurityController::new(TrainSecurityPolicy::for_environment(
            "validator/security/default",
            &package,
            trust_root.clone(),
        ))?;
        let worker = RolloutWorkerIdentity::new(
            "worker-a",
            RolloutWorkerTrustClass::UntrustedWorker,
            "nostr:worker-a",
        );
        let artifact = sample_rollout_artifact(&package.key, "artifact-a", "task-a", "worker-a");
        let attestation = SignedArtifactAttestation::sign_rollout_artifact(
            trust_root.signers[0].signer_id.clone(),
            trust_root.trust_root_id.clone(),
            artifact.artifact_digest.clone(),
            &signing_key,
        );

        let receipt = controller.assess_rollout_submission(
            &worker,
            &package,
            &artifact,
            &[attestation],
            10_000,
        )?;
        assert_eq!(receipt.disposition, TrainSecurityDisposition::Accepted);
        assert_eq!(receipt.valid_signature_count, 1);
        assert!(receipt.reason_codes.is_empty());
        Ok(())
    }

    #[test]
    fn train_security_controller_rejects_unsigned_or_bursty_untrusted_submission()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[9_u8; 32]);
        let package = sample_environment_package();
        let trust_root = sample_trust_root(&signing_key);
        let mut policy =
            TrainSecurityPolicy::for_environment("validator/security/default", &package, trust_root.clone());
        policy.untrusted_worker_admission.minimum_interarrival_ms = 5_000;
        let mut controller = TrainSecurityController::new(policy)?;
        let worker = RolloutWorkerIdentity::new(
            "worker-a",
            RolloutWorkerTrustClass::UntrustedWorker,
            "nostr:worker-a",
        );
        let artifact_a = sample_rollout_artifact(&package.key, "artifact-a", "task-a", "worker-a");
        let attestation_a = SignedArtifactAttestation::sign_rollout_artifact(
            trust_root.signers[0].signer_id.clone(),
            trust_root.trust_root_id.clone(),
            artifact_a.artifact_digest.clone(),
            &signing_key,
        );
        let first = controller.assess_rollout_submission(
            &worker,
            &package,
            &artifact_a,
            &[attestation_a],
            10_000,
        )?;
        assert_eq!(first.disposition, TrainSecurityDisposition::Accepted);

        let artifact_b = sample_rollout_artifact(&package.key, "artifact-b", "task-b", "worker-a");
        let attestation_b = SignedArtifactAttestation::sign_rollout_artifact(
            trust_root.signers[0].signer_id.clone(),
            trust_root.trust_root_id.clone(),
            artifact_b.artifact_digest.clone(),
            &signing_key,
        );
        let burst = controller.assess_rollout_submission(
            &worker,
            &package,
            &artifact_b,
            &[attestation_b],
            12_000,
        )?;
        assert_eq!(burst.disposition, TrainSecurityDisposition::Rejected);
        assert!(burst
            .reason_codes
            .contains(&TrainSecurityReasonCode::UntrustedWorkerBurstLimited));

        let unsigned = controller.assess_rollout_submission(
            &RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::UntrustedWorker,
                "nostr:worker-b",
            ),
            &package,
            &sample_rollout_artifact(&package.key, "artifact-c", "task-c", "worker-b"),
            &[],
            20_000,
        )?;
        assert_eq!(unsigned.disposition, TrainSecurityDisposition::Rejected);
        assert!(unsigned
            .reason_codes
            .contains(&TrainSecurityReasonCode::ArtifactSignatureMissing));
        Ok(())
    }

    #[test]
    fn train_security_controller_quarantines_duplicate_response_signature_poisoning()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let package = sample_environment_package();
        let trust_root = sample_trust_root(&signing_key);
        let mut controller = TrainSecurityController::new(TrainSecurityPolicy::for_environment(
            "validator/security/default",
            &package,
            trust_root.clone(),
        ))?;
        let first = sample_rollout_artifact(&package.key, "artifact-a", "task-a", "worker-a");
        let first_attestation = SignedArtifactAttestation::sign_rollout_artifact(
            trust_root.signers[0].signer_id.clone(),
            trust_root.trust_root_id.clone(),
            first.artifact_digest.clone(),
            &signing_key,
        );
        let first_receipt = controller.assess_rollout_submission(
            &RolloutWorkerIdentity::new(
                "worker-a",
                RolloutWorkerTrustClass::UntrustedWorker,
                "nostr:worker-a",
            ),
            &package,
            &first,
            &[first_attestation],
            10_000,
        )?;
        assert_eq!(first_receipt.disposition, TrainSecurityDisposition::Accepted);

        let second = sample_rollout_artifact(&package.key, "artifact-b", "task-a", "worker-b");
        let second_attestation = SignedArtifactAttestation::sign_rollout_artifact(
            trust_root.signers[0].signer_id.clone(),
            trust_root.trust_root_id.clone(),
            second.artifact_digest.clone(),
            &signing_key,
        );
        let second_receipt = controller.assess_rollout_submission(
            &RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::UntrustedWorker,
                "nostr:worker-b",
            ),
            &package,
            &second,
            &[second_attestation],
            20_000,
        )?;
        assert_eq!(second_receipt.disposition, TrainSecurityDisposition::Quarantined);
        assert!(second_receipt
            .reason_codes
            .contains(&TrainSecurityReasonCode::DuplicateResponseSignature));
        Ok(())
    }

    fn sample_environment_package() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("weather.agent", "1.0.0"),
            EnvironmentPackageFamily::Agentic,
            "Weather Agent",
            EnvironmentExecutionEntrypoint {
                runtime_family: psionic_environments::EnvironmentRuntimeFamily::MultiTurnDialog,
                entrypoint: String::from("weather.run"),
                args: vec![String::from("--local")],
                sandbox_profile_ref: None,
                max_turns: 4,
                state_mode: EnvironmentStateMode::SessionPersistent,
                time_budget_ms: Some(5_000),
            },
        )
        .with_tools(vec![EnvironmentToolContract {
            tool_name: String::from("get_weather"),
            interface: EnvironmentToolInterface::NativeFunction,
            description: String::from("Fetch the weather for one city"),
            args_schema: json!({"type": "object", "required": ["city"]}),
            result_schema: Some(json!({"type": "object"})),
        }])
        .with_expected_artifacts(vec![EnvironmentArtifactExpectation {
            artifact_kind: String::from("trace"),
            required: true,
            verification_policy_ref: Some(String::from("verify://trace")),
        }])
        .with_policy_references(vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Verification,
            policy_ref: String::from("policy://security/root"),
            required: true,
        }])
    }

    fn sample_trust_root(signing_key: &SigningKey) -> ArtifactTrustRoot {
        ArtifactTrustRoot::new(
            "trust-root-weather",
            1,
            vec![ArtifactSigner::new(
                "validator-a",
                hex::encode(signing_key.verifying_key().to_bytes()),
            )],
        )
    }

    fn sample_rollout_artifact(
        environment: &EnvironmentPackageKey,
        artifact_id: &str,
        task_id: &str,
        worker_id: &str,
    ) -> RolloutArtifact {
        RolloutArtifact::new(
            artifact_id,
            worker_id,
            environment.clone(),
            task_id,
            PolicyRevision::new("weather.policy", "rev-1", "policy-digest", 1_000)
                .with_revision_number(1),
            vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                format!("proof-{artifact_id}"),
                format!("proof://{artifact_id}"),
            )],
            2_000,
        )
        .expect("rollout artifact should be valid")
    }
}
