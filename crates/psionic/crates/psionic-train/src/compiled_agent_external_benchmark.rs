use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    compiled_agent_supported_tools, predict_compiled_agent_route,
    CompiledAgentEvidenceClass, CompiledAgentModuleKind, CompiledAgentPublicOutcomeKind,
    CompiledAgentRoute, CompiledAgentRuntimeState, CompiledAgentToolCall,
    CompiledAgentToolResult, CompiledAgentVerifyVerdict,
    evaluate_compiled_agent_grounded_answer, evaluate_compiled_agent_route,
    evaluate_compiled_agent_tool_arguments, evaluate_compiled_agent_tool_policy,
    evaluate_compiled_agent_verify,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_compiled_agent_decentralized_roles_contract,
    canonical_compiled_agent_learning_receipt_ledger,
    canonical_compiled_agent_promoted_artifact_contract, canonical_compiled_agent_replay_bundle,
    build_compiled_agent_learning_receipt_from_source, repo_relative_path,
    CompiledAgentArtifactContractEntry, CompiledAgentArtifactContractError,
    CompiledAgentArtifactPayload, CompiledAgentCorpusSplit, CompiledAgentDecentralizedRolesError,
    CompiledAgentLearningPublicResponse, CompiledAgentReceiptError, CompiledAgentReceiptSupervisionLabel,
    CompiledAgentSourceInternalTrace, CompiledAgentSourceLineage, CompiledAgentSourceManifest,
    CompiledAgentSourcePhaseTraceEntry, CompiledAgentSourcePublicResponse,
    CompiledAgentSourceReceipt, CompiledAgentSourceRun,
    COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH,
    COMPILED_AGENT_LEARNING_RECEIPT_LEDGER_FIXTURE_PATH,
    COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH,
    COMPILED_AGENT_REPLAY_BUNDLE_FIXTURE_PATH,
};

pub const COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_benchmark_kit.v1";
pub const COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_benchmark_run.v1";
pub const COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_benchmark_kit_v1.json";
pub const COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_benchmark_run_v1.json";
pub const COMPILED_AGENT_EXTERNAL_BENCHMARK_DOC_PATH: &str =
    "docs/COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT.md";
pub const COMPILED_AGENT_EXTERNAL_BENCHMARK_BIN_PATH: &str =
    "crates/psionic-train/src/bin/compiled_agent_external_benchmark_kit.rs";

const COMPILED_AGENT_EXTERNAL_BENCHMARK_CONTRACT_ID: &str =
    "compiled_agent.external_benchmark_kit.contract.v1";
const COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_ID: &str =
    "compiled_agent.external_benchmark_run.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalContributorProfile {
    ExternalAlpha,
    TailnetM5Mlx,
    TailnetArchlinuxRtx4080Cuda,
}

impl CompiledAgentExternalContributorProfile {
    pub const fn profile_id(self) -> &'static str {
        match self {
            Self::ExternalAlpha => "external_alpha",
            Self::TailnetM5Mlx => "tailnet_m5_mlx",
            Self::TailnetArchlinuxRtx4080Cuda => "tailnet_archlinux_rtx4080_cuda",
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::ExternalAlpha => "External Contributor Alpha",
            Self::TailnetM5Mlx => "Tailnet M5 MLX",
            Self::TailnetArchlinuxRtx4080Cuda => "Tailnet Archlinux RTX 4080 CUDA",
        }
    }
}

#[derive(Debug, Error)]
pub enum CompiledAgentExternalBenchmarkError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid external benchmark contract: {detail}")]
    InvalidContract { detail: String },
    #[error("invalid external benchmark run: {detail}")]
    InvalidRun { detail: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error("compiled-agent artifact contract was incompatible: {detail}")]
    IncompatibleArtifactContract { detail: String },
    #[error(transparent)]
    ArtifactContract(#[from] CompiledAgentArtifactContractError),
    #[error(transparent)]
    DecentralizedRoles(#[from] CompiledAgentDecentralizedRolesError),
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalContributorIdentity {
    pub contributor_id: String,
    pub display_name: String,
    pub source_machine_id: String,
    pub machine_class: String,
    pub environment_class: String,
    pub declared_capabilities: Vec<String>,
    pub contract_version_accepted: String,
    pub attested_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalArtifactRef {
    pub artifact_ref: String,
    pub schema_version: String,
    pub artifact_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalBenchmarkRow {
    pub row_id: String,
    pub prompt: String,
    pub runtime_state: CompiledAgentRuntimeState,
    pub expected_route: CompiledAgentRoute,
    pub expected_public_response: CompiledAgentLearningPublicResponse,
    pub corpus_split: CompiledAgentCorpusSplit,
    pub tags: Vec<String>,
    pub failure_taxonomy_targets: Vec<String>,
    pub operator_note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentExternalValidatorOutcome {
    Accepted,
    ReviewRequired,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalBenchmarkRowRun {
    pub row_id: String,
    pub source_receipt: CompiledAgentSourceReceipt,
    pub expected_route: CompiledAgentRoute,
    pub expected_public_response: CompiledAgentLearningPublicResponse,
    pub corpus_split: CompiledAgentCorpusSplit,
    pub tags: Vec<String>,
    pub route_artifact_id: Option<String>,
    pub grounded_artifact_id: Option<String>,
    pub primary_confidence: f32,
    pub validator_outcome: CompiledAgentExternalValidatorOutcome,
    pub failure_classes: Vec<String>,
    pub operator_note: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalBenchmarkKit {
    pub schema_version: String,
    pub contract_id: String,
    pub admitted_task_family: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub receipt_schema_version: String,
    pub replay_bundle_schema_version: String,
    pub decentralized_roles_contract_schema_version: String,
    pub contributor_contract_version: String,
    pub contract_artifacts: Vec<CompiledAgentExternalArtifactRef>,
    pub benchmark_rows: Vec<CompiledAgentExternalBenchmarkRow>,
    pub failure_taxonomy: Vec<String>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalBenchmarkRun {
    pub schema_version: String,
    pub run_id: String,
    pub contributor: CompiledAgentExternalContributorIdentity,
    pub contract_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub externally_sourced: bool,
    pub accepted_row_ids: Vec<String>,
    pub review_row_ids: Vec<String>,
    pub row_runs: Vec<CompiledAgentExternalBenchmarkRowRun>,
    pub summary: String,
    pub run_digest: String,
}

impl CompiledAgentExternalBenchmarkKit {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"compiled_agent_external_benchmark_kit|", &clone)
    }

    pub fn validate(&self) -> Result<(), CompiledAgentExternalBenchmarkError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION {
            return Err(CompiledAgentExternalBenchmarkError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != COMPILED_AGENT_EXTERNAL_BENCHMARK_CONTRACT_ID {
            return Err(CompiledAgentExternalBenchmarkError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.benchmark_rows.is_empty() {
            return Err(CompiledAgentExternalBenchmarkError::InvalidContract {
                detail: String::from("benchmark rows must not be empty"),
            });
        }
        if self.contract_artifacts.len() < 4 {
            return Err(CompiledAgentExternalBenchmarkError::InvalidContract {
                detail: String::from("expected retained ledger, replay, decentralized-roles, and promoted-contract refs"),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(CompiledAgentExternalBenchmarkError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentExternalBenchmarkRun {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.run_digest.clear();
        stable_digest(b"compiled_agent_external_benchmark_run|", &clone)
    }

    pub fn validate(
        &self,
        contract: &CompiledAgentExternalBenchmarkKit,
    ) -> Result<(), CompiledAgentExternalBenchmarkError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_SCHEMA_VERSION {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.run_id != COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_ID {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: String::from("run_id drifted"),
            });
        }
        if self.contract_digest != contract.contract_digest {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: String::from("run lost contract linkage"),
            });
        }
        if self.evidence_class != contract.evidence_class {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: String::from("run evidence class drifted"),
            });
        }
        if self.contributor.contract_version_accepted != contract.contributor_contract_version {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: String::from("contributor accepted contract version drifted"),
            });
        }
        if self.row_runs.len() != contract.benchmark_rows.len() {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: String::from("row run count drifted from contract"),
            });
        }
        for row_run in &self.row_runs {
            if row_run.source_receipt.evidence_class != self.evidence_class {
                return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                    detail: format!("row `{}` drifted evidence class", row_run.row_id),
                });
            }
            let Some(row) = contract
                .benchmark_rows
                .iter()
                .find(|candidate| candidate.row_id == row_run.row_id)
            else {
                return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                    detail: format!("row `{}` no longer exists in the contract", row_run.row_id),
                });
            };
            if row.expected_route != row_run.expected_route
                || row.expected_public_response != row_run.expected_public_response
                || row.corpus_split != row_run.corpus_split
                || row.tags != row_run.tags
            {
                return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                    detail: format!("row `{}` drifted from the contract", row_run.row_id),
                });
            }
            let expected_row_digest = stable_digest(
                b"compiled_agent_external_benchmark_row_run|",
                &row_run_without_digest(row_run),
            );
            if row_run.row_digest != expected_row_digest {
                return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                    detail: format!("row `{}` digest drifted", row_run.row_id),
                });
            }
        }
        if self.run_digest != self.stable_digest() {
            return Err(CompiledAgentExternalBenchmarkError::InvalidRun {
                detail: String::from("run digest drifted"),
            });
        }
        Ok(())
    }
}

#[must_use]
pub fn compiled_agent_external_benchmark_kit_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_external_benchmark_run_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH)
}

pub fn load_compiled_agent_external_benchmark_kit(
    path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalBenchmarkKit, CompiledAgentExternalBenchmarkError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| CompiledAgentExternalBenchmarkError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let contract: CompiledAgentExternalBenchmarkKit = serde_json::from_slice(&bytes)?;
    contract.validate()?;
    Ok(contract)
}

pub fn retained_compiled_agent_external_benchmark_kit(
) -> Result<CompiledAgentExternalBenchmarkKit, CompiledAgentExternalBenchmarkError> {
    let path = compiled_agent_external_benchmark_kit_fixture_path();
    if path.exists() {
        load_compiled_agent_external_benchmark_kit(&path)
    } else {
        canonical_compiled_agent_external_benchmark_kit()
    }
}

#[must_use]
pub fn canonical_compiled_agent_external_contributor_identity() -> CompiledAgentExternalContributorIdentity {
    compiled_agent_external_contributor_identity_for_profile(
        CompiledAgentExternalContributorProfile::ExternalAlpha,
    )
}

#[must_use]
pub fn compiled_agent_external_contributor_identity_for_profile(
    profile: CompiledAgentExternalContributorProfile,
) -> CompiledAgentExternalContributorIdentity {
    match profile {
        CompiledAgentExternalContributorProfile::ExternalAlpha => {
            CompiledAgentExternalContributorIdentity {
                contributor_id: String::from("contrib.external.alpha"),
                display_name: String::from("External Contributor Alpha"),
                source_machine_id: String::from("external.alpha.archlinux.rtx4080"),
                machine_class: String::from("consumer_gpu"),
                environment_class: String::from("external_bounded_beta"),
                declared_capabilities: vec![
                    String::from("compiled_agent_benchmark"),
                    String::from("runtime_receipt_collection"),
                ],
                contract_version_accepted: String::from(
                    COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION,
                ),
                attested_at_epoch_ms: 1_774_800_000_000,
            }
        }
        CompiledAgentExternalContributorProfile::TailnetM5Mlx => {
            CompiledAgentExternalContributorIdentity {
                contributor_id: String::from("contrib.tailnet.m5"),
                display_name: String::from("Tailnet M5 MLX"),
                source_machine_id: String::from("tailnet.macbook-pro-m5.mlx"),
                machine_class: String::from("apple_silicon_mlx"),
                environment_class: String::from("tailnet_external_beta"),
                declared_capabilities: vec![
                    String::from("compiled_agent_benchmark"),
                    String::from("runtime_receipt_collection"),
                    String::from("bounded_training_coordination"),
                ],
                contract_version_accepted: String::from(
                    COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION,
                ),
                attested_at_epoch_ms: 1_774_900_010_000,
            }
        }
        CompiledAgentExternalContributorProfile::TailnetArchlinuxRtx4080Cuda => {
            CompiledAgentExternalContributorIdentity {
                contributor_id: String::from("contrib.tailnet.archlinux_rtx4080"),
                display_name: String::from("Tailnet Archlinux RTX 4080 CUDA"),
                source_machine_id: String::from("tailnet.archlinux.rtx4080.cuda"),
                machine_class: String::from("consumer_gpu_cuda"),
                environment_class: String::from("tailnet_external_beta"),
                declared_capabilities: vec![
                    String::from("compiled_agent_benchmark"),
                    String::from("runtime_receipt_collection"),
                    String::from("bounded_module_training"),
                ],
                contract_version_accepted: String::from(
                    COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION,
                ),
                attested_at_epoch_ms: 1_774_900_020_000,
            }
        }
    }
}

#[must_use]
pub fn compiled_agent_external_contributor_profile_from_id(
    profile_id: &str,
) -> Option<CompiledAgentExternalContributorProfile> {
    match profile_id {
        "external_alpha" => Some(CompiledAgentExternalContributorProfile::ExternalAlpha),
        "tailnet_m5_mlx" => Some(CompiledAgentExternalContributorProfile::TailnetM5Mlx),
        "tailnet_archlinux_rtx4080_cuda" => Some(
            CompiledAgentExternalContributorProfile::TailnetArchlinuxRtx4080Cuda,
        ),
        _ => None,
    }
}

pub fn canonical_compiled_agent_external_benchmark_kit(
) -> Result<CompiledAgentExternalBenchmarkKit, CompiledAgentExternalBenchmarkError> {
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let decentralized_roles = canonical_compiled_agent_decentralized_roles_contract()?;
    let promoted_contract = canonical_compiled_agent_promoted_artifact_contract()?;

    let mut contract = CompiledAgentExternalBenchmarkKit {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION),
        contract_id: String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_CONTRACT_ID),
        admitted_task_family: String::from("compiled_agent.first_graph.admitted_family.v1"),
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        receipt_schema_version: learning_ledger.schema_version.clone(),
        replay_bundle_schema_version: replay_bundle.schema_version.clone(),
        decentralized_roles_contract_schema_version: decentralized_roles.schema_version.clone(),
        contributor_contract_version: String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_SCHEMA_VERSION),
        contract_artifacts: vec![
            CompiledAgentExternalArtifactRef {
                artifact_ref: COMPILED_AGENT_LEARNING_RECEIPT_LEDGER_FIXTURE_PATH.to_string(),
                schema_version: learning_ledger.schema_version.clone(),
                artifact_digest: learning_ledger.ledger_digest.clone(),
                detail: String::from("Current compiled-agent governed learning ledger."),
            },
            CompiledAgentExternalArtifactRef {
                artifact_ref: COMPILED_AGENT_REPLAY_BUNDLE_FIXTURE_PATH.to_string(),
                schema_version: replay_bundle.schema_version.clone(),
                artifact_digest: replay_bundle.bundle_digest.clone(),
                detail: String::from("Current compiled-agent replay-bundle contract shape."),
            },
            CompiledAgentExternalArtifactRef {
                artifact_ref: COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH.to_string(),
                schema_version: decentralized_roles.schema_version.clone(),
                artifact_digest: decentralized_roles.contract_digest.clone(),
                detail: String::from("Current bounded decentralized-role contract."),
            },
            CompiledAgentExternalArtifactRef {
                artifact_ref: COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH.to_string(),
                schema_version: promoted_contract.schema_version.clone(),
                artifact_digest: promoted_contract.contract_digest.clone(),
                detail: String::from("Current promoted-artifact authority contract consumed by the runtime."),
            },
        ],
        benchmark_rows: vec![
            CompiledAgentExternalBenchmarkRow {
                row_id: String::from("external.provider_ready.v1"),
                prompt: String::from("Can I go online right now?"),
                runtime_state: CompiledAgentRuntimeState::default(),
                expected_route: CompiledAgentRoute::ProviderStatus,
                expected_public_response: CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                    response: String::from("Provider is ready to go online."),
                },
                corpus_split: CompiledAgentCorpusSplit::Training,
                tags: vec![
                    String::from("external"),
                    String::from("benchmark"),
                    String::from("provider"),
                    String::from("training"),
                ],
                failure_taxonomy_targets: vec![String::from("route_mismatch")],
                operator_note: String::from(
                    "External benchmark row for the canonical supported provider-ready path.",
                ),
            },
            CompiledAgentExternalBenchmarkRow {
                row_id: String::from("external.provider_blocked.v1"),
                prompt: String::from("Why can't I go online yet?"),
                runtime_state: CompiledAgentRuntimeState {
                    provider_ready: false,
                    provider_blockers: vec![String::from("wallet_locked")],
                    wallet_balance_sats: 1_200,
                    recent_earnings_sats: 240,
                },
                expected_route: CompiledAgentRoute::ProviderStatus,
                expected_public_response: CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                    response: String::from("Provider is not ready to go online."),
                },
                corpus_split: CompiledAgentCorpusSplit::HeldOut,
                tags: vec![
                    String::from("external"),
                    String::from("benchmark"),
                    String::from("provider"),
                    String::from("held_out"),
                ],
                failure_taxonomy_targets: vec![String::from("grounded_answer_mismatch")],
                operator_note: String::from(
                    "External benchmark row for provider-blocked grounding without widening into blocker summaries.",
                ),
            },
            CompiledAgentExternalBenchmarkRow {
                row_id: String::from("external.wallet_balance.v1"),
                prompt: String::from("How many sats are in the wallet?"),
                runtime_state: CompiledAgentRuntimeState::default(),
                expected_route: CompiledAgentRoute::WalletStatus,
                expected_public_response: CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                    response: String::from(
                        "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                    ),
                },
                corpus_split: CompiledAgentCorpusSplit::Training,
                tags: vec![
                    String::from("external"),
                    String::from("benchmark"),
                    String::from("wallet"),
                    String::from("training"),
                ],
                failure_taxonomy_targets: vec![String::from("grounded_answer_mismatch")],
                operator_note: String::from(
                    "External benchmark row for the admitted wallet-balance path under the promoted grounded model.",
                ),
            },
            CompiledAgentExternalBenchmarkRow {
                row_id: String::from("external.wallet_variant.v1"),
                prompt: String::from("What's the wallet balance in sats?"),
                runtime_state: CompiledAgentRuntimeState {
                    provider_ready: true,
                    provider_blockers: Vec::new(),
                    wallet_balance_sats: 3_400,
                    recent_earnings_sats: 180,
                },
                expected_route: CompiledAgentRoute::WalletStatus,
                expected_public_response: CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                    response: String::from(
                        "Wallet balance is 3400 sats, with 180 sats of recent earnings.",
                    ),
                },
                corpus_split: CompiledAgentCorpusSplit::HeldOut,
                tags: vec![
                    String::from("external"),
                    String::from("benchmark"),
                    String::from("wallet"),
                    String::from("held_out"),
                ],
                failure_taxonomy_targets: vec![String::from("grounded_answer_mismatch")],
                operator_note: String::from(
                    "External benchmark row for a held-out wallet phrasing variation.",
                ),
            },
            CompiledAgentExternalBenchmarkRow {
                row_id: String::from("external.unsupported_gpu_poem.v1"),
                prompt: String::from("Write a poem about GPUs."),
                runtime_state: CompiledAgentRuntimeState::default(),
                expected_route: CompiledAgentRoute::Unsupported,
                expected_public_response: CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                    response: String::from(
                        "I can currently answer only provider readiness and wallet balance questions.",
                    ),
                },
                corpus_split: CompiledAgentCorpusSplit::Training,
                tags: vec![
                    String::from("external"),
                    String::from("benchmark"),
                    String::from("unsupported"),
                    String::from("training"),
                ],
                failure_taxonomy_targets: vec![String::from("unexpected_tool_exposure")],
                operator_note: String::from(
                    "External benchmark row for a clean unsupported refusal outside the admitted family.",
                ),
            },
            CompiledAgentExternalBenchmarkRow {
                row_id: String::from("external.negated_wallet.v1"),
                prompt: String::from("What is not my wallet balance?"),
                runtime_state: CompiledAgentRuntimeState::default(),
                expected_route: CompiledAgentRoute::Unsupported,
                expected_public_response: CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                    response: String::from(
                        "I can currently answer only provider readiness and wallet balance questions.",
                    ),
                },
                corpus_split: CompiledAgentCorpusSplit::HeldOut,
                tags: vec![
                    String::from("external"),
                    String::from("benchmark"),
                    String::from("wallet"),
                    String::from("negated"),
                    String::from("held_out"),
                ],
                failure_taxonomy_targets: vec![
                    String::from("negated_route_false_positive"),
                    String::from("grounded_answer_mismatch"),
                ],
                operator_note: String::from(
                    "External benchmark row that should stay unsupported and expose the retained negated-wallet weakness if it regresses.",
                ),
            },
        ],
        failure_taxonomy: vec![
            String::from("route_mismatch"),
            String::from("negated_route_false_positive"),
            String::from("unexpected_tool_exposure"),
            String::from("tool_argument_mismatch"),
            String::from("grounded_answer_mismatch"),
            String::from("unsafe_final_outcome"),
        ],
        claim_boundary: String::from(
            "This package stays on the first narrow compiled-agent family. It does not claim broad agent evaluation, external promotion authority, or raw-log admission into training.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn canonical_compiled_agent_external_benchmark_run(
) -> Result<CompiledAgentExternalBenchmarkRun, CompiledAgentExternalBenchmarkError> {
    run_compiled_agent_external_benchmark_kit(
        &canonical_compiled_agent_external_benchmark_kit()?,
        &canonical_compiled_agent_external_contributor_identity(),
    )
}

pub fn run_compiled_agent_external_benchmark_kit(
    contract: &CompiledAgentExternalBenchmarkKit,
    contributor: &CompiledAgentExternalContributorIdentity,
) -> Result<CompiledAgentExternalBenchmarkRun, CompiledAgentExternalBenchmarkError> {
    let promoted_contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let route_entry = promoted_contract
        .promoted_entry(CompiledAgentModuleKind::Route)
        .ok_or_else(|| CompiledAgentExternalBenchmarkError::InvalidContract {
            detail: String::from("promoted route entry missing from artifact contract"),
        })?;
    let tool_policy_entry = promoted_contract
        .promoted_entry(CompiledAgentModuleKind::ToolPolicy)
        .ok_or_else(|| CompiledAgentExternalBenchmarkError::InvalidContract {
            detail: String::from("promoted tool_policy entry missing from artifact contract"),
        })?;
    let tool_arguments_entry = promoted_contract
        .promoted_entry(CompiledAgentModuleKind::ToolArguments)
        .ok_or_else(|| CompiledAgentExternalBenchmarkError::InvalidContract {
            detail: String::from("promoted tool_arguments entry missing from artifact contract"),
        })?;
    let grounded_entry = promoted_contract
        .promoted_entry(CompiledAgentModuleKind::GroundedAnswer)
        .ok_or_else(|| CompiledAgentExternalBenchmarkError::InvalidContract {
            detail: String::from("promoted grounded_answer entry missing from artifact contract"),
        })?;
    let verify_entry = promoted_contract
        .promoted_entry(CompiledAgentModuleKind::Verify)
        .ok_or_else(|| CompiledAgentExternalBenchmarkError::InvalidContract {
            detail: String::from("promoted verify entry missing from artifact contract"),
        })?;

    let mut row_runs = Vec::new();
    for row in &contract.benchmark_rows {
        let route = evaluate_route(route_entry, row.prompt.as_str())?;
        let selected_tools =
            evaluate_compiled_agent_tool_policy(route, &compiled_agent_supported_tools());
        let selected_tool_names = selected_tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<Vec<_>>();
        let tool_calls = evaluate_compiled_agent_tool_arguments(&selected_tool_names);
        let tool_results = tool_results_for_route(route, &row.runtime_state);
        let grounded_answer = evaluate_grounded_answer(grounded_entry, route, &tool_results)?;
        let verify_verdict = evaluate_verify(
            verify_entry,
            route,
            &selected_tool_names,
            &tool_results,
            grounded_answer.as_str(),
        )?;
        let public_response = public_response_from_verdict(
            verify_verdict,
            grounded_answer,
            &grounded_entry.artifact_id,
        );
        let source_receipt = build_external_source_receipt(
            row,
            &row.runtime_state,
            contributor,
            route_entry,
            tool_policy_entry,
            tool_arguments_entry,
            grounded_entry,
            verify_entry,
            route,
            tool_calls,
            tool_results,
            public_response,
        );
        let learning_receipt = build_compiled_agent_learning_receipt_from_source(
            format!(
                "{}#{}",
                COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH, row.row_id
            )
            .as_str(),
            &source_receipt,
            &CompiledAgentReceiptSupervisionLabel {
                expected_route: row.expected_route,
                expected_public_response: row.expected_public_response.clone(),
                corpus_split: row.corpus_split,
                tags: row.tags.clone(),
                operator_note: row.operator_note.clone(),
            },
        )?;
        let validator_outcome = if learning_receipt.assessment.overall_success {
            CompiledAgentExternalValidatorOutcome::Accepted
        } else {
            CompiledAgentExternalValidatorOutcome::ReviewRequired
        };
        let mut row_run = CompiledAgentExternalBenchmarkRowRun {
            row_id: row.row_id.clone(),
            route_artifact_id: Some(route_entry.artifact_id.clone()),
            grounded_artifact_id: Some(grounded_entry.artifact_id.clone()),
            source_receipt,
            expected_route: row.expected_route,
            expected_public_response: row.expected_public_response.clone(),
            corpus_split: row.corpus_split,
            tags: row.tags.clone(),
            primary_confidence: primary_confidence_from_receipt(&learning_receipt),
            validator_outcome,
            failure_classes: learning_receipt.assessment.failure_classes.clone(),
            operator_note: row.operator_note.clone(),
            row_digest: String::new(),
        };
        row_run.row_digest = stable_digest(
            b"compiled_agent_external_benchmark_row_run|",
            &row_run_without_digest(&row_run),
        );
        row_runs.push(row_run);
    }

    let accepted_row_ids = row_runs
        .iter()
        .filter(|row| row.validator_outcome == CompiledAgentExternalValidatorOutcome::Accepted)
        .map(|row| row.row_id.clone())
        .collect::<Vec<_>>();
    let review_row_ids = row_runs
        .iter()
        .filter(|row| row.validator_outcome == CompiledAgentExternalValidatorOutcome::ReviewRequired)
        .map(|row| row.row_id.clone())
        .collect::<Vec<_>>();

    let mut run = CompiledAgentExternalBenchmarkRun {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_SCHEMA_VERSION),
        run_id: String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_ID),
        contributor: contributor.clone(),
        contract_digest: contract.contract_digest.clone(),
        evidence_class: contract.evidence_class,
        externally_sourced: true,
        accepted_row_ids,
        review_row_ids,
        row_runs,
        summary: String::new(),
        run_digest: String::new(),
    };
    run.summary = format!(
        "External benchmark run retained {} rows for contributor `{}` with {} accepted rows and {} review-required rows on the admitted compiled-agent family.",
        run.row_runs.len(),
        run.contributor.contributor_id,
        run.accepted_row_ids.len(),
        run.review_row_ids.len(),
    );
    run.run_digest = run.stable_digest();
    run.validate(contract)?;
    Ok(run)
}

pub fn write_compiled_agent_external_benchmark_kit(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalBenchmarkKit, CompiledAgentExternalBenchmarkError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentExternalBenchmarkError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_compiled_agent_external_benchmark_kit()?;
    let json = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentExternalBenchmarkError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

pub fn write_compiled_agent_external_benchmark_run(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalBenchmarkRun, CompiledAgentExternalBenchmarkError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentExternalBenchmarkError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let run = canonical_compiled_agent_external_benchmark_run()?;
    let json = serde_json::to_string_pretty(&run)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentExternalBenchmarkError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(run)
}

pub fn verify_compiled_agent_external_benchmark_fixtures(
) -> Result<(), CompiledAgentExternalBenchmarkError> {
    let expected_contract = canonical_compiled_agent_external_benchmark_kit()?;
    let expected_run = canonical_compiled_agent_external_benchmark_run()?;
    let contract_bytes =
        fs::read(compiled_agent_external_benchmark_kit_fixture_path()).map_err(|error| {
            CompiledAgentExternalBenchmarkError::Read {
                path: compiled_agent_external_benchmark_kit_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?;
    let run_bytes =
        fs::read(compiled_agent_external_benchmark_run_fixture_path()).map_err(|error| {
            CompiledAgentExternalBenchmarkError::Read {
                path: compiled_agent_external_benchmark_run_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?;
    let committed_contract: CompiledAgentExternalBenchmarkKit =
        serde_json::from_slice(&contract_bytes)?;
    let committed_run: CompiledAgentExternalBenchmarkRun = serde_json::from_slice(&run_bytes)?;
    if committed_contract != expected_contract {
        return Err(CompiledAgentExternalBenchmarkError::FixtureDrift {
            path: compiled_agent_external_benchmark_kit_fixture_path()
                .display()
                .to_string(),
        });
    }
    if committed_run != expected_run {
        return Err(CompiledAgentExternalBenchmarkError::FixtureDrift {
            path: compiled_agent_external_benchmark_run_fixture_path()
                .display()
                .to_string(),
        });
    }
    Ok(())
}

fn evaluate_route(
    entry: &CompiledAgentArtifactContractEntry,
    prompt: &str,
) -> Result<CompiledAgentRoute, CompiledAgentExternalBenchmarkError> {
    match &entry.payload {
        CompiledAgentArtifactPayload::RouteModel { artifact } => {
            Ok(predict_compiled_agent_route(artifact, prompt).route)
        }
        CompiledAgentArtifactPayload::RevisionSet { revision } => {
            Ok(evaluate_compiled_agent_route(prompt, revision))
        }
        // The compiled-agent route benchmark scores route-model / revision-set
        // payloads. A coordinator-head candidate is a frozen-backbone routing
        // head evaluated through the coordinator `ShadowComparison`, not this
        // prompt-route benchmark.
        CompiledAgentArtifactPayload::CoordinatorHead { .. } => {
            Err(CompiledAgentExternalBenchmarkError::IncompatibleArtifactContract {
                detail: format!(
                    "module `{}` carries a coordinator-head payload, which is not scored by the route benchmark",
                    entry.module_name
                ),
            })
        }
    }
}

fn evaluate_grounded_answer(
    entry: &CompiledAgentArtifactContractEntry,
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> Result<String, CompiledAgentExternalBenchmarkError> {
    let CompiledAgentArtifactPayload::RevisionSet { revision } = &entry.payload else {
        return Err(CompiledAgentExternalBenchmarkError::IncompatibleArtifactContract {
            detail: format!(
                "module `{}` expected a revision-set payload for grounded answer",
                entry.module_name
            ),
        });
    };
    Ok(evaluate_compiled_agent_grounded_answer(
        route,
        tool_results,
        revision,
    ))
}

fn evaluate_verify(
    entry: &CompiledAgentArtifactContractEntry,
    route: CompiledAgentRoute,
    selected_tool_names: &[String],
    tool_results: &[CompiledAgentToolResult],
    candidate_answer: &str,
) -> Result<CompiledAgentVerifyVerdict, CompiledAgentExternalBenchmarkError> {
    let CompiledAgentArtifactPayload::RevisionSet { revision } = &entry.payload else {
        return Err(CompiledAgentExternalBenchmarkError::IncompatibleArtifactContract {
            detail: format!(
                "module `{}` expected a revision-set payload for verify",
                entry.module_name
            ),
        });
    };
    Ok(evaluate_compiled_agent_verify(
        route,
        selected_tool_names,
        tool_results,
        candidate_answer,
        revision,
    ))
}

fn public_response_from_verdict(
    verdict: CompiledAgentVerifyVerdict,
    candidate_answer: String,
    grounded_artifact_id: &str,
) -> CompiledAgentSourcePublicResponse {
    match verdict {
        CompiledAgentVerifyVerdict::AcceptGroundedAnswer => CompiledAgentSourcePublicResponse {
            kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
            response: candidate_answer,
        },
        CompiledAgentVerifyVerdict::UnsupportedRefusal => CompiledAgentSourcePublicResponse {
            kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
            response: candidate_answer,
        },
        CompiledAgentVerifyVerdict::NeedsFallback => CompiledAgentSourcePublicResponse {
            kind: CompiledAgentPublicOutcomeKind::ConfidenceFallback,
            response: format!(
                "Confidence fallback preserved instead of promoting `{grounded_artifact_id}`."
            ),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn build_external_source_receipt(
    row: &CompiledAgentExternalBenchmarkRow,
    runtime_state: &CompiledAgentRuntimeState,
    contributor: &CompiledAgentExternalContributorIdentity,
    route_entry: &CompiledAgentArtifactContractEntry,
    tool_policy_entry: &CompiledAgentArtifactContractEntry,
    tool_arguments_entry: &CompiledAgentArtifactContractEntry,
    grounded_entry: &CompiledAgentArtifactContractEntry,
    verify_entry: &CompiledAgentArtifactContractEntry,
    route: CompiledAgentRoute,
    tool_calls: Vec<CompiledAgentToolCall>,
    tool_results: Vec<CompiledAgentToolResult>,
    public_response: CompiledAgentSourcePublicResponse,
) -> CompiledAgentSourceReceipt {
    let selected_tools = evaluate_compiled_agent_tool_policy(route, &compiled_agent_supported_tools())
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
            })
        })
        .collect::<Vec<_>>();
    let verify_verdict = if public_response.kind == CompiledAgentPublicOutcomeKind::UnsupportedRefusal {
        "unsupported_refusal"
    } else if public_response.kind == CompiledAgentPublicOutcomeKind::ConfidenceFallback {
        "needs_fallback"
    } else {
        "accept_grounded_answer"
    };
    let primary_phases = vec![
        phase_trace_entry(
            "intent_route",
            route_entry,
            json!({ "user_request": row.prompt }),
            json!({ "route": route }),
            route_confidence(route, row.prompt.as_str()),
            json!({
                "artifact_id": route_entry.artifact_id,
                "artifact_digest": route_entry.artifact_digest,
                "source": "external_benchmark_kit",
            }),
        ),
        phase_trace_entry(
            "tool_policy",
            tool_policy_entry,
            json!({
                "user_request": row.prompt,
                "route": route,
                "available_tools": compiled_agent_supported_tools(),
            }),
            json!({ "selected_tools": selected_tools }),
            0.92,
            json!({
                "artifact_id": tool_policy_entry.artifact_id,
                "artifact_digest": tool_policy_entry.artifact_digest,
            }),
        ),
        phase_trace_entry(
            "tool_arguments",
            tool_arguments_entry,
            json!({
                "user_request": row.prompt,
                "route": route,
                "selected_tools": selected_tools,
            }),
            json!({ "calls": tool_calls }),
            0.96,
            json!({
                "artifact_id": tool_arguments_entry.artifact_id,
                "artifact_digest": tool_arguments_entry.artifact_digest,
            }),
        ),
        phase_trace_entry(
            "grounded_answer",
            grounded_entry,
            json!({
                "user_request": row.prompt,
                "route": route,
                "tool_results": tool_results,
            }),
            json!({
                "answer": public_response.response,
                "response_kind": public_response.kind,
            }),
            grounded_confidence(route, &tool_results),
            json!({
                "artifact_id": grounded_entry.artifact_id,
                "artifact_digest": grounded_entry.artifact_digest,
            }),
        ),
        phase_trace_entry(
            "verify",
            verify_entry,
            json!({
                "user_request": row.prompt,
                "route": route,
                "tool_calls": tool_calls,
                "tool_results": tool_results,
                "candidate_answer": public_response.response,
            }),
            json!({
                "verdict": verify_verdict,
            }),
            0.9,
            json!({
                "artifact_id": verify_entry.artifact_id,
                "artifact_digest": verify_entry.artifact_digest,
            }),
        ),
    ];
    let authority_manifest_ids = primary_phases
        .iter()
        .map(|phase| phase.manifest.manifest_id())
        .collect::<Vec<_>>();
    CompiledAgentSourceReceipt {
        schema_version: 1,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        captured_at_epoch_ms: contributor.attested_at_epoch_ms,
        state: runtime_state.clone(),
        run: CompiledAgentSourceRun {
            public_response: public_response.clone(),
            internal_trace: CompiledAgentSourceInternalTrace {
                primary_phases,
                shadow_phases: Vec::new(),
            },
            lineage: CompiledAgentSourceLineage {
                user_request: row.prompt.clone(),
                route,
                tool_calls,
                tool_results,
                public_response,
                authority_manifest_ids,
                shadow_manifest_ids: Vec::new(),
            },
        },
    }
}

fn phase_trace_entry(
    phase: &str,
    entry: &CompiledAgentArtifactContractEntry,
    input: Value,
    output: Value,
    confidence: f32,
    trace: Value,
) -> CompiledAgentSourcePhaseTraceEntry {
    CompiledAgentSourcePhaseTraceEntry {
        phase: String::from(phase),
        manifest: CompiledAgentSourceManifest {
            module_name: entry.module_name.clone(),
            signature_name: entry.signature_name.clone(),
            implementation_family: entry.implementation_family.clone(),
            implementation_label: entry.implementation_label.clone(),
            version: entry.version.clone(),
            promotion_state: match entry.lifecycle_state {
                crate::CompiledAgentArtifactLifecycleState::Promoted => String::from("promoted"),
                crate::CompiledAgentArtifactLifecycleState::Candidate => String::from("candidate"),
            },
            confidence_floor: entry.confidence_floor,
        },
        authority: String::from("promoted"),
        candidate_label: None,
        input,
        output,
        confidence,
        trace,
    }
}

fn route_confidence(route: CompiledAgentRoute, prompt: &str) -> f32 {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("not") || lower.contains("besides") {
        0.79
    } else if route == CompiledAgentRoute::Unsupported {
        0.88
    } else {
        0.93
    }
}

fn grounded_confidence(route: CompiledAgentRoute, tool_results: &[CompiledAgentToolResult]) -> f32 {
    match route {
        CompiledAgentRoute::Unsupported => 0.95,
        CompiledAgentRoute::ProviderStatus | CompiledAgentRoute::WalletStatus if tool_results.is_empty() => 0.4,
        CompiledAgentRoute::ProviderStatus | CompiledAgentRoute::WalletStatus => 0.91,
    }
}

fn tool_results_for_route(
    route: CompiledAgentRoute,
    runtime_state: &CompiledAgentRuntimeState,
) -> Vec<CompiledAgentToolResult> {
    match route {
        CompiledAgentRoute::ProviderStatus => vec![CompiledAgentToolResult {
            tool_name: String::from("provider_status"),
            payload: json!({
                "ready": runtime_state.provider_ready,
                "blockers": runtime_state.provider_blockers,
            }),
        }],
        CompiledAgentRoute::WalletStatus => vec![CompiledAgentToolResult {
            tool_name: String::from("wallet_status"),
            payload: json!({
                "balance_sats": runtime_state.wallet_balance_sats,
                "recent_earnings_sats": runtime_state.recent_earnings_sats,
            }),
        }],
        CompiledAgentRoute::Unsupported => Vec::new(),
    }
}

fn primary_confidence_from_receipt(
    receipt: &crate::CompiledAgentLearningReceipt,
) -> f32 {
    receipt
        .primary_phase_confidences
        .values()
        .copied()
        .reduce(f32::min)
        .unwrap_or(0.0)
}

fn row_run_without_digest(
    row_run: &CompiledAgentExternalBenchmarkRowRun,
) -> CompiledAgentExternalBenchmarkRowRun {
    let mut clone = row_run.clone();
    clone.row_digest.clear();
    clone
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("stable digest serialization must succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_external_benchmark_kit,
        canonical_compiled_agent_external_benchmark_run,
        verify_compiled_agent_external_benchmark_fixtures,
    };

    #[test]
    fn external_benchmark_contract_is_valid() -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_external_benchmark_kit()?;
        contract.validate()?;
        assert_eq!(contract.benchmark_rows.len(), 6);
        Ok(())
    }

    #[test]
    fn external_benchmark_run_keeps_review_row_for_negated_wallet_case(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_external_benchmark_kit()?;
        let run = canonical_compiled_agent_external_benchmark_run()?;
        run.validate(&contract)?;
        assert!(run
            .review_row_ids
            .iter()
            .any(|row_id| row_id == "external.negated_wallet.v1"));
        Ok(())
    }

    #[test]
    fn committed_external_benchmark_fixtures_match_canonical_output(
    ) -> Result<(), Box<dyn std::error::Error>> {
        verify_compiled_agent_external_benchmark_fixtures()?;
        Ok(())
    }
}
