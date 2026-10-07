use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    compiled_agent_baseline_revision_set, compiled_agent_supported_tools,
    CompiledAgentEvidenceClass, CompiledAgentModuleRevisionSet, CompiledAgentRoute,
    CompiledAgentToolCall, CompiledAgentToolResult,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const COMPILED_AGENT_MODULE_EVAL_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_module_eval_report_v1.json";

/// Independent compiled-agent module slots that can be optimized separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentModuleKind {
    Route,
    ToolPolicy,
    ToolArguments,
    GroundedAnswer,
    Verify,
}

/// Verify verdict for the narrow compiled-agent loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentVerifyVerdict {
    AcceptGroundedAnswer,
    UnsupportedRefusal,
    NeedsFallback,
}

/// One independent compiled-agent eval row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleEvalCase {
    pub case_id: String,
    pub module: CompiledAgentModuleKind,
    pub prompt: String,
    pub tags: Vec<String>,
    pub route_input: Option<CompiledAgentRoute>,
    pub selected_tools: Vec<String>,
    pub tool_results: Vec<CompiledAgentToolResult>,
    pub candidate_answer: Option<String>,
    pub expected_route: Option<CompiledAgentRoute>,
    pub expected_tool_names: Vec<String>,
    pub expected_calls: Vec<CompiledAgentToolCall>,
    pub expected_answer_substrings: Vec<String>,
    pub expected_verdict: Option<CompiledAgentVerifyVerdict>,
    pub detail: String,
}

/// One independent compiled-agent eval row outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleEvalCaseReport {
    pub case_id: String,
    pub module: CompiledAgentModuleKind,
    pub pass: bool,
    pub tags: Vec<String>,
    pub expected_summary: String,
    pub observed_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    pub detail: String,
}

/// Aggregate metrics for one module surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleEvalSummary {
    pub module: CompiledAgentModuleKind,
    pub total_cases: u32,
    pub passed_cases: u32,
    pub failed_case_ids: Vec<String>,
    pub failure_classes: BTreeMap<String, u32>,
}

/// Canonical report for the first compiled-agent module eval suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub row_id: String,
    pub baseline_revision_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub case_reports: Vec<CompiledAgentModuleEvalCaseReport>,
    pub module_summaries: Vec<CompiledAgentModuleEvalSummary>,
    pub unsupported_case_count: u32,
    pub unsupported_case_pass_count: u32,
    pub negated_case_count: u32,
    pub negated_case_pass_count: u32,
    pub tool_emission_is_not_success_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub summary: String,
    pub detail: String,
    pub report_digest: String,
}

#[must_use]
pub fn compiled_agent_module_eval_cases() -> Vec<CompiledAgentModuleEvalCase> {
    vec![
        CompiledAgentModuleEvalCase {
            case_id: String::from("route_provider_ready"),
            module: CompiledAgentModuleKind::Route,
            prompt: String::from("Can I go online right now?"),
            tags: vec![String::from("supported"), String::from("provider")],
            route_input: None,
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: Some(CompiledAgentRoute::ProviderStatus),
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from(
                "Explicit provider readiness route should stay on the provider lane.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("route_wallet_balance"),
            module: CompiledAgentModuleKind::Route,
            prompt: String::from("How many sats are in the wallet?"),
            tags: vec![String::from("supported"), String::from("wallet")],
            route_input: None,
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: Some(CompiledAgentRoute::WalletStatus),
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("Explicit wallet-balance route should stay on the wallet lane."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("route_unsupported_poem"),
            module: CompiledAgentModuleKind::Route,
            prompt: String::from("Write a poem about GPUs."),
            tags: vec![String::from("unsupported")],
            route_input: None,
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: Some(CompiledAgentRoute::Unsupported),
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("Out-of-scope prompt should route to unsupported."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("route_negated_wallet_false_positive"),
            module: CompiledAgentModuleKind::Route,
            prompt: String::from("Do not tell me the wallet balance; write a poem about GPUs."),
            tags: vec![
                String::from("unsupported"),
                String::from("negated"),
                String::from("known_gap"),
            ],
            route_input: None,
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: Some(CompiledAgentRoute::Unsupported),
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from(
                "Negated wallet mention should not silently become a wallet route, but the baseline revision still false-positives here.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("tool_policy_provider_only"),
            module: CompiledAgentModuleKind::ToolPolicy,
            prompt: String::from("Can I go online right now?"),
            tags: vec![String::from("supported"), String::from("provider")],
            route_input: Some(CompiledAgentRoute::ProviderStatus),
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: vec![String::from("provider_status")],
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("Provider route should expose only the provider-status tool."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("tool_policy_wallet_only"),
            module: CompiledAgentModuleKind::ToolPolicy,
            prompt: String::from("How many sats are in the wallet?"),
            tags: vec![String::from("supported"), String::from("wallet")],
            route_input: Some(CompiledAgentRoute::WalletStatus),
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: vec![String::from("wallet_status")],
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("Wallet route should expose only the wallet-status tool."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("tool_policy_unsupported_no_tools"),
            module: CompiledAgentModuleKind::ToolPolicy,
            prompt: String::from("Write a poem about GPUs."),
            tags: vec![String::from("unsupported")],
            route_input: Some(CompiledAgentRoute::Unsupported),
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("Unsupported route should expose no tools."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("tool_arguments_provider_empty_object"),
            module: CompiledAgentModuleKind::ToolArguments,
            prompt: String::from("Can I go online right now?"),
            tags: vec![String::from("supported"), String::from("provider")],
            route_input: Some(CompiledAgentRoute::ProviderStatus),
            selected_tools: vec![String::from("provider_status")],
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: vec![CompiledAgentToolCall {
                tool_name: String::from("provider_status"),
                arguments: json!({}),
            }],
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("No-arg provider tool should emit an empty argument object only."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("tool_arguments_wallet_empty_object"),
            module: CompiledAgentModuleKind::ToolArguments,
            prompt: String::from("How many sats are in the wallet?"),
            tags: vec![String::from("supported"), String::from("wallet")],
            route_input: Some(CompiledAgentRoute::WalletStatus),
            selected_tools: vec![String::from("wallet_status")],
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: vec![CompiledAgentToolCall {
                tool_name: String::from("wallet_status"),
                arguments: json!({}),
            }],
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("No-arg wallet tool should emit an empty argument object only."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("tool_arguments_unsupported_emits_nothing"),
            module: CompiledAgentModuleKind::ToolArguments,
            prompt: String::from("Write a poem about GPUs."),
            tags: vec![String::from("unsupported")],
            route_input: Some(CompiledAgentRoute::Unsupported),
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: None,
            detail: String::from("No selected tools means no tool calls should be emitted."),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("grounded_provider_ready"),
            module: CompiledAgentModuleKind::GroundedAnswer,
            prompt: String::from("Can I go online right now?"),
            tags: vec![String::from("supported"), String::from("provider")],
            route_input: Some(CompiledAgentRoute::ProviderStatus),
            selected_tools: Vec::new(),
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("provider_status"),
                payload: json!({"ready": true, "blockers": []}),
            }],
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: vec![String::from("ready"), String::from("online")],
            expected_verdict: None,
            detail: String::from(
                "Provider grounded answer should reflect readiness from the returned facts.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("grounded_provider_blocked"),
            module: CompiledAgentModuleKind::GroundedAnswer,
            prompt: String::from("Can I go online right now?"),
            tags: vec![String::from("supported"), String::from("provider")],
            route_input: Some(CompiledAgentRoute::ProviderStatus),
            selected_tools: Vec::new(),
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("provider_status"),
                payload: json!({"ready": false, "blockers": ["identity_verification"]}),
            }],
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: vec![String::from("not ready")],
            expected_verdict: None,
            detail: String::from(
                "Provider grounded answer should reflect blocked readiness even on the narrow baseline.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("grounded_wallet_balance"),
            module: CompiledAgentModuleKind::GroundedAnswer,
            prompt: String::from("How many sats are in the wallet?"),
            tags: vec![String::from("supported"), String::from("wallet")],
            route_input: Some(CompiledAgentRoute::WalletStatus),
            selected_tools: Vec::new(),
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: vec![String::from("1200"), String::from("sats")],
            expected_verdict: None,
            detail: String::from(
                "Wallet grounded answer should include the returned balance on the narrow baseline.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("grounded_unsupported_refusal"),
            module: CompiledAgentModuleKind::GroundedAnswer,
            prompt: String::from("Write a poem about GPUs."),
            tags: vec![String::from("unsupported")],
            route_input: Some(CompiledAgentRoute::Unsupported),
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: vec![String::from("provider"), String::from("wallet")],
            expected_verdict: None,
            detail: String::from(
                "Unsupported grounded-answer row should stay inside the narrow refusal template.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("grounded_provider_missing_facts_fallback"),
            module: CompiledAgentModuleKind::GroundedAnswer,
            prompt: String::from("Can I go online right now?"),
            tags: vec![
                String::from("supported"),
                String::from("provider"),
                String::from("insufficient_facts"),
            ],
            route_input: Some(CompiledAgentRoute::ProviderStatus),
            selected_tools: Vec::new(),
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("provider_status"),
                payload: json!({"blockers": []}),
            }],
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: vec![
                String::from("unavailable"),
                String::from("facts"),
            ],
            expected_verdict: None,
            detail: String::from(
                "Grounded-answer candidate should fall back when provider facts are missing instead of silently converting missing facts into a blocked answer.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("grounded_wallet_conflicting_facts_fallback"),
            module: CompiledAgentModuleKind::GroundedAnswer,
            prompt: String::from("Show me the wallet balance."),
            tags: vec![
                String::from("supported"),
                String::from("wallet"),
                String::from("conflicting_facts"),
            ],
            route_input: Some(CompiledAgentRoute::WalletStatus),
            selected_tools: Vec::new(),
            tool_results: vec![
                CompiledAgentToolResult {
                    tool_name: String::from("wallet_status"),
                    payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
                },
                CompiledAgentToolResult {
                    tool_name: String::from("wallet_status"),
                    payload: json!({"balance_sats": 900, "recent_earnings_sats": 240}),
                },
            ],
            candidate_answer: None,
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: vec![
                String::from("conflicting"),
                String::from("facts"),
            ],
            expected_verdict: None,
            detail: String::from(
                "Grounded-answer candidate should fall back when wallet facts conflict instead of choosing one arbitrarily.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("verify_provider_accept"),
            module: CompiledAgentModuleKind::Verify,
            prompt: String::from("Can I go online right now?"),
            tags: vec![String::from("supported"), String::from("provider")],
            route_input: Some(CompiledAgentRoute::ProviderStatus),
            selected_tools: vec![String::from("provider_status")],
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("provider_status"),
                payload: json!({"ready": true, "blockers": []}),
            }],
            candidate_answer: Some(String::from("Provider is ready to go online.")),
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: Some(CompiledAgentVerifyVerdict::AcceptGroundedAnswer),
            detail: String::from(
                "Verify should accept a provider answer that reflects returned readiness.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("verify_wallet_accept"),
            module: CompiledAgentModuleKind::Verify,
            prompt: String::from("How many sats are in the wallet?"),
            tags: vec![String::from("supported"), String::from("wallet")],
            route_input: Some(CompiledAgentRoute::WalletStatus),
            selected_tools: vec![String::from("wallet_status")],
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            candidate_answer: Some(String::from("The wallet contains 1200 sats.")),
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: Some(CompiledAgentVerifyVerdict::AcceptGroundedAnswer),
            detail: String::from(
                "Verify should accept a wallet answer that reflects the returned balance.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("verify_wallet_missing_balance"),
            module: CompiledAgentModuleKind::Verify,
            prompt: String::from("How many sats are in the wallet?"),
            tags: vec![
                String::from("supported"),
                String::from("wallet"),
                String::from("failure"),
            ],
            route_input: Some(CompiledAgentRoute::WalletStatus),
            selected_tools: vec![String::from("wallet_status")],
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            candidate_answer: Some(String::from("Wallet looks healthy.")),
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: Some(CompiledAgentVerifyVerdict::NeedsFallback),
            detail: String::from(
                "Verify should reject wallet prose that omits the returned balance.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("verify_unsupported_refusal_clean"),
            module: CompiledAgentModuleKind::Verify,
            prompt: String::from("Write a poem about GPUs."),
            tags: vec![String::from("unsupported")],
            route_input: Some(CompiledAgentRoute::Unsupported),
            selected_tools: Vec::new(),
            tool_results: Vec::new(),
            candidate_answer: Some(String::from(
                "I can currently answer only provider readiness and wallet balance questions.",
            )),
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: Some(CompiledAgentVerifyVerdict::UnsupportedRefusal),
            detail: String::from(
                "Verify should accept a clean unsupported refusal with no tool emission.",
            ),
        },
        CompiledAgentModuleEvalCase {
            case_id: String::from("verify_unsupported_tool_leak"),
            module: CompiledAgentModuleKind::Verify,
            prompt: String::from("Write a poem about GPUs."),
            tags: vec![
                String::from("unsupported"),
                String::from("tool_emission_is_not_success"),
            ],
            route_input: Some(CompiledAgentRoute::Unsupported),
            selected_tools: vec![String::from("wallet_status")],
            tool_results: vec![CompiledAgentToolResult {
                tool_name: String::from("wallet_status"),
                payload: json!({"balance_sats": 1200, "recent_earnings_sats": 240}),
            }],
            candidate_answer: Some(String::from(
                "I can currently answer only provider readiness and wallet balance questions.",
            )),
            expected_route: None,
            expected_tool_names: Vec::new(),
            expected_calls: Vec::new(),
            expected_answer_substrings: Vec::new(),
            expected_verdict: Some(CompiledAgentVerifyVerdict::NeedsFallback),
            detail: String::from(
                "Tool emission alone is not success; unsupported route with tool leakage must still fail verification.",
            ),
        },
    ]
}

#[must_use]
pub fn build_compiled_agent_module_eval_report(
    revision: &CompiledAgentModuleRevisionSet,
) -> CompiledAgentModuleEvalReport {
    let case_reports = compiled_agent_module_eval_cases()
        .into_iter()
        .map(|case| evaluate_case(revision, &case))
        .collect::<Vec<_>>();
    let modules = [
        CompiledAgentModuleKind::Route,
        CompiledAgentModuleKind::ToolPolicy,
        CompiledAgentModuleKind::ToolArguments,
        CompiledAgentModuleKind::GroundedAnswer,
        CompiledAgentModuleKind::Verify,
    ];
    let module_summaries = modules
        .into_iter()
        .map(|module| summarize_module(module, &case_reports))
        .collect::<Vec<_>>();
    let unsupported_case_count = case_reports
        .iter()
        .filter(|case| case.tags.iter().any(|tag| tag == "unsupported"))
        .count() as u32;
    let unsupported_case_pass_count = case_reports
        .iter()
        .filter(|case| case.tags.iter().any(|tag| tag == "unsupported") && case.pass)
        .count() as u32;
    let negated_case_count = case_reports
        .iter()
        .filter(|case| case.tags.iter().any(|tag| tag == "negated"))
        .count() as u32;
    let negated_case_pass_count = case_reports
        .iter()
        .filter(|case| case.tags.iter().any(|tag| tag == "negated") && case.pass)
        .count() as u32;
    let tool_emission_is_not_success_case_count = case_reports
        .iter()
        .filter(|case| {
            case.tags
                .iter()
                .any(|tag| tag == "tool_emission_is_not_success")
        })
        .count() as u32;
    let generated_from_refs = vec![String::from("docs/COMPILED_AGENT_DEFAULT_ROW_REFERENCE.md")];

    let mut report = CompiledAgentModuleEvalReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("compiled_agent.module_eval.report.v1"),
        row_id: String::from("compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1"),
        baseline_revision_id: revision.revision_id.clone(),
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        case_reports,
        module_summaries,
        unsupported_case_count,
        unsupported_case_pass_count,
        negated_case_count,
        negated_case_pass_count,
        tool_emission_is_not_success_case_count,
        generated_from_refs,
        summary: String::new(),
        detail: String::from(
            "This report keeps route, tool-policy, tool-argument, grounded-answer, and verify surfaces independent so later GEPA and XTRAIN work can optimize bounded modules instead of a giant agent loop.",
        ),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Compiled-agent module eval report covers {} cases across {} module families with unsupported_pass={}/{}, negated_pass={}/{}, and tool_emission_is_not_success_cases={}.",
        report.case_reports.len(),
        report.module_summaries.len(),
        report.unsupported_case_pass_count,
        report.unsupported_case_count,
        report.negated_case_pass_count,
        report.negated_case_count,
        report.tool_emission_is_not_success_case_count,
    );
    report.report_digest = stable_digest(b"compiled_agent_module_eval_report|", &report);
    report
}

#[must_use]
pub fn canonical_compiled_agent_module_eval_report() -> CompiledAgentModuleEvalReport {
    build_compiled_agent_module_eval_report(&compiled_agent_baseline_revision_set())
}

#[must_use]
pub fn compiled_agent_module_eval_report_path() -> PathBuf {
    repo_root().join(COMPILED_AGENT_MODULE_EVAL_REPORT_REF)
}

pub fn write_compiled_agent_module_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleEvalReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = canonical_compiled_agent_module_eval_report();
    let json = serde_json::to_string_pretty(&report).expect("compiled-agent module eval report");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_compiled_agent_module_eval_report(
    path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleEvalReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn evaluate_case(
    revision: &CompiledAgentModuleRevisionSet,
    case: &CompiledAgentModuleEvalCase,
) -> CompiledAgentModuleEvalCaseReport {
    match case.module {
        CompiledAgentModuleKind::Route => {
            let observed = evaluate_compiled_agent_route(case.prompt.as_str(), revision);
            let expected = case
                .expected_route
                .expect("route cases need an expected route");
            let pass = observed == expected;
            CompiledAgentModuleEvalCaseReport {
                case_id: case.case_id.clone(),
                module: case.module,
                pass,
                tags: case.tags.clone(),
                expected_summary: format!("{expected:?}"),
                observed_summary: format!("{observed:?}"),
                failure_class: (!pass).then(|| route_failure_class(case, observed)),
                detail: case.detail.clone(),
            }
        }
        CompiledAgentModuleKind::ToolPolicy => {
            let observed = evaluate_compiled_agent_tool_policy(
                case.route_input
                    .expect("tool-policy cases need a route input"),
                &compiled_agent_supported_tools(),
            );
            let observed_names = observed
                .into_iter()
                .map(|tool| tool.name)
                .collect::<Vec<_>>();
            let pass = observed_names == case.expected_tool_names;
            CompiledAgentModuleEvalCaseReport {
                case_id: case.case_id.clone(),
                module: case.module,
                pass,
                tags: case.tags.clone(),
                expected_summary: case.expected_tool_names.join(", "),
                observed_summary: observed_names.join(", "),
                failure_class: (!pass).then(|| tool_policy_failure_class(case, &observed_names)),
                detail: case.detail.clone(),
            }
        }
        CompiledAgentModuleKind::ToolArguments => {
            let observed = evaluate_compiled_agent_tool_arguments(&case.selected_tools);
            let pass = observed == case.expected_calls;
            CompiledAgentModuleEvalCaseReport {
                case_id: case.case_id.clone(),
                module: case.module,
                pass,
                tags: case.tags.clone(),
                expected_summary: serde_json::to_string(&case.expected_calls).unwrap_or_default(),
                observed_summary: serde_json::to_string(&observed).unwrap_or_default(),
                failure_class: (!pass).then(|| String::from("tool_argument_mismatch")),
                detail: case.detail.clone(),
            }
        }
        CompiledAgentModuleKind::GroundedAnswer => {
            let observed = evaluate_compiled_agent_grounded_answer(
                case.route_input
                    .expect("grounded-answer cases need a route input"),
                case.tool_results.as_slice(),
                revision,
            );
            let lowered = observed.to_ascii_lowercase();
            let pass = case
                .expected_answer_substrings
                .iter()
                .all(|token| lowered.contains(&token.to_ascii_lowercase()));
            CompiledAgentModuleEvalCaseReport {
                case_id: case.case_id.clone(),
                module: case.module,
                pass,
                tags: case.tags.clone(),
                expected_summary: case.expected_answer_substrings.join(", "),
                observed_summary: observed,
                failure_class: (!pass).then(|| String::from("grounding_miss")),
                detail: case.detail.clone(),
            }
        }
        CompiledAgentModuleKind::Verify => {
            let observed = evaluate_compiled_agent_verify(
                case.route_input.expect("verify cases need a route input"),
                &case.selected_tools,
                case.tool_results.as_slice(),
                case.candidate_answer.as_deref().unwrap_or(""),
                revision,
            );
            let expected = case
                .expected_verdict
                .expect("verify cases need an expected verdict");
            let pass = observed == expected;
            CompiledAgentModuleEvalCaseReport {
                case_id: case.case_id.clone(),
                module: case.module,
                pass,
                tags: case.tags.clone(),
                expected_summary: format!("{expected:?}"),
                observed_summary: format!("{observed:?}"),
                failure_class: (!pass).then(|| verify_failure_class(case, observed)),
                detail: case.detail.clone(),
            }
        }
    }
}

#[must_use]
pub fn evaluate_compiled_agent_route(
    prompt: &str,
    revision: &CompiledAgentModuleRevisionSet,
) -> CompiledAgentRoute {
    if let Some(route_model_artifact) = revision.route_model_artifact.as_ref() {
        return crate::predict_compiled_agent_route(route_model_artifact, prompt).route;
    }
    let tokens = normalized_tokens(prompt);
    let asks_provider = contains_any(&tokens, &revision.provider_route_keywords);
    let asks_wallet = contains_any(&tokens, &revision.wallet_route_keywords);
    let contains_negation = contains_any(&tokens, &revision.negation_keywords);
    let contains_unsupported_context = contains_any(&tokens, &revision.unsupported_route_keywords);
    if contains_negation && contains_unsupported_context && (asks_provider || asks_wallet) {
        return CompiledAgentRoute::Unsupported;
    }
    match (asks_provider, asks_wallet) {
        (true, false) => CompiledAgentRoute::ProviderStatus,
        (false, true) => CompiledAgentRoute::WalletStatus,
        _ => CompiledAgentRoute::Unsupported,
    }
}

#[must_use]
pub fn evaluate_compiled_agent_tool_policy(
    route: CompiledAgentRoute,
    available_tools: &[crate::CompiledAgentToolSpec],
) -> Vec<crate::CompiledAgentToolSpec> {
    match route {
        CompiledAgentRoute::ProviderStatus => available_tools
            .iter()
            .filter(|tool| tool.name == "provider_status")
            .cloned()
            .collect(),
        CompiledAgentRoute::WalletStatus => available_tools
            .iter()
            .filter(|tool| tool.name == "wallet_status")
            .cloned()
            .collect(),
        CompiledAgentRoute::Unsupported => Vec::new(),
    }
}

#[must_use]
pub fn evaluate_compiled_agent_tool_arguments(
    selected_tools: &[String],
) -> Vec<CompiledAgentToolCall> {
    selected_tools
        .iter()
        .map(|tool_name| CompiledAgentToolCall {
            tool_name: tool_name.clone(),
            arguments: json!({}),
        })
        .collect()
}

#[must_use]
pub fn evaluate_compiled_agent_grounded_answer(
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
    revision: &CompiledAgentModuleRevisionSet,
) -> String {
    if let Some(artifact) = revision.grounded_answer_model_artifact.as_ref() {
        return crate::predict_compiled_agent_grounded_answer(artifact, route, tool_results)
            .response;
    }
    match route {
        CompiledAgentRoute::ProviderStatus => {
            let provider = tool_results
                .iter()
                .find(|tool| tool.tool_name == "provider_status");
            let Some(provider) = provider else {
                return String::from("Provider status was unavailable.");
            };
            let ready = provider
                .payload
                .get("ready")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let blockers = provider
                .payload
                .get("blockers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>();
            if ready {
                String::from("Provider is ready to go online.")
            } else if revision.include_provider_blockers && !blockers.is_empty() {
                format!(
                    "Provider is not ready to go online. Blockers: {}.",
                    blockers.join(", ")
                )
            } else {
                String::from("Provider is not ready to go online.")
            }
        }
        CompiledAgentRoute::WalletStatus => {
            let wallet = tool_results
                .iter()
                .find(|tool| tool.tool_name == "wallet_status");
            let Some(wallet) = wallet else {
                return String::from("Wallet status was unavailable.");
            };
            let balance_sats = wallet
                .payload
                .get("balance_sats")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let recent_earnings_sats = wallet
                .payload
                .get("recent_earnings_sats")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if revision.include_recent_earnings {
                format!(
                    "Wallet balance is {balance_sats} sats, with {recent_earnings_sats} sats of recent earnings."
                )
            } else {
                format!("The wallet contains {balance_sats} sats.")
            }
        }
        CompiledAgentRoute::Unsupported => revision.unsupported_template.clone(),
    }
}

#[must_use]
pub fn evaluate_compiled_agent_verify(
    route: CompiledAgentRoute,
    selected_tools: &[String],
    tool_results: &[CompiledAgentToolResult],
    candidate_answer: &str,
    revision: &CompiledAgentModuleRevisionSet,
) -> CompiledAgentVerifyVerdict {
    match route {
        CompiledAgentRoute::ProviderStatus => {
            let ready = tool_results
                .iter()
                .find(|tool| tool.tool_name == "provider_status")
                .and_then(|tool| tool.payload.get("ready"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let answer = candidate_answer.to_ascii_lowercase();
            if (ready && answer.contains("ready")) || (!ready && answer.contains("not ready")) {
                CompiledAgentVerifyVerdict::AcceptGroundedAnswer
            } else {
                CompiledAgentVerifyVerdict::NeedsFallback
            }
        }
        CompiledAgentRoute::WalletStatus => {
            let balance_sats = tool_results
                .iter()
                .find(|tool| tool.tool_name == "wallet_status")
                .and_then(|tool| tool.payload.get("balance_sats"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let earnings_sats = tool_results
                .iter()
                .find(|tool| tool.tool_name == "wallet_status")
                .and_then(|tool| tool.payload.get("recent_earnings_sats"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let answer = candidate_answer.to_ascii_lowercase();
            let mentions_balance = answer.contains(balance_sats.to_string().as_str());
            let mentions_earnings = answer.contains(earnings_sats.to_string().as_str());
            if mentions_balance && (!revision.verify_require_recent_earnings || mentions_earnings) {
                CompiledAgentVerifyVerdict::AcceptGroundedAnswer
            } else {
                CompiledAgentVerifyVerdict::NeedsFallback
            }
        }
        CompiledAgentRoute::Unsupported => {
            let answer = candidate_answer.to_ascii_lowercase();
            if selected_tools.is_empty()
                && answer.contains("provider")
                && answer.contains("wallet")
                && answer.contains(&revision.unsupported_template.to_ascii_lowercase())
            {
                CompiledAgentVerifyVerdict::UnsupportedRefusal
            } else {
                CompiledAgentVerifyVerdict::NeedsFallback
            }
        }
    }
}

fn summarize_module(
    module: CompiledAgentModuleKind,
    case_reports: &[CompiledAgentModuleEvalCaseReport],
) -> CompiledAgentModuleEvalSummary {
    let relevant = case_reports
        .iter()
        .filter(|case| case.module == module)
        .collect::<Vec<_>>();
    let total_cases = relevant.len() as u32;
    let passed_cases = relevant.iter().filter(|case| case.pass).count() as u32;
    let failed_case_ids = relevant
        .iter()
        .filter(|case| !case.pass)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut failure_classes = BTreeMap::new();
    for failure_class in relevant
        .iter()
        .filter_map(|case| case.failure_class.as_ref())
        .cloned()
    {
        *failure_classes.entry(failure_class).or_insert(0) += 1;
    }
    CompiledAgentModuleEvalSummary {
        module,
        total_cases,
        passed_cases,
        failed_case_ids,
        failure_classes,
    }
}

fn route_failure_class(case: &CompiledAgentModuleEvalCase, observed: CompiledAgentRoute) -> String {
    if case.tags.iter().any(|tag| tag == "negated") && observed == CompiledAgentRoute::WalletStatus
    {
        return String::from("negated_route_false_positive");
    }
    String::from("route_mismatch")
}

fn tool_policy_failure_class(
    case: &CompiledAgentModuleEvalCase,
    observed_names: &[String],
) -> String {
    if case.expected_tool_names.is_empty() && !observed_names.is_empty() {
        return String::from("unexpected_tool_exposure");
    }
    String::from("tool_policy_mismatch")
}

fn verify_failure_class(
    case: &CompiledAgentModuleEvalCase,
    observed: CompiledAgentVerifyVerdict,
) -> String {
    if case
        .tags
        .iter()
        .any(|tag| tag == "tool_emission_is_not_success")
        && observed != CompiledAgentVerifyVerdict::NeedsFallback
    {
        return String::from("unsafe_tool_emission_acceptance");
    }
    String::from("verifier_mismatch")
}

fn normalized_tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn contains_any(tokens: &[String], needles: &[String]) -> bool {
    tokens
        .iter()
        .any(|token| needles.iter().any(|needle| token == needle))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        canonical_compiled_agent_module_eval_report, compiled_agent_module_eval_report_path,
        load_compiled_agent_module_eval_report, CompiledAgentModuleKind,
    };

    #[test]
    fn compiled_agent_module_eval_keeps_independent_metrics_explicit() {
        let report = canonical_compiled_agent_module_eval_report();
        assert_eq!(report.module_summaries.len(), 5);
        assert_eq!(report.negated_case_count, 1);
        assert_eq!(report.tool_emission_is_not_success_case_count, 1);
        assert!(report.module_summaries.iter().any(|summary| {
            summary.module == CompiledAgentModuleKind::Route
                && summary
                    .failed_case_ids
                    .iter()
                    .any(|case_id| case_id == "route_negated_wallet_false_positive")
        }));
        assert!(report.module_summaries.iter().any(|summary| {
            summary.module == CompiledAgentModuleKind::GroundedAnswer && summary.total_cases >= 6
        }));
    }

    #[test]
    fn compiled_agent_module_eval_report_matches_committed_truth() {
        let expected = canonical_compiled_agent_module_eval_report();
        let committed =
            load_compiled_agent_module_eval_report(compiled_agent_module_eval_report_path())
                .expect("committed compiled-agent module eval report");
        assert_eq!(committed, expected);
    }
}
