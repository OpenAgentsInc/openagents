use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CompiledAgentGroundedAnswerModelArtifact, CompiledAgentRouteModelArtifact};

pub const COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION: &str = "psionic.compiled_agent_default_row.v1";
pub const COMPILED_AGENT_DEFAULT_ROW_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_default_row_v1.json";
pub const COMPILED_AGENT_DEFAULT_ROW_DOC_PATH: &str =
    "docs/COMPILED_AGENT_DEFAULT_ROW_REFERENCE.md";
pub const COMPILED_AGENT_DEFAULT_ROW_CHECK_SCRIPT_PATH: &str =
    "scripts/check-compiled-agent-default-row-contract.sh";
pub const COMPILED_AGENT_DEFAULT_ROW_PROBE_BIN_PATH: &str =
    "crates/psionic-train/src/bin/compiled_agent_default_row_probe.rs";
pub const COMPILED_AGENT_DEFAULT_ROW_LIVE_REPORT_PATH: &str = "fixtures/compiled_agent/compiled_agent_default_row_live_report_20260328_archlinux_qwen35_9b.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentRoute {
    ProviderStatus,
    WalletStatus,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentPublicOutcomeKind {
    GroundedAnswer,
    UnsupportedRefusal,
    ConfidenceFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentEvidenceClass {
    LearnedLane,
    StrongerEvidenceLane,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentToolSpec {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentToolCall {
    pub tool_name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentToolResult {
    pub tool_name: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRuntimeState {
    pub provider_ready: bool,
    pub provider_blockers: Vec<String>,
    pub wallet_balance_sats: u64,
    pub recent_earnings_sats: u64,
}

impl Default for CompiledAgentRuntimeState {
    fn default() -> Self {
        Self {
            provider_ready: true,
            provider_blockers: Vec::new(),
            wallet_balance_sats: 1_200,
            recent_earnings_sats: 240,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleRevisionSet {
    pub revision_id: String,
    pub provider_route_keywords: Vec<String>,
    pub wallet_route_keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_model_artifact: Option<CompiledAgentRouteModelArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounded_answer_model_artifact: Option<CompiledAgentGroundedAnswerModelArtifact>,
    pub negation_keywords: Vec<String>,
    pub unsupported_route_keywords: Vec<String>,
    pub include_provider_blockers: bool,
    pub include_recent_earnings: bool,
    pub verify_require_recent_earnings: bool,
    pub unsupported_template: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDefaultLearnedRowContract {
    pub schema_version: String,
    pub row_id: String,
    pub backend_label: String,
    pub host_label: String,
    pub accelerator_label: String,
    pub model_family: String,
    pub model_artifact: String,
    pub model_path_hint: String,
    pub admitted_tasks: Vec<String>,
    pub refusal_boundary: Vec<String>,
    pub latency_target_p50_ms: u32,
    pub prompt_token_budget: u32,
    pub completion_token_budget: u32,
    pub baseline_revision_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDefaultRowBenchmarkCaseReport {
    pub case_id: String,
    pub prompt: String,
    pub pass: bool,
    pub latency_ms: f64,
    pub observed_text: String,
    pub expected_summary: String,
    pub usage: CompiledAgentDefaultRowUsage,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDefaultRowUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDefaultRowBenchmarkReport {
    pub schema_version: String,
    pub row_id: String,
    pub backend_label: String,
    pub host_label: String,
    pub psionic_revision: String,
    pub measured_at: String,
    pub model_artifact: String,
    pub model_path: String,
    pub case_reports: Vec<CompiledAgentDefaultRowBenchmarkCaseReport>,
    pub all_passed: bool,
    pub detail: String,
}

#[must_use]
pub fn compiled_agent_supported_tools() -> Vec<CompiledAgentToolSpec> {
    vec![
        CompiledAgentToolSpec {
            name: String::from("provider_status"),
            description: String::from("Read provider readiness and blocker state."),
        },
        CompiledAgentToolSpec {
            name: String::from("wallet_status"),
            description: String::from("Read wallet balance and recent earnings."),
        },
    ]
}

#[must_use]
pub fn compiled_agent_baseline_revision_set() -> CompiledAgentModuleRevisionSet {
    CompiledAgentModuleRevisionSet {
        revision_id: String::from("compiled_agent.baseline.rule_v1"),
        provider_route_keywords: vec![
            String::from("provider"),
            String::from("online"),
            String::from("ready"),
            String::from("readiness"),
        ],
        wallet_route_keywords: vec![
            String::from("wallet"),
            String::from("balance"),
            String::from("sats"),
        ],
        route_model_artifact: None,
        grounded_answer_model_artifact: None,
        negation_keywords: Vec::new(),
        unsupported_route_keywords: Vec::new(),
        include_provider_blockers: false,
        include_recent_earnings: false,
        verify_require_recent_earnings: false,
        unsupported_template: String::from(
            "I can currently answer only provider readiness and wallet balance questions.",
        ),
    }
}

#[must_use]
pub fn canonical_compiled_agent_default_row_contract() -> CompiledAgentDefaultLearnedRowContract {
    CompiledAgentDefaultLearnedRowContract {
        schema_version: String::from(COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION),
        row_id: String::from("compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1"),
        backend_label: String::from("psionic_openai_server"),
        host_label: String::from("archlinux"),
        accelerator_label: String::from("rtx_4080_16gb"),
        model_family: String::from("qwen35"),
        model_artifact: String::from("qwen3.5-9b-q4_k_m-registry.gguf"),
        model_path_hint: String::from("~/models/qwen3.5/qwen3.5-9b-q4_k_m-registry.gguf"),
        admitted_tasks: vec![
            String::from("intent_route"),
            String::from("grounded_answer_from_supplied_facts"),
            String::from("unsupported_refusal"),
        ],
        refusal_boundary: vec![
            String::from("broad autonomous task execution"),
            String::from("silent tool selection without bounded contracts"),
            String::from("unbounded long-context planning"),
        ],
        latency_target_p50_ms: 2_500,
        prompt_token_budget: 512,
        completion_token_budget: 96,
        baseline_revision_id: compiled_agent_baseline_revision_set().revision_id,
        detail: String::from(
            "This is the first honest learned-row target for the compiled-agent loop. It is the local consumer-GPU qwen35 9B Q4_K_M row on archlinux, used for structured route selection across explicit provider-versus-wallet requests, grounded-answer-from-facts, and refusal work only.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_default_row_contract, compiled_agent_baseline_revision_set,
        compiled_agent_supported_tools, COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION,
    };

    #[test]
    fn compiled_agent_default_row_contract_stays_on_the_qwen35_9b_lane() {
        let contract = canonical_compiled_agent_default_row_contract();
        assert_eq!(
            contract.schema_version,
            COMPILED_AGENT_DEFAULT_ROW_SCHEMA_VERSION
        );
        assert_eq!(contract.host_label, "archlinux");
        assert_eq!(contract.model_family, "qwen35");
        assert!(
            contract.model_artifact.contains("9b"),
            "default row should stay on the 9B artifact"
        );
    }

    #[test]
    fn compiled_agent_baseline_revision_set_is_narrow() {
        let baseline = compiled_agent_baseline_revision_set();
        assert!(baseline.route_model_artifact.is_none());
        assert!(baseline.grounded_answer_model_artifact.is_none());
        assert!(baseline.negation_keywords.is_empty());
        assert!(baseline.unsupported_route_keywords.is_empty());
        assert!(!baseline.include_provider_blockers);
        assert!(!baseline.include_recent_earnings);
        assert!(!baseline.verify_require_recent_earnings);
        assert_eq!(baseline.wallet_route_keywords.len(), 3);
    }

    #[test]
    fn compiled_agent_supported_tools_stay_bounded() {
        let tools = compiled_agent_supported_tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "provider_status");
        assert_eq!(tools[1].name, "wallet_status");
    }
}
