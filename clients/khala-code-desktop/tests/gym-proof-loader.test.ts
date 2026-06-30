import { describe, expect, test } from "bun:test"

import {
  gymPaneStateFromBridgeProof,
  gymPaneStateFromLocation,
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
  })

  test("missing proof stays honest and empty", () => {
    expect(gymPaneStateFromBridgeProof(null)).toEqual({ phase: "empty" })
    expect(gymPaneStateFromBridgeProof({ proof: null })).toEqual({
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
  })

  test("URL fixture proof is opt-in and can open directly to Gym", () => {
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

    expect(empty).toEqual({ phase: "empty" })
    expect(fixture.phase).toBe("loaded")
    expect(fixtureView).toBe("gym")
    expect(chatView).toBe("chat")
  })
})
