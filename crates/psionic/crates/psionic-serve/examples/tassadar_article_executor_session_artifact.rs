use std::{fs, path::Path};

use psionic_runtime::TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF;
use psionic_serve::{
    tassadar_article_executor_session_stream_events_for_outcome,
    LocalTassadarArticleExecutorSessionService, TassadarArticleExecutorSessionOutcome,
    TassadarArticleExecutorSessionRequest, TassadarArticleExecutorSessionStreamEvent,
    ARTICLE_EXECUTOR_SESSION_PRODUCT_ID, TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct ArticleExecutorSessionArtifactCase {
    name: String,
    request: TassadarArticleExecutorSessionRequest,
    outcome: TassadarArticleExecutorSessionOutcome,
    stream_events: Vec<TassadarArticleExecutorSessionStreamEvent>,
}

#[derive(Serialize)]
struct ArticleExecutorSessionArtifact {
    schema_version: u16,
    product_id: String,
    benchmark_report_ref: String,
    cases: Vec<ArticleExecutorSessionArtifactCase>,
    artifact_digest: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = LocalTassadarArticleExecutorSessionService::new();
    let cases = vec![
        collect_case(
            &service,
            "direct_memory_heavy_hull",
            TassadarArticleExecutorSessionRequest::new(
                "article-session-direct-memory-heavy",
                "memory_heavy_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "fallback_branch_heavy_sparse_top_k",
            TassadarArticleExecutorSessionRequest::new(
                "article-session-fallback-branch-heavy",
                "branch_heavy_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::SparseTopK,
            ),
        )?,
        collect_case(
            &service,
            "refusal_non_article_workload",
            TassadarArticleExecutorSessionRequest::new(
                "article-session-refusal-non-article",
                "locals_add",
                psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        )?,
    ];

    let mut artifact = ArticleExecutorSessionArtifact {
        schema_version: 1,
        product_id: String::from(ARTICLE_EXECUTOR_SESSION_PRODUCT_ID),
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF, artifact.artifact_digest
    );
    Ok(())
}

fn collect_case(
    service: &LocalTassadarArticleExecutorSessionService,
    name: &str,
    request: TassadarArticleExecutorSessionRequest,
) -> Result<ArticleExecutorSessionArtifactCase, Box<dyn std::error::Error>> {
    let outcome = service.execute(&request)?;
    let stream_events = tassadar_article_executor_session_stream_events_for_outcome(&outcome);
    Ok(ArticleExecutorSessionArtifactCase {
        name: String::from(name),
        request,
        outcome,
        stream_events,
    })
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_executor_session_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
