use std::{error::Error, fs, path::PathBuf};

use psionic_serve::{
    record_psion_served_evidence_bundle, PsionCapabilityMatrix, PsionCapabilityRefusalReason,
    PsionCapabilityRegionId, PsionNoImplicitExecutionStatus, PsionServedArtifactReference,
    PsionServedEvidenceBundle, PsionServedEvidenceLabel, PsionServedRefusalReceipt,
    PsionServedRouteReceipt, PsionServedSourceReference,
};
use psionic_train::{
    PsionPromotionDecisionReceipt, PsionRefusalCalibrationReceipt, PsionRouteClass,
    PsionRouteClassEvaluationReceipt, PsionRouteKind,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_dir = repo_root.join("fixtures/psion/serve");
    fs::create_dir_all(&fixtures_dir)?;

    let capability_matrix: PsionCapabilityMatrix =
        load_json(repo_root.join("fixtures/psion/capability/psion_capability_matrix_v1.json"))?;
    let route_receipt: PsionRouteClassEvaluationReceipt =
        load_json(repo_root.join("fixtures/psion/route/psion_route_class_evaluation_receipt_v1.json"))?;
    let refusal_receipt: PsionRefusalCalibrationReceipt = load_json(
        repo_root.join("fixtures/psion/refusal/psion_refusal_calibration_receipt_v1.json"),
    )?;
    let promotion_decision: PsionPromotionDecisionReceipt = load_json(
        repo_root.join("fixtures/psion/acceptance/psion_promotion_decision_receipt_v1.json"),
    )?;
    let source_manifest: Value =
        load_json(repo_root.join("fixtures/psion/corpus_admission/psion_source_admission_manifest_v1.json"))?;
    let tassadar_article_executor_artifact: Value = load_json(
        repo_root.join("fixtures/tassadar/reports/tassadar_article_executor_session_artifact.json"),
    )?;

    let direct_grounded = direct_grounded_bundle(
        &capability_matrix,
        &route_receipt,
        &promotion_decision,
        &source_manifest,
    )?;
    let executor_backed = executor_backed_bundle(
        &capability_matrix,
        &route_receipt,
        &tassadar_article_executor_artifact,
    )?;
    let refusal = refusal_bundle(&capability_matrix, &refusal_receipt)?;

    write_bundle(
        fixtures_dir.join("psion_served_evidence_direct_grounded_v1.json"),
        &direct_grounded,
    )?;
    write_bundle(
        fixtures_dir.join("psion_served_evidence_executor_backed_v1.json"),
        &executor_backed,
    )?;
    write_bundle(
        fixtures_dir.join("psion_served_evidence_refusal_v1.json"),
        &refusal,
    )?;
    Ok(())
}

fn direct_grounded_bundle(
    capability_matrix: &PsionCapabilityMatrix,
    route_receipt: &PsionRouteClassEvaluationReceipt,
    promotion_decision: &PsionPromotionDecisionReceipt,
    source_manifest: &Value,
) -> Result<PsionServedEvidenceBundle, Box<dyn Error>> {
    let architecture_benchmark = promotion_decision
        .benchmark_receipts
        .first()
        .ok_or("promotion decision is missing benchmark receipts")?;
    let bundle = record_psion_served_evidence_bundle(
        "psion-served-evidence-direct-grounded-v1",
        capability_matrix.matrix_id.clone(),
        capability_matrix.matrix_version.clone(),
        Some(PsionServedRouteReceipt {
            route_kind: PsionRouteKind::DirectModelAnswer,
            route_class: PsionRouteClass::AnswerWithUncertainty,
            capability_region_id: PsionCapabilityRegionId::BoundedTechnicalReasoningShortContext,
            route_boundary_ref: String::from(
                "capability_matrix.supported.bounded_technical_reasoning_short_context",
            ),
            route_calibration_receipt_id: capability_matrix
                .acceptance_basis
                .route_calibration_receipt_ref
                .clone(),
            route_class_evaluation_receipt_id: route_receipt.receipt_id.clone(),
            route_class_evaluation_receipt_digest: route_receipt.receipt_digest.clone(),
            detail: String::from(
                "The served answer stayed in the bounded learned lane with explicit uncertainty rather than silently escalating or implying exact execution.",
            ),
        }),
        None,
        PsionNoImplicitExecutionStatus {
            execution_only_via_explicit_surface: true,
            executor_surface_invoked: false,
            explicit_executor_artifact: None,
            detail: String::from(
                "No exact executor surface was invoked for this answer, so the output must stay a learned judgment plus source-grounded synthesis only.",
            ),
        },
        vec![
            PsionServedEvidenceLabel::LearnedJudgment {
                uncertainty_disclosed: true,
                detail: String::from(
                    "The answer is a bounded learned synthesis over admitted technical material and explicitly marks uncertainty where the model is reasoning beyond quoted source text.",
                ),
            },
            PsionServedEvidenceLabel::SourceGroundedStatement {
                sources: vec![
                    admitted_source(
                        source_manifest,
                        "arch_textbook_foster_1985",
                        "chapter_05.section_02",
                        "Historical architecture grounding for cache and memory-tradeoff language.",
                    )?,
                    admitted_source(
                        source_manifest,
                        "wasm_core_spec_release_2",
                        "section_2.4.7",
                        "Normative spec anchor for the statement about explicit execution semantics and stated constraints.",
                    )?,
                ],
                detail: String::from(
                    "The served statement cites admitted source digests and stable anchors instead of implying generic source-grounding for the whole response.",
                ),
            },
            PsionServedEvidenceLabel::BenchmarkBackedCapabilityClaim {
                capability_region_id: PsionCapabilityRegionId::BoundedTechnicalReasoningShortContext,
                claim_boundary_ref: String::from(
                    "capability_matrix.supported.bounded_technical_reasoning_short_context",
                ),
                promotion_decision_id: promotion_decision.decision_id.clone(),
                benchmark_receipt_id: architecture_benchmark.receipt_id.clone(),
                benchmark_artifact: PsionServedArtifactReference {
                    artifact_id: architecture_benchmark.benchmark_artifact_id.clone(),
                    artifact_digest: architecture_benchmark.benchmark_artifact_digest.clone(),
                },
                detail: String::from(
                    "The capability claim is limited to the published short-context architecture-reasoning region and cites the promoted benchmark artifact plus receipt directly.",
                ),
            },
        ],
        "Direct served example showing learned judgment, source grounding, benchmark-backed capability posture, route evidence, and explicit no-implicit-execution status.",
    )?;
    bundle.validate_against_capability_matrix(capability_matrix)?;
    Ok(bundle)
}

fn executor_backed_bundle(
    capability_matrix: &PsionCapabilityMatrix,
    route_receipt: &PsionRouteClassEvaluationReceipt,
    tassadar_article_executor_artifact: &Value,
) -> Result<PsionServedEvidenceBundle, Box<dyn Error>> {
    let proof_identity = tassadar_article_executor_artifact["cases"][0]["outcome"]["response"]
        ["proof_identity"]
        .as_object()
        .ok_or("missing Tassadar proof identity")?;
    let executor_artifact = PsionServedArtifactReference {
        artifact_id: string_field(proof_identity, "trace_artifact_id")?,
        artifact_digest: string_field(proof_identity, "trace_artifact_digest")?,
    };
    let bundle = record_psion_served_evidence_bundle(
        "psion-served-evidence-executor-backed-v1",
        capability_matrix.matrix_id.clone(),
        capability_matrix.matrix_version.clone(),
        Some(PsionServedRouteReceipt {
            route_kind: PsionRouteKind::ExactExecutorHandoff,
            route_class: PsionRouteClass::DelegateToExactExecutor,
            capability_region_id: PsionCapabilityRegionId::VerifiedOrExactExecutionRequests,
            route_boundary_ref: String::from(
                "capability_matrix.route_required.verified_or_exact_execution_requests",
            ),
            route_calibration_receipt_id: capability_matrix
                .acceptance_basis
                .route_calibration_receipt_ref
                .clone(),
            route_class_evaluation_receipt_id: route_receipt.receipt_id.clone(),
            route_class_evaluation_receipt_digest: route_receipt.receipt_digest.clone(),
            detail: String::from(
                "The request was routed to an explicit exact executor surface instead of letting the learned lane imply execution through prose.",
            ),
        }),
        None,
        PsionNoImplicitExecutionStatus {
            execution_only_via_explicit_surface: true,
            executor_surface_invoked: true,
            explicit_executor_artifact: Some(executor_artifact.clone()),
            detail: String::from(
                "Execution was only claimed because the request used a published executor surface and attached a concrete executor artifact reference.",
            ),
        },
        vec![PsionServedEvidenceLabel::ExecutorBackedResult {
            executor_surface_product_id: String::from("psionic.executor_trace"),
            executor_artifact,
            detail: String::from(
                "The result is backed by a concrete Tassadar trace artifact rather than by an ungrounded language-only answer.",
            ),
        }],
        "Exact-executor example showing explicit route evidence, executor-backed result labeling, and no-implicit-execution enforcement.",
    )?;
    bundle.validate_against_capability_matrix(capability_matrix)?;
    Ok(bundle)
}

fn refusal_bundle(
    capability_matrix: &PsionCapabilityMatrix,
    refusal_receipt: &PsionRefusalCalibrationReceipt,
) -> Result<PsionServedEvidenceBundle, Box<dyn Error>> {
    let bundle = record_psion_served_evidence_bundle(
        "psion-served-evidence-refusal-v1",
        capability_matrix.matrix_id.clone(),
        capability_matrix.matrix_version.clone(),
        None,
        Some(PsionServedRefusalReceipt {
            capability_region_id:
                PsionCapabilityRegionId::UnsupportedExactExecutionWithoutExecutorSurface,
            refusal_reason: PsionCapabilityRefusalReason::UnsupportedExactnessRequest,
            refusal_boundary_ref: String::from(
                "capability_matrix.refusal_required.unsupported_exact_execution_without_executor_surface",
            ),
            refusal_calibration_receipt_id: refusal_receipt.receipt_id.clone(),
            refusal_calibration_receipt_digest: refusal_receipt.receipt_digest.clone(),
            detail: String::from(
                "The refusal names the unsupported exactness boundary directly instead of implying hidden executor availability.",
            ),
        }),
        PsionNoImplicitExecutionStatus {
            execution_only_via_explicit_surface: true,
            executor_surface_invoked: false,
            explicit_executor_artifact: None,
            detail: String::from(
                "No executor surface was invoked, and the refusal makes that explicit instead of implying unavailable exact execution.",
            ),
        },
        Vec::new(),
        "Refusal example showing typed refusal reason, refusal receipt binding, and explicit no-implicit-execution posture without answer-side evidence labels.",
    )?;
    bundle.validate_against_capability_matrix(capability_matrix)?;
    Ok(bundle)
}

fn admitted_source(
    source_manifest: &Value,
    source_id: &str,
    boundary_ref: &str,
    detail: &str,
) -> Result<PsionServedSourceReference, Box<dyn Error>> {
    let source = source_manifest["sources"]
        .as_array()
        .ok_or("source manifest is missing `sources`")?
        .iter()
        .find(|source| source["source_id"].as_str() == Some(source_id))
        .ok_or("source manifest is missing the requested source id")?;
    Ok(PsionServedSourceReference {
        source_id: source_id.to_string(),
        source_digest: source["content_digest"]
            .as_str()
            .ok_or("source manifest entry is missing `content_digest`")?
            .to_string(),
        boundary_ref: boundary_ref.to_string(),
        detail: detail.to_string(),
    })
}

fn write_bundle(path: PathBuf, bundle: &PsionServedEvidenceBundle) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string_pretty(bundle)?)?;
    Ok(())
}

fn string_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<String, Box<dyn Error>> {
    Ok(object
        .get(key)
        .and_then(Value::as_str)
        .ok_or("missing string field")?
        .to_string())
}

fn load_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}
