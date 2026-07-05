import { describe, expect, test } from "bun:test"

import {
  bridgeProofFromOptimizationProjection,
  defaultKhalaFleetDelegationActiveParameters,
  gymOptimizationRunFromProjection,
  gymPaneStateFromBridgeProof,
  gymPaneStateFromLocation,
  gymPaneStateFromOptimizationRun,
  initialKhalaCodeViewFromLocation,
  khalaCodeGymDemoBridgeProof,
} from "../src/ui/gym-proof-loader"
import type { KhalaGymBridgeProofLike } from "../src/ui/gym-graph-projection"

const blockedProof = (): KhalaGymBridgeProofLike => {
  const blockerRefs = [
    "blocker.gym.khala_delegation.fixture_missing_live_gate",
  ]
  const progress = khalaCodeGymDemoBridgeProof.progress ?? []
  const {
    actionSubmissionProposalRef: _actionSubmissionProposalRef,
    ...latestProgress
  } = progress.at(-1) ?? {}
  return {
    ...khalaCodeGymDemoBridgeProof,
    progress: [
      ...progress.slice(0, -1),
      {
        ...latestProgress,
        admissionDecision: "blocked",
        blockerRefs,
      },
    ],
    summary: {
      ...khalaCodeGymDemoBridgeProof.summary,
      blockerRefs,
    },
    admission: {
      ...khalaCodeGymDemoBridgeProof.admission,
      decision: "blocked",
      actionSubmissionProposalRefs: [],
      blockerRefs,
    },
    admissionDecision: "blocked",
    actionSubmissionProposalRef: null,
    blockerRefs,
    decisionGrade: false,
  }
}

describe("Khala Code Gym proof loader", () => {
  test("loads the deterministic fixture into a successful pane state", () => {
    const state = gymPaneStateFromBridgeProof({
      proof: khalaCodeGymDemoBridgeProof,
      generatedAt: "time.test.fixture",
    })

    expect(state.phase).toBe("loaded")
    if (state.phase !== "loaded") throw new Error("expected loaded state")
    expect(state.status).toBe("proposal_ready")
    expect(state.runRef).toBe("gym.run.khala_code_delegation_gepa.part2_fixture")
    expect(state.graph?.status).toBe("proposal_ready")
    expect(state.details).toContainEqual({
      label: "metricValueBps",
      value: "10000 bps",
    })
    expect(state.details).toContainEqual({
      label: "admissionDecision",
      value: "gated_proposal_ready",
    })
    expect(state.details).toContainEqual({
      label: "decisionGrade",
      value: "false",
    })
    expect(state.details?.some(detail =>
      detail.label === "candidate refs" &&
      detail.value.includes("candidate.khala_fleet_delegation.part2_fixture.v1"),
    )).toBe(true)
    expect(state.details?.some(detail =>
      detail.label === "Action Submission proposal refs" &&
      detail.value.includes(
        "action_submission.proposal.khala_delegation.part2_fixture.v1",
      ),
    )).toBe(true)
    expect(state.activeParameters).toMatchObject({
      actionSubmissionProposalRef:
        "action_submission.proposal.khala_delegation.part2_fixture.v1",
      candidateManifestRef: "manifest.khala_fleet_delegation.part2_fixture.v1",
      candidateRef: "candidate.khala_fleet_delegation.part2_fixture.v1",
      parameterRef: "parameters.khala_fleet_delegation.default.v1",
      schemaVersion: "openagents.khala.fleet_delegation.parameters.v0",
      source: "default",
    })
  })

  test("missing proof stays honest and empty", () => {
    expect(gymPaneStateFromBridgeProof(null)).toEqual({
      activeParameters: defaultKhalaFleetDelegationActiveParameters,
      phase: "empty",
    })
    expect(gymPaneStateFromBridgeProof({ proof: null })).toEqual({
      activeParameters: defaultKhalaFleetDelegationActiveParameters,
      phase: "empty",
    })
  })

  test("blocked proof keeps blocker refs and blocked graph state visible", () => {
    const state = gymPaneStateFromBridgeProof({ proof: blockedProof() })

    expect(state.phase).toBe("blocked")
    if (state.phase !== "blocked") throw new Error("expected blocked state")
    expect(state.blockerRefs).toEqual([
      "blocker.gym.khala_delegation.fixture_missing_live_gate",
    ])
    expect(state.graph?.status).toBe("blocked")
    expect(state.graph?.links.some(link => link.status === "blocked")).toBe(true)
    expect(state.details).toContainEqual({
      label: "admissionDecision",
      value: "blocked",
    })
    expect(state.details).toContainEqual({
      label: "decisionGrade",
      value: "false",
    })
    expect(state.details?.some(detail =>
      detail.label === "blocker refs" &&
      detail.value.includes("fixture_missing_live_gate"),
    )).toBe(true)
    expect(state.activeParameters?.blockerRefs).toEqual([
      "blocker.gym.khala_delegation.fixture_missing_live_gate",
    ])
  })

  test("URL fixture proof is opt-in without opening a top-level Gym screen", () => {
    const empty = gymPaneStateFromLocation({ search: "", hash: "" })
    const fixture = gymPaneStateFromLocation({
      search: "?gymProof=fixture",
      hash: "",
    })
    const fixtureView = initialKhalaCodeViewFromLocation({
      search: "?gymProof=fixture",
      hash: "",
    })
    const chatView = initialKhalaCodeViewFromLocation({ search: "", hash: "" })
    const legacyGymView = initialKhalaCodeViewFromLocation({
      search: "?view=gym",
      hash: "",
    })
    const legacyInboxView = initialKhalaCodeViewFromLocation({
      search: "?view=inbox",
      hash: "",
    })
    const editorView = initialKhalaCodeViewFromLocation({
      search: "?view=editor",
      hash: "",
    })

    expect(empty).toEqual({
      activeParameters: defaultKhalaFleetDelegationActiveParameters,
      phase: "empty",
    })
    expect(fixture.phase).toBe("loaded")
    expect(fixtureView).toBe("chat")
    expect(chatView).toBe("chat")
    expect(legacyGymView).toBe("chat")
    expect(legacyInboxView).toBe("inbox")
    expect(editorView).toBe("editor")
  })

  test("Worker optimization projection maps into the same Gym pane state", () => {
    const projection = {
      actionSubmissionProposalRef:
        "action_submission.proposal.khala_delegation.worker_projection.v1",
      admissionDecision: "gated_proposal_ready",
      baseModuleRef: "module.khala_fleet_delegation.base.v1",
      blockerRefs: [],
      candidateManifestRef:
        "manifest.khala_fleet_delegation.worker_projection.v1",
      candidateRef: "candidate.khala_fleet_delegation.worker_projection.v1",
      caveatRefs: [
        "caveat.gym.khala_delegation_gepa.decision_grade_false_until_live_evidence",
      ],
      datasetRef: "eval.mutalisk.fixtures.khala_fleet_delegation_demo.v1",
      feedbackSchemaRef: "openagents.khala.delegation_gepa_feedback.v0",
      jobRef: "gym.job.mutalisk_khala_delegation.worker_projection",
      latestStage: "completed",
      metricValueBps: 9400,
      ownerApprovalRef: "approval.owner.khala_delegation.operator_review.v1",
      publicSafetyPolicyRef:
        "policy.public_safe.mutalisk_khala_delegation_summary.v0",
      runRef: "gym.run.khala_code_delegation_gepa.worker_projection",
      trainSplitRefs: ["eval_split.khala_fleet_delegation_demo.train.v1"],
      validationSplitRefs: ["eval_split.khala_fleet_delegation_demo.val.v1"],
    } as const

    const proof = bridgeProofFromOptimizationProjection(projection)
    const run = gymOptimizationRunFromProjection(projection)
    const state = gymPaneStateFromOptimizationRun(projection)

    expect(proof.job?.runRef).toBe(
      "gym.run.khala_code_delegation_gepa.worker_projection",
    )
    expect(run).toMatchObject({
      actionSubmissionProposalRef:
        "action_submission.proposal.khala_delegation.worker_projection.v1",
      admissionDecision: "gated_proposal_ready",
      candidateManifestRef:
        "manifest.khala_fleet_delegation.worker_projection.v1",
      metricValueBps: 9400,
      phase: "proposal_ready",
      runRef: "gym.run.khala_code_delegation_gepa.worker_projection",
      stage: "completed",
    })
    expect(run.datasetRefs).toEqual([
      "eval.mutalisk.fixtures.khala_fleet_delegation_demo.v1",
      "eval_split.khala_fleet_delegation_demo.train.v1",
      "eval_split.khala_fleet_delegation_demo.val.v1",
    ])
    expect(state.phase).toBe("loaded")
    if (state.phase !== "loaded") throw new Error("expected loaded state")
    expect(state.activeParameters).toMatchObject({
      actionSubmissionProposalRef:
        "action_submission.proposal.khala_delegation.worker_projection.v1",
      candidateManifestRef:
        "manifest.khala_fleet_delegation.worker_projection.v1",
      source: "default",
    })
  })
})
