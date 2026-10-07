use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{
    TassadarComputationalModuleManifest, TassadarComputationalModuleManifestError,
    TassadarModuleTrustPosture,
};

/// Explicit compiler-side compatibility request over one computational module manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCompatibilityRequest {
    /// Stable consumer family that wants to link the module.
    pub consumer_family: String,
    /// Exported symbols required by the consumer.
    pub required_exports: Vec<String>,
    /// Required benchmark refs that must appear in the manifest.
    pub required_benchmark_refs: Vec<String>,
    /// Minimum trust posture allowed by the consumer.
    pub minimum_trust_posture: TassadarModuleTrustPosture,
    /// Allowed claim classes for the consumer.
    pub allowed_claim_classes: Vec<String>,
}

/// Compatibility outcome for one module manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleCompatibilityStatus {
    /// The manifest satisfies the current consumer constraints.
    Compatible,
    /// The manifest is invalid or incompatible.
    Refused,
}

/// Compiler-side compatibility receipt over one module manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleCompatibilityReceipt {
    /// Stable consumer family.
    pub consumer_family: String,
    /// Stable module ref.
    pub module_ref: String,
    /// Stable compatibility digest from the manifest.
    pub compatibility_digest: String,
    /// Stable compatibility status.
    pub status: TassadarModuleCompatibilityStatus,
    /// Export symbols satisfied during compatibility evaluation.
    pub satisfied_exports: Vec<String>,
    /// Required benchmark refs satisfied during evaluation.
    pub satisfied_benchmark_refs: Vec<String>,
    /// Typed refusal reason when compatibility failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarModuleCompatibilityRefusalReason>,
    /// Plain-language detail.
    pub detail: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl TassadarModuleCompatibilityReceipt {
    fn new(
        consumer_family: impl Into<String>,
        module_ref: impl Into<String>,
        compatibility_digest: impl Into<String>,
        status: TassadarModuleCompatibilityStatus,
        mut satisfied_exports: Vec<String>,
        mut satisfied_benchmark_refs: Vec<String>,
        refusal_reason: Option<TassadarModuleCompatibilityRefusalReason>,
        detail: impl Into<String>,
    ) -> Self {
        satisfied_exports.sort();
        satisfied_exports.dedup();
        satisfied_benchmark_refs.sort();
        satisfied_benchmark_refs.dedup();
        let mut receipt = Self {
            consumer_family: consumer_family.into(),
            module_ref: module_ref.into(),
            compatibility_digest: compatibility_digest.into(),
            status,
            satisfied_exports,
            satisfied_benchmark_refs,
            refusal_reason,
            detail: detail.into(),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest =
            stable_digest(b"psionic_tassadar_module_compatibility_receipt|", &receipt);
        receipt
    }
}

/// Typed refusal reason for compiler-side compatibility checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleCompatibilityRefusalReason {
    /// The manifest failed schema validation.
    InvalidManifest,
    /// The manifest trust posture was too weak.
    TrustPostureTooWeak,
    /// The manifest claim class was not allowed.
    ClaimClassDisallowed,
    /// One required export was missing.
    MissingRequiredExport,
    /// One required benchmark ref was missing.
    MissingBenchmarkEvidence,
}

/// Error returned by compiler-side manifest validation or compatibility checks.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarModuleCompatibilityError {
    /// The manifest was malformed.
    #[error(transparent)]
    InvalidManifest(#[from] TassadarComputationalModuleManifestError),
    /// The manifest trust posture was too weak.
    #[error("module trust posture {actual:?} is weaker than required {required:?}")]
    TrustPostureTooWeak {
        /// Minimum required trust posture.
        required: TassadarModuleTrustPosture,
        /// Actual trust posture.
        actual: TassadarModuleTrustPosture,
    },
    /// The manifest claim class was not allowed.
    #[error("module claim class `{claim_class}` is not allowed for this consumer")]
    ClaimClassDisallowed {
        /// Actual manifest claim class.
        claim_class: String,
    },
    /// A required export symbol was missing.
    #[error("module is missing required export `{symbol}`")]
    MissingRequiredExport {
        /// Missing export symbol.
        symbol: String,
    },
    /// One required benchmark reference was missing.
    #[error("module is missing required benchmark ref `{benchmark_ref}`")]
    MissingBenchmarkEvidence {
        /// Missing benchmark ref.
        benchmark_ref: String,
    },
}

/// Validates a computational module manifest independent of link or install flow.
pub fn validate_tassadar_computational_module_manifest(
    manifest: &TassadarComputationalModuleManifest,
) -> Result<(), TassadarComputationalModuleManifestError> {
    manifest.validate()
}

/// Checks whether one module manifest satisfies the current bounded consumer constraints.
pub fn check_tassadar_module_manifest_compatibility(
    manifest: &TassadarComputationalModuleManifest,
    request: &TassadarModuleCompatibilityRequest,
) -> Result<TassadarModuleCompatibilityReceipt, TassadarModuleCompatibilityError> {
    validate_tassadar_computational_module_manifest(manifest)?;
    if manifest.trust_posture < request.minimum_trust_posture {
        return Err(TassadarModuleCompatibilityError::TrustPostureTooWeak {
            required: request.minimum_trust_posture,
            actual: manifest.trust_posture,
        });
    }
    if !request.allowed_claim_classes.is_empty()
        && !request
            .allowed_claim_classes
            .iter()
            .any(|claim_class| claim_class == &manifest.claim_class)
    {
        return Err(TassadarModuleCompatibilityError::ClaimClassDisallowed {
            claim_class: manifest.claim_class.clone(),
        });
    }
    for required_export in &request.required_exports {
        if !manifest
            .exports
            .iter()
            .any(|export| export.symbol == *required_export)
        {
            return Err(TassadarModuleCompatibilityError::MissingRequiredExport {
                symbol: required_export.clone(),
            });
        }
    }
    for benchmark_ref in &request.required_benchmark_refs {
        if !manifest
            .benchmark_lineage_refs
            .iter()
            .any(|candidate| candidate == benchmark_ref)
        {
            return Err(TassadarModuleCompatibilityError::MissingBenchmarkEvidence {
                benchmark_ref: benchmark_ref.clone(),
            });
        }
    }
    Ok(TassadarModuleCompatibilityReceipt::new(
        request.consumer_family.clone(),
        manifest.module_ref.clone(),
        manifest.compatibility_digest.clone(),
        TassadarModuleCompatibilityStatus::Compatible,
        request.required_exports.clone(),
        request.required_benchmark_refs.clone(),
        None,
        format!(
            "consumer `{}` can link module `{}` under trust posture {:?} with compatibility digest {}",
            request.consumer_family,
            manifest.module_ref,
            manifest.trust_posture,
            manifest.compatibility_digest,
        ),
    ))
}

/// Builds a refusal receipt for one failed compatibility check.
#[must_use]
pub fn tassadar_module_compatibility_refusal_receipt(
    manifest: &TassadarComputationalModuleManifest,
    request: &TassadarModuleCompatibilityRequest,
    error: &TassadarModuleCompatibilityError,
) -> TassadarModuleCompatibilityReceipt {
    let refusal_reason = match error {
        TassadarModuleCompatibilityError::InvalidManifest(_) => {
            TassadarModuleCompatibilityRefusalReason::InvalidManifest
        }
        TassadarModuleCompatibilityError::TrustPostureTooWeak { .. } => {
            TassadarModuleCompatibilityRefusalReason::TrustPostureTooWeak
        }
        TassadarModuleCompatibilityError::ClaimClassDisallowed { .. } => {
            TassadarModuleCompatibilityRefusalReason::ClaimClassDisallowed
        }
        TassadarModuleCompatibilityError::MissingRequiredExport { .. } => {
            TassadarModuleCompatibilityRefusalReason::MissingRequiredExport
        }
        TassadarModuleCompatibilityError::MissingBenchmarkEvidence { .. } => {
            TassadarModuleCompatibilityRefusalReason::MissingBenchmarkEvidence
        }
    };
    TassadarModuleCompatibilityReceipt::new(
        request.consumer_family.clone(),
        manifest.module_ref.clone(),
        manifest.compatibility_digest.clone(),
        TassadarModuleCompatibilityStatus::Refused,
        vec![],
        vec![],
        Some(refusal_reason),
        error.to_string(),
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarModuleCompatibilityError, TassadarModuleCompatibilityRefusalReason,
        TassadarModuleCompatibilityRequest, TassadarModuleCompatibilityStatus,
        check_tassadar_module_manifest_compatibility,
        tassadar_module_compatibility_refusal_receipt,
        validate_tassadar_computational_module_manifest,
    };
    use psionic_ir::{
        TassadarComputationalModuleManifest, TassadarComputationalModuleManifestError,
        TassadarModuleTrustPosture, seeded_tassadar_computational_module_manifests,
    };

    #[test]
    fn module_manifest_validation_reuses_ir_contract() {
        let manifest = seeded_tassadar_computational_module_manifests()
            .into_iter()
            .next()
            .expect("seeded manifest");

        validate_tassadar_computational_module_manifest(&manifest).expect("valid manifest");
    }

    #[test]
    fn module_manifest_compatibility_accepts_matching_consumer_constraints() {
        let manifest = seeded_tassadar_computational_module_manifests()
            .into_iter()
            .find(|manifest| manifest.module_ref == "frontier_relax_core@1.0.0")
            .expect("frontier manifest");
        let request = TassadarModuleCompatibilityRequest {
            consumer_family: String::from("clrs_shortest_path"),
            required_exports: vec![String::from("frontier_relax_step")],
            required_benchmark_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            )],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let receipt =
            check_tassadar_module_manifest_compatibility(&manifest, &request).expect("receipt");

        assert_eq!(
            receipt.status,
            TassadarModuleCompatibilityStatus::Compatible
        );
        assert_eq!(
            receipt.satisfied_exports,
            vec![String::from("frontier_relax_step")]
        );
        assert_eq!(receipt.consumer_family, "clrs_shortest_path");
        assert!(receipt.refusal_reason.is_none());
    }

    #[test]
    fn module_manifest_compatibility_refuses_trust_downgrade_and_missing_exports() {
        let manifest = seeded_tassadar_computational_module_manifests()
            .into_iter()
            .find(|manifest| manifest.module_ref == "frontier_relax_core@1.0.0")
            .expect("frontier manifest");
        let request = TassadarModuleCompatibilityRequest {
            consumer_family: String::from("worker_mount_install"),
            required_exports: vec![String::from("missing_export")],
            required_benchmark_refs: vec![],
            minimum_trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let error =
            check_tassadar_module_manifest_compatibility(&manifest, &request).expect_err("error");
        assert_eq!(
            error,
            TassadarModuleCompatibilityError::TrustPostureTooWeak {
                required: TassadarModuleTrustPosture::ChallengeGatedInstall,
                actual: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            }
        );
        let receipt = tassadar_module_compatibility_refusal_receipt(&manifest, &request, &error);
        assert_eq!(receipt.status, TassadarModuleCompatibilityStatus::Refused);
        assert_eq!(
            receipt.refusal_reason,
            Some(TassadarModuleCompatibilityRefusalReason::TrustPostureTooWeak)
        );
    }

    #[test]
    fn module_manifest_compatibility_refuses_invalid_manifest() {
        let manifest = TassadarComputationalModuleManifest {
            compatibility_digest: String::from("drift"),
            ..seeded_tassadar_computational_module_manifests()
                .into_iter()
                .next()
                .expect("seeded manifest")
        };
        let request = TassadarModuleCompatibilityRequest {
            consumer_family: String::from("clrs_shortest_path"),
            required_exports: vec![String::from("frontier_relax_step")],
            required_benchmark_refs: vec![],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from(
                "compiled bounded exactness / promotion discipline",
            )],
        };

        let error =
            check_tassadar_module_manifest_compatibility(&manifest, &request).expect_err("error");
        assert_eq!(
            error,
            TassadarModuleCompatibilityError::InvalidManifest(
                TassadarComputationalModuleManifestError::CompatibilityDigestDrift
            )
        );
    }
}
