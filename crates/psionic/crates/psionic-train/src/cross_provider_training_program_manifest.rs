use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use psionic_environments::EnvironmentPackageKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{TrainingRunState, TrainingStageKind};

/// Stable schema version for the first cross-provider training-program manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.cross_provider_training_program_manifest.v1";
/// Stable fixture path for the canonical manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/training/cross_provider_training_program_manifest_v1.json";
/// Stable checker path for the canonical manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-provider-training-program-manifest.sh";
/// Stable reference doc path for the canonical manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_DOC_PATH: &str =
    "docs/TRAIN_PROGRAM_MANIFEST_REFERENCE.md";
/// Stable train-system doc path.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_TRAIN_SYSTEM_DOC_PATH: &str =
    "docs/TRAIN_SYSTEM.md";
/// Stable audit path that motivated the manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_AUDIT_PATH: &str =
    "docs/audits/2026-03-25-cross-provider-pretraining-system-readiness-audit.md";
/// Stable manifest id for the first cross-provider pretraining program.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_ID: &str =
    "psionic-cross-provider-pretraining-program-v1";
/// Stable program family id.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_FAMILY_ID: &str =
    "psionic.cross_provider_pretraining_program.v1";
/// Stable run-id template prefix.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_RUN_ID_TEMPLATE: &str =
    "psion-xprovider-pretrain-${RUN_ID}";
/// Stable stage id frozen by the first manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID: &str = "psion_pretrain";
/// Stable checkpoint family frozen by the first manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY: &str =
    "psion.cross_provider.pretrain.v1";
/// Stable dataset family id frozen by the first manifest.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID: &str =
    "psion.curated_pretrain.dataset_family.v1";
/// Stable environment ref for the cross-provider pretraining program.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_REF: &str =
    "psion.pretrain.reference_runtime";
/// Stable environment version for the cross-provider pretraining program.
pub const CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_VERSION: &str = "v1";

const PSION_PRETRAIN_STAGE_CONFIG_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_pretrain_stage_config_v1.json";
const PSION_PRETRAIN_STAGE_DOC_PATH: &str = "docs/PSION_PRETRAIN_STAGE.md";

/// Errors surfaced while building, validating, binding, or writing the manifest.
#[derive(Debug, Error)]
pub enum CrossProviderTrainingProgramManifestError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("cross-provider training-program manifest is invalid: {detail}")]
    InvalidManifest { detail: String },
    #[error("cross-provider training-program manifest cannot bind run state: {detail}")]
    RunStateBinding { detail: String },
}

/// Provider class admitted by the first cross-provider pretraining manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderComputeSourceClass {
    /// Google Cloud managed training nodes.
    GoogleCloud,
    /// RunPod rented pods.
    #[serde(rename = "runpod")]
    RunPod,
    /// Local operator-managed workstations.
    LocalWorkstation,
    /// Trusted-LAN cluster machines.
    TrustedLanCluster,
}

/// Execution class admitted by the first cross-provider pretraining manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderExecutionClass {
    /// Dense full-model rank in one distributed run.
    DenseFullModelRank,
    /// Bounded validated contributor window.
    ValidatedContributorWindow,
    /// Validator-owned verification work.
    Validator,
    /// Checkpoint writer role.
    CheckpointWriter,
    /// Evaluation worker role.
    EvalWorker,
    /// Data-builder role.
    DataBuilder,
}

/// One baseline artifact this manifest explicitly depends on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramBaselineArtifact {
    /// Repo-local artifact path.
    pub path: String,
    /// SHA256 over the current artifact bytes.
    pub sha256: String,
    /// Why the artifact still matters to the manifest.
    pub detail: String,
}

/// Stage authority frozen by the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramStageAuthority {
    /// Stage id bound to the manifest.
    pub stage_id: String,
    /// Stage kind bound to the manifest.
    pub stage_kind: TrainingStageKind,
    /// Canonical stage-config fixture path.
    pub stage_config_fixture_path: String,
    /// SHA256 over the committed stage-config fixture.
    pub stage_config_fixture_sha256: String,
    /// Canonical stage doc path.
    pub stage_doc_path: String,
}

/// Artifact-root templates frozen by the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramArtifactRoots {
    /// Root prefix template for one run.
    pub run_root_template: String,
    /// Launch-artifact root.
    pub launch_root_template: String,
    /// Checkpoint root.
    pub checkpoint_root_template: String,
    /// Metrics root.
    pub metrics_root_template: String,
    /// Visualization root.
    pub visualization_root_template: String,
    /// Final evidence root.
    pub final_root_template: String,
}

/// Budget posture frozen by the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramBudgetPolicy {
    /// Max dense ranks admitted by the first manifest.
    pub max_dense_full_model_ranks: u16,
    /// Max validated contributors admitted by the first manifest.
    pub max_validated_contributors: u16,
    /// Max validators admitted by the first manifest.
    pub max_validators: u16,
    /// Max wallclock budget in minutes.
    pub max_program_wallclock_minutes: u32,
    /// Max cost budget in USD cents.
    pub max_program_cost_usd_cents: u32,
}

/// Reserved final-evidence surface frozen by the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramEvidenceAuthority {
    /// Stable reserved evidence family id.
    pub evidence_family_id: String,
    /// Final evidence bundle template.
    pub final_evidence_bundle_template: String,
    /// Final manifest template.
    pub final_manifest_template: String,
    /// After-action audit template.
    pub after_action_audit_template: String,
}

/// Paths that remain authoritative for the manifest itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramAuthorityPaths {
    /// Manifest fixture path.
    pub fixture_path: String,
    /// Checker script path.
    pub check_script_path: String,
    /// Reference doc path.
    pub reference_doc_path: String,
    /// Train-system doc path.
    pub train_system_doc_path: String,
    /// Audit path.
    pub audit_path: String,
}

/// Full machine-legible cross-provider training-program manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderTrainingProgramManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable manifest id.
    pub program_manifest_id: String,
    /// Stable program family id.
    pub program_family_id: String,
    /// Stable run-id template.
    pub run_id_template: String,
    /// Stage authority frozen by the manifest.
    pub stage_authority: CrossProviderTrainingProgramStageAuthority,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable dataset family id.
    pub dataset_family_id: String,
    /// Environment package key bound to the run graph.
    pub environment: EnvironmentPackageKey,
    /// Admitted compute-source classes.
    pub admitted_compute_source_classes: Vec<CrossProviderComputeSourceClass>,
    /// Admitted execution classes.
    pub admitted_execution_classes: Vec<CrossProviderExecutionClass>,
    /// Artifact-root templates.
    pub artifact_roots: CrossProviderTrainingProgramArtifactRoots,
    /// Budget posture.
    pub budget_policy: CrossProviderTrainingProgramBudgetPolicy,
    /// Reserved final-evidence authority.
    pub evidence_authority: CrossProviderTrainingProgramEvidenceAuthority,
    /// Paths authoritative for the manifest itself.
    pub authority_paths: CrossProviderTrainingProgramAuthorityPaths,
    /// Explicit baseline artifacts this manifest still depends on.
    pub baseline_artifacts: Vec<CrossProviderTrainingProgramBaselineArtifact>,
    /// Explicit non-goals.
    pub non_goals: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable manifest digest.
    pub program_manifest_digest: String,
}

impl CrossProviderTrainingProgramManifest {
    /// Returns the stable digest over the manifest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.program_manifest_digest.clear();
        stable_digest(b"psionic_cross_provider_training_program_manifest|", &clone)
    }

    /// Validates the manifest invariants.
    pub fn validate(&self) -> Result<(), CrossProviderTrainingProgramManifestError> {
        if self.schema_version != CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_SCHEMA_VERSION {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_ID {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "program_manifest_id must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_ID, self.program_manifest_id
                ),
            });
        }
        if self.program_family_id != CROSS_PROVIDER_TRAINING_PROGRAM_FAMILY_ID {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "program_family_id must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_FAMILY_ID, self.program_family_id
                ),
            });
        }
        if self.run_id_template != CROSS_PROVIDER_TRAINING_PROGRAM_RUN_ID_TEMPLATE {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "run_id_template must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_RUN_ID_TEMPLATE, self.run_id_template
                ),
            });
        }
        if self.stage_authority.stage_id != CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "stage_authority.stage_id must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID, self.stage_authority.stage_id
                ),
            });
        }
        if self.stage_authority.stage_kind != TrainingStageKind::Pretrain {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from("stage_authority.stage_kind must stay pretrain"),
            });
        }
        if self.stage_authority.stage_config_fixture_path
            != PSION_PRETRAIN_STAGE_CONFIG_FIXTURE_PATH
        {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from(
                    "stage_authority.stage_config_fixture_path drifted away from the canonical Psion pretrain fixture",
                ),
            });
        }
        if self.checkpoint_family != CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "checkpoint_family must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY, self.checkpoint_family
                ),
            });
        }
        if self.dataset_family_id != CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "dataset_family_id must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID, self.dataset_family_id
                ),
            });
        }
        if self.environment.environment_ref != CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_REF {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "environment.environment_ref must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_REF,
                    self.environment.environment_ref
                ),
            });
        }
        if self.environment.version != CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_VERSION {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: format!(
                    "environment.version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_VERSION, self.environment.version
                ),
            });
        }
        require_exact_members(
            &self.admitted_compute_source_classes,
            &[
                CrossProviderComputeSourceClass::GoogleCloud,
                CrossProviderComputeSourceClass::RunPod,
                CrossProviderComputeSourceClass::LocalWorkstation,
                CrossProviderComputeSourceClass::TrustedLanCluster,
            ],
            "admitted_compute_source_classes",
        )?;
        require_exact_members(
            &self.admitted_execution_classes,
            &[
                CrossProviderExecutionClass::DenseFullModelRank,
                CrossProviderExecutionClass::ValidatedContributorWindow,
                CrossProviderExecutionClass::Validator,
                CrossProviderExecutionClass::CheckpointWriter,
                CrossProviderExecutionClass::EvalWorker,
                CrossProviderExecutionClass::DataBuilder,
            ],
            "admitted_execution_classes",
        )?;
        if self.artifact_roots.run_root_template != "runs/${RUN_ID}" {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from("artifact_roots.run_root_template must stay `runs/${RUN_ID}`"),
            });
        }
        if self.evidence_authority.evidence_family_id
            != "psionic.training_execution_evidence_bundle.v1.reserved"
        {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from(
                    "evidence_authority.evidence_family_id drifted away from the reserved family id",
                ),
            });
        }
        if self.authority_paths.fixture_path
            != CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_FIXTURE_PATH
        {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from("authority_paths.fixture_path drifted"),
            });
        }
        if self.authority_paths.check_script_path
            != CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_CHECK_SCRIPT_PATH
        {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from("authority_paths.check_script_path drifted"),
            });
        }
        if self.baseline_artifacts.len() < 3 {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from("expected at least three baseline artifacts"),
            });
        }
        if self.program_manifest_digest != self.stable_digest() {
            return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from(
                    "program_manifest_digest does not match the stable manifest digest",
                ),
            });
        }
        Ok(())
    }

    /// Binds the manifest identity and invariants into one run-graph state.
    pub fn bind_run_state(
        &self,
        run_state: &mut TrainingRunState,
    ) -> Result<(), CrossProviderTrainingProgramManifestError> {
        self.validate()?;
        if run_state.stage_id != self.stage_authority.stage_id {
            return Err(CrossProviderTrainingProgramManifestError::RunStateBinding {
                detail: format!(
                    "run stage_id `{}` does not match manifest stage_id `{}`",
                    run_state.stage_id, self.stage_authority.stage_id
                ),
            });
        }
        if run_state.checkpoint_family != self.checkpoint_family {
            return Err(CrossProviderTrainingProgramManifestError::RunStateBinding {
                detail: format!(
                    "run checkpoint_family `{}` does not match manifest checkpoint_family `{}`",
                    run_state.checkpoint_family, self.checkpoint_family
                ),
            });
        }
        if run_state.environment != self.environment {
            return Err(CrossProviderTrainingProgramManifestError::RunStateBinding {
                detail: format!(
                    "run environment `{}` does not match manifest environment `{}`",
                    run_state.environment.storage_key(),
                    self.environment.storage_key()
                ),
            });
        }
        let expected_prefix = run_id_prefix(self.run_id_template.as_str());
        if !run_state.run_id.starts_with(expected_prefix.as_str()) {
            return Err(CrossProviderTrainingProgramManifestError::RunStateBinding {
                detail: format!(
                    "run_id `{}` does not start with required manifest prefix `{}`",
                    run_state.run_id, expected_prefix
                ),
            });
        }
        run_state.program_manifest_id = Some(self.program_manifest_id.clone());
        run_state.program_manifest_digest = Some(self.program_manifest_digest.clone());
        Ok(())
    }
}

/// Returns the canonical cross-provider training-program manifest.
static CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_CACHE: OnceLock<
    CrossProviderTrainingProgramManifest,
> = OnceLock::new();

pub fn cross_provider_training_program_manifest(
) -> Result<CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError> {
    if let Some(manifest) = CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_CACHE.get() {
        return Ok(manifest.clone());
    }
    let stage_config_sha = artifact_sha256(PSION_PRETRAIN_STAGE_CONFIG_FIXTURE_PATH)?;
    let baseline_artifacts = vec![
        baseline_artifact(
            PSION_PRETRAIN_STAGE_CONFIG_FIXTURE_PATH,
            "The first cross-provider program manifest still binds directly to the canonical Psion pretrain stage-config fixture instead of inventing a second stage-authority plane.",
        )?,
        baseline_artifact(
            PSION_PRETRAIN_STAGE_DOC_PATH,
            "The first cross-provider program manifest still depends on the canonical pretrain-stage doc as the stage contract authority.",
        )?,
        baseline_artifact(
            CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_AUDIT_PATH,
            "The first cross-provider program manifest still depends on the readiness audit that froze the implementation path and issue spine.",
        )?,
    ];
    let mut manifest = CrossProviderTrainingProgramManifest {
        schema_version: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_SCHEMA_VERSION),
        program_manifest_id: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_ID),
        program_family_id: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_FAMILY_ID),
        run_id_template: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_RUN_ID_TEMPLATE),
        stage_authority: CrossProviderTrainingProgramStageAuthority {
            stage_id: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID),
            stage_kind: TrainingStageKind::Pretrain,
            stage_config_fixture_path: String::from(PSION_PRETRAIN_STAGE_CONFIG_FIXTURE_PATH),
            stage_config_fixture_sha256: stage_config_sha,
            stage_doc_path: String::from(PSION_PRETRAIN_STAGE_DOC_PATH),
        },
        checkpoint_family: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY),
        dataset_family_id: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID),
        environment: EnvironmentPackageKey::new(
            CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_REF,
            CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_VERSION,
        ),
        admitted_compute_source_classes: vec![
            CrossProviderComputeSourceClass::GoogleCloud,
            CrossProviderComputeSourceClass::RunPod,
            CrossProviderComputeSourceClass::LocalWorkstation,
            CrossProviderComputeSourceClass::TrustedLanCluster,
        ],
        admitted_execution_classes: vec![
            CrossProviderExecutionClass::DenseFullModelRank,
            CrossProviderExecutionClass::ValidatedContributorWindow,
            CrossProviderExecutionClass::Validator,
            CrossProviderExecutionClass::CheckpointWriter,
            CrossProviderExecutionClass::EvalWorker,
            CrossProviderExecutionClass::DataBuilder,
        ],
        artifact_roots: CrossProviderTrainingProgramArtifactRoots {
            run_root_template: String::from("runs/${RUN_ID}"),
            launch_root_template: String::from("runs/${RUN_ID}/launch"),
            checkpoint_root_template: String::from("runs/${RUN_ID}/checkpoints"),
            metrics_root_template: String::from("runs/${RUN_ID}/metrics"),
            visualization_root_template: String::from("runs/${RUN_ID}/training_visualization"),
            final_root_template: String::from("runs/${RUN_ID}/final"),
        },
        budget_policy: CrossProviderTrainingProgramBudgetPolicy {
            max_dense_full_model_ranks: 256,
            max_validated_contributors: 1024,
            max_validators: 64,
            max_program_wallclock_minutes: 10_080,
            max_program_cost_usd_cents: 5_000_000,
        },
        evidence_authority: CrossProviderTrainingProgramEvidenceAuthority {
            evidence_family_id: String::from(
                "psionic.training_execution_evidence_bundle.v1.reserved",
            ),
            final_evidence_bundle_template: String::from(
                "runs/${RUN_ID}/final/cross_provider_training_execution_evidence_bundle.json",
            ),
            final_manifest_template: String::from(
                "runs/${RUN_ID}/final/cross_provider_training_final_manifest.json",
            ),
            after_action_audit_template: String::from(
                "docs/audits/${RUN_ID}-cross-provider-training-after-action-audit.md",
            ),
        },
        authority_paths: CrossProviderTrainingProgramAuthorityPaths {
            fixture_path: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_FIXTURE_PATH),
            check_script_path: String::from(
                CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_CHECK_SCRIPT_PATH,
            ),
            reference_doc_path: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_DOC_PATH),
            train_system_doc_path: String::from(
                CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_TRAIN_SYSTEM_DOC_PATH,
            ),
            audit_path: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_AUDIT_PATH),
        },
        baseline_artifacts,
        non_goals: vec![
            String::from("provider-specific training control planes"),
            String::from("same-job mixed-backend dense training closure in the root manifest"),
            String::from("public or adversarial swarm compute"),
        ],
        claim_boundary: String::from(
            "This manifest freezes one provider-neutral cross-provider pretraining authority object over the current Psionic train substrate. It binds the canonical pretrain stage id, checkpoint family, environment key, artifact-root templates, admitted compute-source classes, and admitted execution classes under one stable manifest id and digest. It does not claim that every admitted execution class or backend family is already implemented; later issues close those runtime and provider-binding gaps.",
        ),
        program_manifest_digest: String::new(),
    };
    manifest.program_manifest_digest = manifest.stable_digest();
    manifest.validate()?;
    let _ = CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_CACHE.set(manifest.clone());
    Ok(manifest)
}

/// Writes the canonical cross-provider training-program manifest to one JSON path.
pub fn write_cross_provider_training_program_manifest(
    output_path: impl AsRef<Path>,
) -> Result<CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CrossProviderTrainingProgramManifestError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let manifest = cross_provider_training_program_manifest()?;
    let encoded = serde_json::to_string_pretty(&manifest)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        CrossProviderTrainingProgramManifestError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(manifest)
}

fn baseline_artifact(
    path: &str,
    detail: &str,
) -> Result<CrossProviderTrainingProgramBaselineArtifact, CrossProviderTrainingProgramManifestError>
{
    Ok(CrossProviderTrainingProgramBaselineArtifact {
        path: String::from(path),
        sha256: artifact_sha256(path)?,
        detail: String::from(detail),
    })
}

fn artifact_sha256(path: &str) -> Result<String, CrossProviderTrainingProgramManifestError> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(
            || CrossProviderTrainingProgramManifestError::InvalidManifest {
                detail: String::from("could not derive the repo root from CARGO_MANIFEST_DIR"),
            },
        )?
        .to_path_buf();
    let artifact_path = repo_root.join(path);
    let bytes = fs::read(&artifact_path).map_err(|error| {
        CrossProviderTrainingProgramManifestError::Read {
            path: artifact_path.display().to_string(),
            error,
        }
    })?;
    Ok(hex_sha256(bytes.as_slice()))
}

fn require_exact_members<T: Ord + Copy + std::fmt::Debug>(
    observed: &[T],
    expected: &[T],
    field_name: &str,
) -> Result<(), CrossProviderTrainingProgramManifestError> {
    let observed_set = observed.iter().copied().collect::<BTreeSet<_>>();
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    if observed_set != expected_set {
        return Err(CrossProviderTrainingProgramManifestError::InvalidManifest {
            detail: format!(
                "{field_name} drifted: expected {:?} but found {:?}",
                expected_set, observed_set
            ),
        });
    }
    Ok(())
}

fn run_id_prefix(template: &str) -> String {
    template
        .split("${RUN_ID}")
        .next()
        .unwrap_or(template)
        .to_string()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization should succeed"));
    format!("{:x}", hasher.finalize())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_environments::EnvironmentPackageKey;

    use super::*;
    use crate::TrainingRunGraphError;

    fn bound_run_state() -> Result<TrainingRunState, TrainingRunGraphError> {
        TrainingRunState::new(
            "psion-xprovider-pretrain-demo-001",
            CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID,
            "cluster-xprovider-001",
            CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY,
            EnvironmentPackageKey::new(
                CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_REF,
                CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_VERSION,
            ),
        )
    }

    #[test]
    fn cross_provider_training_program_manifest_stays_valid() {
        let manifest =
            cross_provider_training_program_manifest().expect("manifest should build successfully");
        manifest.validate().expect("manifest should validate");
        assert_eq!(manifest.program_manifest_digest, manifest.stable_digest());
        assert_eq!(
            manifest.stage_authority.stage_id,
            String::from(CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID)
        );
    }

    #[test]
    fn cross_provider_training_program_manifest_binds_training_run_state() {
        let manifest =
            cross_provider_training_program_manifest().expect("manifest should build successfully");
        let mut run = bound_run_state().expect("run graph should build");
        manifest
            .bind_run_state(&mut run)
            .expect("manifest should bind run state");
        assert_eq!(
            run.program_manifest_id,
            Some(String::from(CROSS_PROVIDER_TRAINING_PROGRAM_MANIFEST_ID))
        );
        assert_eq!(
            run.program_manifest_digest,
            Some(manifest.program_manifest_digest.clone())
        );
    }

    #[test]
    fn cross_provider_training_program_manifest_rejects_checkpoint_family_mismatch() {
        let manifest =
            cross_provider_training_program_manifest().expect("manifest should build successfully");
        let mut run = TrainingRunState::new(
            "psion-xprovider-pretrain-demo-002",
            CROSS_PROVIDER_TRAINING_PROGRAM_STAGE_ID,
            "cluster-xprovider-002",
            "wrong-checkpoint-family",
            EnvironmentPackageKey::new(
                CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_REF,
                CROSS_PROVIDER_TRAINING_PROGRAM_ENVIRONMENT_VERSION,
            ),
        )
        .expect("run graph should build");
        let error = manifest
            .bind_run_state(&mut run)
            .expect_err("checkpoint family mismatch must refuse");
        match error {
            CrossProviderTrainingProgramManifestError::RunStateBinding { detail } => {
                assert!(detail.contains("checkpoint_family"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
