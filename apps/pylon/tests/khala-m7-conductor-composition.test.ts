import { describe, expect, test } from "bun:test"
import {
  proveKhalaM7ConductorComposition,
  type KhalaM7ConductorCompositionStep,
} from "../src/khala-m7-conductor-composition"
import { assertPublicProjectionSafe } from "../src/state"

const observedAt = "2026-06-24T17:20:00.000Z"

describe("Khala M7 Conductor composition proof", () => {
  test("fails closed before the executed planner, topology, verdict, fan-out, and cost evidence exist", () => {
    const proof = proveKhalaM7ConductorComposition({ observedAt })

    expect(proof).toMatchObject({
      schema: "openagents.khala.m7.conductor_composition_proof.v0.1",
      canPublishCompositionProof: false,
      compositionProofRef: null,
      contentRedacted: true,
    })
    expect(proof.blockerRefs).toEqual([
      "blocker.khala.m7.conductor_composition.cost_comparison_missing",
      "blocker.khala.m7.conductor_composition.non_tmax_recipe",
      "blocker.khala.m7.conductor_composition.policy_backend_missing",
      "blocker.khala.m7.conductor_composition.publication_ref_missing",
      "blocker.khala.m7.conductor_composition.quality_not_comparable",
      "blocker.khala.m7.conductor_composition.run_ref_missing",
      "blocker.khala.m7.conductor_composition.topology_incomplete",
      "blocker.khala.m7.conductor_composition.topology_missing",
      "blocker.khala.m7.conductor_composition.training_run_missing",
      "blocker.khala.m7.conductor_composition.training_run_not_executed",
      "blocker.khala.m7.conductor_composition.verdict_missing",
      "blocker.khala.m7.conductor_composition.verdict_not_accepted",
      "blocker.khala.m7.conductor_composition.verse_fanout_missing",
      "blocker.khala.m7.conductor_composition.worker_pool_incomplete",
      "blocker.khala.m7.conductor_composition.worker_pool_missing",
    ])
    assertPublicProjectionSafe(proof)
  })

  test("publishes a proof for a verified lower-cost crossy-road composition", () => {
    const proof = proveKhalaM7ConductorComposition(validCompositionInput())

    expect(proof.canPublishCompositionProof).toBe(true)
    expect(proof.blockerRefs).toEqual([])
    expect(proof.compositionProofRef).toMatch(/^receipt\.khala\.m7\.conductor_composition\.[a-f0-9]{20}$/)
    expect(proof.evidenceRefs).toContain("run.psionic.m7.grpo.conductor.crossy_road.v1")
    expect(proof.evidenceRefs).toContain("step.khala.m7.crossy_road.verify.v1")
    expect(proof.evidenceRefs).toContain("verdict.khala.m2.crossy_road.accepted.v1")
    expect(proof.compositionCostMsats).toBe(4_200)
    expect(proof.singleModelBaselineCostMsats).toBe(9_400)
    assertPublicProjectionSafe(proof)
  })

  test("blocks invalid worker topology and non-lower-cost claims", () => {
    const proof = proveKhalaM7ConductorComposition({
      ...validCompositionInput(),
      topology: [
        ...validTopology().slice(0, 2),
        {
          stepRef: "step.khala.m7.crossy_road.verify.v1",
          role: "verify",
          workerId: "worker.khala.m7.unknown_verifier",
          workerKind: "verifier",
          dependsOn: ["step.khala.m7.crossy_road.implement.v1"],
          accessList: ["worker.khala.m7.unknown_verifier"],
          verdictRef: "verdict.khala.m2.crossy_road.accepted.v1",
        },
        validTopology()[3],
      ],
      compositionCostMsats: 9_400,
      singleModelBaselineCostMsats: 9_400,
    })

    expect(proof.canPublishCompositionProof).toBe(false)
    expect(proof.blockerRefs).toContain("blocker.khala.m7.conductor_composition.topology_invalid")
    expect(proof.blockerRefs).toContain("blocker.khala.m7.conductor_composition.cost_not_lower")
  })

  test("blocks unsafe refs before they reach public projection", () => {
    const proof = proveKhalaM7ConductorComposition({
      ...validCompositionInput(),
      trainingRunRef: "/Users/chris/private/run.json",
    })

    expect(proof.canPublishCompositionProof).toBe(false)
    expect(proof.trainingRunRef).toBeNull()
    expect(proof.blockerRefs).toContain("blocker.khala.m7.conductor_composition.training_run_missing")
    expect(proof.blockerRefs).toContain("blocker.khala.m7.conductor_composition.unsafe_ref")
  })
})

function validCompositionInput() {
  return {
    observedAt,
    compositionRunRef: "run.openagents.khala.m7.crossy_road.composition.v1",
    policyBackendRef: "backend.psionic.m7.conductor_7b.fp32_head.v1",
    trainingRunRef: "run.psionic.m7.grpo.conductor.crossy_road.v1",
    trainingRunExecuted: true,
    trainerConfigRef: "config.psionic.m7.tmax_table13.dppo_fp32_lm_head.v1",
    plannerAlgorithm: "grpo_dppo" as const,
    fp32LmHead: true,
    zeroStdFiltered: true,
    workerPoolRefs: [
      "worker.khala.m7.frontier_gateway.planner.v1",
      "worker.pylon.open.khala_code_writer.v1",
      "worker.tassadar.crossy_road_module.v1",
      "worker.khala.m2.crossy_road_verifier.v1",
    ],
    topology: validTopology(),
    verdictRef: "verdict.khala.m2.crossy_road.accepted.v1",
    rubricRef: "rubric.khala.m2.crossy_road.v1",
    verdictAccepted: true,
    verseFanoutRef: "verse.khala.m7.crossy_road.multi_worker_fanout.v1",
    fanoutVisible: true,
    compositionCostMsats: 4_200,
    singleModelBaselineCostMsats: 9_400,
    qualityComparable: true,
    publicationRef: "publication.khala.m7.conductor_crossy_road.v1",
  }
}

function validTopology(): KhalaM7ConductorCompositionStep[] {
  return [
    {
      stepRef: "step.khala.m7.crossy_road.plan.v1",
      role: "plan",
      workerId: "worker.khala.m7.frontier_gateway.planner.v1",
      workerKind: "frontier_gateway",
      dependsOn: [],
      accessList: [
        "worker.khala.m7.frontier_gateway.planner.v1",
        "worker.pylon.open.khala_code_writer.v1",
        "worker.tassadar.crossy_road_module.v1",
        "worker.khala.m2.crossy_road_verifier.v1",
      ],
      artifactRef: "artifact.khala.m7.crossy_road.plan.v1",
    },
    {
      stepRef: "step.khala.m7.crossy_road.implement.v1",
      role: "implement",
      workerId: "worker.pylon.open.khala_code_writer.v1",
      workerKind: "open_pylon",
      dependsOn: ["step.khala.m7.crossy_road.plan.v1"],
      accessList: ["worker.pylon.open.khala_code_writer.v1", "worker.tassadar.crossy_road_module.v1"],
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
  ]
}
