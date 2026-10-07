use std::{fs, path::Path};

use psionic_runtime::TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF;
use psionic_serve::{
    LocalTassadarArticleHybridWorkflowService, TassadarArticleHybridWorkflowOutcome,
    TassadarArticleHybridWorkflowRequest,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_HYBRID_WORKFLOW_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_hybrid_workflow_artifact.json";

#[derive(Serialize)]
struct ArticleFastRouteExactnessHybridWorkflowArtifactCase {
    name: String,
    request: TassadarArticleHybridWorkflowRequest,
    outcome: TassadarArticleHybridWorkflowOutcome,
}

#[derive(Serialize)]
struct ArticleFastRouteExactnessHybridWorkflowArtifact {
    schema_version: u16,
    benchmark_report_ref: String,
    cases: Vec<ArticleFastRouteExactnessHybridWorkflowArtifactCase>,
    artifact_digest: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = LocalTassadarArticleHybridWorkflowService::new();
    let cases = vec![
        collect_case(
            &service,
            "delegated_long_loop_hull",
            TassadarArticleHybridWorkflowRequest::new(
                "article-fast-route-exactness-hybrid-long-loop",
                "planner-session-article-long-loop",
                "planner-article-fixture-v0",
                "workflow-step-long-loop",
                "delegate exact long-loop article workload into Tassadar",
                "long_loop_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "delegated_sudoku_v0_hull",
            TassadarArticleHybridWorkflowRequest::new(
                "article-fast-route-exactness-hybrid-sudoku-v0",
                "planner-session-article-sudoku",
                "planner-article-fixture-v0",
                "workflow-step-sudoku-v0",
                "delegate exact Sudoku article workload into Tassadar",
                "sudoku_v0_test_a",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "delegated_hungarian_hull",
            TassadarArticleHybridWorkflowRequest::new(
                "article-fast-route-exactness-hybrid-hungarian",
                "planner-session-article-hungarian",
                "planner-article-fixture-v0",
                "workflow-step-hungarian",
                "delegate exact Hungarian article workload into Tassadar",
                "hungarian_matching",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
    ];

    let mut artifact = ArticleFastRouteExactnessHybridWorkflowArtifact {
        schema_version: 1,
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_HYBRID_WORKFLOW_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_HYBRID_WORKFLOW_ARTIFACT_REF,
        artifact.artifact_digest
    );
    Ok(())
}

fn collect_case(
    service: &LocalTassadarArticleHybridWorkflowService,
    name: &str,
    request: TassadarArticleHybridWorkflowRequest,
) -> Result<ArticleFastRouteExactnessHybridWorkflowArtifactCase, Box<dyn std::error::Error>> {
    let outcome = service.execute(&request)?;
    Ok(ArticleFastRouteExactnessHybridWorkflowArtifactCase {
        name: String::from(name),
        request,
        outcome,
    })
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_fast_route_exactness_hybrid_workflow_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
