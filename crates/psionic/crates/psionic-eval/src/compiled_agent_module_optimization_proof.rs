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

use psionic_optimize::{
    OptimizationCandidateManifest, OptimizationCandidateProposal, OptimizationCandidateProposer,
    OptimizationCaseEvaluationReceipt, OptimizationCaseManifest, OptimizationCaseSplit,
    OptimizationComponentDiff, OptimizationComponentFeedback, OptimizationEngine,
    OptimizationEvaluationCache, OptimizationEvaluator, OptimizationFrontierMode,
    OptimizationFrontierSnapshot, OptimizationProposerReceipt, OptimizationRunReceipt,
    OptimizationRunSpec, OptimizationSearchState, OptimizationSequentialMinibatchSampler,
    OptimizationSharedFeedback, OptimizationStopReason,
};

use crate::{
    canonical_compiled_agent_default_row_contract, canonical_compiled_agent_module_eval_report,
    compiled_agent_baseline_revision_set, evaluate_compiled_agent_route,
    evaluate_compiled_agent_verify, CompiledAgentModuleEvalCase,
    CompiledAgentModuleEvalCaseReport, CompiledAgentModuleEvalReport,
    CompiledAgentModuleEvalSummary, CompiledAgentModuleKind, CompiledAgentModuleRevisionSet,
    CompiledAgentRoute, CompiledAgentToolCall, CompiledAgentToolResult,
    CompiledAgentVerifyVerdict, COMPILED_AGENT_DEFAULT_ROW_DOC_PATH,
    COMPILED_AGENT_MODULE_EVAL_REPORT_REF, compiled_agent_module_eval_cases,
};

const PROOF_SCHEMA_VERSION: u16 = 1;

pub const COMPILED_AGENT_MODULE_OPTIMIZATION_PROOF_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_module_optimization_proof_report_v1.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleOptimizationFamilyProof {
    pub schema_version: u16,
    pub module: CompiledAgentModuleKind,
    pub family_id: String,
    pub run_spec: OptimizationRunSpec,
    pub run_receipt: OptimizationRunReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_snapshot: Option<OptimizationFrontierSnapshot>,
    pub seed_candidate_id: String,
    pub selected_candidate_id: String,
    pub candidate_manifests: Vec<OptimizationCandidateManifest>,
    pub baseline_case_reports: Vec<CompiledAgentModuleEvalCaseReport>,
    pub optimized_case_reports: Vec<CompiledAgentModuleEvalCaseReport>,
    pub baseline_summary: CompiledAgentModuleEvalSummary,
    pub optimized_summary: CompiledAgentModuleEvalSummary,
    pub generated_from_refs: Vec<String>,
    pub detail: String,
    pub proof_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentModuleOptimizationProofReport {
    pub schema_version: u16,
    pub report_id: String,
    pub row_id: String,
    pub baseline_revision_id: String,
    pub baseline_eval_report_digest: String,
    pub family_proofs: Vec<CompiledAgentModuleOptimizationFamilyProof>,
    pub generated_from_refs: Vec<String>,
    pub summary: String,
    pub detail: String,
    pub report_digest: String,
}

#[derive(Clone)]
struct ModuleFamilyConfig {
    module: CompiledAgentModuleKind,
    family_id: String,
    run_id: String,
    seed_candidate: OptimizationCandidateManifest,
    proposal_candidates: Vec<OptimizationCandidateManifest>,
    generated_from_refs: Vec<String>,
}

#[derive(Clone)]
struct CompiledAgentModuleOptimizerEvaluator {
    module: CompiledAgentModuleKind,
    cases: BTreeMap<String, CompiledAgentModuleEvalCase>,
}

impl CompiledAgentModuleOptimizerEvaluator {
    fn new(module: CompiledAgentModuleKind, cases: Vec<CompiledAgentModuleEvalCase>) -> Self {
        Self {
            module,
            cases: cases
                .into_iter()
                .map(|case| (case.case_id.clone(), case))
                .collect(),
        }
    }

    fn case_report(
        &self,
        candidate: &OptimizationCandidateManifest,
        case: &CompiledAgentModuleEvalCase,
    ) -> CompiledAgentModuleEvalCaseReport {
        match self.module {
            CompiledAgentModuleKind::Route => evaluate_route_candidate_case(candidate, case),
            CompiledAgentModuleKind::ToolPolicy => {
                evaluate_tool_policy_candidate_case(candidate, case)
            }
            CompiledAgentModuleKind::ToolArguments => {
                evaluate_tool_arguments_candidate_case(candidate, case)
            }
            CompiledAgentModuleKind::GroundedAnswer => {
                evaluate_grounded_answer_candidate_case(candidate, case)
            }
            CompiledAgentModuleKind::Verify => evaluate_verify_candidate_case(candidate, case),
        }
    }

    fn case_reports_for_candidate(
        &self,
        candidate: &OptimizationCandidateManifest,
    ) -> Vec<CompiledAgentModuleEvalCaseReport> {
        self.cases
            .values()
            .cloned()
            .map(|case| self.case_report(candidate, &case))
            .collect()
    }
}

impl OptimizationEvaluator for CompiledAgentModuleOptimizerEvaluator {
    fn evaluate_candidate(
        &mut self,
        run_id: &str,
        candidate: &OptimizationCandidateManifest,
        cases: &[OptimizationCaseManifest],
        cache: &mut OptimizationEvaluationCache,
    ) -> psionic_optimize::OptimizationBatchEvaluationReceipt {
        let mut case_receipts = Vec::new();
        let mut cache_hit_count = 0_u32;
        let mut cache_miss_count = 0_u32;

        for case_manifest in cases {
            if let Some(cached) = cache.lookup(candidate, case_manifest).cloned() {
                cache_hit_count += 1;
                case_receipts.push(cached);
                continue;
            }

            let case = self
                .cases
                .get(case_manifest.case_id.as_str())
                .expect("compiled-agent optimizer case");
            let case_report = self.case_report(candidate, case);
            let scalar_score = if case_report.pass { 10_000 } else { 0 };
            let mut objective_scores =
                BTreeMap::from([(String::from("module_pass_bps"), scalar_score)]);
            if case.tags.iter().any(|tag| tag == "unsupported") {
                objective_scores.insert(
                    String::from("unsupported_bps"),
                    if case_report.pass { 10_000 } else { 0 },
                );
            }
            if case.tags.iter().any(|tag| tag == "negated") {
                objective_scores.insert(
                    String::from("negated_bps"),
                    if case_report.pass { 10_000 } else { 0 },
                );
            }
            let shared_feedback = if case_report.pass {
                OptimizationSharedFeedback::new("candidate matched expected module behavior")
            } else {
                OptimizationSharedFeedback::new("candidate missed expected module behavior")
            }
            .with_details(vec![
                format!("expected: {}", case_report.expected_summary),
                format!("observed: {}", case_report.observed_summary),
                case_report.detail.clone(),
            ]);
            let component_feedback = candidate
                .components
                .keys()
                .cloned()
                .map(|component_id| {
                    let summary = if case_report.pass {
                        "component stayed inside the bounded module contract"
                    } else {
                        "component contributed to a bounded module contract miss"
                    };
                    (
                        component_id,
                        OptimizationComponentFeedback::new(summary),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let receipt = OptimizationCaseEvaluationReceipt::new(
                candidate,
                case_manifest,
                scalar_score,
                objective_scores,
                shared_feedback,
                component_feedback,
            );
            cache.insert(candidate, case_manifest, receipt.clone());
            cache_miss_count += 1;
            case_receipts.push(receipt);
        }

        psionic_optimize::OptimizationBatchEvaluationReceipt::new(
            run_id,
            candidate,
            case_receipts,
            cache_hit_count,
            cache_miss_count,
        )
    }
}

#[derive(Clone)]
struct ProofSequenceProposer {
    proposer_kind: String,
    queued_candidates: Vec<OptimizationCandidateManifest>,
}

impl ProofSequenceProposer {
    fn new(
        proposer_kind: impl Into<String>,
        queued_candidates: Vec<OptimizationCandidateManifest>,
    ) -> Self {
        Self {
            proposer_kind: proposer_kind.into(),
            queued_candidates,
        }
    }
}

impl OptimizationCandidateProposer for ProofSequenceProposer {
    fn propose_candidate(
        &mut self,
        state: &OptimizationSearchState,
        current_candidate: &OptimizationCandidateManifest,
        minibatch_receipt: &psionic_optimize::OptimizationBatchEvaluationReceipt,
    ) -> Option<OptimizationCandidateProposal> {
        let candidate = self.queued_candidates.first().cloned()?;
        self.queued_candidates.remove(0);
        let component_diffs = candidate
            .components
            .iter()
            .filter_map(|(component_id, proposed_value)| {
                let previous_value = current_candidate.components.get(component_id)?;
                if previous_value == proposed_value {
                    None
                } else {
                    Some(OptimizationComponentDiff {
                        component_id: component_id.clone(),
                        previous_value: previous_value.clone(),
                        proposed_value: proposed_value.clone(),
                    })
                }
            })
            .collect::<Vec<_>>();
        let proposer_receipt = OptimizationProposerReceipt {
            schema_version: 1,
            report_id: String::from("psionic.optimize.proposer_receipt.v1"),
            run_id: state.run_spec.run_id.clone(),
            proposer_kind: self.proposer_kind.clone(),
            parent_candidate_id: current_candidate.candidate_id.clone(),
            proposed_candidate_id: candidate.candidate_id.clone(),
            source_batch_receipt_digest: minibatch_receipt.receipt_digest.clone(),
            reflective_dataset_digest: None,
            selected_component_ids: candidate.components.keys().cloned().collect(),
            component_diffs,
            prompts: Vec::new(),
            metadata: BTreeMap::new(),
            receipt_digest: String::new(),
        }
        .with_stable_digest();
        Some(OptimizationCandidateProposal {
            candidate,
            proposer_receipt,
            gating_candidate_ids: Vec::new(),
            merge_context: None,
        })
    }
}

#[must_use]
pub fn build_compiled_agent_module_optimization_proof_report(
) -> CompiledAgentModuleOptimizationProofReport {
    let baseline_report = canonical_compiled_agent_module_eval_report();
    let family_proofs = compiled_agent_module_family_configs()
        .into_iter()
        .map(|config| build_family_proof(config, &baseline_report))
        .collect::<Vec<_>>();
    let improved_family_count = family_proofs
        .iter()
        .filter(|proof| proof.optimized_summary.passed_cases > proof.baseline_summary.passed_cases)
        .count();
    let generated_from_refs = vec![
        String::from(COMPILED_AGENT_MODULE_EVAL_REPORT_REF),
        String::from(COMPILED_AGENT_DEFAULT_ROW_DOC_PATH),
        String::from("crates/psionic-eval/src/compiled_agent_module_eval.rs"),
        String::from("crates/psionic-eval/src/compiled_agent_module_optimization_proof.rs"),
    ];
    let mut report = CompiledAgentModuleOptimizationProofReport {
        schema_version: PROOF_SCHEMA_VERSION,
        report_id: String::from("compiled_agent.module_optimization_proof_report.v1"),
        row_id: canonical_compiled_agent_default_row_contract().row_id,
        baseline_revision_id: baseline_report.baseline_revision_id.clone(),
        baseline_eval_report_digest: baseline_report.report_digest.clone(),
        family_proofs,
        generated_from_refs,
        summary: String::new(),
        detail: String::from(
            "This proof report runs the Rust-native optimizer substrate independently over route, tool-policy, tool-argument, grounded-answer, and verify module families on the fixed compiled-agent module corpus. It preserves baseline and optimized case reports plus optimizer receipts, so the proof is bounded fixed-corpus closure rather than a held-out generalization claim.",
        ),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Compiled-agent optimizer proof covers {} bounded module families on one fixed corpus; {} families improve over the baseline while the remaining families keep the seed candidate without cross-module coupling.",
        report.family_proofs.len(),
        improved_family_count,
    );
    report.report_digest = stable_digest(b"compiled_agent_module_optimization_proof_report|", &report);
    report
}

#[must_use]
pub fn canonical_compiled_agent_module_optimization_proof_report(
) -> CompiledAgentModuleOptimizationProofReport {
    build_compiled_agent_module_optimization_proof_report()
}

#[must_use]
pub fn compiled_agent_module_optimization_proof_report_path() -> PathBuf {
    repo_root().join(COMPILED_AGENT_MODULE_OPTIMIZATION_PROOF_REPORT_REF)
}

pub fn write_compiled_agent_module_optimization_proof_report(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleOptimizationProofReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = canonical_compiled_agent_module_optimization_proof_report();
    let json =
        serde_json::to_string_pretty(&report).expect("compiled-agent module optimization proof");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_compiled_agent_module_optimization_proof_report(
    path: impl AsRef<Path>,
) -> Result<CompiledAgentModuleOptimizationProofReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn build_family_proof(
    config: ModuleFamilyConfig,
    baseline_report: &CompiledAgentModuleEvalReport,
) -> CompiledAgentModuleOptimizationFamilyProof {
    let cases = compiled_agent_module_eval_cases()
        .into_iter()
        .filter(|case| case.module == config.module)
        .collect::<Vec<_>>();
    let train_cases = optimizer_case_manifests(cases.as_slice(), OptimizationCaseSplit::Train);
    let validation_cases =
        optimizer_case_manifests(cases.as_slice(), OptimizationCaseSplit::Validation);
    let run_spec = OptimizationRunSpec::new(config.run_id.clone(), config.family_id.clone())
        .with_dataset_refs(vec![String::from(COMPILED_AGENT_MODULE_EVAL_REPORT_REF)])
        .with_issue_ref("OpenAgentsInc/psionic#812")
        .with_frontier_mode(OptimizationFrontierMode::Scalar)
        .with_iteration_budget(config.proposal_candidates.len().max(1) as u32)
        .with_candidate_budget((config.proposal_candidates.len() + 2) as u32);
    let mut evaluator = CompiledAgentModuleOptimizerEvaluator::new(config.module, cases);
    let state = OptimizationEngine::initialize(
        run_spec.clone(),
        config.seed_candidate.clone(),
        train_cases.clone(),
        validation_cases.clone(),
        &mut evaluator,
    )
    .expect("initialize compiled-agent optimizer proof state");

    let (state, run_receipt) = if config.proposal_candidates.is_empty() {
        let frontier_snapshot_refs = state
            .latest_frontier_snapshot
            .as_ref()
            .map(|snapshot| vec![format!("frontier_snapshot_digest:{}", snapshot.snapshot_digest)])
            .unwrap_or_default();
        let run_receipt = OptimizationRunReceipt::from_state(
            &state.lineage_state,
            frontier_snapshot_refs,
            OptimizationStopReason::NoSearchRequired,
        );
        (state, run_receipt)
    } else {
        let mut proposer =
            ProofSequenceProposer::new("compiled_agent_module_sequence", config.proposal_candidates);
        let mut sampler = OptimizationSequentialMinibatchSampler::new(train_cases.len());
        let outcome = OptimizationEngine::run(
            state,
            &mut evaluator,
            &mut proposer,
            &mut sampler,
            Some(1),
        )
        .expect("run compiled-agent optimizer proof");
        (outcome.state, outcome.run_receipt)
    };

    let selected_candidate = state
        .lineage_state
        .candidate(state.current_candidate_id.as_str())
        .expect("selected proof candidate")
        .clone();
    let optimized_case_reports = evaluator.case_reports_for_candidate(&selected_candidate);
    let optimized_summary = summarize_case_reports(config.module, &optimized_case_reports);
    let baseline_case_reports = baseline_report
        .case_reports
        .iter()
        .filter(|case| case.module == config.module)
        .cloned()
        .collect::<Vec<_>>();
    let baseline_summary = baseline_report
        .module_summaries
        .iter()
        .find(|summary| summary.module == config.module)
        .cloned()
        .expect("baseline module summary");
    let candidate_manifests = state
        .lineage_state
        .discovery_order
        .iter()
        .filter_map(|candidate_id| state.lineage_state.candidate(candidate_id.as_str()).cloned())
        .collect::<Vec<_>>();
    let mut proof = CompiledAgentModuleOptimizationFamilyProof {
        schema_version: PROOF_SCHEMA_VERSION,
        module: config.module,
        family_id: config.family_id,
        run_spec,
        run_receipt,
        frontier_snapshot: state.latest_frontier_snapshot.clone(),
        seed_candidate_id: config.seed_candidate.candidate_id,
        selected_candidate_id: selected_candidate.candidate_id.clone(),
        candidate_manifests,
        baseline_case_reports,
        optimized_case_reports,
        baseline_summary: baseline_summary.clone(),
        optimized_summary: optimized_summary.clone(),
        generated_from_refs: config.generated_from_refs,
        detail: format!(
            "Module family `{}` runs on the fixed compiled-agent corpus with seed_pass={}/{} and selected_pass={}/{}.",
            module_family_name(config.module),
            baseline_summary.passed_cases,
            baseline_summary.total_cases,
            optimized_summary.passed_cases,
            optimized_summary.total_cases,
        ),
        proof_digest: String::new(),
    };
    proof.proof_digest = stable_digest(b"compiled_agent_module_optimization_family_proof|", &proof);
    proof
}

fn compiled_agent_module_family_configs() -> Vec<ModuleFamilyConfig> {
    let baseline = compiled_agent_baseline_revision_set();
    vec![
        ModuleFamilyConfig {
            module: CompiledAgentModuleKind::Route,
            family_id: String::from("compiled_agent.route"),
            run_id: String::from("compiled_agent.route.proof_run_v1"),
            seed_candidate: OptimizationCandidateManifest::new(
                "route_seed",
                "compiled_agent.route",
                "compiled_agent.route.proof_run_v1",
                BTreeMap::from([
                    (
                        String::from("provider_route_keywords_json"),
                        serde_json::to_string(&baseline.provider_route_keywords)
                            .expect("provider keywords json"),
                    ),
                    (
                        String::from("wallet_route_keywords_json"),
                        serde_json::to_string(&baseline.wallet_route_keywords)
                            .expect("wallet keywords json"),
                    ),
                    (
                        String::from("negation_keywords_json"),
                        serde_json::to_string(&baseline.negation_keywords)
                            .expect("negation keywords json"),
                    ),
                    (
                        String::from("unsupported_route_keywords_json"),
                        serde_json::to_string(&baseline.unsupported_route_keywords)
                            .expect("unsupported keywords json"),
                    ),
                ]),
            ),
            proposal_candidates: vec![OptimizationCandidateManifest::new(
                "route_negation_guard",
                "compiled_agent.route",
                "compiled_agent.route.proof_run_v1",
                BTreeMap::from([
                    (
                        String::from("provider_route_keywords_json"),
                        serde_json::to_string(&baseline.provider_route_keywords)
                            .expect("provider keywords json"),
                    ),
                    (
                        String::from("wallet_route_keywords_json"),
                        serde_json::to_string(&baseline.wallet_route_keywords)
                            .expect("wallet keywords json"),
                    ),
                    (
                        String::from("negation_keywords_json"),
                        serde_json::to_string(&vec![
                            String::from("not"),
                            String::from("dont"),
                            String::from("don't"),
                        ])
                        .expect("negation keywords json"),
                    ),
                    (
                        String::from("unsupported_route_keywords_json"),
                        serde_json::to_string(&vec![
                            String::from("poem"),
                            String::from("gpus"),
                        ])
                        .expect("unsupported keywords json"),
                    ),
                ]),
            )
            .with_parent_candidate_ids(vec![String::from("route_seed")])],
            generated_from_refs: vec![String::from("crates/psionic-eval/src/compiled_agent_module_eval.rs")],
        },
        ModuleFamilyConfig {
            module: CompiledAgentModuleKind::ToolPolicy,
            family_id: String::from("compiled_agent.tool_policy"),
            run_id: String::from("compiled_agent.tool_policy.proof_run_v1"),
            seed_candidate: OptimizationCandidateManifest::new(
                "tool_policy_seed",
                "compiled_agent.tool_policy",
                "compiled_agent.tool_policy.proof_run_v1",
                BTreeMap::from([
                    (
                        String::from("provider_tool_names_json"),
                        serde_json::to_string(&vec![String::from("provider_status")])
                            .expect("provider tool names"),
                    ),
                    (
                        String::from("wallet_tool_names_json"),
                        serde_json::to_string(&vec![String::from("wallet_status")])
                            .expect("wallet tool names"),
                    ),
                    (
                        String::from("unsupported_tool_names_json"),
                        serde_json::to_string(&Vec::<String>::new())
                            .expect("unsupported tool names"),
                    ),
                ]),
            ),
            proposal_candidates: Vec::new(),
            generated_from_refs: vec![String::from("crates/psionic-eval/src/compiled_agent_module_eval.rs")],
        },
        ModuleFamilyConfig {
            module: CompiledAgentModuleKind::ToolArguments,
            family_id: String::from("compiled_agent.tool_arguments"),
            run_id: String::from("compiled_agent.tool_arguments.proof_run_v1"),
            seed_candidate: OptimizationCandidateManifest::new(
                "tool_arguments_seed",
                "compiled_agent.tool_arguments",
                "compiled_agent.tool_arguments.proof_run_v1",
                BTreeMap::from([(
                    String::from("default_arguments_json"),
                    String::from("{}"),
                )]),
            ),
            proposal_candidates: Vec::new(),
            generated_from_refs: vec![String::from("crates/psionic-eval/src/compiled_agent_module_eval.rs")],
        },
        ModuleFamilyConfig {
            module: CompiledAgentModuleKind::GroundedAnswer,
            family_id: String::from("compiled_agent.grounded_answer"),
            run_id: String::from("compiled_agent.grounded_answer.proof_run_v1"),
            seed_candidate: OptimizationCandidateManifest::new(
                "grounded_answer_seed",
                "compiled_agent.grounded_answer",
                "compiled_agent.grounded_answer.proof_run_v1",
                grounded_answer_components(
                    baseline.include_provider_blockers,
                    baseline.include_recent_earnings,
                    false,
                    baseline.unsupported_template.as_str(),
                    "Grounded facts were unavailable.",
                    "Grounded facts were conflicting.",
                ),
            ),
            proposal_candidates: vec![OptimizationCandidateManifest::new(
                "grounded_answer_fact_fallback",
                "compiled_agent.grounded_answer",
                "compiled_agent.grounded_answer.proof_run_v1",
                grounded_answer_components(
                    true,
                    false,
                    true,
                    baseline.unsupported_template.as_str(),
                    "Grounded facts were unavailable.",
                    "Grounded facts were conflicting.",
                ),
            )
            .with_parent_candidate_ids(vec![String::from("grounded_answer_seed")])],
            generated_from_refs: vec![
                String::from("crates/psionic-eval/src/compiled_agent_module_eval.rs"),
                String::from("crates/psionic-eval/src/compiled_agent_grounded_model.rs"),
            ],
        },
        ModuleFamilyConfig {
            module: CompiledAgentModuleKind::Verify,
            family_id: String::from("compiled_agent.verify"),
            run_id: String::from("compiled_agent.verify.proof_run_v1"),
            seed_candidate: OptimizationCandidateManifest::new(
                "verify_seed",
                "compiled_agent.verify",
                "compiled_agent.verify.proof_run_v1",
                BTreeMap::from([
                    (
                        String::from("verify_require_recent_earnings"),
                        baseline.verify_require_recent_earnings.to_string(),
                    ),
                    (
                        String::from("unsupported_template"),
                        baseline.unsupported_template,
                    ),
                ]),
            ),
            proposal_candidates: Vec::new(),
            generated_from_refs: vec![String::from("crates/psionic-eval/src/compiled_agent_module_eval.rs")],
        },
    ]
}

fn grounded_answer_components(
    include_provider_blockers: bool,
    include_recent_earnings: bool,
    strict_fact_fallback: bool,
    unsupported_template: &str,
    missing_facts_template: &str,
    conflicting_facts_template: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            String::from("include_provider_blockers"),
            include_provider_blockers.to_string(),
        ),
        (
            String::from("include_recent_earnings"),
            include_recent_earnings.to_string(),
        ),
        (
            String::from("strict_fact_fallback"),
            strict_fact_fallback.to_string(),
        ),
        (
            String::from("unsupported_template"),
            String::from(unsupported_template),
        ),
        (
            String::from("missing_facts_template"),
            String::from(missing_facts_template),
        ),
        (
            String::from("conflicting_facts_template"),
            String::from(conflicting_facts_template),
        ),
    ])
}

fn optimizer_case_manifests(
    cases: &[CompiledAgentModuleEvalCase],
    split: OptimizationCaseSplit,
) -> Vec<OptimizationCaseManifest> {
    cases
        .iter()
        .map(|case| {
            OptimizationCaseManifest::new(case.case_id.clone(), split)
                .with_label(expected_summary(case))
                .with_metadata(BTreeMap::from([
                    (
                        String::from("module"),
                        module_family_name(case.module).to_string(),
                    ),
                    (String::from("tag_count"), case.tags.len().to_string()),
                ]))
                .with_evidence_refs(vec![format!("compiled_agent_case:{}", case.case_id)])
        })
        .collect()
}

fn expected_summary(case: &CompiledAgentModuleEvalCase) -> String {
    match case.module {
        CompiledAgentModuleKind::Route => format!(
            "{:?}",
            case.expected_route.expect("route expected summary")
        ),
        CompiledAgentModuleKind::ToolPolicy => case.expected_tool_names.join(", "),
        CompiledAgentModuleKind::ToolArguments => {
            serde_json::to_string(&case.expected_calls).unwrap_or_default()
        }
        CompiledAgentModuleKind::GroundedAnswer => case.expected_answer_substrings.join(", "),
        CompiledAgentModuleKind::Verify => format!(
            "{:?}",
            case.expected_verdict.expect("verify expected summary")
        ),
    }
}

fn summarize_case_reports(
    module: CompiledAgentModuleKind,
    case_reports: &[CompiledAgentModuleEvalCaseReport],
) -> CompiledAgentModuleEvalSummary {
    let total_cases = case_reports.len() as u32;
    let passed_cases = case_reports.iter().filter(|case| case.pass).count() as u32;
    let failed_case_ids = case_reports
        .iter()
        .filter(|case| !case.pass)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut failure_classes = BTreeMap::new();
    for failure_class in case_reports
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

fn module_family_name(module: CompiledAgentModuleKind) -> &'static str {
    match module {
        CompiledAgentModuleKind::Route => "route",
        CompiledAgentModuleKind::ToolPolicy => "tool_policy",
        CompiledAgentModuleKind::ToolArguments => "tool_arguments",
        CompiledAgentModuleKind::GroundedAnswer => "grounded_answer",
        CompiledAgentModuleKind::Verify => "verify",
    }
}

fn route_revision_from_candidate(candidate: &OptimizationCandidateManifest) -> CompiledAgentModuleRevisionSet {
    let mut revision = compiled_agent_baseline_revision_set();
    revision.revision_id = format!("compiled_agent.route.{}", candidate.candidate_id);
    revision.provider_route_keywords =
        parse_string_vec_component(candidate, "provider_route_keywords_json");
    revision.wallet_route_keywords =
        parse_string_vec_component(candidate, "wallet_route_keywords_json");
    revision.negation_keywords = parse_string_vec_component(candidate, "negation_keywords_json");
    revision.unsupported_route_keywords =
        parse_string_vec_component(candidate, "unsupported_route_keywords_json");
    revision
}

fn evaluate_route_candidate_case(
    candidate: &OptimizationCandidateManifest,
    case: &CompiledAgentModuleEvalCase,
) -> CompiledAgentModuleEvalCaseReport {
    let revision = route_revision_from_candidate(candidate);
    let observed = evaluate_compiled_agent_route(case.prompt.as_str(), &revision);
    let expected = case.expected_route.expect("route case");
    let pass = observed == expected;
    CompiledAgentModuleEvalCaseReport {
        case_id: case.case_id.clone(),
        module: case.module,
        pass,
        tags: case.tags.clone(),
        expected_summary: format!("{expected:?}"),
        observed_summary: format!("{observed:?}"),
        failure_class: (!pass).then(|| {
            if case.tags.iter().any(|tag| tag == "negated")
                && observed == CompiledAgentRoute::WalletStatus
            {
                String::from("negated_route_false_positive")
            } else {
                String::from("route_mismatch")
            }
        }),
        detail: case.detail.clone(),
    }
}

fn evaluate_tool_policy_candidate_case(
    candidate: &OptimizationCandidateManifest,
    case: &CompiledAgentModuleEvalCase,
) -> CompiledAgentModuleEvalCaseReport {
    let observed_names = match case.route_input.expect("tool policy route input") {
        CompiledAgentRoute::ProviderStatus => {
            parse_string_vec_component(candidate, "provider_tool_names_json")
        }
        CompiledAgentRoute::WalletStatus => {
            parse_string_vec_component(candidate, "wallet_tool_names_json")
        }
        CompiledAgentRoute::Unsupported => {
            parse_string_vec_component(candidate, "unsupported_tool_names_json")
        }
    };
    let pass = observed_names == case.expected_tool_names;
    CompiledAgentModuleEvalCaseReport {
        case_id: case.case_id.clone(),
        module: case.module,
        pass,
        tags: case.tags.clone(),
        expected_summary: case.expected_tool_names.join(", "),
        observed_summary: observed_names.join(", "),
        failure_class: (!pass).then(|| {
            if case.expected_tool_names.is_empty() && !observed_names.is_empty() {
                String::from("unexpected_tool_exposure")
            } else {
                String::from("tool_policy_mismatch")
            }
        }),
        detail: case.detail.clone(),
    }
}

fn evaluate_tool_arguments_candidate_case(
    candidate: &OptimizationCandidateManifest,
    case: &CompiledAgentModuleEvalCase,
) -> CompiledAgentModuleEvalCaseReport {
    let default_arguments = parse_json_component(candidate, "default_arguments_json");
    let observed = case
        .selected_tools
        .iter()
        .map(|tool_name| CompiledAgentToolCall {
            tool_name: tool_name.clone(),
            arguments: default_arguments.clone(),
        })
        .collect::<Vec<_>>();
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

fn evaluate_grounded_answer_candidate_case(
    candidate: &OptimizationCandidateManifest,
    case: &CompiledAgentModuleEvalCase,
) -> CompiledAgentModuleEvalCaseReport {
    let observed = evaluate_grounded_answer_candidate(
        candidate,
        case.route_input.expect("grounded answer route input"),
        case.tool_results.as_slice(),
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

fn evaluate_grounded_answer_candidate(
    candidate: &OptimizationCandidateManifest,
    route: CompiledAgentRoute,
    tool_results: &[CompiledAgentToolResult],
) -> String {
    let include_provider_blockers = parse_bool_component(candidate, "include_provider_blockers");
    let include_recent_earnings = parse_bool_component(candidate, "include_recent_earnings");
    let strict_fact_fallback = parse_bool_component(candidate, "strict_fact_fallback");
    let unsupported_template = string_component(candidate, "unsupported_template", "");
    let missing_facts_template = string_component(candidate, "missing_facts_template", "");
    let conflicting_facts_template = string_component(candidate, "conflicting_facts_template", "");

    match route {
        CompiledAgentRoute::ProviderStatus => {
            let provider_results = tool_results
                .iter()
                .filter(|tool| tool.tool_name == "provider_status")
                .collect::<Vec<_>>();
            if strict_fact_fallback {
                if provider_results.is_empty()
                    || provider_results
                        .iter()
                        .any(|tool| tool.payload.get("ready").is_none())
                {
                    return missing_facts_template;
                }
                if provider_results.len() > 1 {
                    let signatures = provider_results
                        .iter()
                        .map(|tool| {
                            json!({
                                "ready": tool.payload.get("ready").and_then(Value::as_bool),
                                "blockers": tool.payload.get("blockers"),
                            })
                        })
                        .collect::<Vec<_>>();
                    if signatures.windows(2).any(|window| window[0] != window[1]) {
                        return conflicting_facts_template;
                    }
                }
            }
            let Some(provider) = provider_results.first() else {
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
            } else if include_provider_blockers && !blockers.is_empty() {
                format!(
                    "Provider is not ready to go online. Blockers: {}.",
                    blockers.join(", ")
                )
            } else {
                String::from("Provider is not ready to go online.")
            }
        }
        CompiledAgentRoute::WalletStatus => {
            let wallet_results = tool_results
                .iter()
                .filter(|tool| tool.tool_name == "wallet_status")
                .collect::<Vec<_>>();
            if strict_fact_fallback {
                if wallet_results.is_empty()
                    || wallet_results
                        .iter()
                        .any(|tool| tool.payload.get("balance_sats").is_none())
                {
                    return missing_facts_template;
                }
                if wallet_results.len() > 1 {
                    let signatures = wallet_results
                        .iter()
                        .map(|tool| {
                            json!({
                                "balance_sats": tool.payload.get("balance_sats").and_then(Value::as_u64),
                                "recent_earnings_sats": tool.payload.get("recent_earnings_sats").and_then(Value::as_u64),
                            })
                        })
                        .collect::<Vec<_>>();
                    if signatures.windows(2).any(|window| window[0] != window[1]) {
                        return conflicting_facts_template;
                    }
                }
            }
            let Some(wallet) = wallet_results.first() else {
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
            if include_recent_earnings {
                format!(
                    "Wallet balance is {balance_sats} sats, with {recent_earnings_sats} sats of recent earnings."
                )
            } else {
                format!("The wallet contains {balance_sats} sats.")
            }
        }
        CompiledAgentRoute::Unsupported => unsupported_template,
    }
}

fn evaluate_verify_candidate_case(
    candidate: &OptimizationCandidateManifest,
    case: &CompiledAgentModuleEvalCase,
) -> CompiledAgentModuleEvalCaseReport {
    let mut revision = compiled_agent_baseline_revision_set();
    revision.verify_require_recent_earnings =
        parse_bool_component(candidate, "verify_require_recent_earnings");
    revision.unsupported_template = string_component(candidate, "unsupported_template", "");
    let observed = evaluate_compiled_agent_verify(
        case.route_input.expect("verify route input"),
        &case.selected_tools,
        case.tool_results.as_slice(),
        case.candidate_answer.as_deref().unwrap_or(""),
        &revision,
    );
    let expected = case.expected_verdict.expect("verify expected verdict");
    let pass = observed == expected;
    CompiledAgentModuleEvalCaseReport {
        case_id: case.case_id.clone(),
        module: case.module,
        pass,
        tags: case.tags.clone(),
        expected_summary: format!("{expected:?}"),
        observed_summary: format!("{observed:?}"),
        failure_class: (!pass).then(|| {
            if case
                .tags
                .iter()
                .any(|tag| tag == "tool_emission_is_not_success")
                && observed != CompiledAgentVerifyVerdict::NeedsFallback
            {
                String::from("unsafe_tool_emission_acceptance")
            } else {
                String::from("verifier_mismatch")
            }
        }),
        detail: case.detail.clone(),
    }
}

fn parse_string_vec_component(
    candidate: &OptimizationCandidateManifest,
    component_id: &str,
) -> Vec<String> {
    candidate
        .components
        .get(component_id)
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default()
}

fn parse_json_component(candidate: &OptimizationCandidateManifest, component_id: &str) -> Value {
    candidate
        .components
        .get(component_id)
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_else(|| json!({}))
}

fn parse_bool_component(candidate: &OptimizationCandidateManifest, component_id: &str) -> bool {
    candidate
        .components
        .get(component_id)
        .is_some_and(|value| value == "true")
}

fn string_component(
    candidate: &OptimizationCandidateManifest,
    component_id: &str,
    default_value: &str,
) -> String {
    candidate
        .components
        .get(component_id)
        .cloned()
        .unwrap_or_else(|| String::from(default_value))
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
        canonical_compiled_agent_module_optimization_proof_report,
        compiled_agent_module_optimization_proof_report_path,
        load_compiled_agent_module_optimization_proof_report,
    };
    use crate::CompiledAgentModuleKind;

    #[test]
    fn compiled_agent_module_optimization_proof_keeps_module_families_independent() {
        let report = canonical_compiled_agent_module_optimization_proof_report();
        assert_eq!(report.family_proofs.len(), 5);
        let route = report
            .family_proofs
            .iter()
            .find(|proof| proof.module == CompiledAgentModuleKind::Route)
            .expect("route proof");
        assert!(route.optimized_summary.passed_cases > route.baseline_summary.passed_cases);
        let grounded = report
            .family_proofs
            .iter()
            .find(|proof| proof.module == CompiledAgentModuleKind::GroundedAnswer)
            .expect("grounded proof");
        assert!(
            grounded.optimized_summary.passed_cases > grounded.baseline_summary.passed_cases
        );
        for module in [
            CompiledAgentModuleKind::ToolPolicy,
            CompiledAgentModuleKind::ToolArguments,
            CompiledAgentModuleKind::Verify,
        ] {
            let proof = report
                .family_proofs
                .iter()
                .find(|proof| proof.module == module)
                .expect("seed-only family proof");
            assert_eq!(proof.selected_candidate_id, proof.seed_candidate_id);
            assert_eq!(proof.optimized_summary, proof.baseline_summary);
        }
    }

    #[test]
    fn compiled_agent_module_optimization_proof_report_matches_committed_truth() {
        let expected = canonical_compiled_agent_module_optimization_proof_report();
        let committed = load_compiled_agent_module_optimization_proof_report(
            compiled_agent_module_optimization_proof_report_path(),
        )
        .expect("committed compiled-agent optimization proof report");
        assert_eq!(committed, expected);
    }
}
