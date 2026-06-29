import { describe, expect, test } from "bun:test"
import { proveKhalaM7ConductorComposition } from "../src/khala-m7-conductor-composition"
import { preflightKhalaM7Conductor } from "../src/khala-m7-conductor-preflight"
import type { KhalaM6ShadowPreflightProjection } from "../src/khala-m6-shadow-preflight"
import { assertPublicProjectionSafe } from "../src/state"

const observedAt = "2026-06-23T18:43:00.000Z"

describe("Khala M7 Conductor readiness preflight", () => {
  test("fails closed before the M6 shadow win, backend, training run, and demo evidence exist", () => {
    const preflight = preflightKhalaM7Conductor({ observedAt })

    expect(preflight).toMatchObject({
      schema: "openagents.khala.m7.conductor_preflight.v0.1",
      canStartConductorTraining: false,
      canPublishM7Claim: false,
      contentRedacted: true,
    })
    expect(preflight.blockerRefs).toEqual([
      "blocker.khala.m7.conductor_preflight.crossy_road_composition_missing",
      "blocker.khala.m7.conductor_preflight.m6_shadow_win_missing",
      "blocker.khala.m7.conductor_preflight.owner_approval_ref_missing",
      "blocker.khala.m7.conductor_preflight.owner_confirmation_missing",
      "blocker.khala.m7.conductor_preflight.paid_verdict_source_missing",
      "blocker.khala.m7.conductor_preflight.paid_verdict_source_not_armed",
      "blocker.khala.m7.conductor_preflight.policy_backend_missing",
      "blocker.khala.m7.conductor_preflight.policy_backend_not_wired",
      "blocker.khala.m7.conductor_preflight.publication_ref_missing",
      "blocker.khala.m7.conductor_preflight.spend_cap_missing",
      "blocker.khala.m7.conductor_preflight.training_run_missing",
      "blocker.khala.m7.conductor_preflight.training_run_not_executed",
      "blocker.khala.m7.conductor_preflight.verse_fanout_missing",
    ])
    assertPublicProjectionSafe(preflight)
  })

  test("can start the capped Conductor training run before demo publication evidence exists", () => {
    const preflight = preflightKhalaM7Conductor({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m7.conductor_training.v1",
      dailySpendCapMsats: 10_000_000,
      plannedTrainingSpendMsats: 4_000_000,
      m6ShadowPreflight: publishableM6Preflight(),
      policyBackendRef: "backend.psionic.m7.conductor_7b.fp32_head.v1",
      policyBackendWired: true,
      paidVerdictSourceRef: "verdict.khala.m7.paid_source.v1",
      paidVerdictSourceArmed: true,
    })

    expect(preflight.canStartConductorTraining).toBe(true)
    expect(preflight.canPublishM7Claim).toBe(false)
    expect(preflight.blockerRefs).toEqual([
      "blocker.khala.m7.conductor_preflight.crossy_road_composition_missing",
      "blocker.khala.m7.conductor_preflight.publication_ref_missing",
      "blocker.khala.m7.conductor_preflight.training_run_missing",
      "blocker.khala.m7.conductor_preflight.training_run_not_executed",
      "blocker.khala.m7.conductor_preflight.verse_fanout_missing",
    ])
    expect(preflight.evidenceRefs).toContain("preflight.khala.m6.shadow_run.publishable.v0_1")
    expect(preflight.evidenceRefs).toContain("receipt.khala.m6.paid_shadow_win.v1")
    assertPublicProjectionSafe(preflight)
  })

  test("requires executed training and verified composition before public M7 claim", () => {
    const preflight = preflightKhalaM7Conductor({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m7.conductor_training.v1",
      dailySpendCapMsats: 10_000_000,
      plannedTrainingSpendMsats: 9_000_000,
      m6ShadowPreflight: publishableM6Preflight(),
      policyBackendRef: "backend.psionic.m7.conductor_7b.fp32_head.v1",
      policyBackendWired: true,
      trainingRunRef: "run.psionic.m7.grpo.conductor.v1",
      trainingRunExecuted: true,
      paidVerdictSourceRef: "verdict.khala.m7.paid_source.v1",
      paidVerdictSourceArmed: true,
      verseFanoutRef: "verse.khala.m7.multi_worker_fanout.v1",
      crossyRoadCompositionRef: "receipt.khala.m7.crossy_road_composition.v1",
      crossyRoadCompositionVerified: true,
      publicationRef: "publication.khala.m7.conductor_composition.v1",
    })

    expect(preflight.canStartConductorTraining).toBe(true)
    expect(preflight.canPublishM7Claim).toBe(true)
    expect(preflight.blockerRefs).toEqual([])
    expect(preflight.evidenceRefs).toContain("run.psionic.m7.grpo.conductor.v1")
    expect(preflight.evidenceRefs).toContain("receipt.khala.m7.crossy_road_composition.v1")
    assertPublicProjectionSafe(preflight)
  })

  test("can publish the M7 claim from a structured composition proof", () => {
    const compositionProof = proveKhalaM7ConductorComposition({
      observedAt,
      compositionRunRef: "run.openagents.khala.m7.crossy_road.composition.v1",
      policyBackendRef: "backend.psionic.m7.conductor_7b.fp32_head.v1",
      trainingRunRef: "run.psionic.m7.grpo.conductor.crossy_road.v1",
      trainingRunExecuted: true,
      trainerConfigRef: "config.psionic.m7.tmax_table13.dppo_fp32_lm_head.v1",
      plannerAlgorithm: "grpo_dppo",
      fp32LmHead: true,
      zeroStdFiltered: true,
      workerPoolRefs: [
        "worker.khala.m7.frontier_gateway.planner.v1",
        "worker.pylon.open.khala_code_writer.v1",
        "worker.khala.m2.crossy_road_verifier.v1",
      ],
      topology: [
        {
          stepRef: "step.khala.m7.crossy_road.plan.v1",
          role: "plan",
          workerId: "worker.khala.m7.frontier_gateway.planner.v1",
          workerKind: "frontier_gateway",
          dependsOn: [],
          accessList: [
            "worker.khala.m7.frontier_gateway.planner.v1",
            "worker.pylon.open.khala_code_writer.v1",
            "worker.khala.m2.crossy_road_verifier.v1",
          ],
        },
        {
          stepRef: "step.khala.m7.crossy_road.implement.v1",
          role: "implement",
          workerId: "worker.pylon.open.khala_code_writer.v1",
          workerKind: "open_pylon",
          dependsOn: ["step.khala.m7.crossy_road.plan.v1"],
          accessList: ["worker.pylon.open.khala_code_writer.v1"],
          artifactRef: "artifact.khala.m7.crossy_road.single_html.v1",
        },
        {
          stepRef: "step.khala.m7.crossy_road.verify.v1",
          role: "verify",
          workerId: "worker.khala.m2.crossy_road_verifier.v1",
          workerKind: "verifier",
          dependsOn: ["step.khala.m7.crossy_road.implement.v1"],
          accessList: ["worker.khala.m2.crossy_road_verifier.v1"],
          verdictRef: "verdict.khala.m2.crossy_road.accepted.v1",
        },
        {
          stepRef: "step.khala.m7.crossy_road.refine.v1",
          role: "refine",
          workerId: "worker.pylon.open.khala_code_writer.v1",
          workerKind: "open_pylon",
          dependsOn: ["step.khala.m7.crossy_road.verify.v1"],
          accessList: ["worker.pylon.open.khala_code_writer.v1", "worker.khala.m2.crossy_road_verifier.v1"],
          artifactRef: "artifact.khala.m7.crossy_road.refined_single_html.v1",
        },
      ],
      verdictRef: "verdict.khala.m2.crossy_road.accepted.v1",
      rubricRef: "rubric.khala.m2.crossy_road.v1",
      verdictAccepted: true,
      verseFanoutRef: "verse.khala.m7.crossy_road.multi_worker_fanout.v1",
      fanoutVisible: true,
      compositionCostMsats: 4_200,
      singleModelBaselineCostMsats: 9_400,
      qualityComparable: true,
      publicationRef: "publication.khala.m7.conductor_crossy_road.v1",
    })
    const preflight = preflightKhalaM7Conductor({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m7.conductor_training.v1",
      dailySpendCapMsats: 10_000_000,
      plannedTrainingSpendMsats: 9_000_000,
      m6ShadowPreflight: publishableM6Preflight(),
      policyBackendRef: "backend.psionic.m7.conductor_7b.fp32_head.v1",
      policyBackendWired: true,
      trainingRunRef: "run.psionic.m7.grpo.conductor.crossy_road.v1",
      trainingRunExecuted: true,
      paidVerdictSourceRef: "verdict.khala.m7.paid_source.v1",
      paidVerdictSourceArmed: true,
      compositionProof,
      publicationRef: "publication.khala.m7.conductor_crossy_road.v1",
    })

    expect(compositionProof.canPublishCompositionProof).toBe(true)
    expect(preflight.canPublishM7Claim).toBe(true)
    expect(preflight.blockerRefs).toEqual([])
    expect(preflight.compositionProofRef).toBe(compositionProof.compositionProofRef)
    expect(preflight.verseFanoutRef).toBe("verse.khala.m7.crossy_road.multi_worker_fanout.v1")
    expect(preflight.crossyRoadCompositionRef).toBe(compositionProof.compositionProofRef)
    assertPublicProjectionSafe(preflight)
  })

  test("blocks unsafe refs and over-budget plans", () => {
    const preflight = preflightKhalaM7Conductor({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m7.conductor_training.v1",
      dailySpendCapMsats: 5_000,
      plannedTrainingSpendMsats: 10_000,
      m6ShadowPreflight: publishableM6Preflight(),
      policyBackendRef: "/Users/chris/private/policy.json",
      policyBackendWired: true,
      trainingRunRef: "run.psionic.m7.grpo.conductor.v1",
      trainingRunExecuted: true,
      paidVerdictSourceRef: "verdict.khala.m7.paid_source.v1",
      paidVerdictSourceArmed: true,
      verseFanoutRef: "verse.khala.m7.multi_worker_fanout.v1",
      crossyRoadCompositionRef: "receipt.khala.m7.crossy_road_composition.v1",
      crossyRoadCompositionVerified: true,
      publicationRef: "publication.khala.m7.conductor_composition.v1",
    })

    expect(preflight.canStartConductorTraining).toBe(false)
    expect(preflight.canPublishM7Claim).toBe(false)
    expect(preflight.blockerRefs).toContain("blocker.khala.m7.conductor_preflight.policy_backend_missing")
    expect(preflight.blockerRefs).toContain("blocker.khala.m7.conductor_preflight.spend_cap_exceeded")
    expect(preflight.blockerRefs).toContain("blocker.khala.m7.conductor_preflight.unsafe_ref")
  })
})

function publishableM6Preflight(): KhalaM6ShadowPreflightProjection {
  return {
    schema: "openagents.khala.m6.shadow_run_preflight.v0.1",
    observedAt,
    canStartShadowRun: true,
    canPublishM6Claim: true,
    ownerApprovalRef: "approval.owner.khala_m6.shadow_run.v1",
    dailySpendCapMsats: 10_000_000,
    plannedShadowSpendMsats: 5_000_000,
    psionicTrainingBoundaryRef: "boundary.pylon.psionic_training.ready.v0_3",
    pylonServingPreflightRef: "preflight.pylon.real_serving.ready.v0_1",
    verdictSourceRef: "verdict.khala.m6.live_source.v1",
    shadowCandidateRef: "artifact.psionic.m6.coordinator_candidate.v1",
    baselineRouterRef: "artifact.openagents.khala.heuristic_router.v1",
    liveRolloutRef: "rollout.khala.m6.shadow.live.v1",
    paidShadowWinRef: "receipt.khala.m6.paid_shadow_win.v1",
    publicationRef: "publication.khala.m6.shadow_result.v1",
    evidenceRefs: [
      "approval.owner.khala_m6.shadow_run.v1",
      "artifact.psionic.m6.coordinator_candidate.v1",
      "preflight.pylon.real_serving.ready.v0_1",
      "receipt.khala.m6.paid_shadow_win.v1",
      "rollout.khala.m6.shadow.live.v1",
    ],
    blockerRefs: [],
    externalDependencyRefs: [
      "external.psionic.m6.live_training_driver",
      "external.psionic.m6.shadow_candidate_contract",
    ],
    authorityBoundary:
      "Read-only M6 fixture for the M7 preflight test; no dispatch, spend, or promotion.",
    contentRedacted: true,
  }
}
