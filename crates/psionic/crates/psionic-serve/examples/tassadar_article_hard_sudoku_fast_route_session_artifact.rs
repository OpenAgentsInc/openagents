use std::{fs, path::Path};

use psionic_runtime::{
    TassadarExecutorDecodeMode, TassadarExecutorSelectionState,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
};
use psionic_serve::{
    LocalTassadarArticleExecutorSessionService, TassadarArticleExecutorSessionOutcome,
    TassadarArticleExecutorSessionRequest,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const TASSADAR_ARTICLE_HARD_SUDOKU_FAST_ROUTE_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hard_sudoku_fast_route_session_artifact.json";

#[derive(Serialize)]
struct TassadarArticleHardSudokuFastRouteSessionArtifactCase {
    name: String,
    request: CompactTassadarArticleExecutorSessionRequest,
    outcome: CompactTassadarArticleExecutorSessionOutcome,
}

#[derive(Serialize)]
struct TassadarArticleHardSudokuFastRouteSessionArtifact {
    schema_version: u16,
    benchmark_report_ref: String,
    cases: Vec<TassadarArticleHardSudokuFastRouteSessionArtifactCase>,
    artifact_digest: String,
}

#[derive(Serialize)]
struct CompactTassadarArticleExecutorSessionRequest {
    article_case_id: String,
    requested_decode_mode: TassadarExecutorDecodeMode,
}

#[derive(Serialize)]
struct CompactTassadarArticleExecutorSessionOutcome {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<CompactTassadarArticleExecutorSessionCompletedResponse>,
}

#[derive(Serialize)]
struct CompactTassadarArticleExecutorSessionCompletedResponse {
    benchmark_identity: CompactArticleBenchmarkIdentity,
    executor_response: CompactArticleExecutorResponse,
}

#[derive(Serialize)]
struct CompactArticleBenchmarkIdentity {
    case_id: String,
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
    let service = LocalTassadarArticleExecutorSessionService::new();
    let cases = vec![
        collect_case(
            &service,
            "direct_sudoku_9x9_test_a_hull",
            TassadarArticleExecutorSessionRequest::new(
                "article-hard-sudoku-fast-route-session-test-a",
                "sudoku_9x9_test_a",
                TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "direct_arto_inkala_hull",
            TassadarArticleExecutorSessionRequest::new(
                "article-hard-sudoku-fast-route-session-arto",
                "sudoku_9x9_arto_inkala",
                TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
    ];

    let mut artifact = TassadarArticleHardSudokuFastRouteSessionArtifact {
        schema_version: 1,
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_ARTICLE_HARD_SUDOKU_FAST_ROUTE_SESSION_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_HARD_SUDOKU_FAST_ROUTE_SESSION_ARTIFACT_REF, artifact.artifact_digest
    );
    Ok(())
}

fn collect_case(
    service: &LocalTassadarArticleExecutorSessionService,
    name: &str,
    request: TassadarArticleExecutorSessionRequest,
) -> Result<TassadarArticleHardSudokuFastRouteSessionArtifactCase, Box<dyn std::error::Error>> {
    let outcome = service.execute(&request)?;
    Ok(TassadarArticleHardSudokuFastRouteSessionArtifactCase {
        name: String::from(name),
        request: CompactTassadarArticleExecutorSessionRequest {
            article_case_id: request.article_case_id,
            requested_decode_mode: request.requested_decode_mode,
        },
        outcome: compact_outcome(outcome),
    })
}

fn compact_outcome(
    outcome: TassadarArticleExecutorSessionOutcome,
) -> CompactTassadarArticleExecutorSessionOutcome {
    match outcome {
        TassadarArticleExecutorSessionOutcome::Completed { response } => {
            CompactTassadarArticleExecutorSessionOutcome {
                status: String::from("completed"),
                response: Some(CompactTassadarArticleExecutorSessionCompletedResponse {
                    benchmark_identity: CompactArticleBenchmarkIdentity {
                        case_id: response.benchmark_identity.case_id,
                    },
                    executor_response: CompactArticleExecutorResponse {
                        model_descriptor: CompactArticleExecutorModelDescriptor {
                            model: CompactArticleExecutorModel {
                                model_id: response
                                    .executor_response
                                    .model_descriptor
                                    .model
                                    .model_id,
                            },
                        },
                        execution_report: CompactArticleExecutorExecutionReport {
                            selection: CompactArticleExecutorSelection {
                                effective_decode_mode: response
                                    .executor_response
                                    .execution_report
                                    .selection
                                    .effective_decode_mode,
                                selection_state: response
                                    .executor_response
                                    .execution_report
                                    .selection
                                    .selection_state,
                            },
                            execution: CompactArticleExecution {
                                outputs: response
                                    .executor_response
                                    .execution_report
                                    .execution
                                    .outputs,
                            },
                        },
                    },
                }),
            }
        }
        TassadarArticleExecutorSessionOutcome::Refused { .. } => {
            CompactTassadarArticleExecutorSessionOutcome {
                status: String::from("refused"),
                response: None,
            }
        }
    }
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_hard_sudoku_fast_route_session_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
