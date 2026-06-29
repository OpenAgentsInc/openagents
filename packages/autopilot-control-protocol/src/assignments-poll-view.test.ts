import { describe, expect, test } from "bun:test"

import { summarizeAssignments } from "./assignments-poll-view.js"
import type { AssignmentRow } from "./assignments-view.js"

describe("assignments poll view summary", () => {
  test("summarizes normalized assignment lifecycle states", () => {
    const rows: AssignmentRow[] = [
      row("lease.open.1", "open", 100),
      row("lease.open.2", "open", 200),
      row("lease.accepted", "accepted", 300),
      row("lease.progress", "in_progress", 400),
      row("lease.done", "done", 500),
    ]

    expect(summarizeAssignments(rows)).toEqual({
      open: 2,
      accepted: 1,
      inProgress: 1,
      done: 1,
      totalRewardSats: 1500,
    })
  })

  test("ignores unknown states in lifecycle counts but includes valid rewards", () => {
    expect(summarizeAssignments([
      row("lease.unknown", "unknown", 250),
      row("lease.done", "done", 750),
    ])).toEqual({
      open: 0,
      accepted: 0,
      inProgress: 0,
      done: 1,
      totalRewardSats: 1000,
    })
  })

  test("treats null and invalid reward values as zero", () => {
    expect(summarizeAssignments([
      row("lease.open", "open", null),
      { ...row("lease.accepted", "accepted", 1), rewardSats: Number.NaN },
      { ...row("lease.progress", "in_progress", 2), rewardSats: Number.POSITIVE_INFINITY },
      { ...row("lease.done", "done", 3), rewardSats: -1 },
    ])).toEqual({
      open: 1,
      accepted: 1,
      inProgress: 1,
      done: 1,
      totalRewardSats: 0,
    })
  })

  test("returns an empty summary for empty or non-array input", () => {
    const empty = {
      open: 0,
      accepted: 0,
      inProgress: 0,
      done: 0,
      totalRewardSats: 0,
    }

    expect(summarizeAssignments([])).toEqual(empty)
    expect(summarizeAssignments(null as unknown as any[])).toEqual(empty)
    expect(summarizeAssignments({ rows: [] } as unknown as any[])).toEqual(empty)
  })

  test("skips malformed array entries without throwing", () => {
    expect(summarizeAssignments([
      "bad",
      null,
      ["lease.array"],
      row("lease.open", "open", 100),
    ] as unknown as any[])).toEqual({
      open: 1,
      accepted: 0,
      inProgress: 0,
      done: 0,
      totalRewardSats: 100,
    })
  })

  test("does not mutate the provided rows", () => {
    const rows = [
      row("lease.open", "open", 100),
      row("lease.done", "done", 200),
    ]
    const before = structuredClone(rows)

    summarizeAssignments(rows)

    expect(rows).toEqual(before)
  })
})

function row(
  leaseRef: string,
  state: AssignmentRow["state"],
  rewardSats: AssignmentRow["rewardSats"],
): AssignmentRow {
  return {
    leaseRef,
    title: leaseRef,
    state,
    rewardSats,
    updatedAt: "2026-06-13T12:00:00.000Z",
  }
}
