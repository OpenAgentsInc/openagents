use std::{fs, path::Path};

use psionic_runtime::TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF;
use psionic_serve::{
    LocalTassadarArticleExecutorSessionService, TassadarArticleExecutorSessionOutcome,
    TassadarArticleExecutorSessionRequest,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_session_artifact.json";

#[derive(Serialize)]
struct ArticleFastRouteExactnessSessionArtifactCase {
    name: String,
    request: TassadarArticleExecutorSessionRequest,
    outcome: TassadarArticleExecutorSessionOutcome,
}

#[derive(Serialize)]
struct ArticleFastRouteExactnessSessionArtifact {
    schema_version: u16,
    benchmark_report_ref: String,
    cases: Vec<ArticleFastRouteExactnessSessionArtifactCase>,
    artifact_digest: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = LocalTassadarArticleExecutorSessionService::new();
    let cases = vec![
        collect_case(
            &service,
            "direct_long_loop_hull",
            TassadarArticleExecutorSessionRequest::new(
                "article-fast-route-exactness-session-long-loop",
                "long_loop_kernel",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "direct_sudoku_v0_hull",
            TassadarArticleExecutorSessionRequest::new(
                "article-fast-route-exactness-session-sudoku-v0",
                "sudoku_v0_test_a",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
        collect_case(
            &service,
            "direct_hungarian_hull",
            TassadarArticleExecutorSessionRequest::new(
                "article-fast-route-exactness-session-hungarian",
                "hungarian_matching",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            ),
        )?,
    ];

    let mut artifact = ArticleFastRouteExactnessSessionArtifact {
        schema_version: 1,
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SESSION_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SESSION_ARTIFACT_REF, artifact.artifact_digest
    );
    Ok(())
}

fn collect_case(
    service: &LocalTassadarArticleExecutorSessionService,
    name: &str,
    request: TassadarArticleExecutorSessionRequest,
) -> Result<ArticleFastRouteExactnessSessionArtifactCase, Box<dyn std::error::Error>> {
    let outcome = service.execute(&request)?;
    Ok(ArticleFastRouteExactnessSessionArtifactCase {
        name: String::from(name),
        request,
        outcome,
    })
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_article_fast_route_exactness_session_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
