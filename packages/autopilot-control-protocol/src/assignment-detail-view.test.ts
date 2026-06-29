import { describe, expect, test } from "bun:test"

import {
  projectAssignmentDetail,
  type AssignmentDetailView,
} from "./assignment-detail-view.js"

describe("assignment detail view projection", () => {
  test("projects a direct camelCase assignment detail", () => {
    expect(projectAssignmentDetail({
      assignmentRef: "assignment.public.0001",
      title: "Fix the work assignment view",
      state: "open",
      rewardSats: 1_000,
      claimedByHash: "agent.hash.001",
    })).toEqual({
      assignmentRef: "assignment.public.0001",
      title: "Fix the work assignment view",
      state: "open",
      rewardSats: 1_000,
      claimedByHash: "agent.hash.001",
    } satisfies AssignmentDetailView)
  })

  test("projects snake_case fields and nested claim details", () => {
    expect(projectAssignmentDetail({
      assignment_ref: "assignment.public.0002",
      assignment_title: "Review payout receipt",
      status: "claimed",
      payout_sats: 2_500,
      claim: {
        claimed_by_hash: "agent.hash.002",
      },
    })).toEqual({
      assignmentRef: "assignment.public.0002",
      title: "Review payout receipt",
      state: "claimed",
      rewardSats: 2_500,
      claimedByHash: "agent.hash.002",
    } satisfies AssignmentDetailView)
  })

  test("reads nested assignment aliases", () => {
    expect(projectAssignmentDetail({
      reward: { sats: 3_000 },
      assignment: {
        lease_ref: "assignment.public.0003",
        objective: "Run the acceptance smoke",
        status: "ready_for_review",
      },
      assignee: {
        agent_hash: "agent.hash.003",
      },
    })).toEqual({
      assignmentRef: "assignment.public.0003",
      title: "Run the acceptance smoke",
      state: "submitted",
      rewardSats: 3_000,
      claimedByHash: "agent.hash.003",
    } satisfies AssignmentDetailView)
  })

  test("normalizes common detail lifecycle aliases", () => {
    expect([
      projectAssignmentDetail({ state: "offered" }).state,
      projectAssignmentDetail({ state: "running" }).state,
      projectAssignmentDetail({ state: "completed" }).state,
      projectAssignmentDetail({ state: "settled" }).state,
      projectAssignmentDetail({ state: "declined" }).state,
      projectAssignmentDetail({ state: "paused" }).state,
    ]).toEqual([
      "open",
      "claimed",
      "submitted",
      "accepted",
      "rejected",
      "unknown",
    ])
  })

  test("returns a stable empty projection for bad input", () => {
    const empty = {
      assignmentRef: "",
      title: "",
      state: "unknown",
      rewardSats: null,
      claimedByHash: null,
    } satisfies AssignmentDetailView

    expect(projectAssignmentDetail(undefined)).toEqual(empty)
    expect(projectAssignmentDetail(null)).toEqual(empty)
    expect(projectAssignmentDetail(["assignment.public.nope"])).toEqual(empty)
    expect(projectAssignmentDetail("not-json")).toEqual(empty)
  })

  test("tolerates missing and invalid fields with stable fallbacks", () => {
    expect(projectAssignmentDetail({
      ref: "assignment.public.missing",
      rewardSats: "1000",
      claimedByHash: 42,
    })).toEqual({
      assignmentRef: "assignment.public.missing",
      title: "assignment.public.missing",
      state: "unknown",
      rewardSats: null,
      claimedByHash: null,
    } satisfies AssignmentDetailView)
  })
})
