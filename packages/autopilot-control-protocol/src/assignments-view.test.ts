import { describe, expect, test } from "bun:test"

import { projectAssignments, type AssignmentRow } from "./assignments-view.js"

describe("assignments view projection", () => {
  test("projects valid camelCase leases", () => {
    expect(projectAssignments([
      {
        leaseRef: "lease.public.0001",
        title: "Fix the forum intake",
        state: "open",
        rewardSats: 1_000,
        updatedAt: "2026-06-13T12:00:00.000Z",
      },
      {
        leaseRef: "lease.public.0002",
        title: "Verify payout closeout",
        state: "accepted",
        rewardSats: 2_500,
        updatedAt: "2026-06-13T12:01:00.000Z",
      },
    ])).toEqual([
      {
        leaseRef: "lease.public.0001",
        title: "Fix the forum intake",
        state: "open",
        rewardSats: 1_000,
        updatedAt: "2026-06-13T12:00:00.000Z",
      },
      {
        leaseRef: "lease.public.0002",
        title: "Verify payout closeout",
        state: "accepted",
        rewardSats: 2_500,
        updatedAt: "2026-06-13T12:01:00.000Z",
      },
    ] satisfies AssignmentRow[])
  })

  test("projects snake_case lease rows", () => {
    expect(projectAssignments([
      {
        lease_ref: "lease.public.0003",
        assignment_title: "Run the mobile smoke",
        status: "in_progress",
        payout_sats: 3_000,
        updated_at: "2026-06-13T12:02:00.000Z",
      },
    ])).toEqual([
      {
        leaseRef: "lease.public.0003",
        title: "Run the mobile smoke",
        state: "in_progress",
        rewardSats: 3_000,
        updatedAt: "2026-06-13T12:02:00.000Z",
      },
    ] satisfies AssignmentRow[])
  })

  test("tolerates missing fields with stable fallbacks", () => {
    expect(projectAssignments([
      {
        leaseRef: "lease.public.missing",
      },
    ])).toEqual([
      {
        leaseRef: "lease.public.missing",
        title: "lease.public.missing",
        state: "unknown",
        rewardSats: null,
        updatedAt: "",
      },
    ] satisfies AssignmentRow[])
  })

  test("normalizes common lifecycle aliases", () => {
    expect(projectAssignments([
      { leaseRef: "lease.open", state: "offered" },
      { leaseRef: "lease.progress", state: "running" },
      { leaseRef: "lease.done", state: "completed" },
      { leaseRef: "lease.unknown", state: "paused" },
    ]).map(row => row.state)).toEqual([
      "open",
      "in_progress",
      "done",
      "unknown",
    ])
  })

  test("returns an empty list for bad input", () => {
    expect(projectAssignments(undefined)).toEqual([])
    expect(projectAssignments(null)).toEqual([])
    expect(projectAssignments({ leases: [] })).toEqual([])
    expect(projectAssignments("not-json")).toEqual([])
  })

  test("skips non-object array entries and reads nested assignment details", () => {
    expect(projectAssignments([
      "bad",
      null,
      {
        ref: "lease.public.nested",
        state: "settled",
        reward: { sats: 4_000 },
        assignment: {
          title: "Nested assignment title",
          updated_at: "2026-06-13T12:03:00.000Z",
        },
      },
    ])).toEqual([
      {
        leaseRef: "lease.public.nested",
        title: "Nested assignment title",
        state: "done",
        rewardSats: 4_000,
        updatedAt: "2026-06-13T12:03:00.000Z",
      },
    ] satisfies AssignmentRow[])
  })
})
