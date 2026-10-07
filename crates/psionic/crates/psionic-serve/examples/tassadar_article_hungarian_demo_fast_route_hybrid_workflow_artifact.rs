use std::{fs, path::Path};

use psionic_runtime::{
    TassadarExecutorDecodeMode, TassadarExecutorSelectionState,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
};
use psionic_serve::{
    LocalTassadarArticleHybridWorkflowService, TassadarArticleHybridWorkflowOutcome,
    TassadarArticleHybridWorkflowRequest,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const TASSADAR_ARTICLE_HUNGARIAN_DEMO_FAST_ROUTE_HYBRID_WORKFLOW_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hungarian_demo_fast_route_hybrid_workflow_artifact.json";

#[derive(Serialize)]
struct TassadarArticleHungarianDemoFastRouteHybridWorkflowArtifactCase {
    name: String,
    request: CompactTassadarArticleHybridWorkflowRequest,
    outcome: CompactTassadarArticleHybridWorkflowOutcome,
}

#[derive(Serialize)]
struct TassadarArticleHungarianDemoFastRouteHybridWorkflowArtifact {
    schema_version: u16,
    benchmark_report_ref: String,
    cases: Vec<TassadarArticleHungarianDemoFastRouteHybridWorkflowArtifactCase>,
    artifact_digest: String,
}

#[derive(Serialize)]
struct CompactTassadarArticleHybridWorkflowRequest {
    article_case_id: String,
    requested_decode_mode: TassadarExecutorDecodeMode,
}

#[derive(Serialize)]
struct CompactTassadarArticleHybridWorkflowOutcome {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<CompactTassadarArticleHybridWorkflowCompletedResponse>,
}

#[derive(Serialize)]
struct CompactTassadarArticleHybridWorkflowCompletedResponse {
    benchmark_identity: CompactArticleBenchmarkIdentity,
    planner_response: CompactArticleHybridPlannerResponse,
}

#[derive(Serialize)]
struct CompactArticleBenchmarkIdentity {
    case_id: String,
}

#[derive(Serialize)]
struct CompactArticleHybridPlannerResponse {
    routing_decision: CompactArticleHybridRoutingDecision,
    executor_response: CompactArticleExecutorResponse,
}

#[derive(Serialize)]
struct CompactArticleHybridRoutingDecision {
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
}

#[derive(Serialize)]
struct CompactArticleExecutorResponse {
    model_descriptor: CompactArticleExecutorModelDescriptor,
    execution_report: CompactArticleExecutorExecutionReport,
}

#[derive(Serialize)]
struct CompactArticleExecutorModelDescriptor {
    model: CompactArticleExecutorModel,
}

#[derive(Serialize)]
struct CompactArticleExecutorModel {
    model_id: String,
}

#[derive(Serialize)]
struct CompactArticleExecutorExecutionReport {
    selection: CompactArticleExecutorSelection,
    execution: CompactArticleExecution,
}

#[derive(Serialize)]
struct CompactArticleExecutorSelection {
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
}

#[derive(Serialize)]
struct CompactArticleExecution {
    outputs: Vec<i32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = LocalTassadarArticleHybridWorkflowService::new();
    let cases = vec![collect_case(
        &service,
        "delegated_hungarian_10x10_hull",
        TassadarArticleHybridWorkflowRequest::new(
            "article-hungarian-demo-fast-route-hybrid",
            "planner-session-article-hungarian-10x10",
            "planner-article-fixture-v0",
            "workflow-step-hungarian-10x10",
            "delegate exact Hungarian 10x10 article workload into Tassadar",
            "hungarian_10x10_test_a",
            psionic_runtime::TassadarExecutorDecodeMode::HullCache,
        ),
    )?];

    let mut artifact = TassadarArticleHungarianDemoFastRouteHybridWorkflowArtifact {
        schema_version: 1,
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_ARTICLE_HUNGARIAN_DEMO_FAST_ROUTE_HYBRID_WORKFLOW_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_HUNGARIAN_DEMO_FAST_ROUTE_HYBRID_WORKFLOW_ARTIFACT_REF,
        artifact.artifact_digest
    );
    Ok(())
}

fn collect_case(
    service: &LocalTassadarArticleHybridWorkflowService,
    name: &str,
    request: TassadarArticleHybridWorkflowRequest,
) -> Result<
    TassadarArticleHungarianDemoFastRouteHybridWorkflowArtifactCase,
    Box<dyn std::error::Error>,
> {
    let outcome = service.execute(&request)?;
    Ok(
        TassadarArticleHungarianDemoFastRouteHybridWorkflowArtifactCase {
            name: String::from(name),
            request: CompactTassadarArticleHybridWorkflowRequest {
                article_case_id: request.article_case_id,
                requested_decode_mode: request.requested_decode_mode,
            },
            outcome: compact_outcome(outcome),
        },
    )
}

fn compact_outcome(
    outcome: TassadarArticleHybridWorkflowOutcome,
) -> CompactTassadarArticleHybridWorkflowOutcome {
    match outcome {
        TassadarArticleHybridWorkflowOutcome::Completed { response } => {
            CompactTassadarArticleHybridWorkflowOutcome {
                status: String::from("completed"),
                response: Some(CompactTassadarArticleHybridWorkflowCompletedResponse {
                    benchmark_identity: CompactArticleBenchmarkIdentity {
                        case_id: response.benchmark_identity.case_id,
                    },
                    planner_response: CompactArticleHybridPlannerResponse {
                        routing_decision: CompactArticleHybridRoutingDecision {
                            effective_decode_mode: response
                                .planner_response
                                .routing_decision
                                .effective_decode_mode,
                        },
                        executor_response: CompactArticleExecutorResponse {
                            model_descriptor: CompactArticleExecutorModelDescriptor {
                                model: CompactArticleExecutorModel {
                                    model_id: response
                                        .planner_response
                                        .executor_response
                                        .model_descriptor
                                        .model
                                        .model_id,
                                },
                            },
                            execution_report: CompactArticleExecutorExecutionReport {
                                selection: CompactArticleExecutorSelection {
                                    effective_decode_mode: response
                                        .planner_response
                                        .executor_response
                                        .execution_report
                                        .selection
                                        .effective_decode_mode,
                                    selection_state: response
                                        .planner_response
                                        .executor_response
                                        .execution_report
                                        .selection
                                        .selection_state,
                                },
                                execution: CompactArticleExecution {
                                    outputs: response
                                        .planner_response
                                        .executor_response
                                        .execution_report
                                        .execution
                                        .outputs,
                                },
                            },
                        },
                    },
                }),
            }
        }
        TassadarArticleHybridWorkflowOutcome::Refused { .. } => {
            CompactTassadarArticleHybridWorkflowOutcome {
                status: String::from("refused"),
                response: None,
            }
        }
        TassadarArticleHybridWorkflowOutcome::Fallback { .. } => {
            CompactTassadarArticleHybridWorkflowOutcome {
                status: String::from("fallback"),
                response: None,
            }
        }
    }
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_hungarian_demo_fast_route_hybrid_workflow_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
