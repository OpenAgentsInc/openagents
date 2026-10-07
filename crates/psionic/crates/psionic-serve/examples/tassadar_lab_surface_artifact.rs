use std::{fs, path::Path};

use psionic_serve::{
    LocalTassadarLabService, TassadarArticleExecutorSessionRequest,
    TassadarArticleHybridWorkflowRequest, TassadarLabPreparedView, TassadarLabReplayCatalogEntry,
    TassadarLabReplayId, TassadarLabRequest, TASSADAR_LAB_SURFACE_ARTIFACT_REF,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct TassadarLabSurfaceLiveCase {
    name: String,
    request: TassadarLabRequest,
    prepared_view: TassadarLabPreparedView,
}

#[derive(Serialize)]
struct TassadarLabSurfaceReplayCase {
    replay_id: TassadarLabReplayId,
    prepared_view: TassadarLabPreparedView,
}

#[derive(Serialize)]
struct TassadarLabSurfaceArtifact {
    schema_version: u16,
    surface_id: String,
    replay_catalog: Vec<TassadarLabReplayCatalogEntry>,
    live_cases: Vec<TassadarLabSurfaceLiveCase>,
    replay_cases: Vec<TassadarLabSurfaceReplayCase>,
    artifact_digest: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = LocalTassadarLabService::new();
    let replay_catalog = service.replay_catalog();
    let branch_heavy_fallback_request = {
        let request = TassadarArticleHybridWorkflowRequest::new(
            "tassadar-lab-live-hybrid-fallback-branch-heavy",
            "planner-session-lab-beta",
            "planner-article-fixture-v0",
            "workflow-step-branch-heavy",
            "delegate exact branch-heavy article workload into Tassadar",
            "branch_heavy_kernel",
            psionic_runtime::TassadarExecutorDecodeMode::SparseTopK,
        )
        .with_routing_policy(
            psionic_serve::TassadarPlannerRoutingPolicy::exact_executor_default()
                .with_fallback_policy(psionic_serve::TassadarPlannerFallbackPolicy::PlannerSummary),
        );
        TassadarArticleHybridWorkflowRequest {
            routing_policy: psionic_serve::TassadarPlannerRoutingPolicy {
                allow_runtime_decode_fallback: false,
                ..request.routing_policy
            },
            ..request
        }
    };
    let live_cases = vec![
        collect_live_case(
            &service,
            "article_session_direct_memory_heavy",
            TassadarLabRequest::ArticleExecutorSession {
                request: TassadarArticleExecutorSessionRequest::new(
                    "tassadar-lab-live-direct-memory-heavy",
                    "memory_heavy_kernel",
                    psionic_runtime::TassadarExecutorDecodeMode::HullCache,
                ),
            },
        )?,
        collect_live_case(
            &service,
            "article_session_fallback_branch_heavy",
            TassadarLabRequest::ArticleExecutorSession {
                request: TassadarArticleExecutorSessionRequest::new(
                    "tassadar-lab-live-fallback-branch-heavy",
                    "branch_heavy_kernel",
                    psionic_runtime::TassadarExecutorDecodeMode::SparseTopK,
                ),
            },
        )?,
        collect_live_case(
            &service,
            "article_session_refusal_non_article",
            TassadarLabRequest::ArticleExecutorSession {
                request: TassadarArticleExecutorSessionRequest::new(
                    "tassadar-lab-live-refusal-non-article",
                    "locals_add",
                    psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear,
                ),
            },
        )?,
        collect_live_case(
            &service,
            "article_hybrid_delegated_memory_heavy",
            TassadarLabRequest::ArticleHybridWorkflow {
                request: TassadarArticleHybridWorkflowRequest::new(
                    "tassadar-lab-live-hybrid-delegated-memory-heavy",
                    "planner-session-lab-alpha",
                    "planner-article-fixture-v0",
                    "workflow-step-memory-heavy",
                    "delegate exact memory-heavy article workload into Tassadar",
                    "memory_heavy_kernel",
                    psionic_runtime::TassadarExecutorDecodeMode::HullCache,
                ),
            },
        )?,
        collect_live_case(
            &service,
            "article_hybrid_fallback_branch_heavy",
            TassadarLabRequest::ArticleHybridWorkflow {
                request: branch_heavy_fallback_request,
            },
        )?,
        collect_live_case(
            &service,
            "article_hybrid_refusal_over_budget",
            TassadarLabRequest::ArticleHybridWorkflow {
                request: TassadarArticleHybridWorkflowRequest::new(
                    "tassadar-lab-live-hybrid-refusal-memory-heavy",
                    "planner-session-lab-gamma",
                    "planner-article-fixture-v0",
                    "workflow-step-memory-heavy-refusal",
                    "delegate exact memory-heavy article workload into Tassadar",
                    "memory_heavy_kernel",
                    psionic_runtime::TassadarExecutorDecodeMode::HullCache,
                )
                .with_routing_budget(psionic_serve::TassadarPlannerRoutingBudget::new(1, 32, 8)),
            },
        )?,
    ];
    let replay_cases = replay_catalog
        .iter()
        .map(|entry| {
            let prepared_view = service.prepare(&TassadarLabRequest::Replay {
                replay_id: entry.replay_id,
            })?;
            Ok(TassadarLabSurfaceReplayCase {
                replay_id: entry.replay_id,
                prepared_view,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

    let mut artifact = TassadarLabSurfaceArtifact {
        schema_version: 1,
        surface_id: String::from("psionic.tassadar_lab_surface"),
        replay_catalog,
        live_cases,
        replay_cases,
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(&artifact);

    let path = Path::new(TASSADAR_LAB_SURFACE_ARTIFACT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    println!(
        "wrote {} ({})",
        TASSADAR_LAB_SURFACE_ARTIFACT_REF, artifact.artifact_digest
    );
    Ok(())
}

fn collect_live_case(
    service: &LocalTassadarLabService,
    name: &str,
    request: TassadarLabRequest,
) -> Result<TassadarLabSurfaceLiveCase, Box<dyn std::error::Error>> {
    let prepared_view = service.prepare(&request)?;
    Ok(TassadarLabSurfaceLiveCase {
        name: String::from(name),
        request,
        prepared_view,
    })
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tassadar_lab_surface_artifact|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}
