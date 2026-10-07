use std::{error::Error, fs, path::PathBuf};

use psionic_serve::{
    record_psion_served_output_claim_posture, PsionCapabilityMatrix,
    PsionServedAssumptionKind, PsionServedAssumptionNotice, PsionServedBehaviorVisibility,
    PsionServedContextEnvelopeSurface, PsionServedEvidenceBundle,
    PsionServedLatencyEnvelopeSurface, PsionServedOutputClaimPosture,
    PsionServedVisibleClaims,
};
use serde::de::DeserializeOwned;

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures_dir = repo_root.join("fixtures/psion/serve");
    fs::create_dir_all(&fixtures_dir)?;

    let capability_matrix: PsionCapabilityMatrix =
        load_json(repo_root.join("fixtures/psion/capability/psion_capability_matrix_v1.json"))?;
    let direct_evidence: PsionServedEvidenceBundle = load_json(
        repo_root.join("fixtures/psion/serve/psion_served_evidence_direct_grounded_v1.json"),
    )?;
    let executor_evidence: PsionServedEvidenceBundle = load_json(
        repo_root.join("fixtures/psion/serve/psion_served_evidence_executor_backed_v1.json"),
    )?;
    let refusal_evidence: PsionServedEvidenceBundle = load_json(
        repo_root.join("fixtures/psion/serve/psion_served_evidence_refusal_v1.json"),
    )?;

    let direct = direct_claim_posture(&capability_matrix, &direct_evidence)?;
    let executor = executor_claim_posture(&capability_matrix, &executor_evidence)?;
    let refusal = refusal_claim_posture(&capability_matrix, &refusal_evidence)?;

    write_posture(
        fixtures_dir.join("psion_served_output_claim_direct_v1.json"),
        &direct,
    )?;
    write_posture(
        fixtures_dir.join("psion_served_output_claim_executor_backed_v1.json"),
        &executor,
    )?;
    write_posture(
        fixtures_dir.join("psion_served_output_claim_refusal_v1.json"),
        &refusal,
    )?;
    Ok(())
}

fn direct_claim_posture(
    capability_matrix: &PsionCapabilityMatrix,
    evidence_bundle: &PsionServedEvidenceBundle,
) -> Result<PsionServedOutputClaimPosture, Box<dyn Error>> {
    Ok(record_psion_served_output_claim_posture(
        "psion-served-output-claim-direct-v1",
        capability_matrix,
        evidence_bundle,
        PsionServedVisibleClaims {
            learned_judgment_visible: true,
            source_grounding_visible: true,
            executor_backing_visible: false,
            benchmark_backing_visible: true,
            verification_visible: false,
        },
        vec![
            PsionServedAssumptionNotice {
                assumption_id: String::from("assume_short_context_envelope"),
                kind: PsionServedAssumptionKind::InputConstraint,
                required_for_interpretation: true,
                detail: String::from(
                    "The answer assumes the request stays inside the published short-context technical reasoning envelope instead of silently stretching into longer hidden context.",
                ),
            },
            PsionServedAssumptionNotice {
                assumption_id: String::from("assume_no_hidden_run_state"),
                kind: PsionServedAssumptionKind::EnvironmentBoundary,
                required_for_interpretation: true,
                detail: String::from(
                    "The answer assumes no hidden runtime artifact, current tool state, or fresh external fact is needed beyond the attached source and benchmark artifacts.",
                ),
            },
        ],
        PsionServedBehaviorVisibility::Route {
            route_kind: psionic_train::PsionRouteKind::DirectModelAnswer,
            route_class: psionic_train::PsionRouteClass::AnswerWithUncertainty,
            detail: String::from(
                "The served output stayed in the bounded learned lane and surfaced uncertainty explicitly instead of implying exact execution or hidden proof.",
            ),
        },
        context_surface(capability_matrix, 2048),
        latency_surface(capability_matrix),
        "Direct-output claim posture showing explicit assumptions, visible direct-route behavior, and only the claims justified by attached source and benchmark evidence.",
    )?)
}

fn executor_claim_posture(
    capability_matrix: &PsionCapabilityMatrix,
    evidence_bundle: &PsionServedEvidenceBundle,
) -> Result<PsionServedOutputClaimPosture, Box<dyn Error>> {
    Ok(record_psion_served_output_claim_posture(
        "psion-served-output-claim-executor-backed-v1",
        capability_matrix,
        evidence_bundle,
        PsionServedVisibleClaims {
            learned_judgment_visible: false,
            source_grounding_visible: false,
            executor_backing_visible: true,
            benchmark_backing_visible: false,
            verification_visible: false,
        },
        vec![PsionServedAssumptionNotice {
            assumption_id: String::from("assume_declared_executor_inputs"),
            kind: PsionServedAssumptionKind::InputConstraint,
            required_for_interpretation: true,
            detail: String::from(
                "The exact result only covers the declared inputs and the attached executor artifact; it does not widen into benchmark proof or formal verification claims.",
            ),
        }],
        PsionServedBehaviorVisibility::Route {
            route_kind: psionic_train::PsionRouteKind::ExactExecutorHandoff,
            route_class: psionic_train::PsionRouteClass::DelegateToExactExecutor,
            detail: String::from(
                "The served output makes the exact-executor handoff visible instead of presenting the result as if the learned lane executed it directly.",
            ),
        },
        context_surface(capability_matrix, 4096),
        latency_surface(capability_matrix),
        "Executor-backed claim posture showing one explicit handoff, one executor-backed claim, and no unsupported verification or benchmark-proof implication.",
    )?)
}

fn refusal_claim_posture(
    capability_matrix: &PsionCapabilityMatrix,
    evidence_bundle: &PsionServedEvidenceBundle,
) -> Result<PsionServedOutputClaimPosture, Box<dyn Error>> {
    Ok(record_psion_served_output_claim_posture(
        "psion-served-output-claim-refusal-v1",
        capability_matrix,
        evidence_bundle,
        PsionServedVisibleClaims {
            learned_judgment_visible: false,
            source_grounding_visible: false,
            executor_backing_visible: false,
            benchmark_backing_visible: false,
            verification_visible: false,
        },
        vec![PsionServedAssumptionNotice {
            assumption_id: String::from("assume_no_published_executor_surface"),
            kind: PsionServedAssumptionKind::EnvironmentBoundary,
            required_for_interpretation: true,
            detail: String::from(
                "The refusal assumes no published exact executor surface is attached on the current learned-lane endpoint, so the request must stay refused rather than answered speculatively.",
            ),
        }],
        PsionServedBehaviorVisibility::Refusal {
            capability_region_id: psionic_serve::PsionCapabilityRegionId::UnsupportedExactExecutionWithoutExecutorSurface,
            refusal_reason: psionic_serve::PsionCapabilityRefusalReason::UnsupportedExactnessRequest,
            detail: String::from(
                "The served output keeps the refusal visible and names the unsupported exactness boundary instead of hinting that hidden execution might still be available.",
            ),
        },
        context_surface(capability_matrix, 1024),
        latency_surface(capability_matrix),
        "Refusal claim posture showing one explicit assumption, visible typed refusal behavior, and zero unsupported source, benchmark, executor, or verification claims.",
    )?)
}

fn context_surface(
    capability_matrix: &PsionCapabilityMatrix,
    prompt_tokens_observed: u32,
) -> PsionServedContextEnvelopeSurface {
    PsionServedContextEnvelopeSurface {
        supported_prompt_tokens: capability_matrix.context_envelope.supported_prompt_tokens,
        supported_completion_tokens: capability_matrix.context_envelope.supported_completion_tokens,
        route_required_above_prompt_tokens: capability_matrix
            .context_envelope
            .route_required_above_prompt_tokens,
        hard_refusal_above_prompt_tokens: capability_matrix
            .context_envelope
            .hard_refusal_above_prompt_tokens,
        prompt_tokens_observed,
        detail: capability_matrix.context_envelope.detail.clone(),
    }
}

fn latency_surface(capability_matrix: &PsionCapabilityMatrix) -> PsionServedLatencyEnvelopeSurface {
    PsionServedLatencyEnvelopeSurface {
        p50_first_token_latency_ms: capability_matrix
            .latency_envelope
            .p50_first_token_latency_ms,
        p95_first_token_latency_ms: capability_matrix
            .latency_envelope
            .p95_first_token_latency_ms,
        p95_end_to_end_latency_ms: capability_matrix
            .latency_envelope
            .p95_end_to_end_latency_ms,
        detail: capability_matrix.latency_envelope.detail.clone(),
    }
}

fn write_posture(
    path: PathBuf,
    posture: &PsionServedOutputClaimPosture,
) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string_pretty(posture)?)?;
    Ok(())
}

fn load_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}
