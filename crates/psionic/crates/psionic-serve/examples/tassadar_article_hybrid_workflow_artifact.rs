use std::{fs, path::Path};

use psionic_runtime::TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF;
use psionic_serve::{
    ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID, LocalTassadarArticleHybridWorkflowService,
    TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF, TassadarArticleHybridWorkflowOutcome,
    TassadarArticleHybridWorkflowRequest, TassadarPlannerFallbackPolicy,
    TassadarPlannerRoutingBudget, TassadarPlannerRoutingPolicy,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct ArticleHybridWorkflowArtifactCase {
    name: String,
    request: TassadarArticleHybridWorkflowRequest,
    outcome: TassadarArticleHybridWorkflowOutcome,
}

#[derive(Serialize)]
struct ArticleHybridWorkflowArtifact {
    schema_version: u16,
    product_id: String,
    benchmark_report_ref: String,
    cases: Vec<ArticleHybridWorkflowArtifactCase>,
    artifact_digest: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = LocalTassadarArticleHybridWorkflowService::new();
    let cases = vec![
        collect_case(
            &service,
            "delegated_memory_heavy_hull",
            TassadarArticleHybridWorkflowRequest::new(
                "article-hybrid-delegated-memory-heavy",
                "planner-session-article-alpha",
                "planner-article-fixture-v0",
                "workflow-step-memory-heavy",
                "delegate exact memory-heavy article workload into Tassadar",
                "memory_heavy_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "fallback_branch_heavy_sparse_top_k",
            TassadarArticleHybridWorkflowRequest::new(
                "article-hybrid-fallback-branch-heavy",
                "planner-session-article-beta",
                "planner-article-fixture-v0",
                "workflow-step-branch-heavy",
                "delegate exact branch-heavy article workload into Tassadar",
                "branch_heavy_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::SparseTopK,
            )
            .with_routing_policy(
                TassadarPlannerRoutingPolicy::exact_executor_default()
                    .with_fallback_policy(TassadarPlannerFallbackPolicy::PlannerSummary),
            )
            .with_routing_budget(TassadarPlannerRoutingBudget::new(128, 512, 8)),
        )?,
        collect_case(
            &service,
            "refusal_overbudget_memory_heavy",
            TassadarArticleHybridWorkflowRequest::new(
                "article-hybrid-refusal-memory-heavy",
                "planner-session-article-gamma",
                "planner-article-fixture-v0",
                "workflow-step-memory-heavy-refusal",
                "delegate exact memory-heavy article workload into Tassadar",
                "memory_heavy_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            )
            .with_routing_budget(TassadarPlannerRoutingBudget::new(1, 32, 8)),
        )?,
    ];

    let mut artifact = ArticleHybridWorkflowArtifact {
        schema_version: 1,
        product_id: String::from(ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID),
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF, artifact.artifact_digest
    );
    Ok(())
}

fn collect_case(
    service: &LocalTassadarArticleHybridWorkflowService,
    name: &str,
    mut request: TassadarArticleHybridWorkflowRequest,
) -> Result<ArticleHybridWorkflowArtifactCase, Box<dyn std::error::Error>> {
    if name == "fallback_branch_heavy_sparse_top_k" {
        request = TassadarArticleHybridWorkflowRequest {
            routing_policy: TassadarPlannerRoutingPolicy {
                allow_runtime_decode_fallback: false,
                ..request.routing_policy
            },
            ..request
        };
    }
    let outcome = service.execute(&request)?;
    Ok(ArticleHybridWorkflowArtifactCase {
        name: String::from(name),
        request,
        outcome,
    })
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_hybrid_workflow_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
