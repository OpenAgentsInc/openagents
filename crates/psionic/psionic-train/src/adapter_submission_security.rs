use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use psionic_datastream::DatastreamManifestRef;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterAssignmentClaim, AdapterContributionArtifactReceipt,
    AdapterContributionSubmissionOutcome, AdapterContributionSubmissionReceipt,
    AdapterContributionWorkAssignment, AdapterWorkerIdentity, AdapterWorkerProtocolState,
    AdapterWorkerTrustClass,
};

/// Error returned by adapter-submission provenance verification.
#[derive(Debug, Error)]
pub enum AdapterContributionSecurityError {
    /// The submission referenced a claim that is not present in protocol truth.
    #[error("unknown adapter claim `{claim_id}`")]
    UnknownClaim {
        /// Stable claim identifier.
        claim_id: String,
    },
    /// The submission referenced an assignment that is not present in protocol truth.
    #[error("unknown adapter assignment `{assignment_id}`")]
    UnknownAssignment {
        /// Stable assignment identifier.
        assignment_id: String,
    },
    /// The submission referenced a worker/session pair that is not present in protocol truth.
    #[error("unknown adapter worker session for worker `{worker_id}` and session `{session_id}`")]
    UnknownSession {
        /// Stable worker identifier.
        worker_id: String,
        /// Stable session identifier.
        session_id: String,
    },
}

/// Signed scope used for adapter contribution provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionSigningScope {
    /// Signed manifest envelope for one adapter contribution submission.
    ManifestEnvelope,
}

/// Signed manifest envelope bound to worker, session, claim, and artifact truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionManifestAttestation {
    /// Attestation scope.
    pub scope: AdapterContributionSigningScope,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable assignment digest.
    pub assignment_digest: String,
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable claim digest.
    pub claim_digest: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Stable auth subject bound to the session.
    pub auth_subject: String,
    /// Trust posture for the worker session.
    pub trust_class: AdapterWorkerTrustClass,
    /// Stable target policy revision expected by the contribution.
    pub target_policy_revision_id: String,
    /// Stable target checkpoint pointer digest expected by the contribution.
    pub target_checkpoint_pointer_digest: String,
    /// Stable upload expectation digest.
    pub upload_expectation_digest: String,
    /// Stable submission receipt digest this attestation seals.
    pub submission_receipt_digest: String,
    /// Stable uploaded artifact reference.
    pub upload_reference: String,
    /// Stable datastream manifest digest.
    pub manifest_digest: String,
    /// Stable datastream object digest.
    pub object_digest: String,
    /// Attestation time.
    pub signed_at_ms: u64,
    /// Hex-encoded Ed25519 signature over the attestation payload.
    pub signature_hex: String,
}

impl AdapterContributionManifestAttestation {
    fn new_signed(
        assignment: &AdapterContributionWorkAssignment,
        claim: &AdapterAssignmentClaim,
        session_identity: &AdapterWorkerIdentity,
        submission: &AdapterContributionSubmissionReceipt,
        artifact: &AdapterContributionArtifactReceipt,
        signing_key: &SigningKey,
        signed_at_ms: u64,
    ) -> Self {
        let mut attestation = Self {
            scope: AdapterContributionSigningScope::ManifestEnvelope,
            assignment_id: assignment.assignment_id.clone(),
            assignment_digest: assignment.assignment_digest.clone(),
            claim_id: claim.claim_id.clone(),
            claim_digest: claim.claim_digest.clone(),
            contribution_id: submission.contribution_id.clone(),
            window_id: submission.window_id.clone(),
            worker_id: session_identity.worker_id.clone(),
            session_id: session_identity.session_id.clone(),
            auth_subject: session_identity.auth_subject.clone(),
            trust_class: session_identity.trust_class,
            target_policy_revision_id: submission.target_policy_revision_id.clone(),
            target_checkpoint_pointer_digest: submission.target_checkpoint_pointer_digest.clone(),
            upload_expectation_digest: submission.upload_expectation.expectation_digest.clone(),
            submission_receipt_digest: submission.receipt_digest.clone(),
            upload_reference: artifact.upload.upload_reference.clone(),
            manifest_digest: artifact.manifest.manifest_digest.clone(),
            object_digest: artifact.manifest.object_digest.clone(),
            signed_at_ms,
            signature_hex: String::new(),
        };
        let signature = signing_key.sign(&attestation.payload_bytes());
        attestation.signature_hex = hex::encode(signature.to_bytes());
        attestation
    }

    fn payload_bytes(&self) -> Vec<u8> {
        adapter_manifest_attestation_payload(
            self.scope,
            self.assignment_id.as_str(),
            self.assignment_digest.as_str(),
            self.claim_id.as_str(),
            self.claim_digest.as_str(),
            self.contribution_id.as_str(),
            self.window_id.as_str(),
            self.worker_id.as_str(),
            self.session_id.as_str(),
            self.auth_subject.as_str(),
            self.trust_class,
            self.target_policy_revision_id.as_str(),
            self.target_checkpoint_pointer_digest.as_str(),
            self.upload_expectation_digest.as_str(),
            self.submission_receipt_digest.as_str(),
            self.upload_reference.as_str(),
            self.manifest_digest.as_str(),
            self.object_digest.as_str(),
            self.signed_at_ms,
        )
    }
}

/// Independently verifiable provenance bundle preserved for one contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionProvenanceBundle {
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Session identity whose key signed the bundle.
    pub session_identity: AdapterWorkerIdentity,
    /// Datastream manifest reference for the uploaded artifact.
    pub manifest: DatastreamManifestRef,
    /// Signed manifest attestation.
    pub attestation: AdapterContributionManifestAttestation,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl AdapterContributionProvenanceBundle {
    /// Creates one signed provenance bundle over a completed adapter contribution artifact.
    #[must_use]
    pub fn new_signed(
        assignment: &AdapterContributionWorkAssignment,
        claim: &AdapterAssignmentClaim,
        session_identity: &AdapterWorkerIdentity,
        submission: &AdapterContributionSubmissionReceipt,
        artifact: &AdapterContributionArtifactReceipt,
        signing_key: &SigningKey,
        signed_at_ms: u64,
    ) -> Self {
        let attestation = AdapterContributionManifestAttestation::new_signed(
            assignment,
            claim,
            session_identity,
            submission,
            artifact,
            signing_key,
            signed_at_ms,
        );
        Self {
            bundle_id: format!("adapter-provenance:{}", artifact.artifact_id),
            session_identity: session_identity.clone(),
            manifest: artifact.manifest.clone(),
            bundle_digest: stable_provenance_bundle_digest(
                artifact.artifact_id.as_str(),
                session_identity.worker_id.as_str(),
                session_identity.session_id.as_str(),
                session_identity.auth_subject.as_str(),
                session_identity.trust_class,
                session_identity.submission_signing_public_key_hex.as_str(),
                artifact.manifest.manifest_digest.as_str(),
                artifact.manifest.object_digest.as_str(),
                attestation.signature_hex.as_str(),
                attestation.signed_at_ms,
            ),
            attestation,
        }
    }

    /// Verifies the signed manifest envelope using only the bundle contents.
    #[must_use]
    pub fn verify_signature(&self) -> bool {
        let Ok(verifying_key) = decode_verifying_key(
            self.session_identity
                .submission_signing_public_key_hex
                .as_str(),
        ) else {
            return false;
        };
        let Ok(signature) = decode_signature(self.attestation.signature_hex.as_str()) else {
            return false;
        };
        verifying_key
            .verify(&self.attestation.payload_bytes(), &signature)
            .is_ok()
    }
}

/// Verification policy for adapter contribution provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionSecurityPolicy {
    /// Maximum allowed time since the last worker heartbeat when verifying provenance.
    pub session_freshness_grace_ms: u64,
    /// Whether every worker session must surface a submission-signing key.
    pub require_submission_signing_key: bool,
}

impl Default for AdapterContributionSecurityPolicy {
    fn default() -> Self {
        Self {
            session_freshness_grace_ms: 10_000,
            require_submission_signing_key: true,
        }
    }
}

/// Final disposition for one adapter contribution provenance check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionSecurityDisposition {
    /// Provenance matched the assignment, session, and artifact truth.
    Accepted,
    /// Provenance is retained for review but not aggregation-ready.
    Quarantined,
    /// Provenance is not admissible and the contribution must be refused.
    Rejected,
}

/// Machine-readable refusal or quarantine reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionSecurityReasonCode {
    /// The submission outcome itself was not accepted by the worker protocol.
    SubmissionNotAccepted,
    /// Worker id mismatched assignment, claim, submission, or artifact truth.
    WorkerIdentityMismatch,
    /// Session id mismatched claim, submission, or live session truth.
    SessionIdentityMismatch,
    /// Auth subject mismatched the live session identity.
    AuthSubjectMismatch,
    /// Trust class mismatched the live session identity.
    TrustClassMismatch,
    /// The live session or bundle omitted the signing key.
    SigningKeyMissing,
    /// The bundle key did not match the live session key.
    SigningKeyMismatch,
    /// The bundle signature was missing.
    SignatureMissing,
    /// The bundle signature did not verify.
    SignatureInvalid,
    /// Assignment id or digest mismatched control-plane truth.
    AssignmentMismatch,
    /// Claim id or digest mismatched control-plane truth.
    ClaimMismatch,
    /// Submission receipt digest mismatched control-plane truth.
    SubmissionReceiptMismatch,
    /// Policy-revision binding mismatched claim, submission, or artifact truth.
    PolicyRevisionMismatch,
    /// Checkpoint-pointer binding mismatched claim, submission, or artifact truth.
    CheckpointPointerMismatch,
    /// Upload expectation mismatched claim or submission truth.
    UploadExpectationMismatch,
    /// Upload reference mismatched submission or artifact truth.
    UploadReferenceMismatch,
    /// Manifest or object digest mismatched the staged artifact truth.
    ManifestDigestMismatch,
    /// The verifying session heartbeat was stale at verification time.
    StaleSession,
}

/// Typed receipt for one adapter contribution provenance decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionSecurityReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Final security disposition.
    pub disposition: AdapterContributionSecurityDisposition,
    /// Whether the bundle signature verified successfully.
    pub signature_verified: bool,
    /// Machine-readable reason codes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<AdapterContributionSecurityReasonCode>,
    /// Stable digest of the preserved provenance bundle.
    pub bundle_digest: String,
    /// Verification time.
    pub verified_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stateful controller that preserves accepted adapter provenance bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionSecurityController {
    /// Active verification policy.
    pub policy: AdapterContributionSecurityPolicy,
    /// Accepted provenance bundles preserved for validator and aggregator use.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_provenance_bundles: Vec<AdapterContributionProvenanceBundle>,
    /// Historical security receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<AdapterContributionSecurityReceipt>,
}

impl AdapterContributionSecurityController {
    /// Creates a controller with the requested policy.
    #[must_use]
    pub fn new(policy: AdapterContributionSecurityPolicy) -> Self {
        Self {
            policy,
            accepted_provenance_bundles: Vec::new(),
            receipts: Vec::new(),
        }
    }

    /// Verifies one signed provenance bundle against worker-protocol and artifact truth.
    pub fn assess_submission(
        &mut self,
        protocol: &AdapterWorkerProtocolState,
        artifact: &AdapterContributionArtifactReceipt,
        submission: &AdapterContributionSubmissionReceipt,
        bundle: AdapterContributionProvenanceBundle,
        verified_at_ms: u64,
    ) -> Result<AdapterContributionSecurityReceipt, AdapterContributionSecurityError> {
        let claim = protocol
            .claims
            .iter()
            .find(|claim| claim.claim_id == submission.claim_id)
            .ok_or_else(|| AdapterContributionSecurityError::UnknownClaim {
                claim_id: submission.claim_id.clone(),
            })?;
        let assignment = protocol
            .assignments
            .iter()
            .find(|assignment| assignment.assignment_id == submission.assignment_id)
            .ok_or_else(|| AdapterContributionSecurityError::UnknownAssignment {
                assignment_id: submission.assignment_id.clone(),
            })?;
        let session = protocol
            .sessions
            .iter()
            .find(|session| {
                session.identity.worker_id == submission.worker_id
                    && session.identity.session_id == submission.session_id
            })
            .ok_or_else(|| AdapterContributionSecurityError::UnknownSession {
                worker_id: submission.worker_id.clone(),
                session_id: submission.session_id.clone(),
            })?;

        let mut reason_codes = Vec::new();

        if submission.outcome != AdapterContributionSubmissionOutcome::Accepted {
            reason_codes.push(AdapterContributionSecurityReasonCode::SubmissionNotAccepted);
        }
        if bundle.session_identity.worker_id != session.identity.worker_id
            || bundle.attestation.worker_id != session.identity.worker_id
            || submission.worker_id != session.identity.worker_id
            || artifact.worker_id != session.identity.worker_id
            || claim.worker_id != session.identity.worker_id
            || assignment.worker_id != session.identity.worker_id
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::WorkerIdentityMismatch);
        }
        if bundle.session_identity.session_id != session.identity.session_id
            || bundle.attestation.session_id != session.identity.session_id
            || submission.session_id != session.identity.session_id
            || claim.session_id != session.identity.session_id
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::SessionIdentityMismatch);
        }
        if bundle.session_identity.auth_subject != session.identity.auth_subject
            || bundle.attestation.auth_subject != session.identity.auth_subject
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::AuthSubjectMismatch);
        }
        if bundle.session_identity.trust_class != session.identity.trust_class
            || bundle.attestation.trust_class != session.identity.trust_class
            || submission.trust_class != session.identity.trust_class
            || claim.trust_class != session.identity.trust_class
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::TrustClassMismatch);
        }
        if verified_at_ms.saturating_sub(session.last_heartbeat_at_ms)
            > self.policy.session_freshness_grace_ms
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::StaleSession);
        }

        if self.policy.require_submission_signing_key
            && (session
                .identity
                .submission_signing_public_key_hex
                .trim()
                .is_empty()
                || bundle
                    .session_identity
                    .submission_signing_public_key_hex
                    .trim()
                    .is_empty())
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::SigningKeyMissing);
        } else if bundle.session_identity.submission_signing_public_key_hex
            != session.identity.submission_signing_public_key_hex
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::SigningKeyMismatch);
        }

        if bundle.attestation.assignment_id != assignment.assignment_id
            || bundle.attestation.assignment_digest != assignment.assignment_digest
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::AssignmentMismatch);
        }
        if bundle.attestation.claim_id != claim.claim_id
            || bundle.attestation.claim_digest != claim.claim_digest
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::ClaimMismatch);
        }
        if bundle.attestation.submission_receipt_digest != submission.receipt_digest {
            reason_codes.push(AdapterContributionSecurityReasonCode::SubmissionReceiptMismatch);
        }
        if bundle.attestation.target_policy_revision_id != claim.target_policy_revision_id
            || submission.target_policy_revision_id != claim.target_policy_revision_id
            || submission.source_policy_revision_id != claim.target_policy_revision_id
            || artifact.source_policy_revision_id != claim.target_policy_revision_id
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::PolicyRevisionMismatch);
        }
        if bundle.attestation.target_checkpoint_pointer_digest
            != claim.target_checkpoint_pointer_digest
            || submission.target_checkpoint_pointer_digest != claim.target_checkpoint_pointer_digest
            || submission.source_checkpoint_pointer_digest != claim.target_checkpoint_pointer_digest
            || artifact.source_checkpoint_pointer_digest != claim.target_checkpoint_pointer_digest
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::CheckpointPointerMismatch);
        }
        if bundle.attestation.upload_expectation_digest
            != claim.upload_expectation.expectation_digest
            || submission.upload_expectation.expectation_digest
                != claim.upload_expectation.expectation_digest
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::UploadExpectationMismatch);
        }
        if bundle.attestation.upload_reference != artifact.upload.upload_reference
            || submission.upload.upload_reference != artifact.upload.upload_reference
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::UploadReferenceMismatch);
        }
        if bundle.manifest.manifest_digest != artifact.manifest.manifest_digest
            || bundle.attestation.manifest_digest != artifact.manifest.manifest_digest
            || bundle.manifest.object_digest != artifact.manifest.object_digest
            || bundle.attestation.object_digest != artifact.manifest.object_digest
        {
            reason_codes.push(AdapterContributionSecurityReasonCode::ManifestDigestMismatch);
        }

        let signature_verified = if bundle.attestation.signature_hex.trim().is_empty() {
            reason_codes.push(AdapterContributionSecurityReasonCode::SignatureMissing);
            false
        } else {
            let verified = bundle.verify_signature();
            if !verified {
                reason_codes.push(AdapterContributionSecurityReasonCode::SignatureInvalid);
            }
            verified
        };

        let disposition = security_disposition(reason_codes.as_slice());
        let receipt = AdapterContributionSecurityReceipt {
            receipt_id: format!("adapter-security:{}", artifact.artifact_id),
            artifact_id: artifact.artifact_id.clone(),
            claim_id: claim.claim_id.clone(),
            assignment_id: assignment.assignment_id.clone(),
            contribution_id: artifact.contribution_id.clone(),
            worker_id: session.identity.worker_id.clone(),
            session_id: session.identity.session_id.clone(),
            disposition,
            signature_verified,
            reason_codes,
            bundle_digest: bundle.bundle_digest.clone(),
            verified_at_ms,
            receipt_digest: stable_security_receipt_digest(
                artifact.artifact_id.as_str(),
                claim.claim_id.as_str(),
                assignment.assignment_id.as_str(),
                artifact.contribution_id.as_str(),
                session.identity.worker_id.as_str(),
                session.identity.session_id.as_str(),
                disposition,
                signature_verified,
                bundle.bundle_digest.as_str(),
                verified_at_ms,
            ),
        };
        if receipt.disposition == AdapterContributionSecurityDisposition::Accepted {
            self.accepted_provenance_bundles.push(bundle);
        }
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

fn security_disposition(
    reason_codes: &[AdapterContributionSecurityReasonCode],
) -> AdapterContributionSecurityDisposition {
    let has_hard_reject = reason_codes.iter().any(|reason_code| {
        !matches!(
            reason_code,
            AdapterContributionSecurityReasonCode::StaleSession
        )
    });
    if has_hard_reject {
        AdapterContributionSecurityDisposition::Rejected
    } else if reason_codes.contains(&AdapterContributionSecurityReasonCode::StaleSession) {
        AdapterContributionSecurityDisposition::Quarantined
    } else {
        AdapterContributionSecurityDisposition::Accepted
    }
}

fn adapter_manifest_attestation_payload(
    scope: AdapterContributionSigningScope,
    assignment_id: &str,
    assignment_digest: &str,
    claim_id: &str,
    claim_digest: &str,
    contribution_id: &str,
    window_id: &str,
    worker_id: &str,
    session_id: &str,
    auth_subject: &str,
    trust_class: AdapterWorkerTrustClass,
    target_policy_revision_id: &str,
    target_checkpoint_pointer_digest: &str,
    upload_expectation_digest: &str,
    submission_receipt_digest: &str,
    upload_reference: &str,
    manifest_digest: &str,
    object_digest: &str,
    signed_at_ms: u64,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"psionic_adapter_manifest_attestation|");
    payload.extend_from_slice(adapter_signing_scope_label(scope));
    for part in [
        assignment_id,
        assignment_digest,
        claim_id,
        claim_digest,
        contribution_id,
        window_id,
        worker_id,
        session_id,
        auth_subject,
        adapter_worker_trust_class_label(trust_class),
        target_policy_revision_id,
        target_checkpoint_pointer_digest,
        upload_expectation_digest,
        submission_receipt_digest,
        upload_reference,
        manifest_digest,
        object_digest,
        signed_at_ms.to_string().as_str(),
    ] {
        payload.extend_from_slice(b"|");
        payload.extend_from_slice(part.as_bytes());
    }
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

fn stable_provenance_bundle_digest(
    artifact_id: &str,
    worker_id: &str,
    session_id: &str,
    auth_subject: &str,
    trust_class: AdapterWorkerTrustClass,
    public_key_hex: &str,
    manifest_digest: &str,
    object_digest: &str,
    signature_hex: &str,
    signed_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_provenance_bundle",
        artifact_id,
        worker_id,
        session_id,
        auth_subject,
        adapter_worker_trust_class_label(trust_class),
        public_key_hex,
        manifest_digest,
        object_digest,
        signature_hex,
        signed_at_ms.to_string().as_str(),
    ])
}

fn stable_security_receipt_digest(
    artifact_id: &str,
    claim_id: &str,
    assignment_id: &str,
    contribution_id: &str,
    worker_id: &str,
    session_id: &str,
    disposition: AdapterContributionSecurityDisposition,
    signature_verified: bool,
    bundle_digest: &str,
    verified_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_contribution_security_receipt",
        artifact_id,
        claim_id,
        assignment_id,
        contribution_id,
        worker_id,
        session_id,
        adapter_security_disposition_label(disposition),
        if signature_verified {
            "signature_verified"
        } else {
            "signature_failed"
        },
        bundle_digest,
        verified_at_ms.to_string().as_str(),
    ])
}

fn stable_digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

fn adapter_signing_scope_label(scope: AdapterContributionSigningScope) -> &'static [u8] {
    match scope {
        AdapterContributionSigningScope::ManifestEnvelope => b"manifest_envelope",
    }
}

fn adapter_security_disposition_label(
    disposition: AdapterContributionSecurityDisposition,
) -> &'static str {
    match disposition {
        AdapterContributionSecurityDisposition::Accepted => "accepted",
        AdapterContributionSecurityDisposition::Quarantined => "quarantined",
        AdapterContributionSecurityDisposition::Rejected => "rejected",
    }
}

fn adapter_worker_trust_class_label(trust_class: AdapterWorkerTrustClass) -> &'static str {
    match trust_class {
        AdapterWorkerTrustClass::TrustedOperator => "trusted_operator",
        AdapterWorkerTrustClass::SemiTrustedContributor => "semi_trusted_contributor",
        AdapterWorkerTrustClass::UntrustedContributor => "untrusted_contributor",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use ed25519_dalek::SigningKey;
    use psionic_cluster::{
        AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterStabilityPosture, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};

    use super::{
        AdapterContributionProvenanceBundle, AdapterContributionSecurityController,
        AdapterContributionSecurityDisposition, AdapterContributionSecurityPolicy,
        AdapterContributionSecurityReasonCode,
    };
    use crate::{
        AdapterArtifactRetentionPolicy, AdapterArtifactStorageState,
        AdapterContributionExecutionSummary, AdapterContributionUploadLocator,
        AdapterContributorCapabilityPolicy, AdapterDatasetSliceIdentity, AdapterTargetIdentity,
        AdapterTrainingClusterCoordinator, AdapterWorkerIdentity, AdapterWorkerProtocolPolicy,
        AdapterWorkerProtocolState, AdapterWorkerTrustClass, CheckpointPointer,
        CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision,
    };

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("adapter-submission-security"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterMembershipRecord::new(
                ClusterNodeIdentity {
                    cluster_id: cluster_id.clone(),
                    node_id: NodeId::new("worker-b"),
                    node_epoch: NodeEpoch::initial(),
                    role: NodeRole::ExecutorOnly,
                    auth_public_key: String::from("worker-b-pk"),
                    attestation: None,
                },
                Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33_300)),
                ClusterMembershipStatus::Ready,
            ),
        )]);
        snapshot.telemetry = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterNodeTelemetry::new(NodeId::new("worker-b"))
                .with_memory(Some(24 * GIB_BYTES), Some(24 * GIB_BYTES))
                .with_accelerator_count(1)
                .with_backend_readiness(
                    AdapterContributorCapabilityPolicy::default().backend_label,
                    ClusterBackendReadinessStatus::Ready,
                )
                .with_stability_posture(ClusterStabilityPosture::Stable),
        )]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn checkpoint_reference(
        checkpoint_ref: &str,
        started_at_ms: u64,
    ) -> psionic_runtime::TrainingCheckpointReference {
        psionic_runtime::TrainingCheckpointReference::new(
            "apple.weather.policy",
            format!("stream://{checkpoint_ref}"),
            format!("manifest://{checkpoint_ref}"),
            format!("object://{checkpoint_ref}"),
            "worker-b",
            7,
            "cluster-digest-weather",
            "topology-digest-weather",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(70)
    }

    fn protocol_and_artifact(
        signing_key: &SigningKey,
    ) -> Result<
        (
            AdapterWorkerProtocolState,
            crate::AdapterContributionWorkAssignment,
            crate::AdapterAssignmentClaim,
            crate::AdapterContributionSubmissionReceipt,
            crate::AdapterContributionArtifactReceipt,
        ),
        Box<dyn std::error::Error>,
    > {
        let state = cluster_state();
        let run = crate::TrainingRunState::new(
            "adapter-run-security",
            "adapter-sft",
            state.cluster_id().as_str(),
            "apple.weather.policy",
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
        )?;
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            run,
            AdapterTargetIdentity::new(
                "apple.weather.adapter",
                "apple.foundation_models",
                "apple://foundation-model/base",
                "apple.fmadapter",
            )?,
            PolicyRevision::new(
                "apple.weather.policy",
                "policy-r7",
                "policy-digest-r7",
                1_000,
            )
            .with_revision_number(7),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-5"),
                "apple.weather.policy",
                checkpoint_reference("checkpoint/weather/r7", 1_000),
                "manifest-digest-r7",
                1_001,
            )?,
            AdapterContributorCapabilityPolicy {
                minimum_free_memory_bytes: 12 * GIB_BYTES,
                ..AdapterContributorCapabilityPolicy::default()
            },
        );
        coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-b",
                "slice-digest-b",
            )?],
            1,
            1_020,
        )?;

        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let identity = AdapterWorkerIdentity::new(
            "worker-b",
            "session-1",
            AdapterWorkerTrustClass::SemiTrustedContributor,
            "auth://worker-b",
        )
        .with_submission_signing_public_key_hex(public_key_hex);
        protocol.record_heartbeat(identity, None, None, 1_030)?;

        let assignment = protocol.assignments[0].clone();
        let claim =
            protocol.claim_assignment("worker-b", assignment.assignment_id.as_str(), 1_031)?;
        protocol.acknowledge_assignment("worker-b", "session-1", claim.claim_id.as_str(), 1_032)?;
        let submission = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "session-1",
            "policy-r7",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            AdapterContributionExecutionSummary::new(
                1_033,
                1_040,
                5,
                20,
                Some(205),
                "delta-digest-security",
            )?,
            AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, claim.contribution_id
                ),
                "upload-manifest-security",
                4_096,
            )?,
            1_041,
        )?;

        let payload = b"adapter-delta-payload".repeat(4);
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            &payload,
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            assignment
                .upload_expectation
                .upload_reference_prefix
                .clone(),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            &payload,
            8,
            "worker-b",
            1_042,
        )?;
        for chunk in payload.chunks(8) {
            storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
        }
        let artifact = storage.complete_contribution_upload(cursor.upload_id.as_str(), 1_043)?;

        Ok((protocol, assignment, claim, submission, artifact))
    }

    #[test]
    fn accepted_submission_preserves_independently_verifiable_bundle()
    -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
        let (protocol, assignment, claim, submission, artifact) =
            protocol_and_artifact(&signing_key)?;
        let session = &protocol.sessions[0].identity;
        let bundle = AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            session,
            &submission,
            &artifact,
            &signing_key,
            1_044,
        );
        assert!(bundle.verify_signature());

        let mut controller = AdapterContributionSecurityController::new(
            AdapterContributionSecurityPolicy::default(),
        );
        let receipt = controller.assess_submission(
            &protocol,
            &artifact,
            &submission,
            bundle.clone(),
            1_045,
        )?;
        assert_eq!(
            receipt.disposition,
            AdapterContributionSecurityDisposition::Accepted
        );
        assert!(receipt.signature_verified);
        assert!(receipt.reason_codes.is_empty());
        assert_eq!(controller.accepted_provenance_bundles, vec![bundle]);
        Ok(())
    }

    #[test]
    fn signature_mismatch_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[19_u8; 32]);
        let (protocol, assignment, claim, submission, artifact) =
            protocol_and_artifact(&signing_key)?;
        let session = &protocol.sessions[0].identity;
        let mut bundle = AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            session,
            &submission,
            &artifact,
            &signing_key,
            1_044,
        );
        bundle.attestation.signature_hex = String::from("00");

        let mut controller = AdapterContributionSecurityController::new(
            AdapterContributionSecurityPolicy::default(),
        );
        let receipt =
            controller.assess_submission(&protocol, &artifact, &submission, bundle, 1_045)?;
        assert_eq!(
            receipt.disposition,
            AdapterContributionSecurityDisposition::Rejected
        );
        assert!(
            receipt
                .reason_codes
                .contains(&AdapterContributionSecurityReasonCode::SignatureInvalid)
        );
        Ok(())
    }

    #[test]
    fn reassigned_worker_bundle_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let assignment_key = SigningKey::from_bytes(&[23_u8; 32]);
        let wrong_worker_key = SigningKey::from_bytes(&[29_u8; 32]);
        let (protocol, assignment, claim, submission, artifact) =
            protocol_and_artifact(&assignment_key)?;
        let wrong_identity = AdapterWorkerIdentity::new(
            "worker-a",
            "session-9",
            AdapterWorkerTrustClass::SemiTrustedContributor,
            "auth://worker-a",
        )
        .with_submission_signing_public_key_hex(hex::encode(
            wrong_worker_key.verifying_key().to_bytes(),
        ));
        let bundle = AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            &wrong_identity,
            &submission,
            &artifact,
            &wrong_worker_key,
            1_044,
        );

        let mut controller = AdapterContributionSecurityController::new(
            AdapterContributionSecurityPolicy::default(),
        );
        let receipt =
            controller.assess_submission(&protocol, &artifact, &submission, bundle, 1_045)?;
        assert_eq!(
            receipt.disposition,
            AdapterContributionSecurityDisposition::Rejected
        );
        assert!(
            receipt
                .reason_codes
                .contains(&AdapterContributionSecurityReasonCode::WorkerIdentityMismatch)
        );
        assert!(
            receipt
                .reason_codes
                .contains(&AdapterContributionSecurityReasonCode::SessionIdentityMismatch)
        );
        Ok(())
    }

    #[test]
    fn stale_session_is_quarantined() -> Result<(), Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[31_u8; 32]);
        let (protocol, assignment, claim, submission, artifact) =
            protocol_and_artifact(&signing_key)?;
        let session = &protocol.sessions[0].identity;
        let bundle = AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            session,
            &submission,
            &artifact,
            &signing_key,
            1_044,
        );

        let mut controller =
            AdapterContributionSecurityController::new(AdapterContributionSecurityPolicy {
                session_freshness_grace_ms: 1,
                require_submission_signing_key: true,
            });
        let receipt =
            controller.assess_submission(&protocol, &artifact, &submission, bundle, 1_050)?;
        assert_eq!(
            receipt.disposition,
            AdapterContributionSecurityDisposition::Quarantined
        );
        assert!(
            receipt
                .reason_codes
                .contains(&AdapterContributionSecurityReasonCode::StaleSession)
        );
        Ok(())
    }
}
