use std::{fs, path::Path};

use psionic_models::{
    PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILENAME, ParameterGolfPromotedBundleArtifactRef,
    ParameterGolfPromotedBundleManifest, ParameterGolfPromotedGenerationOptions,
    ParameterGolfPromotedProfileKind, ParameterGolfPromotedRuntimeBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ParameterGolfCheckpointManifest, ParameterGolfPromotedCheckpointSurfaceReport,
    ParameterGolfPromotedDatasetIdentity, ParameterGolfPromotedEvaluationIdentity,
    ParameterGolfPromotedReferenceRunSummary, ParameterGolfPromotedResumeProof,
    ParameterGolfPromotedTokenizerIdentity, ParameterGolfPromotedTrainingProfile,
    ParameterGolfReferenceTrainingConfig, check_parameter_golf_promoted_bundle,
};

pub const PARAMETER_GOLF_PROMOTED_BUNDLE_INSPECTION_SCHEMA_VERSION: &str =
    "psionic.parameter_golf_promoted_bundle_inspection.v1";
pub const PARAMETER_GOLF_PROMOTED_INFERENCE_PROMOTION_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.parameter_golf_promoted_inference_promotion_receipt.v1";

#[derive(Debug, Error)]
pub enum ParameterGolfPromotedBundleOperatorError {
    #[error("failed to read promoted PGOLF bundle artifact `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to parse promoted PGOLF bundle artifact `{path}` as {context}: {error}")]
    Parse {
        path: String,
        context: &'static str,
        error: serde_json::Error,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedProfileAssumption {
    BundleDeclaredProfile,
    GeneralPsionSmallDecoder,
    StrictPgolfChallenge,
}

impl ParameterGolfPromotedProfileAssumption {
    #[must_use]
    pub const fn expected_kind(self) -> Option<ParameterGolfPromotedProfileKind> {
        match self {
            Self::BundleDeclaredProfile => None,
            Self::GeneralPsionSmallDecoder => {
                Some(ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder)
            }
            Self::StrictPgolfChallenge => {
                Some(ParameterGolfPromotedProfileKind::StrictPgolfChallenge)
            }
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BundleDeclaredProfile => "bundle_declared_profile",
            Self::GeneralPsionSmallDecoder => "general_psion_small_decoder",
            Self::StrictPgolfChallenge => "strict_pgolf_challenge",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedInferencePromotionDisposition {
    Promoted,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedInferencePromotionGateKind {
    BundleIntegrity,
    RuntimeLoadability,
    ProfileAssumption,
    TokenizerInferenceReadiness,
    MetadataClosure,
    ProfileSpecificRules,
    LocalInferenceSmoke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedInferencePromotionGateDisposition {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfPromotedInferenceRefusalKind {
    BundleIntegrity,
    RuntimeLoadability,
    AssumptionMismatch,
    TokenizerIncomplete,
    MetadataMismatch,
    ProfilePolicyMismatch,
    InferenceSmokeFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedBundleArtifactInspectionRow {
    pub artifact_kind: String,
    pub relative_path: String,
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    pub expected_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_sha256: Option<String>,
    pub sha256_matches_expected: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedBundleInspection {
    pub schema_version: String,
    pub bundle_root: String,
    pub bundle_id: String,
    pub family_id: String,
    pub model_id: String,
    pub model_revision: String,
    pub profile_id: String,
    pub profile_kind: String,
    pub manifest_valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_validation_detail: Option<String>,
    pub artifact_rows: Vec<ParameterGolfPromotedBundleArtifactInspectionRow>,
    pub detail: String,
    pub inspection_digest: String,
}

impl ParameterGolfPromotedBundleInspection {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.inspection_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_promoted_bundle_inspection|",
            &canonical,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedInferencePromotionGate {
    pub gate_kind: ParameterGolfPromotedInferencePromotionGateKind,
    pub disposition: ParameterGolfPromotedInferencePromotionGateDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<ParameterGolfPromotedInferenceRefusalKind>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfPromotedInferencePromotionReceipt {
    pub schema_version: String,
    pub bundle_root: String,
    pub bundle_id: String,
    pub bundle_digest: String,
    pub model_id: String,
    pub model_revision: String,
    pub profile_id: String,
    pub profile_kind: String,
    pub assumption: ParameterGolfPromotedProfileAssumption,
    pub disposition: ParameterGolfPromotedInferencePromotionDisposition,
    pub failed_gate_kinds: Vec<ParameterGolfPromotedInferencePromotionGateKind>,
    pub gates: Vec<ParameterGolfPromotedInferencePromotionGate>,
    pub detail: String,
    pub receipt_digest: String,
}

impl ParameterGolfPromotedInferencePromotionReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_promoted_inference_promotion_receipt|",
            &canonical,
        )
    }
}

pub fn inspect_parameter_golf_promoted_bundle(
    bundle_root: &Path,
) -> Result<ParameterGolfPromotedBundleInspection, ParameterGolfPromotedBundleOperatorError> {
    let manifest = read_promoted_manifest(bundle_root)?;
    let manifest_validation_detail = manifest.validate().err().map(|error| error.to_string());
    let mut inspection = ParameterGolfPromotedBundleInspection {
        schema_version: String::from(PARAMETER_GOLF_PROMOTED_BUNDLE_INSPECTION_SCHEMA_VERSION),
        bundle_root: bundle_root.display().to_string(),
        bundle_id: manifest.bundle_id.clone(),
        family_id: manifest.family_id.clone(),
        model_id: manifest.model_id.clone(),
        model_revision: manifest.model_revision.clone(),
        profile_id: manifest.profile_id.clone(),
        profile_kind: manifest.profile_kind.clone(),
        manifest_valid: manifest_validation_detail.is_none(),
        manifest_validation_detail,
        artifact_rows: artifact_rows(bundle_root, &manifest),
        detail: format!(
            "Promoted PGOLF bundle inspection freezes the declared model `{}` revision `{}` under profile `{}` and reports whether the current file inventory still matches the manifest.",
            manifest.model_id, manifest.model_revision, manifest.profile_kind
        ),
        inspection_digest: String::new(),
    };
    inspection.inspection_digest = inspection.stable_digest();
    Ok(inspection)
}

pub fn build_parameter_golf_promoted_inference_promotion_receipt(
    bundle_root: &Path,
    assumption: ParameterGolfPromotedProfileAssumption,
) -> Result<ParameterGolfPromotedInferencePromotionReceipt, ParameterGolfPromotedBundleOperatorError>
{
    let manifest = read_promoted_manifest(bundle_root)?;
    let mut gates = Vec::new();

    let bundle_integrity_passed = match check_parameter_golf_promoted_bundle(bundle_root) {
        Ok(_) => {
            gates.push(passed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::BundleIntegrity,
                format!(
                    "Bundle manifest `{}` and all declared artifacts still pass hash, shape, lineage, and checkpoint-surface verification.",
                    PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILENAME
                ),
            ));
            true
        }
        Err(error) => {
            gates.push(failed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::BundleIntegrity,
                ParameterGolfPromotedInferenceRefusalKind::BundleIntegrity,
                error.to_string(),
            ));
            false
        }
    };

    let runtime_bundle = if bundle_integrity_passed {
        match ParameterGolfPromotedRuntimeBundle::load_dir(bundle_root) {
            Ok(bundle) => {
                gates.push(passed_gate(
                    ParameterGolfPromotedInferencePromotionGateKind::RuntimeLoadability,
                    String::from(
                        "Public runtime bundle loader restored the promoted tokenizer, descriptor, generation defaults, and model weights without drift.",
                    ),
                ));
                Some(bundle)
            }
            Err(error) => {
                gates.push(failed_gate(
                    ParameterGolfPromotedInferencePromotionGateKind::RuntimeLoadability,
                    ParameterGolfPromotedInferenceRefusalKind::RuntimeLoadability,
                    error.to_string(),
                ));
                None
            }
        }
    } else {
        gates.push(skipped_gate(
            ParameterGolfPromotedInferencePromotionGateKind::RuntimeLoadability,
            "Skipped because bundle-integrity validation did not pass.",
        ));
        None
    };

    let promotion_artifacts = if bundle_integrity_passed {
        match load_promotion_artifacts(bundle_root, &manifest) {
            Ok(artifacts) => Some(artifacts),
            Err(error) => {
                gates.push(failed_gate(
                    ParameterGolfPromotedInferencePromotionGateKind::MetadataClosure,
                    ParameterGolfPromotedInferenceRefusalKind::MetadataMismatch,
                    error.to_string(),
                ));
                None
            }
        }
    } else {
        None
    };

    let actual_profile_kind = promotion_artifacts
        .as_ref()
        .map(|artifacts| artifacts.training_config.promoted_profile.kind)
        .or_else(|| profile_kind_from_manifest(manifest.profile_kind.as_str()))
        .unwrap_or(ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder);

    match assumption.expected_kind() {
        Some(expected_kind) if expected_kind != actual_profile_kind => gates.push(failed_gate(
            ParameterGolfPromotedInferencePromotionGateKind::ProfileAssumption,
            ParameterGolfPromotedInferenceRefusalKind::AssumptionMismatch,
            format!(
                "requested assumption `{}` mismatched bundle-declared profile `{}`",
                assumption.label(),
                profile_kind_label(actual_profile_kind)
            ),
        )),
        _ => gates.push(passed_gate(
            ParameterGolfPromotedInferencePromotionGateKind::ProfileAssumption,
            format!(
                "Operator assumption `{}` is compatible with the bundle-declared profile `{}`.",
                assumption.label(),
                profile_kind_label(actual_profile_kind)
            ),
        )),
    }

    if let Some(bundle) = runtime_bundle.as_ref() {
        let tokenizer_asset = bundle.tokenizer().asset();
        let mut failures = Vec::new();
        if tokenizer_asset.vocab_size as usize != bundle.descriptor().config.vocab_size {
            failures.push(format!(
                "tokenizer vocab size {} drifted from descriptor vocab size {}",
                tokenizer_asset.vocab_size,
                bundle.descriptor().config.vocab_size
            ));
        }
        if tokenizer_asset.add_bos && tokenizer_asset.bos_token_id.is_none() {
            failures.push(String::from(
                "tokenizer requests default BOS insertion but did not declare a bos_token_id",
            ));
        }
        if bundle.generation_config().stop_on_eos && tokenizer_asset.eos_token_ids.is_empty() {
            failures.push(String::from(
                "generation defaults require EOS stopping but the tokenizer declared no eos_token_ids",
            ));
        }
        if tokenizer_asset.unknown_token_id.is_none() {
            failures.push(String::from(
                "tokenizer did not declare an explicit unknown_token_id for honest operator-facing inference",
            ));
        }
        if failures.is_empty() {
            gates.push(passed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::TokenizerInferenceReadiness,
                format!(
                    "Tokenizer asset exposes vocab_size={}, explicit unknown handling, and BOS/EOS defaults that match the runtime generation contract.",
                    tokenizer_asset.vocab_size
                ),
            ));
        } else {
            gates.push(failed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::TokenizerInferenceReadiness,
                ParameterGolfPromotedInferenceRefusalKind::TokenizerIncomplete,
                failures.join("; "),
            ));
        }
    } else {
        gates.push(skipped_gate(
            ParameterGolfPromotedInferencePromotionGateKind::TokenizerInferenceReadiness,
            "Skipped because the runtime bundle did not load.",
        ));
    }

    if let Some(artifacts) = promotion_artifacts.as_ref() {
        let metadata_failures = metadata_mismatches(&manifest, artifacts);
        if metadata_failures.is_empty() {
            gates.push(passed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::MetadataClosure,
                String::from(
                    "Training config, summary, checkpoint manifest, checkpoint-surface report, and resume proof remain internally aligned with the emitted promoted bundle.",
                ),
            ));
        } else if !gates.iter().any(|gate| {
            gate.gate_kind == ParameterGolfPromotedInferencePromotionGateKind::MetadataClosure
                && gate.disposition
                    == ParameterGolfPromotedInferencePromotionGateDisposition::Failed
        }) {
            gates.push(failed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::MetadataClosure,
                ParameterGolfPromotedInferenceRefusalKind::MetadataMismatch,
                metadata_failures.join("; "),
            ));
        }

        let profile_failures =
            profile_policy_mismatches(bundle_root, &artifacts.training_config.promoted_profile);
        if profile_failures.is_empty() {
            gates.push(passed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::ProfileSpecificRules,
                format!(
                    "Bundle obeys the declared `{}` profile policy without silently widening into the other overlay.",
                    profile_kind_label(artifacts.training_config.promoted_profile.kind)
                ),
            ));
        } else {
            gates.push(failed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::ProfileSpecificRules,
                ParameterGolfPromotedInferenceRefusalKind::ProfilePolicyMismatch,
                profile_failures.join("; "),
            ));
        }
    } else {
        if !gates.iter().any(|gate| {
            gate.gate_kind == ParameterGolfPromotedInferencePromotionGateKind::MetadataClosure
        }) {
            gates.push(skipped_gate(
                ParameterGolfPromotedInferencePromotionGateKind::MetadataClosure,
                "Skipped because bundle-integrity validation did not yield trusted metadata artifacts.",
            ));
        }
        gates.push(skipped_gate(
            ParameterGolfPromotedInferencePromotionGateKind::ProfileSpecificRules,
            "Skipped because trusted training-profile metadata was unavailable.",
        ));
    }

    if let Some(bundle) = runtime_bundle.as_ref() {
        let mut options = ParameterGolfPromotedGenerationOptions::greedy(1);
        options.stop_on_eos = bundle.generation_config().stop_on_eos;
        match bundle.generate_text("abcd", &options) {
            Ok(output) if output.generated_tokens.len() == 1 => gates.push(passed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::LocalInferenceSmoke,
                format!(
                    "Local inference smoke generated one token with termination `{:?}` and text `{}`.",
                    output.termination, output.text
                ),
            )),
            Ok(output) => gates.push(failed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::LocalInferenceSmoke,
                ParameterGolfPromotedInferenceRefusalKind::InferenceSmokeFailed,
                format!(
                    "local inference smoke emitted {} tokens instead of exactly one",
                    output.generated_tokens.len()
                ),
            )),
            Err(error) => gates.push(failed_gate(
                ParameterGolfPromotedInferencePromotionGateKind::LocalInferenceSmoke,
                ParameterGolfPromotedInferenceRefusalKind::InferenceSmokeFailed,
                error.to_string(),
            )),
        }
    } else {
        gates.push(skipped_gate(
            ParameterGolfPromotedInferencePromotionGateKind::LocalInferenceSmoke,
            "Skipped because the runtime bundle did not load.",
        ));
    }

    let failed_gate_kinds = gates
        .iter()
        .filter(|gate| {
            gate.disposition == ParameterGolfPromotedInferencePromotionGateDisposition::Failed
        })
        .map(|gate| gate.gate_kind)
        .collect::<Vec<_>>();
    let disposition = if failed_gate_kinds.is_empty() {
        ParameterGolfPromotedInferencePromotionDisposition::Promoted
    } else {
        ParameterGolfPromotedInferencePromotionDisposition::Refused
    };
    let mut receipt = ParameterGolfPromotedInferencePromotionReceipt {
        schema_version: String::from(
            PARAMETER_GOLF_PROMOTED_INFERENCE_PROMOTION_RECEIPT_SCHEMA_VERSION,
        ),
        bundle_root: bundle_root.display().to_string(),
        bundle_id: manifest.bundle_id.clone(),
        bundle_digest: manifest.bundle_digest.clone(),
        model_id: manifest.model_id.clone(),
        model_revision: manifest.model_revision.clone(),
        profile_id: manifest.profile_id.clone(),
        profile_kind: manifest.profile_kind.clone(),
        assumption,
        disposition,
        failed_gate_kinds: failed_gate_kinds.clone(),
        gates,
        detail: if failed_gate_kinds.is_empty() {
            format!(
                "Bundle `{}` is promoted for inference under assumption `{}` because it passed bundle integrity, runtime load, metadata, profile, and local inference smoke checks.",
                manifest.bundle_id,
                assumption.label()
            )
        } else {
            format!(
                "Bundle `{}` is refused for inference under assumption `{}` because gate failures remained in {:?}.",
                manifest.bundle_id,
                assumption.label(),
                failed_gate_kinds
            )
        },
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    Ok(receipt)
}

#[derive(Clone, Debug)]
struct PromotionArtifacts {
    training_config: ParameterGolfReferenceTrainingConfig,
    summary: ParameterGolfPromotedReferenceRunSummary,
    checkpoint_manifest: ParameterGolfCheckpointManifest,
    checkpoint_surface_report: ParameterGolfPromotedCheckpointSurfaceReport,
    resume_proof: ParameterGolfPromotedResumeProof,
}

fn read_promoted_manifest(
    bundle_root: &Path,
) -> Result<ParameterGolfPromotedBundleManifest, ParameterGolfPromotedBundleOperatorError> {
    let manifest_path = bundle_root.join(PARAMETER_GOLF_PROMOTED_BUNDLE_MANIFEST_FILENAME);
    let bytes = fs::read(manifest_path.as_path()).map_err(|error| {
        ParameterGolfPromotedBundleOperatorError::Read {
            path: manifest_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfPromotedBundleOperatorError::Parse {
            path: manifest_path.display().to_string(),
            context: "promoted bundle manifest",
            error,
        }
    })
}

fn artifact_rows(
    bundle_root: &Path,
    manifest: &ParameterGolfPromotedBundleManifest,
) -> Vec<ParameterGolfPromotedBundleArtifactInspectionRow> {
    [
        ("descriptor", &manifest.artifacts.descriptor),
        ("model", &manifest.artifacts.model),
        ("tokenizer_asset", &manifest.artifacts.tokenizer_asset),
        ("generation_config", &manifest.artifacts.generation_config),
        ("profile_contract", &manifest.artifacts.profile_contract),
        ("training_config", &manifest.artifacts.training_config),
        ("summary", &manifest.artifacts.summary),
        (
            "checkpoint_manifest",
            &manifest.artifacts.checkpoint_manifest,
        ),
        (
            "checkpoint_surface_report",
            &manifest.artifacts.checkpoint_surface_report,
        ),
        ("resume_proof", &manifest.artifacts.resume_proof),
    ]
    .into_iter()
    .map(|(artifact_kind, artifact)| inspect_artifact(bundle_root, artifact_kind, artifact))
    .collect()
}

fn inspect_artifact(
    bundle_root: &Path,
    artifact_kind: &str,
    artifact: &ParameterGolfPromotedBundleArtifactRef,
) -> ParameterGolfPromotedBundleArtifactInspectionRow {
    let path = bundle_root.join(artifact.relative_path.as_str());
    match fs::read(path.as_path()) {
        Ok(bytes) => {
            let observed_sha256 = sha256_hex(bytes.as_slice());
            let size_bytes = bytes.len().try_into().unwrap_or(u64::MAX);
            let sha256_matches_expected = observed_sha256 == artifact.sha256;
            ParameterGolfPromotedBundleArtifactInspectionRow {
                artifact_kind: String::from(artifact_kind),
                relative_path: artifact.relative_path.clone(),
                exists: true,
                size_bytes: Some(size_bytes),
                expected_sha256: artifact.sha256.clone(),
                observed_sha256: Some(observed_sha256),
                sha256_matches_expected,
                detail: if sha256_matches_expected {
                    String::from("artifact exists and its raw SHA-256 matches the manifest")
                } else {
                    String::from("artifact exists but its raw SHA-256 drifted from the manifest")
                },
            }
        }
        Err(error) => ParameterGolfPromotedBundleArtifactInspectionRow {
            artifact_kind: String::from(artifact_kind),
            relative_path: artifact.relative_path.clone(),
            exists: false,
            size_bytes: None,
            expected_sha256: artifact.sha256.clone(),
            observed_sha256: None,
            sha256_matches_expected: false,
            detail: format!("artifact is missing or unreadable: {error}"),
        },
    }
}

fn load_promotion_artifacts(
    bundle_root: &Path,
    manifest: &ParameterGolfPromotedBundleManifest,
) -> Result<PromotionArtifacts, ParameterGolfPromotedBundleOperatorError> {
    Ok(PromotionArtifacts {
        training_config: read_json_artifact(
            bundle_root,
            &manifest.artifacts.training_config,
            "promoted training config",
        )?,
        summary: read_json_artifact(bundle_root, &manifest.artifacts.summary, "promoted summary")?,
        checkpoint_manifest: read_json_artifact(
            bundle_root,
            &manifest.artifacts.checkpoint_manifest,
            "promoted checkpoint manifest",
        )?,
        checkpoint_surface_report: read_json_artifact(
            bundle_root,
            &manifest.artifacts.checkpoint_surface_report,
            "promoted checkpoint surface report",
        )?,
        resume_proof: read_json_artifact(
            bundle_root,
            &manifest.artifacts.resume_proof,
            "promoted resume proof",
        )?,
    })
}

fn read_json_artifact<T: for<'de> Deserialize<'de>>(
    bundle_root: &Path,
    artifact: &ParameterGolfPromotedBundleArtifactRef,
    context: &'static str,
) -> Result<T, ParameterGolfPromotedBundleOperatorError> {
    let path = bundle_root.join(artifact.relative_path.as_str());
    let bytes = fs::read(path.as_path()).map_err(|error| {
        ParameterGolfPromotedBundleOperatorError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfPromotedBundleOperatorError::Parse {
            path: path.display().to_string(),
            context,
            error,
        }
    })
}

fn metadata_mismatches(
    manifest: &ParameterGolfPromotedBundleManifest,
    artifacts: &PromotionArtifacts,
) -> Vec<String> {
    let mut failures = Vec::new();
    if artifacts.training_config.promoted_profile.profile_id != manifest.profile_id {
        failures.push(String::from(
            "training_config.promoted_profile.profile_id drifted from the bundle profile id",
        ));
    }
    if artifacts.summary.profile_id != manifest.profile_id {
        failures.push(String::from(
            "summary.profile_id drifted from the bundle profile id",
        ));
    }
    if artifacts.summary.profile_kind != manifest.profile_kind {
        failures.push(String::from(
            "summary.profile_kind drifted from the bundle profile kind",
        ));
    }
    if artifacts.summary.descriptor_digest != manifest.lineage.descriptor_digest {
        failures.push(String::from(
            "summary.descriptor_digest drifted from the bundle lineage descriptor digest",
        ));
    }
    if artifacts.summary.final_checkpoint_ref != manifest.lineage.final_checkpoint_ref {
        failures.push(String::from(
            "summary.final_checkpoint_ref drifted from the bundle lineage final checkpoint ref",
        ));
    }
    if artifacts.summary.final_checkpoint_manifest_digest
        != manifest.lineage.final_checkpoint_manifest_digest
    {
        failures.push(String::from(
            "summary.final_checkpoint_manifest_digest drifted from the bundle lineage final checkpoint manifest digest",
        ));
    }
    if artifacts.summary.training_dataset_digest != manifest.lineage.training_dataset_digest {
        failures.push(String::from(
            "summary.training_dataset_digest drifted from the bundle lineage training dataset digest",
        ));
    }
    if artifacts.summary.validation_dataset_digest != manifest.lineage.validation_dataset_digest {
        failures.push(String::from(
            "summary.validation_dataset_digest drifted from the bundle lineage validation dataset digest",
        ));
    }
    if artifacts.checkpoint_manifest.stable_digest()
        != manifest.lineage.final_checkpoint_manifest_digest
    {
        failures.push(String::from(
            "checkpoint_manifest digest drifted from the bundle lineage final checkpoint manifest digest",
        ));
    }
    if artifacts.checkpoint_surface_report.profile_id != manifest.profile_id {
        failures.push(String::from(
            "checkpoint_surface_report.profile_id drifted from the bundle profile id",
        ));
    }
    if artifacts.checkpoint_surface_report.report_digest
        != artifacts.summary.checkpoint_surface_report_digest
    {
        failures.push(String::from(
            "summary.checkpoint_surface_report_digest drifted from checkpoint_surface_report.report_digest",
        ));
    }
    if !artifacts.checkpoint_surface_report.exact_match {
        failures.push(String::from(
            "checkpoint_surface_report did not declare exact tensor-surface parity",
        ));
    }
    if artifacts.resume_proof.profile_id != manifest.profile_id {
        failures.push(String::from(
            "resume_proof.profile_id drifted from the bundle profile id",
        ));
    }
    if artifacts.resume_proof.proof_digest != artifacts.summary.resume_proof_digest {
        failures.push(String::from(
            "summary.resume_proof_digest drifted from resume_proof.proof_digest",
        ));
    }
    if !artifacts.resume_proof.exact_final_parity {
        failures.push(String::from(
            "resume_proof did not keep exact_final_parity=true",
        ));
    }
    failures
}

fn profile_policy_mismatches(
    bundle_root: &Path,
    profile: &ParameterGolfPromotedTrainingProfile,
) -> Vec<String> {
    let mut failures = Vec::new();
    if profile.profile_id != profile.kind.profile_id() {
        failures.push(format!(
            "training profile id `{}` drifted from frozen kind `{}`",
            profile.profile_id,
            profile.kind.profile_id()
        ));
    }
    match profile.kind {
        ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder => {
            if profile.tokenizer_identity
                != ParameterGolfPromotedTokenizerIdentity::RepoLocalReferenceSentencePiece
            {
                failures.push(String::from(
                    "general small-decoder profile must use the repo-local reference SentencePiece tokenizer identity",
                ));
            }
            if profile.dataset_identity
                != ParameterGolfPromotedDatasetIdentity::RepoLocalReferenceFixture
            {
                failures.push(String::from(
                    "general small-decoder profile must use the repo-local reference dataset identity",
                ));
            }
            if profile.evaluation_policy.evaluation_identity
                != ParameterGolfPromotedEvaluationIdentity::LocalReferenceValidation
            {
                failures.push(String::from(
                    "general small-decoder profile must use local-reference validation rather than challenge BPB evaluation",
                ));
            }
            if profile.evaluation_policy.legal_score_first_ttt_required
                || profile
                    .evaluation_policy
                    .contest_bits_per_byte_accounting_required
                || profile
                    .artifact_policy
                    .exact_compressed_artifact_cap_required
                || profile
                    .artifact_policy
                    .compressed_artifact_cap_bytes
                    .is_some()
            {
                failures.push(String::from(
                    "general small-decoder profile must keep challenge-only score, accounting, and artifact-cap rules disabled",
                ));
            }
        }
        ParameterGolfPromotedProfileKind::StrictPgolfChallenge => {
            if profile.tokenizer_identity
                != ParameterGolfPromotedTokenizerIdentity::ChallengeSp1024SentencePiece
            {
                failures.push(String::from(
                    "strict PGOLF challenge profile must keep the exact public SP1024 tokenizer identity",
                ));
            }
            if profile.dataset_identity
                != ParameterGolfPromotedDatasetIdentity::ChallengeFinewebSp1024
            {
                failures.push(String::from(
                    "strict PGOLF challenge profile must keep the FineWeb SP1024 dataset identity",
                ));
            }
            if profile.evaluation_policy.evaluation_identity
                != ParameterGolfPromotedEvaluationIdentity::ChallengeBitsPerByte
                || !profile.evaluation_policy.legal_score_first_ttt_required
                || !profile
                    .evaluation_policy
                    .contest_bits_per_byte_accounting_required
            {
                failures.push(String::from(
                    "strict PGOLF challenge profile must keep challenge bits-per-byte evaluation plus score-first TTT/accounting rules enabled",
                ));
            }
            if !profile
                .artifact_policy
                .exact_compressed_artifact_cap_required
                || profile.artifact_policy.compressed_artifact_cap_bytes != Some(16_000_000)
            {
                failures.push(String::from(
                    "strict PGOLF challenge profile must keep the exact 16,000,000-byte compressed artifact cap",
                ));
            }
            let final_artifact_path = bundle_root.join("parameter_golf_final_model_int8_zlib.st");
            match fs::metadata(final_artifact_path.as_path()) {
                Ok(metadata) if metadata.len() <= 16_000_000 => {}
                Ok(metadata) => failures.push(format!(
                    "strict PGOLF challenge artifact `{}` was {} bytes, above the 16,000,000-byte cap",
                    final_artifact_path.display(),
                    metadata.len()
                )),
                Err(error) => failures.push(format!(
                    "strict PGOLF challenge artifact `{}` is missing or unreadable: {error}",
                    final_artifact_path.display()
                )),
            }
        }
    }
    failures
}

fn profile_kind_from_manifest(profile_kind: &str) -> Option<ParameterGolfPromotedProfileKind> {
    match profile_kind {
        "general_psion_small_decoder" => {
            Some(ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder)
        }
        "strict_pgolf_challenge" => Some(ParameterGolfPromotedProfileKind::StrictPgolfChallenge),
        _ => None,
    }
}

fn profile_kind_label(kind: ParameterGolfPromotedProfileKind) -> &'static str {
    match kind {
        ParameterGolfPromotedProfileKind::GeneralPsionSmallDecoder => "general_psion_small_decoder",
        ParameterGolfPromotedProfileKind::StrictPgolfChallenge => "strict_pgolf_challenge",
    }
}

fn passed_gate(
    gate_kind: ParameterGolfPromotedInferencePromotionGateKind,
    detail: String,
) -> ParameterGolfPromotedInferencePromotionGate {
    ParameterGolfPromotedInferencePromotionGate {
        gate_kind,
        disposition: ParameterGolfPromotedInferencePromotionGateDisposition::Passed,
        refusal_kind: None,
        detail,
    }
}

fn failed_gate(
    gate_kind: ParameterGolfPromotedInferencePromotionGateKind,
    refusal_kind: ParameterGolfPromotedInferenceRefusalKind,
    detail: String,
) -> ParameterGolfPromotedInferencePromotionGate {
    ParameterGolfPromotedInferencePromotionGate {
        gate_kind,
        disposition: ParameterGolfPromotedInferencePromotionGateDisposition::Failed,
        refusal_kind: Some(refusal_kind),
        detail,
    }
}

fn skipped_gate(
    gate_kind: ParameterGolfPromotedInferencePromotionGateKind,
    detail: &str,
) -> ParameterGolfPromotedInferencePromotionGate {
    ParameterGolfPromotedInferencePromotionGate {
        gate_kind,
        disposition: ParameterGolfPromotedInferencePromotionGateDisposition::Skipped,
        refusal_kind: None,
        detail: String::from(detail),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher
        .update(serde_json::to_vec(value).expect("promoted PGOLF operator value should serialize"));
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        ParameterGolfPromotedInferencePromotionDisposition,
        ParameterGolfPromotedInferencePromotionGateDisposition,
        ParameterGolfPromotedInferencePromotionGateKind, ParameterGolfPromotedProfileAssumption,
        build_parameter_golf_promoted_inference_promotion_receipt,
        inspect_parameter_golf_promoted_bundle,
    };
    use crate::{
        ParameterGolfLocalReferenceFixture, ParameterGolfReferenceTrainingConfig,
        run_parameter_golf_promoted_reference_run, write_parameter_golf_promoted_reference_run,
    };
    use std::error::Error;
    use tempfile::tempdir;

    #[test]
    fn inspection_reports_manifest_and_artifact_inventory() -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;

        let inspection = inspect_parameter_golf_promoted_bundle(output_dir.path())?;
        assert!(inspection.manifest_valid);
        assert_eq!(inspection.profile_id, config.promoted_profile.profile_id);
        assert_eq!(inspection.artifact_rows.len(), 10);
        assert!(
            inspection
                .artifact_rows
                .iter()
                .all(|row| row.exists && row.sha256_matches_expected)
        );
        Ok(())
    }

    #[test]
    fn general_bundle_promotes_for_inference_under_matching_assumption()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;

        let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
            output_dir.path(),
            ParameterGolfPromotedProfileAssumption::GeneralPsionSmallDecoder,
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfPromotedInferencePromotionDisposition::Promoted
        );
        assert!(receipt.failed_gate_kinds.is_empty());
        assert!(receipt.gates.iter().any(|gate| {
            gate.gate_kind == ParameterGolfPromotedInferencePromotionGateKind::LocalInferenceSmoke
                && gate.disposition
                    == ParameterGolfPromotedInferencePromotionGateDisposition::Passed
        }));
        Ok(())
    }

    #[test]
    fn assumption_mismatch_refuses_even_when_bundle_is_otherwise_green()
    -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;

        let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
            output_dir.path(),
            ParameterGolfPromotedProfileAssumption::StrictPgolfChallenge,
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfPromotedInferencePromotionDisposition::Refused
        );
        assert!(
            receipt
                .failed_gate_kinds
                .contains(&ParameterGolfPromotedInferencePromotionGateKind::ProfileAssumption)
        );
        Ok(())
    }

    #[test]
    fn missing_artifact_refuses_with_typed_bundle_integrity_failure() -> Result<(), Box<dyn Error>>
    {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
        let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
        let output_dir = tempdir()?;
        write_parameter_golf_promoted_reference_run(&run, output_dir.path())?;
        std::fs::remove_file(output_dir.path().join("tokenizer.json"))?;

        let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
            output_dir.path(),
            ParameterGolfPromotedProfileAssumption::BundleDeclaredProfile,
        )?;
        assert_eq!(
            receipt.disposition,
            ParameterGolfPromotedInferencePromotionDisposition::Refused
        );
        assert!(
            receipt
                .failed_gate_kinds
                .contains(&ParameterGolfPromotedInferencePromotionGateKind::BundleIntegrity)
        );
        Ok(())
    }
}
