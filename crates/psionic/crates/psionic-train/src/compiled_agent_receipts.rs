use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    compiled_agent_baseline_revision_set, compiled_agent_supported_tools,
    CompiledAgentEvidenceClass, CompiledAgentModuleKind, CompiledAgentPublicOutcomeKind,
    CompiledAgentRoute, CompiledAgentRuntimeState, CompiledAgentToolCall, CompiledAgentToolResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::repo_relative_path;

pub const COMPILED_AGENT_SOURCE_FIXTURE_DIR: &str = "fixtures/compiled_agent/source";
pub const COMPILED_AGENT_RUNTIME_FIXTURE_DIR: &str = "fixtures/compiled_agent/runtime";
pub const COMPILED_AGENT_LEARNING_RECEIPT_LEDGER_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_learning_receipts_v1.json";
pub const COMPILED_AGENT_REPLAY_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_replay_bundle_v1.json";

const LEARNING_RECEIPT_SCHEMA_VERSION: &str = "psionic.compiled_agent.learning_receipts.v1";
const REPLAY_BUNDLE_SCHEMA_VERSION: &str = "psionic.compiled_agent.replay_bundle.v1";

#[derive(Debug, Error)]
pub enum CompiledAgentReceiptError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("source fixture `{fixture}` has no canonical supervision label")]
    MissingLabel { fixture: String },
    #[error("source receipt `{fixture}` is missing phase `{phase}`")]
    MissingPhase { fixture: String, phase: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(
        "compiled-agent evidence class drifted in `{context}`: expected `{expected:?}` but found `{actual:?}`"
    )]
    MixedEvidenceClass {
        context: String,
        expected: CompiledAgentEvidenceClass,
        actual: CompiledAgentEvidenceClass,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourcePublicResponse {
    pub kind: CompiledAgentPublicOutcomeKind,
    pub response: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourceManifest {
    pub module_name: String,
    pub signature_name: String,
    pub implementation_family: String,
    pub implementation_label: String,
    pub version: String,
    pub promotion_state: String,
    pub confidence_floor: f32,
}

impl CompiledAgentSourceManifest {
    #[must_use]
    pub fn manifest_id(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.module_name, self.implementation_family, self.implementation_label, self.version
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourcePhaseTraceEntry {
    pub phase: String,
    pub manifest: CompiledAgentSourceManifest,
    pub authority: String,
    pub candidate_label: Option<String>,
    pub input: Value,
    pub output: Value,
    pub confidence: f32,
    pub trace: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourceInternalTrace {
    pub primary_phases: Vec<CompiledAgentSourcePhaseTraceEntry>,
    pub shadow_phases: Vec<CompiledAgentSourcePhaseTraceEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourceLineage {
    pub user_request: String,
    pub route: CompiledAgentRoute,
    pub tool_calls: Vec<CompiledAgentToolCall>,
    pub tool_results: Vec<CompiledAgentToolResult>,
    pub public_response: CompiledAgentSourcePublicResponse,
    pub authority_manifest_ids: Vec<String>,
    pub shadow_manifest_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourceRun {
    pub public_response: CompiledAgentSourcePublicResponse,
    pub internal_trace: CompiledAgentSourceInternalTrace,
    pub lineage: CompiledAgentSourceLineage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSourceReceipt {
    pub schema_version: u32,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub captured_at_epoch_ms: u64,
    pub state: CompiledAgentRuntimeState,
    pub run: CompiledAgentSourceRun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentLearningPublicResponse {
    pub kind: CompiledAgentPublicOutcomeKind,
    pub response: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentCorpusSplit {
    Training,
    HeldOut,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentLearningAssessment {
    pub route_correct: bool,
    pub tool_policy_correct: bool,
    pub tool_arguments_correct: bool,
    pub grounded_answer_correct: bool,
    pub verify_correct: bool,
    pub overall_success: bool,
    pub failure_classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentLearningReceipt {
    pub receipt_id: String,
    pub source_fixture_ref: String,
    pub source_receipt_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub captured_at_epoch_ms: u64,
    pub user_request: String,
    pub runtime_state: CompiledAgentRuntimeState,
    pub observed_route: CompiledAgentRoute,
    pub expected_route: CompiledAgentRoute,
    pub observed_tool_names: Vec<String>,
    pub expected_tool_names: Vec<String>,
    pub observed_tool_calls: Vec<CompiledAgentToolCall>,
    pub observed_tool_results: Vec<CompiledAgentToolResult>,
    pub observed_public_response: CompiledAgentLearningPublicResponse,
    pub expected_public_response: CompiledAgentLearningPublicResponse,
    pub authority_manifest_ids: Vec<String>,
    pub shadow_manifest_ids: Vec<String>,
    pub primary_phase_confidences: BTreeMap<String, f32>,
    pub corpus_split: CompiledAgentCorpusSplit,
    pub assessment: CompiledAgentLearningAssessment,
    pub tags: Vec<String>,
    pub operator_note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentLearningReceiptLedger {
    pub schema_version: String,
    pub ledger_id: String,
    pub row_id: String,
    pub baseline_revision_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_fixture_refs: Vec<String>,
    pub training_receipt_ids: Vec<String>,
    pub held_out_receipt_ids: Vec<String>,
    pub correction_receipt_ids: Vec<String>,
    pub split_receipt_counts: BTreeMap<String, u32>,
    pub task_family_counts: BTreeMap<String, u32>,
    pub module_failure_counts: BTreeMap<String, u32>,
    pub failure_class_counts: BTreeMap<String, u32>,
    pub receipts: Vec<CompiledAgentLearningReceipt>,
    pub summary: String,
    pub ledger_digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentReplayCorrectionKind {
    BehavioralClone,
    FailureCorrection,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentReplaySample {
    pub sample_id: String,
    pub module: CompiledAgentModuleKind,
    pub source_receipt_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub correction_kind: CompiledAgentReplayCorrectionKind,
    pub tags: Vec<String>,
    pub failure_classes: Vec<String>,
    pub input: Value,
    pub expected_output: Value,
    pub observed_output: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentReplayBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub row_id: String,
    pub baseline_revision_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_ledger_digest: String,
    pub training_receipt_ids: Vec<String>,
    pub excluded_held_out_receipt_ids: Vec<String>,
    pub module_sample_counts: BTreeMap<String, u32>,
    pub correction_sample_count: u32,
    pub samples: Vec<CompiledAgentReplaySample>,
    pub summary: String,
    pub bundle_digest: String,
}

struct CanonicalSupervisionScenario {
    fixture_name: &'static str,
    captured_at_epoch_ms: u64,
    user_request: &'static str,
    runtime_state: CompiledAgentRuntimeState,
    observed_route: CompiledAgentRoute,
    observed_public_response: CompiledAgentLearningPublicResponse,
    expected_route: CompiledAgentRoute,
    expected_public_response: CompiledAgentLearningPublicResponse,
    corpus_split: CompiledAgentCorpusSplit,
    tags: &'static [&'static str],
    operator_note: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentReceiptSupervisionLabel {
    pub expected_route: CompiledAgentRoute,
    pub expected_public_response: CompiledAgentLearningPublicResponse,
    pub corpus_split: CompiledAgentCorpusSplit,
    pub tags: Vec<String>,
    pub operator_note: String,
}

type CompiledAgentGovernedReceiptLabel = CompiledAgentReceiptSupervisionLabel;

struct CanonicalRuntimeReceiptScenario {
    fixture_name: &'static str,
    source_receipt: CompiledAgentSourceReceipt,
    label: CompiledAgentGovernedReceiptLabel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledAgentGovernedSourceReceipt {
    pub source_fixture_ref: String,
    pub source_receipt: CompiledAgentSourceReceipt,
    pub label: CompiledAgentGovernedReceiptLabel,
}

#[must_use]
pub fn compiled_agent_source_fixture_dir() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_SOURCE_FIXTURE_DIR)
}

#[must_use]
pub fn compiled_agent_runtime_fixture_dir() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_RUNTIME_FIXTURE_DIR)
}

#[must_use]
pub fn compiled_agent_learning_receipt_ledger_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_LEARNING_RECEIPT_LEDGER_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_replay_bundle_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_REPLAY_BUNDLE_FIXTURE_PATH)
}

fn canonical_supervision_scenarios() -> Vec<CanonicalSupervisionScenario> {
    let unsupported = String::from(
        "I can currently answer only provider readiness and wallet balance questions.",
    );

    vec![
        CanonicalSupervisionScenario {
            fixture_name: "openagents_provider_ready_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_001,
            user_request: "Can I go online right now?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["supported", "provider", "training"],
            operator_note: "Training success row for the narrow provider-ready path.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_provider_blocked_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_002,
            user_request: "Why can't I go online yet?",
            runtime_state: CompiledAgentRuntimeState {
                provider_ready: false,
                provider_blockers: vec![String::from("identity_verification")],
                ..CompiledAgentRuntimeState::default()
            },
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is not ready to go online."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is not ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["supported", "provider", "blocked", "training"],
            operator_note: "Training success row for provider-blocked grounding without widening into blocker summaries.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_provider_readiness_variant_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_003,
            user_request: "Is the provider ready for me to go online?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["supported", "provider", "phrasing_variation", "training"],
            operator_note: "Training success row for a second provider-readiness phrasing.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_004,
            user_request: "How many sats are in the wallet?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "grounded_synthesis_drift",
                "recent_earnings_target",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row showing that the current authority still omits recent earnings on a plain wallet-balance request.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_recent_earnings_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_005,
            user_request: "How many sats are in the wallet, and what are the recent earnings?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "recent_earnings",
                "grounded_synthesis_drift",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for an explicit wallet-plus-earnings request.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_recent_earnings_phrase_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_006,
            user_request: "What are my recent earnings in sats?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "recent_earnings",
                "phrasing_variation",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for a recent-earnings phrasing that still lands on the wallet lane.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_balance_variant_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_006_5,
            user_request: "Tell me the wallet balance.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "phrasing_variation",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for a second wallet-balance phrasing so held-out balance requests are not the only non-question wording in the corpus.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_unsupported_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_007,
            user_request: "Write a poem about GPUs.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["unsupported", "refusal_quality", "training"],
            operator_note: "Training success row for the canonical unsupported refusal.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_unsupported_restart_rig_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_008,
            user_request: "Restart my mining rig.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "refusal_quality",
                "training",
                "provider_unrelated",
            ],
            operator_note: "Training success row for a second unsupported request that stays cleanly refused.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_unsupported_calendar_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_008_5,
            user_request: "Create a calendar reminder for tomorrow.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["unsupported", "refusal_quality", "training"],
            operator_note: "Training success row for a calendar-style unsupported request so held-out scheduling prompts are not out-of-vocabulary.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_schedule_call_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_008_6,
            user_request: "Schedule a call for tomorrow afternoon.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["unsupported", "refusal_quality", "training"],
            operator_note: "Training success row for a second scheduling-style unsupported request so the route learner does not regress on held-out meeting language.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_ambiguous_provider_wallet_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_009,
            user_request: "Should I go online or check the wallet balance first?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["unsupported", "route_ambiguity", "training"],
            operator_note: "Training success row for ambiguous provider-versus-wallet phrasing that should refuse instead of guessing.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_provider_compare_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_009_100,
            user_request: "Should I go online first or wait until the wallet grows more?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "confidence_edge",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for provider-versus-wallet comparison phrasing that should refuse instead of drifting toward provider readiness.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wait_wallet_earnings_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_009_150,
            user_request: "Should I wait for more wallet earnings before going online?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "confidence_edge",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for the wait-for-wallet-earnings phrasing so the route learner refuses mixed earnings-versus-go-online planning language instead of collapsing onto provider status.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_provider_wallet_better_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_009_200,
            user_request: "Is it better to check provider readiness or the wallet first?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "confidence_edge",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for mixed provider-versus-wallet planning language that should stay in the admitted refusal lane.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_grow_compare_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_009_250,
            user_request: "Is it better to go online now or wait for the wallet to grow more?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "confidence_edge",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for the go-online-versus-wallet-growth comparison phrasing so the route learner refuses the admitted ambiguity family instead of collapsing onto wallet status.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_negated_wallet_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010,
            user_request: "Do not tell me the wallet balance; write a poem about GPUs.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &["unsupported", "negated", "training", "correction_required"],
            operator_note: "Training correction row for the first negated wallet false positive.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_exclusion_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010_500,
            user_request: "Not provider readiness. Tell me the wallet balance.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "negated",
                "exclusion",
                "shadow_disagreement_seed",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for exclusion wording where the promoted authority still chases provider tokens instead of the requested wallet answer.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_provider_exclusion_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010_600,
            user_request: "Not the wallet balance. Is the provider ready?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "provider",
                "negated",
                "exclusion",
                "shadow_disagreement_seed",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for exclusion wording where the promoted authority still chases wallet tokens instead of the requested provider answer.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_ignore_provider_wallet_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010_650,
            user_request: "Ignore provider readiness. Show me the wallet balance.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "negated",
                "exclusion",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for an ignore-provider phrasing so exclusion handling does not depend on the earlier retained wording.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_address_unsupported_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010_700,
            user_request: "What is the wallet address?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "wallet",
                "unsupported_wallet_question",
                "shadow_disagreement_seed",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for an unsupported wallet-adjacent question that should refuse instead of pretending the balance answered it.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_best_provider_unsupported_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010_800,
            user_request: "Who is the best provider for me?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "provider",
                "unsupported_provider_question",
                "shadow_disagreement_seed",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for a provider-selection question that stays outside the admitted readiness lane.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_recent_earnings_short_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_010_900,
            user_request: "How many sats did I make lately?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "wallet",
                "recent_earnings",
                "grounded_synthesis_drift",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for a short recent-earnings phrasing so grounded synthesis does not depend on the longer retained wording.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_go_online_guess_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_011_000,
            user_request: "I guess I can go online now?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "supported",
                "provider",
                "phrasing_variation",
                "confidence_edge",
                "training",
            ],
            operator_note: "Training success row for a soft-confidence provider phrasing that still belongs in the admitted readiness lane.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_provider_account_ready_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_011,
            user_request: "Is my provider account ready?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &["supported", "provider", "phrasing_variation", "held_out"],
            operator_note: "Held-out provider-readiness phrasing for route and grounding validation.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_balance_phrase_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_012,
            user_request: "Show me the wallet balance.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "supported",
                "wallet",
                "phrasing_variation",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out wallet phrasing to measure whether grounded-answer gains generalize past the replay rows.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_ambiguous_provider_wallet_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_013,
            user_request: "Should I check provider status or the wallet balance first?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &["unsupported", "route_ambiguity", "held_out"],
            operator_note: "Held-out ambiguity row for route behavior that should still refuse instead of guessing.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_provider_wait_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_013_100,
            user_request: "Should I wait for more wallet earnings before I go online?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "confidence_edge",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out compare row for deciding whether the route candidate learned to refuse provider-versus-wallet planning language instead of picking a side.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_negated_provider_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_014,
            user_request: "Do not tell me if I can go online; write a poem about GPUs.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported.clone(),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &["unsupported", "negated", "held_out", "correction_required"],
            operator_note: "Held-out negated provider row mirroring the wallet false-positive class without sharing the exact replay wording.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_earnings_phrase_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_015,
            user_request: "Give me my wallet balance plus recent earnings.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "supported",
                "wallet",
                "recent_earnings",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out wallet-plus-earnings row for testing generalization beyond the replay bundle.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_unsupported_schedule_meeting_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_016,
            user_request: "Schedule a meeting for tomorrow.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: unsupported,
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &["unsupported", "refusal_quality", "held_out"],
            operator_note: "Held-out unsupported request to keep refusal quality measured away from the replay bundle.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_address_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_017,
            user_request: "Do I have a wallet address I can copy?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "wallet",
                "unsupported_wallet_question",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out unsupported wallet-adjacent row for checking that the route model learned the refusal boundary beyond the replay wording.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_best_provider_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_018,
            user_request: "Which provider should I choose to go online?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "provider",
                "unsupported_provider_question",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out provider-selection row for checking that the readiness route does not overclaim provider-advice coverage.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_negated_wallet_provider_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_019,
            user_request: "Do not check the wallet balance. Tell me if the provider is ready.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "supported",
                "provider",
                "negated",
                "exclusion",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out negated provider row to test whether route learning generalizes past the exact exclusion wording in replay.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_negated_provider_wallet_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_020,
            user_request: "Do not tell me provider status. Give me the wallet balance.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "supported",
                "wallet",
                "negated",
                "exclusion",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out negated wallet row to test whether route learning generalizes past the exact exclusion wording in replay.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_ignore_wallet_provider_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_020_100,
            user_request: "Ignore the wallet balance. Tell me if the provider is ready.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::ProviderStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "supported",
                "provider",
                "negated",
                "exclusion",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out ignore-wallet phrasing for checking that the route candidate generalizes the provider exclusion fix beyond the replay rows.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_recent_earnings_short_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_021,
            user_request: "How many sats have I earned recently in the wallet?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::WalletStatus,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from(
                    "Wallet balance is 1200 sats, with 240 sats of recent earnings.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "supported",
                "wallet",
                "recent_earnings",
                "grounded_synthesis_drift",
                "held_out",
                "correction_required",
            ],
            operator_note: "Held-out recent-earnings drift row for checking that the learned grounded-answer program generalizes beyond the replay phrasings.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_provider_compare_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022,
            user_request: "Should I go online first or wait until the wallet grows?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::Unsupported,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &["unsupported", "route_ambiguity", "confidence_edge", "held_out"],
            operator_note: "Held-out ambiguity row for confidence calibration around mixed provider-versus-wallet intent without widening the task family.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_wallet_provider_better_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_100,
            user_request: "Is it better to go online now or keep waiting for the wallet to grow?",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "confidence_edge",
                "held_out",
                "correction_required",
            ],
            operator_note: "Second held-out compare row for the same narrow ambiguity family so route promotion depends on a real pattern and not one lucky wording.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_ignore_wallet_compare_provider_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_200,
            user_request: "Ignore the wallet balance and compare provider readiness with recent earnings.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "compare_exclusion",
                "confidence_edge",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for compare-plus-exclusion phrasing so route learning does not collapse onto provider readiness when the ask is still outside the admitted family.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_without_provider_compare_wallet_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_300,
            user_request: "Without provider readiness, compare the wallet balance with going online now.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "compare_exclusion",
                "confidence_edge",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for the inverse compare-plus-exclusion phrasing so mixed provider-versus-wallet language is still refused instead of drifting toward wallet status.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_dont_compare_wallet_provider_poem_training_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_400,
            user_request: "Don't compare the wallet and provider status; write a poem instead.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::Training,
            tags: &[
                "unsupported",
                "negated",
                "route_ambiguity",
                "compare_exclusion",
                "training",
                "correction_required",
            ],
            operator_note: "Training correction row for a negated compare-plus-poem ask so route promotion depends on retaining the unsupported boundary under mixed negation and comparison language.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_ignore_wallet_compare_provider_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_500,
            user_request: "Ignore wallet balance and compare provider status with recent earnings first.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "compare_exclusion",
                "held_out",
                "correction_required",
            ],
            operator_note: "Permanent held-out trap for compare-plus-exclusion phrasing on the provider-first surface.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_without_provider_compare_wallet_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_600,
            user_request: "Without provider readiness, compare wallet earnings with going online now.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::WalletStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("The wallet contains 1200 sats."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "route_ambiguity",
                "compare_exclusion",
                "held_out",
                "correction_required",
            ],
            operator_note: "Permanent held-out trap for compare-plus-exclusion phrasing on the wallet-first surface.",
        },
        CanonicalSupervisionScenario {
            fixture_name: "openagents_dont_compare_wallet_provider_poem_heldout_receipt_v1.json",
            captured_at_epoch_ms: 1_774_760_262_022_700,
            user_request: "Don't compare wallet balance and provider status; write me a poem.",
            runtime_state: CompiledAgentRuntimeState::default(),
            observed_route: CompiledAgentRoute::ProviderStatus,
            observed_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                response: String::from("Provider is ready to go online."),
            },
            expected_route: CompiledAgentRoute::Unsupported,
            expected_public_response: CompiledAgentLearningPublicResponse {
                kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
                response: String::from(
                    "I can currently answer only provider readiness and wallet balance questions.",
                ),
            },
            corpus_split: CompiledAgentCorpusSplit::HeldOut,
            tags: &[
                "unsupported",
                "negated",
                "route_ambiguity",
                "compare_exclusion",
                "held_out",
                "correction_required",
            ],
            operator_note: "Permanent held-out trap for mixed negation-plus-comparison language so route promotion fails closed if the unsupported boundary re-breaks.",
        },
    ]
}

fn learning_label_from_scenario(
    scenario: &CanonicalSupervisionScenario,
) -> CompiledAgentGovernedReceiptLabel {
    CompiledAgentGovernedReceiptLabel {
        expected_route: scenario.expected_route,
        expected_public_response: scenario.expected_public_response.clone(),
        corpus_split: scenario.corpus_split,
        tags: scenario.tags.iter().map(|tag| (*tag).to_string()).collect(),
        operator_note: scenario.operator_note.to_string(),
    }
}

fn canonical_runtime_receipt_scenarios() -> Vec<CanonicalRuntimeReceiptScenario> {
    let unsupported = CompiledAgentLearningPublicResponse {
        kind: CompiledAgentPublicOutcomeKind::UnsupportedRefusal,
        response: String::from(
            "I can currently answer only provider readiness and wallet balance questions.",
        ),
    };
    let wallet_grounded = CompiledAgentLearningPublicResponse {
        kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
        response: String::from("Wallet balance is 1200 sats, with 240 sats of recent earnings."),
    };

    vec![
        CanonicalRuntimeReceiptScenario {
            fixture_name: "openagents_runtime_shadow_compare_receipt_v1.json",
            source_receipt: build_runtime_source_receipt(
                "openagents_runtime_shadow_compare_receipt_v1.json",
                1_774_760_262_101,
                "Do not tell me provider readiness. Compare the wallet and provider situation.",
                CompiledAgentRuntimeState::default(),
                CompiledAgentRoute::ProviderStatus,
                CompiledAgentLearningPublicResponse {
                    kind: CompiledAgentPublicOutcomeKind::GroundedAnswer,
                    response: String::from("Provider is ready to go online."),
                },
                Some(CompiledAgentRoute::Unsupported),
                Some(unsupported.clone()),
            ),
            label: CompiledAgentGovernedReceiptLabel {
                expected_route: CompiledAgentRoute::Unsupported,
                expected_public_response: unsupported.clone(),
                corpus_split: CompiledAgentCorpusSplit::Training,
                tags: vec![
                    String::from("runtime"),
                    String::from("provider"),
                    String::from("wallet"),
                    String::from("route_ambiguity"),
                    String::from("shadow_disagreement"),
                    String::from("correction_required"),
                    String::from("unsupported"),
                    String::from("training"),
                ],
                operator_note: String::from(
                    "Sanitized runtime receipt from admitted-family traffic. Promoted authority answered provider readiness, the shadow route candidate refused, and operator review kept the comparison request in the unsupported lane.",
                ),
            },
        },
        CanonicalRuntimeReceiptScenario {
            fixture_name: "openagents_runtime_wallet_recent_earnings_receipt_v1.json",
            source_receipt: build_runtime_source_receipt(
                "openagents_runtime_wallet_recent_earnings_receipt_v1.json",
                1_774_760_262_102,
                "What are the wallet balance and recent earnings right now?",
                CompiledAgentRuntimeState::default(),
                CompiledAgentRoute::WalletStatus,
                wallet_grounded.clone(),
                Some(CompiledAgentRoute::WalletStatus),
                Some(wallet_grounded.clone()),
            ),
            label: CompiledAgentGovernedReceiptLabel {
                expected_route: CompiledAgentRoute::WalletStatus,
                expected_public_response: wallet_grounded,
                corpus_split: CompiledAgentCorpusSplit::HeldOut,
                tags: vec![
                    String::from("runtime"),
                    String::from("wallet"),
                    String::from("recent_earnings"),
                    String::from("held_out"),
                    String::from("supported"),
                ],
                operator_note: String::from(
                    "Sanitized runtime receipt for a supported wallet-plus-recent-earnings request. This row proves real admitted-family runtime evidence can enter the held-out validator split without changing the receipt contract.",
                ),
            },
        },
    ]
}

fn canonical_compiled_agent_governed_source_receipts() -> Vec<CompiledAgentGovernedSourceReceipt> {
    let mut receipts = canonical_supervision_scenarios()
        .into_iter()
        .map(|scenario| CompiledAgentGovernedSourceReceipt {
            source_fixture_ref: format!(
                "{COMPILED_AGENT_SOURCE_FIXTURE_DIR}/{}",
                scenario.fixture_name
            ),
            source_receipt: build_source_receipt(&scenario),
            label: learning_label_from_scenario(&scenario),
        })
        .collect::<Vec<_>>();
    receipts.extend(
        canonical_runtime_receipt_scenarios()
            .into_iter()
            .map(|scenario| CompiledAgentGovernedSourceReceipt {
                source_fixture_ref: format!(
                    "{COMPILED_AGENT_RUNTIME_FIXTURE_DIR}/{}",
                    scenario.fixture_name
                ),
                source_receipt: scenario.source_receipt,
                label: scenario.label,
            }),
    );
    receipts
}

pub fn canonical_compiled_agent_governed_source_receipt(
    fixture_name: &str,
) -> Result<CompiledAgentGovernedSourceReceipt, CompiledAgentReceiptError> {
    canonical_compiled_agent_governed_source_receipts()
        .into_iter()
        .find(|receipt| {
            receipt.source_fixture_ref.ends_with(fixture_name)
                || receipt.source_fixture_ref == fixture_name
        })
        .ok_or_else(|| CompiledAgentReceiptError::Read {
            path: fixture_name.to_string(),
            error: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "canonical compiled-agent governed receipt fixture was not found",
            ),
        })
}

pub fn load_compiled_agent_source_receipt(
    path: impl AsRef<Path>,
) -> Result<CompiledAgentSourceReceipt, CompiledAgentReceiptError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| CompiledAgentReceiptError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[must_use]
pub fn canonical_compiled_agent_source_receipts() -> Vec<(String, CompiledAgentSourceReceipt)> {
    canonical_supervision_scenarios()
        .into_iter()
        .map(|scenario| {
            (
                scenario.fixture_name.to_string(),
                build_source_receipt(&scenario),
            )
        })
        .collect()
}

#[must_use]
pub fn canonical_compiled_agent_runtime_source_receipts(
) -> Vec<(String, CompiledAgentSourceReceipt)> {
    canonical_runtime_receipt_scenarios()
        .into_iter()
        .map(|scenario| (scenario.fixture_name.to_string(), scenario.source_receipt))
        .collect()
}

pub fn write_compiled_agent_source_receipts(
    output_dir: impl AsRef<Path>,
) -> Result<Vec<(String, CompiledAgentSourceReceipt)>, CompiledAgentReceiptError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| CompiledAgentReceiptError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;
    let receipts = canonical_compiled_agent_source_receipts();
    for (fixture_name, receipt) in &receipts {
        let output_path = output_dir.join(fixture_name);
        let json = serde_json::to_string_pretty(receipt)?;
        fs::write(&output_path, format!("{json}\n")).map_err(|error| {
            CompiledAgentReceiptError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })?;
    }
    Ok(receipts)
}

pub fn write_compiled_agent_runtime_receipts(
    output_dir: impl AsRef<Path>,
) -> Result<Vec<(String, CompiledAgentSourceReceipt)>, CompiledAgentReceiptError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| CompiledAgentReceiptError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;
    let receipts = canonical_compiled_agent_runtime_source_receipts();
    for (fixture_name, receipt) in &receipts {
        let output_path = output_dir.join(fixture_name);
        let json = serde_json::to_string_pretty(receipt)?;
        fs::write(&output_path, format!("{json}\n")).map_err(|error| {
            CompiledAgentReceiptError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })?;
    }
    Ok(receipts)
}

pub fn canonical_compiled_agent_learning_receipt_ledger(
) -> Result<CompiledAgentLearningReceiptLedger, CompiledAgentReceiptError> {
    let mut receipts = Vec::new();
    for governed in canonical_compiled_agent_governed_source_receipts() {
        receipts.push(build_learning_receipt(
            &governed.source_fixture_ref,
            &governed.source_receipt,
            &governed.label,
        )?);
    }
    build_learning_receipt_ledger(
        receipts,
        &compiled_agent_baseline_revision_set().revision_id,
    )
}

pub fn build_compiled_agent_learning_receipt_from_source(
    source_fixture_ref: &str,
    source_receipt: &CompiledAgentSourceReceipt,
    label: &CompiledAgentReceiptSupervisionLabel,
) -> Result<CompiledAgentLearningReceipt, CompiledAgentReceiptError> {
    build_learning_receipt(source_fixture_ref, source_receipt, label)
}

pub fn build_compiled_agent_learning_receipt_ledger_from_receipts(
    receipts: Vec<CompiledAgentLearningReceipt>,
    baseline_revision_id: &str,
) -> Result<CompiledAgentLearningReceiptLedger, CompiledAgentReceiptError> {
    build_learning_receipt_ledger(receipts, baseline_revision_id)
}

pub fn build_compiled_agent_replay_bundle_from_ledger(
    ledger: &CompiledAgentLearningReceiptLedger,
) -> Result<CompiledAgentReplayBundle, CompiledAgentReceiptError> {
    build_compiled_agent_replay_bundle(ledger)
}

pub fn canonical_compiled_agent_replay_bundle(
) -> Result<CompiledAgentReplayBundle, CompiledAgentReceiptError> {
    let ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    build_compiled_agent_replay_bundle(&ledger)
}

pub fn write_compiled_agent_learning_receipt_ledger(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentLearningReceiptLedger, CompiledAgentReceiptError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentReceiptError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let json = serde_json::to_string_pretty(&ledger)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentReceiptError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(ledger)
}

pub fn write_compiled_agent_replay_bundle(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentReplayBundle, CompiledAgentReceiptError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CompiledAgentReceiptError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = canonical_compiled_agent_replay_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentReceiptError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn verify_compiled_agent_learning_receipt_fixtures() -> Result<(), CompiledAgentReceiptError> {
    for (fixture_name, expected_source) in canonical_compiled_agent_source_receipts() {
        let source_path = compiled_agent_source_fixture_dir().join(&fixture_name);
        let committed_source = load_compiled_agent_source_receipt(&source_path)?;
        if committed_source != expected_source {
            return Err(CompiledAgentReceiptError::FixtureDrift {
                path: source_path.display().to_string(),
            });
        }
    }
    for (fixture_name, expected_source) in canonical_compiled_agent_runtime_source_receipts() {
        let source_path = compiled_agent_runtime_fixture_dir().join(&fixture_name);
        let committed_source = load_compiled_agent_source_receipt(&source_path)?;
        if committed_source != expected_source {
            return Err(CompiledAgentReceiptError::FixtureDrift {
                path: source_path.display().to_string(),
            });
        }
    }

    let expected_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let expected_bundle = build_compiled_agent_replay_bundle(&expected_ledger)?;

    let committed_ledger_bytes = fs::read(compiled_agent_learning_receipt_ledger_fixture_path())
        .map_err(|error| CompiledAgentReceiptError::Read {
            path: compiled_agent_learning_receipt_ledger_fixture_path()
                .display()
                .to_string(),
            error,
        })?;
    let committed_ledger: CompiledAgentLearningReceiptLedger =
        serde_json::from_slice(&committed_ledger_bytes)?;
    if committed_ledger != expected_ledger {
        return Err(CompiledAgentReceiptError::FixtureDrift {
            path: compiled_agent_learning_receipt_ledger_fixture_path()
                .display()
                .to_string(),
        });
    }

    let committed_bundle_bytes =
        fs::read(compiled_agent_replay_bundle_fixture_path()).map_err(|error| {
            CompiledAgentReceiptError::Read {
                path: compiled_agent_replay_bundle_fixture_path()
                    .display()
                    .to_string(),
                error,
            }
        })?;
    let committed_bundle: CompiledAgentReplayBundle =
        serde_json::from_slice(&committed_bundle_bytes)?;
    if committed_bundle != expected_bundle {
        return Err(CompiledAgentReceiptError::FixtureDrift {
            path: compiled_agent_replay_bundle_fixture_path()
                .display()
                .to_string(),
        });
    }

    Ok(())
}

fn build_source_receipt(scenario: &CanonicalSupervisionScenario) -> CompiledAgentSourceReceipt {
    let observed_tool_calls = expected_tool_calls(scenario.observed_route);
    let observed_tool_results =
        tool_results_for_route(scenario.observed_route, &scenario.runtime_state);

    let route_manifest = manifest(
        "intent_route",
        "intent_route",
        "psionic_route_model",
        "compiled_agent.route.multinomial_nb_v1",
        "2026-03-29",
        0.8,
    );
    let tool_policy_manifest = manifest(
        "tool_policy",
        "tool_policy",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.8,
    );
    let tool_arguments_manifest = manifest(
        "tool_arguments",
        "tool_arguments",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.8,
    );
    let grounded_answer_manifest = manifest(
        "grounded_answer",
        "grounded_answer",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.82,
    );
    let verify_manifest = manifest(
        "verify",
        "verify",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.82,
    );
    let selected_tools = expected_tool_names(scenario.observed_route)
        .into_iter()
        .map(|tool_name| {
            json!({
                "name": tool_name,
                "description": tool_description(tool_name.as_str()),
            })
        })
        .collect::<Vec<_>>();
    let verify_verdict = if scenario.observed_public_response.kind
        == CompiledAgentPublicOutcomeKind::UnsupportedRefusal
    {
        "unsupported_refusal"
    } else {
        "accept_grounded_answer"
    };

    let primary_phases = vec![
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("intent_route"),
            manifest: route_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({ "user_request": scenario.user_request }),
            output: json!({
                "route": scenario.observed_route,
                "rationale": format!(
                    "captured authority route for {}",
                    fixture_slug(scenario.fixture_name)
                ),
            }),
            confidence: route_confidence(scenario),
            trace: json!({
                "artifact_id": "compiled_agent.route.multinomial_nb_v1",
                "captured_from_runtime": true,
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("tool_policy"),
            manifest: tool_policy_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": scenario.user_request,
                "route": scenario.observed_route,
                "available_tools": compiled_agent_supported_tools(),
            }),
            output: json!({
                "selected_tools": &selected_tools,
                "rationale": "captured authority tool policy",
            }),
            confidence: 0.92,
            trace: json!({
                "route": scenario.observed_route,
                "captured_from_runtime": true,
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("tool_arguments"),
            manifest: tool_arguments_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": scenario.user_request,
                "route": scenario.observed_route,
                "selected_tools": &selected_tools,
            }),
            output: json!({
                "calls": &observed_tool_calls,
            }),
            confidence: 0.96,
            trace: json!({
                "tool_count": observed_tool_calls.len(),
                "captured_from_runtime": true,
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("grounded_answer"),
            manifest: grounded_answer_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": scenario.user_request,
                "route": scenario.observed_route,
                "tool_results": &observed_tool_results,
            }),
            output: json!({
                "answer": scenario.observed_public_response.response,
                "grounded_tool_names": observed_tool_calls
                    .iter()
                    .map(|call| call.tool_name.clone())
                    .collect::<Vec<_>>(),
            }),
            confidence: grounded_confidence(scenario),
            trace: json!({
                "captured_from_runtime": true,
                "response_kind": scenario.observed_public_response.kind,
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("verify"),
            manifest: verify_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": scenario.user_request,
                "route": scenario.observed_route,
                "tool_calls": &observed_tool_calls,
                "tool_results": &observed_tool_results,
                "candidate_answer": scenario.observed_public_response.response,
            }),
            output: json!({
                "verdict": verify_verdict,
                "rationale": "captured authority verify verdict",
            }),
            confidence: verify_confidence(scenario),
            trace: json!({
                "captured_from_runtime": true,
            }),
        },
    ];

    let authority_manifest_ids = vec![
        route_manifest.manifest_id(),
        tool_policy_manifest.manifest_id(),
        tool_arguments_manifest.manifest_id(),
        grounded_answer_manifest.manifest_id(),
        verify_manifest.manifest_id(),
    ];

    CompiledAgentSourceReceipt {
        schema_version: 1,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        captured_at_epoch_ms: scenario.captured_at_epoch_ms,
        state: scenario.runtime_state.clone(),
        run: CompiledAgentSourceRun {
            public_response: CompiledAgentSourcePublicResponse {
                kind: scenario.observed_public_response.kind,
                response: scenario.observed_public_response.response.clone(),
            },
            internal_trace: CompiledAgentSourceInternalTrace {
                primary_phases,
                shadow_phases: Vec::new(),
            },
            lineage: CompiledAgentSourceLineage {
                user_request: scenario.user_request.to_string(),
                route: scenario.observed_route,
                tool_calls: observed_tool_calls,
                tool_results: observed_tool_results,
                public_response: CompiledAgentSourcePublicResponse {
                    kind: scenario.observed_public_response.kind,
                    response: scenario.observed_public_response.response.clone(),
                },
                authority_manifest_ids,
                shadow_manifest_ids: Vec::new(),
            },
        },
    }
}

fn build_runtime_source_receipt(
    fixture_name: &str,
    captured_at_epoch_ms: u64,
    user_request: &str,
    runtime_state: CompiledAgentRuntimeState,
    observed_route: CompiledAgentRoute,
    observed_public_response: CompiledAgentLearningPublicResponse,
    shadow_route: Option<CompiledAgentRoute>,
    shadow_public_response: Option<CompiledAgentLearningPublicResponse>,
) -> CompiledAgentSourceReceipt {
    let observed_tool_calls = expected_tool_calls(observed_route);
    let observed_tool_results = tool_results_for_route(observed_route, &runtime_state);

    let route_manifest = manifest_with_state(
        "intent_route",
        "intent_route",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        "promoted",
        0.8,
    );
    let tool_policy_manifest = manifest(
        "tool_policy",
        "tool_policy",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.8,
    );
    let tool_arguments_manifest = manifest(
        "tool_arguments",
        "tool_arguments",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.8,
    );
    let grounded_answer_manifest = manifest_with_state(
        "grounded_answer",
        "grounded_answer",
        "psionic_grounded_model",
        "promoted",
        "2026-03-29",
        "promoted",
        0.82,
    );
    let verify_manifest = manifest(
        "verify",
        "verify",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        0.82,
    );
    let selected_tools = expected_tool_names(observed_route)
        .into_iter()
        .map(|tool_name| {
            json!({
                "name": tool_name,
                "description": tool_description(tool_name.as_str()),
            })
        })
        .collect::<Vec<_>>();
    let verify_verdict =
        if observed_public_response.kind == CompiledAgentPublicOutcomeKind::UnsupportedRefusal {
            "unsupported_refusal"
        } else {
            "accept_grounded_answer"
        };

    let primary_phases = vec![
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("intent_route"),
            manifest: route_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({ "user_request": user_request }),
            output: json!({
                "route": observed_route,
                "rationale": format!(
                    "captured promoted authority route for {}",
                    fixture_slug(fixture_name)
                ),
            }),
            confidence: 0.83,
            trace: json!({
                "artifact_id": "compiled_agent.baseline.rule_v1.route",
                "captured_from_runtime": true,
                "authority_path": "promoted_contract",
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("tool_policy"),
            manifest: tool_policy_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": user_request,
                "route": observed_route,
                "available_tools": compiled_agent_supported_tools(),
            }),
            output: json!({
                "selected_tools": &selected_tools,
                "rationale": "captured promoted tool policy from runtime",
            }),
            confidence: 0.91,
            trace: json!({
                "captured_from_runtime": true,
                "authority_path": "promoted_contract",
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("tool_arguments"),
            manifest: tool_arguments_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": user_request,
                "route": observed_route,
                "selected_tools": &selected_tools,
            }),
            output: json!({
                "calls": &observed_tool_calls,
            }),
            confidence: 0.95,
            trace: json!({
                "tool_count": observed_tool_calls.len(),
                "captured_from_runtime": true,
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("grounded_answer"),
            manifest: grounded_answer_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": user_request,
                "route": observed_route,
                "tool_results": &observed_tool_results,
            }),
            output: json!({
                "answer": observed_public_response.response,
                "grounded_tool_names": observed_tool_calls
                    .iter()
                    .map(|call| call.tool_name.clone())
                    .collect::<Vec<_>>(),
            }),
            confidence: 0.88,
            trace: json!({
                "artifact_id": "compiled_agent.grounded_answer.multinomial_nb_v1",
                "captured_from_runtime": true,
                "response_kind": observed_public_response.kind,
            }),
        },
        CompiledAgentSourcePhaseTraceEntry {
            phase: String::from("verify"),
            manifest: verify_manifest.clone(),
            authority: String::from("promoted"),
            candidate_label: None,
            input: json!({
                "user_request": user_request,
                "route": observed_route,
                "tool_calls": &observed_tool_calls,
                "tool_results": &observed_tool_results,
                "candidate_answer": observed_public_response.response,
            }),
            output: json!({
                "verdict": verify_verdict,
                "rationale": "captured promoted verify verdict from runtime",
            }),
            confidence: 0.9,
            trace: json!({
                "captured_from_runtime": true,
                "authority_path": "promoted_contract",
            }),
        },
    ];

    let mut shadow_phases = Vec::new();
    if let (Some(shadow_route), Some(shadow_public_response)) =
        (shadow_route, shadow_public_response)
    {
        let shadow_tool_results = tool_results_for_route(shadow_route, &runtime_state);
        let shadow_route_manifest = manifest_with_state(
            "intent_route",
            "intent_route",
            "psionic_route_model",
            "compiled_agent.route.multinomial_nb_v1",
            "2026-03-29",
            "candidate",
            0.8,
        );
        let shadow_grounded_manifest = manifest_with_state(
            "grounded_answer",
            "grounded_answer",
            "psionic_grounded_model",
            "psionic_candidate",
            "2026-03-29",
            "candidate",
            0.82,
        );
        let shadow_verify_manifest = manifest_with_state(
            "verify",
            "verify",
            "psionic_grounded_model",
            "psionic_candidate",
            "2026-03-29",
            "candidate",
            0.82,
        );
        shadow_phases = vec![
            CompiledAgentSourcePhaseTraceEntry {
                phase: String::from("intent_route"),
                manifest: shadow_route_manifest.clone(),
                authority: String::from("shadow"),
                candidate_label: Some(String::from("psionic_candidate")),
                input: json!({ "user_request": user_request }),
                output: json!({
                    "route": shadow_route,
                    "rationale": format!(
                        "captured shadow route compare for {}",
                        fixture_slug(fixture_name)
                    ),
                }),
                confidence: 0.79,
                trace: json!({
                    "artifact_id": "compiled_agent.route.multinomial_nb_v1",
                    "captured_from_runtime": true,
                    "shadow_compare": true,
                }),
            },
            CompiledAgentSourcePhaseTraceEntry {
                phase: String::from("grounded_answer"),
                manifest: shadow_grounded_manifest.clone(),
                authority: String::from("shadow"),
                candidate_label: Some(String::from("psionic_candidate")),
                input: json!({
                    "user_request": user_request,
                    "route": shadow_route,
                    "tool_results": &shadow_tool_results,
                }),
                output: json!({
                    "answer": shadow_public_response.response,
                    "response_kind": shadow_public_response.kind,
                }),
                confidence: 0.78,
                trace: json!({
                    "artifact_id": "compiled_agent.grounded_answer.multinomial_nb_v1",
                    "captured_from_runtime": true,
                    "shadow_compare": true,
                }),
            },
            CompiledAgentSourcePhaseTraceEntry {
                phase: String::from("verify"),
                manifest: shadow_verify_manifest.clone(),
                authority: String::from("shadow"),
                candidate_label: Some(String::from("psionic_candidate")),
                input: json!({
                    "user_request": user_request,
                    "route": shadow_route,
                    "tool_results": &shadow_tool_results,
                    "candidate_answer": shadow_public_response.response,
                }),
                output: json!({
                    "verdict": if shadow_public_response.kind == CompiledAgentPublicOutcomeKind::UnsupportedRefusal {
                        "unsupported_refusal"
                    } else {
                        "accept_grounded_answer"
                    },
                    "rationale": "captured shadow verify verdict from runtime",
                }),
                confidence: 0.77,
                trace: json!({
                    "captured_from_runtime": true,
                    "shadow_compare": true,
                }),
            },
        ];
    }

    let authority_manifest_ids = vec![
        route_manifest.manifest_id(),
        tool_policy_manifest.manifest_id(),
        tool_arguments_manifest.manifest_id(),
        grounded_answer_manifest.manifest_id(),
        verify_manifest.manifest_id(),
    ];
    let shadow_manifest_ids = shadow_phases
        .iter()
        .map(|phase| phase.manifest.manifest_id())
        .collect::<Vec<_>>();

    CompiledAgentSourceReceipt {
        schema_version: 1,
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        captured_at_epoch_ms,
        state: runtime_state,
        run: CompiledAgentSourceRun {
            public_response: CompiledAgentSourcePublicResponse {
                kind: observed_public_response.kind,
                response: observed_public_response.response.clone(),
            },
            internal_trace: CompiledAgentSourceInternalTrace {
                primary_phases,
                shadow_phases,
            },
            lineage: CompiledAgentSourceLineage {
                user_request: user_request.to_string(),
                route: observed_route,
                tool_calls: observed_tool_calls,
                tool_results: observed_tool_results,
                public_response: CompiledAgentSourcePublicResponse {
                    kind: observed_public_response.kind,
                    response: observed_public_response.response,
                },
                authority_manifest_ids,
                shadow_manifest_ids,
            },
        },
    }
}

fn manifest(
    module_name: &str,
    signature_name: &str,
    implementation_family: &str,
    implementation_label: &str,
    version: &str,
    confidence_floor: f32,
) -> CompiledAgentSourceManifest {
    manifest_with_state(
        module_name,
        signature_name,
        implementation_family,
        implementation_label,
        version,
        "promoted",
        confidence_floor,
    )
}

fn manifest_with_state(
    module_name: &str,
    signature_name: &str,
    implementation_family: &str,
    implementation_label: &str,
    version: &str,
    promotion_state: &str,
    confidence_floor: f32,
) -> CompiledAgentSourceManifest {
    CompiledAgentSourceManifest {
        module_name: module_name.to_string(),
        signature_name: signature_name.to_string(),
        implementation_family: implementation_family.to_string(),
        implementation_label: implementation_label.to_string(),
        version: version.to_string(),
        promotion_state: promotion_state.to_string(),
        confidence_floor,
    }
}

fn tool_description(tool_name: &str) -> &'static str {
    match tool_name {
        "provider_status" => "Read provider readiness and blocker state.",
        "wallet_status" => "Read wallet balance and recent earnings.",
        _ => "Unknown tool.",
    }
}

fn expected_tool_calls(route: CompiledAgentRoute) -> Vec<CompiledAgentToolCall> {
    expected_tool_names(route)
        .into_iter()
        .map(|tool_name| CompiledAgentToolCall {
            tool_name,
            arguments: json!({}),
        })
        .collect()
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

fn route_confidence(scenario: &CanonicalSupervisionScenario) -> f32 {
    if scenario.tags.contains(&"negated") {
        0.86
    } else if scenario.tags.contains(&"route_ambiguity") {
        0.84
    } else {
        0.94
    }
}

fn grounded_confidence(scenario: &CanonicalSupervisionScenario) -> f32 {
    if scenario.observed_route == CompiledAgentRoute::WalletStatus
        && scenario
            .observed_public_response
            .response
            .contains("The wallet contains")
    {
        0.91
    } else {
        0.94
    }
}

fn verify_confidence(scenario: &CanonicalSupervisionScenario) -> f32 {
    if scenario.tags.contains(&"correction_required") {
        0.89
    } else {
        0.94
    }
}

fn build_learning_receipt(
    source_fixture_ref: &str,
    source_receipt: &CompiledAgentSourceReceipt,
    label: &CompiledAgentGovernedReceiptLabel,
) -> Result<CompiledAgentLearningReceipt, CompiledAgentReceiptError> {
    let observed_route = source_receipt.run.lineage.route;
    let expected_route = label.expected_route;
    let expected_response = label.expected_public_response.clone();
    let observed_response = CompiledAgentLearningPublicResponse {
        kind: source_receipt.run.public_response.kind,
        response: source_receipt.run.public_response.response.clone(),
    };
    let observed_tool_names = source_receipt
        .run
        .lineage
        .tool_calls
        .iter()
        .map(|call| call.tool_name.clone())
        .collect::<Vec<_>>();
    let expected_tool_names = expected_tool_names(expected_route);
    let route_correct = observed_route == expected_route;
    let tool_policy_correct = observed_tool_names == expected_tool_names;
    let tool_arguments_correct = tool_arguments_match(
        source_receipt.run.lineage.tool_calls.as_slice(),
        expected_tool_names.as_slice(),
    );
    let grounded_answer_correct = observed_response == expected_response;
    let verify_correct = verify_matches_expected(
        expected_route,
        observed_tool_names.as_slice(),
        observed_response.kind,
        &expected_response,
        &observed_response,
    );
    let mut failure_classes = Vec::new();
    if !route_correct {
        failure_classes.push(route_failure_class(
            &source_receipt.run.lineage.user_request,
            observed_route,
            expected_route,
        ));
    }
    if !tool_policy_correct {
        failure_classes.push(String::from("unexpected_tool_exposure"));
    }
    if !tool_arguments_correct {
        failure_classes.push(String::from("tool_argument_mismatch"));
    }
    if !grounded_answer_correct {
        failure_classes.push(String::from("grounded_answer_mismatch"));
    }
    if !verify_correct {
        failure_classes.push(String::from("unsafe_final_outcome"));
    }
    failure_classes.sort();
    failure_classes.dedup();
    let primary_phase_confidences = source_receipt
        .run
        .internal_trace
        .primary_phases
        .iter()
        .map(|phase| (phase.phase.clone(), phase.confidence))
        .collect::<BTreeMap<_, _>>();
    let receipt_id = format!(
        "receipt.compiled_agent.learning.{}",
        fixture_slug(source_fixture_ref)
    );
    let source_receipt_digest = stable_digest(b"compiled_agent_source_receipt|", source_receipt);
    let mut receipt = CompiledAgentLearningReceipt {
        receipt_id,
        source_fixture_ref: source_fixture_ref.to_string(),
        source_receipt_digest,
        evidence_class: source_receipt.evidence_class,
        captured_at_epoch_ms: source_receipt.captured_at_epoch_ms,
        user_request: source_receipt.run.lineage.user_request.clone(),
        runtime_state: source_receipt.state.clone(),
        observed_route,
        expected_route,
        observed_tool_names,
        expected_tool_names,
        observed_tool_calls: source_receipt.run.lineage.tool_calls.clone(),
        observed_tool_results: source_receipt.run.lineage.tool_results.clone(),
        observed_public_response: observed_response,
        expected_public_response: expected_response,
        authority_manifest_ids: source_receipt.run.lineage.authority_manifest_ids.clone(),
        shadow_manifest_ids: source_receipt.run.lineage.shadow_manifest_ids.clone(),
        primary_phase_confidences,
        corpus_split: label.corpus_split,
        assessment: CompiledAgentLearningAssessment {
            route_correct,
            tool_policy_correct,
            tool_arguments_correct,
            grounded_answer_correct,
            verify_correct,
            overall_success: route_correct
                && tool_policy_correct
                && tool_arguments_correct
                && grounded_answer_correct
                && verify_correct,
            failure_classes,
        },
        tags: label.tags.clone(),
        operator_note: label.operator_note.clone(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"compiled_agent_learning_receipt|", &receipt);
    Ok(receipt)
}

fn build_learning_receipt_ledger(
    receipts: Vec<CompiledAgentLearningReceipt>,
    baseline_revision_id: &str,
) -> Result<CompiledAgentLearningReceiptLedger, CompiledAgentReceiptError> {
    let evidence_class = expected_receipt_evidence_class(&receipts)?;
    let mut module_failure_counts = BTreeMap::new();
    let mut failure_class_counts = BTreeMap::new();
    let mut split_receipt_counts = BTreeMap::new();
    let mut task_family_counts = BTreeMap::new();
    for receipt in &receipts {
        *split_receipt_counts
            .entry(match receipt.corpus_split {
                CompiledAgentCorpusSplit::Training => String::from("training"),
                CompiledAgentCorpusSplit::HeldOut => String::from("held_out"),
            })
            .or_insert(0) += 1;
        for tag in &receipt.tags {
            if matches!(tag.as_str(), "provider" | "wallet" | "unsupported") {
                *task_family_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
        if !receipt.assessment.route_correct {
            *module_failure_counts
                .entry(String::from("route"))
                .or_insert(0) += 1;
        }
        if !receipt.assessment.tool_policy_correct {
            *module_failure_counts
                .entry(String::from("tool_policy"))
                .or_insert(0) += 1;
        }
        if !receipt.assessment.tool_arguments_correct {
            *module_failure_counts
                .entry(String::from("tool_arguments"))
                .or_insert(0) += 1;
        }
        if !receipt.assessment.grounded_answer_correct {
            *module_failure_counts
                .entry(String::from("grounded_answer"))
                .or_insert(0) += 1;
        }
        if !receipt.assessment.verify_correct {
            *module_failure_counts
                .entry(String::from("verify"))
                .or_insert(0) += 1;
        }
        for failure_class in &receipt.assessment.failure_classes {
            *failure_class_counts
                .entry(failure_class.clone())
                .or_insert(0) += 1;
        }
    }

    let training_receipt_ids = receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::Training)
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let held_out_receipt_ids = receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::HeldOut)
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let correction_receipt_ids = receipts
        .iter()
        .filter(|receipt| !receipt.assessment.overall_success)
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let source_fixture_refs = receipts
        .iter()
        .map(|receipt| receipt.source_fixture_ref.clone())
        .collect::<Vec<_>>();

    let mut ledger = CompiledAgentLearningReceiptLedger {
        schema_version: String::from(LEARNING_RECEIPT_SCHEMA_VERSION),
        ledger_id: String::from("compiled_agent.learning_receipt_ledger.v1"),
        row_id: String::from("compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1"),
        baseline_revision_id: baseline_revision_id.to_string(),
        evidence_class,
        source_fixture_refs,
        training_receipt_ids,
        held_out_receipt_ids,
        correction_receipt_ids,
        split_receipt_counts,
        task_family_counts,
        module_failure_counts,
        failure_class_counts,
        receipts,
        summary: String::new(),
        ledger_digest: String::new(),
    };
    let success_count = ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.assessment.overall_success)
        .count();
    ledger.summary = format!(
        "Compiled-agent learning ledger retains {} source receipts ({} training, {} held-out) with {} fully-correct rows and {} correction rows.",
        ledger.receipts.len(),
        ledger
            .split_receipt_counts
            .get("training")
            .copied()
            .unwrap_or(0),
        ledger
            .split_receipt_counts
            .get("held_out")
            .copied()
            .unwrap_or(0),
        success_count,
        ledger.receipts.len().saturating_sub(success_count),
    );
    ledger.ledger_digest = stable_digest(b"compiled_agent_learning_ledger|", &ledger);
    Ok(ledger)
}

fn build_compiled_agent_replay_bundle(
    ledger: &CompiledAgentLearningReceiptLedger,
) -> Result<CompiledAgentReplayBundle, CompiledAgentReceiptError> {
    let mut samples = Vec::new();
    for receipt in ledger
        .receipts
        .iter()
        .filter(|receipt| receipt.corpus_split == CompiledAgentCorpusSplit::Training)
    {
        samples.push(route_replay_sample(receipt));
        samples.push(grounded_answer_replay_sample(receipt));
    }
    let mut module_sample_counts = BTreeMap::new();
    for sample in &samples {
        let key = match sample.module {
            CompiledAgentModuleKind::Route => "route",
            CompiledAgentModuleKind::GroundedAnswer => "grounded_answer",
            CompiledAgentModuleKind::ToolPolicy => "tool_policy",
            CompiledAgentModuleKind::ToolArguments => "tool_arguments",
            CompiledAgentModuleKind::Verify => "verify",
        };
        *module_sample_counts.entry(String::from(key)).or_insert(0) += 1;
    }
    let correction_sample_count = samples
        .iter()
        .filter(|sample| {
            sample.correction_kind == CompiledAgentReplayCorrectionKind::FailureCorrection
        })
        .count() as u32;
    ensure_replay_samples_match_evidence_class(&samples, ledger.evidence_class)?;
    let mut bundle = CompiledAgentReplayBundle {
        schema_version: String::from(REPLAY_BUNDLE_SCHEMA_VERSION),
        bundle_id: String::from("compiled_agent.replay_bundle.v1"),
        row_id: ledger.row_id.clone(),
        baseline_revision_id: ledger.baseline_revision_id.clone(),
        evidence_class: ledger.evidence_class,
        source_ledger_digest: ledger.ledger_digest.clone(),
        training_receipt_ids: ledger.training_receipt_ids.clone(),
        excluded_held_out_receipt_ids: ledger.held_out_receipt_ids.clone(),
        module_sample_counts,
        correction_sample_count,
        samples,
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Compiled-agent replay bundle retains {} route samples and {} grounded-answer samples from {} training receipts while excluding {} held-out receipts.",
        bundle
            .module_sample_counts
            .get("route")
            .copied()
            .unwrap_or(0),
        bundle
            .module_sample_counts
            .get("grounded_answer")
            .copied()
            .unwrap_or(0),
        bundle.training_receipt_ids.len(),
        bundle.excluded_held_out_receipt_ids.len(),
    );
    bundle.bundle_digest = stable_digest(b"compiled_agent_replay_bundle|", &bundle);
    Ok(bundle)
}

fn expected_receipt_evidence_class(
    receipts: &[CompiledAgentLearningReceipt],
) -> Result<CompiledAgentEvidenceClass, CompiledAgentReceiptError> {
    let Some(expected) = receipts.first().map(|receipt| receipt.evidence_class) else {
        return Ok(CompiledAgentEvidenceClass::LearnedLane);
    };
    for receipt in receipts.iter().skip(1) {
        if receipt.evidence_class != expected {
            return Err(CompiledAgentReceiptError::MixedEvidenceClass {
                context: String::from("compiled_agent_learning_receipt_ledger"),
                expected,
                actual: receipt.evidence_class,
            });
        }
    }
    Ok(expected)
}

fn ensure_replay_samples_match_evidence_class(
    samples: &[CompiledAgentReplaySample],
    expected: CompiledAgentEvidenceClass,
) -> Result<(), CompiledAgentReceiptError> {
    for sample in samples {
        if sample.evidence_class != expected {
            return Err(CompiledAgentReceiptError::MixedEvidenceClass {
                context: format!("compiled_agent_replay_bundle sample `{}`", sample.sample_id),
                expected,
                actual: sample.evidence_class,
            });
        }
    }
    Ok(())
}

fn route_replay_sample(receipt: &CompiledAgentLearningReceipt) -> CompiledAgentReplaySample {
    let correction_kind = if receipt.assessment.route_correct {
        CompiledAgentReplayCorrectionKind::BehavioralClone
    } else {
        CompiledAgentReplayCorrectionKind::FailureCorrection
    };
    CompiledAgentReplaySample {
        sample_id: format!("sample.route.{}", receipt.receipt_id),
        module: CompiledAgentModuleKind::Route,
        source_receipt_id: receipt.receipt_id.clone(),
        evidence_class: receipt.evidence_class,
        correction_kind,
        tags: receipt.tags.clone(),
        failure_classes: if receipt.assessment.route_correct {
            Vec::new()
        } else {
            receipt.assessment.failure_classes.clone()
        },
        input: json!({
            "user_request": receipt.user_request,
        }),
        expected_output: json!({
            "route": receipt.expected_route,
        }),
        observed_output: json!({
            "route": receipt.observed_route,
        }),
    }
}

fn grounded_answer_replay_sample(
    receipt: &CompiledAgentLearningReceipt,
) -> CompiledAgentReplaySample {
    let correction_kind = if receipt.assessment.grounded_answer_correct {
        CompiledAgentReplayCorrectionKind::BehavioralClone
    } else {
        CompiledAgentReplayCorrectionKind::FailureCorrection
    };
    CompiledAgentReplaySample {
        sample_id: format!("sample.grounded_answer.{}", receipt.receipt_id),
        module: CompiledAgentModuleKind::GroundedAnswer,
        source_receipt_id: receipt.receipt_id.clone(),
        evidence_class: receipt.evidence_class,
        correction_kind,
        tags: receipt.tags.clone(),
        failure_classes: if receipt.assessment.grounded_answer_correct {
            Vec::new()
        } else {
            receipt.assessment.failure_classes.clone()
        },
        input: json!({
            "user_request": receipt.user_request,
            "route": receipt.expected_route,
            "tool_results": expected_tool_results(receipt),
        }),
        expected_output: json!({
            "kind": receipt.expected_public_response.kind,
            "response": receipt.expected_public_response.response,
        }),
        observed_output: json!({
            "kind": receipt.observed_public_response.kind,
            "response": receipt.observed_public_response.response,
        }),
    }
}

fn expected_tool_names(route: CompiledAgentRoute) -> Vec<String> {
    match route {
        CompiledAgentRoute::ProviderStatus => vec![String::from("provider_status")],
        CompiledAgentRoute::WalletStatus => vec![String::from("wallet_status")],
        CompiledAgentRoute::Unsupported => Vec::new(),
    }
}

fn expected_tool_results(receipt: &CompiledAgentLearningReceipt) -> Vec<CompiledAgentToolResult> {
    match receipt.expected_route {
        CompiledAgentRoute::ProviderStatus => vec![CompiledAgentToolResult {
            tool_name: String::from("provider_status"),
            payload: json!({
                "ready": receipt.runtime_state.provider_ready,
                "blockers": receipt.runtime_state.provider_blockers,
            }),
        }],
        CompiledAgentRoute::WalletStatus => vec![CompiledAgentToolResult {
            tool_name: String::from("wallet_status"),
            payload: json!({
                "balance_sats": receipt.runtime_state.wallet_balance_sats,
                "recent_earnings_sats": receipt.runtime_state.recent_earnings_sats,
            }),
        }],
        CompiledAgentRoute::Unsupported => Vec::new(),
    }
}

fn tool_arguments_match(
    observed: &[CompiledAgentToolCall],
    expected_tool_names: &[String],
) -> bool {
    let expected_calls = expected_tool_names
        .iter()
        .map(|tool_name| CompiledAgentToolCall {
            tool_name: tool_name.clone(),
            arguments: json!({}),
        })
        .collect::<Vec<_>>();
    observed == expected_calls
}

fn verify_matches_expected(
    expected_route: CompiledAgentRoute,
    observed_tool_names: &[String],
    observed_kind: CompiledAgentPublicOutcomeKind,
    expected_response: &CompiledAgentLearningPublicResponse,
    observed_response: &CompiledAgentLearningPublicResponse,
) -> bool {
    if observed_response != expected_response || observed_kind != expected_response.kind {
        return false;
    }
    match expected_route {
        CompiledAgentRoute::Unsupported => observed_tool_names.is_empty(),
        CompiledAgentRoute::ProviderStatus => {
            observed_tool_names == [String::from("provider_status")]
        }
        CompiledAgentRoute::WalletStatus => observed_tool_names == [String::from("wallet_status")],
    }
}

fn route_failure_class(
    user_request: &str,
    observed_route: CompiledAgentRoute,
    expected_route: CompiledAgentRoute,
) -> String {
    let lowered = user_request.to_ascii_lowercase();
    if expected_route == CompiledAgentRoute::Unsupported
        && lowered.contains("do not")
        && (observed_route == CompiledAgentRoute::WalletStatus
            || observed_route == CompiledAgentRoute::ProviderStatus)
        && (lowered.contains("wallet") || lowered.contains("online"))
    {
        return String::from("negated_route_false_positive");
    }
    String::from("route_mismatch")
}

fn fixture_slug(source_fixture_ref: &str) -> String {
    Path::new(source_fixture_ref)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("receipt")
        .to_string()
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
        canonical_compiled_agent_learning_receipt_ledger, canonical_compiled_agent_replay_bundle,
        canonical_compiled_agent_runtime_source_receipts, canonical_compiled_agent_source_receipts,
        compiled_agent_learning_receipt_ledger_fixture_path,
        compiled_agent_replay_bundle_fixture_path, compiled_agent_runtime_fixture_dir,
        compiled_agent_source_fixture_dir, verify_compiled_agent_learning_receipt_fixtures,
    };
    use psionic_eval::CompiledAgentEvidenceClass;

    #[test]
    fn compiled_agent_learning_ledger_retains_training_and_held_out_rows(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ledger = canonical_compiled_agent_learning_receipt_ledger()?;
        assert_eq!(ledger.receipts.len(), 47);
        assert_eq!(
            ledger.evidence_class,
            CompiledAgentEvidenceClass::LearnedLane
        );
        assert_eq!(ledger.training_receipt_ids.len(), 28);
        assert_eq!(ledger.held_out_receipt_ids.len(), 19);
        assert_eq!(ledger.correction_receipt_ids.len(), 33);
        assert!(ledger
            .correction_receipt_ids
            .iter()
            .any(|receipt_id| receipt_id.contains("negated_wallet")));
        assert!(ledger
            .source_fixture_refs
            .iter()
            .any(|fixture| fixture.contains("/runtime/")));
        assert!(
            ledger
                .task_family_counts
                .get("provider")
                .copied()
                .unwrap_or(0)
                >= 6
        );
        assert!(
            ledger
                .task_family_counts
                .get("wallet")
                .copied()
                .unwrap_or(0)
                >= 10
        );
        assert!(
            ledger
                .task_family_counts
                .get("unsupported")
                .copied()
                .unwrap_or(0)
                >= 12
        );
        assert_eq!(
            ledger
                .failure_class_counts
                .get("negated_route_false_positive"),
            Some(&3)
        );
        Ok(())
    }

    #[test]
    fn compiled_agent_replay_bundle_targets_route_and_grounded_answer_first(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = canonical_compiled_agent_replay_bundle()?;
        assert_eq!(
            bundle.evidence_class,
            CompiledAgentEvidenceClass::LearnedLane
        );
        assert_eq!(bundle.training_receipt_ids.len(), 28);
        assert_eq!(bundle.excluded_held_out_receipt_ids.len(), 19);
        assert_eq!(bundle.module_sample_counts.get("route"), Some(&28));
        assert_eq!(
            bundle.module_sample_counts.get("grounded_answer"),
            Some(&28)
        );
        assert!(bundle.correction_sample_count >= 10);
        Ok(())
    }

    #[test]
    fn compiled_agent_learning_fixtures_match_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (fixture_name, _) in canonical_compiled_agent_source_receipts() {
            assert!(compiled_agent_source_fixture_dir()
                .join(fixture_name)
                .exists());
        }
        for (fixture_name, _) in canonical_compiled_agent_runtime_source_receipts() {
            assert!(compiled_agent_runtime_fixture_dir()
                .join(fixture_name)
                .exists());
        }
        assert!(compiled_agent_learning_receipt_ledger_fixture_path().exists());
        assert!(compiled_agent_replay_bundle_fixture_path().exists());
        verify_compiled_agent_learning_receipt_fixtures()?;
        Ok(())
    }
}
