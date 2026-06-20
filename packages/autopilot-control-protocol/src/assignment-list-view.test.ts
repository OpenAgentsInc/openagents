import { describe, expect, test } from "bun:test"

import {
  projectAssignmentList,
  type AssignmentListView,
} from "./assignment-list-view.js"

describe("assignment list view projection", () => {
  test("splits open and claimed assignments from a direct array", () => {
    expect(projectAssignmentList([
      {
        assignmentRef: "assignment.public.0001",
        title: "Fix the work assignment view",
        state: "open",
        rewardSats: 1_000,
      },
      {
        assignmentRef: "assignment.public.0002",
        title: "Claim the next integration task",
        state: "claimed",
        rewardSats: 2_000,
      },
    ])).toEqual({
      open: [
        {
          ref: "assignment.public.0001",
          title: "Fix the work assignment view",
          rewardSats: 1_000,
        },
      ],
      claimed: [
        {
          ref: "assignment.public.0002",
          title: "Claim the next integration task",
        },
      ],
      counts: {
        open: 1,
        claimed: 1,
        total: 2,
      },
    } satisfies AssignmentListView)
  })

  test("reads wrapped assignment rows and nested aliases", () => {
    expect(projectAssignmentList({
      assignments: [
        {
          status: "offered",
          assignment: {
            lease_ref: "assignment.public.0003",
            objective: "Run the smoke suite",
          },
          reward: {
            sats: 3_500,
          },
        },
        {
          status: "running",
          assignment: {
            ref: "assignment.public.0004",
            summary: "Review the payout receipt",
          },
        },
      ],
    })).toEqual({
      open: [
        {
          ref: "assignment.public.0003",
          title: "Run the smoke suite",
          rewardSats: 3_500,
        },
      ],
      claimed: [
        {
          ref: "assignment.public.0004",
          title: "Review the payout receipt",
        },
      ],
      counts: {
        open: 1,
        claimed: 1,
        total: 2,
      },
    } satisfies AssignmentListView)
  })

  test("combines pre-split open and claimed wrapper lists", () => {
    expect(projectAssignmentList({
      open: [
        {
          assignment_ref: "assignment.public.0005",
          assignment_title: "Queue the public promise check",
          state: "pending",
          payout_sats: 4_000,
        },
      ],
      claimed: [
        {
          ref: "assignment.public.0006",
          title: "Patch the forum intake",
          status: "active",
        },
      ],
    })).toEqual({
      open: [
        {
          ref: "assignment.public.0005",
          title: "Queue the public promise check",
          rewardSats: 4_000,
        },
      ],
      claimed: [
        {
          ref: "assignment.public.0006",
          title: "Patch the forum intake",
        },
      ],
      counts: {
        open: 1,
        claimed: 1,
        total: 2,
      },
    } satisfies AssignmentListView)
  })

  test("ignores non-list input, non-record rows, and non-list states", () => {
    const empty = {
      open: [],
      claimed: [],
      counts: {
        open: 0,
        claimed: 0,
        total: 0,
      },
    } satisfies AssignmentListView

    expect(projectAssignmentList(undefined)).toEqual(empty)
    expect(projectAssignmentList(null)).toEqual(empty)
    expect(projectAssignmentList("not-json")).toEqual(empty)
    expect(projectAssignmentList([
      null,
      "bad",
      { ref: "assignment.public.done", state: "completed" },
      { ref: "assignment.public.accepted", state: "accepted" },
      { ref: "assignment.public.unknown", state: "paused" },
    ])).toEqual(empty)
  })

  test("keeps stable fallbacks for missing titles and invalid rewards", () => {
    expect(projectAssignmentList([
      {
        ref: "assignment.public.0007",
        state: "queued",
        rewardSats: "5000",
      },
      {
        ref: "assignment.public.0008",
        state: "working",
      },
    ])).toEqual({
      open: [
        {
          ref: "assignment.public.0007",
          title: "assignment.public.0007",
          rewardSats: null,
        },
      ],
      claimed: [
        {
          ref: "assignment.public.0008",
          title: "assignment.public.0008",
        },
      ],
      counts: {
        open: 1,
        claimed: 1,
        total: 2,
      },
    } satisfies AssignmentListView)
  })

  test("does not mutate source rows while projecting", () => {
    const row = {
      assignmentRef: "assignment.public.0009",
      title: "Leave source untouched",
      state: "open",
      rewardSats: 6_000,
    }
    const raw = [row]

    expect(projectAssignmentList(raw).open).toEqual([
      {
        ref: "assignment.public.0009",
        title: "Leave source untouched",
        rewardSats: 6_000,
      },
    ])
    expect(raw).toEqual([row])
  })
})
