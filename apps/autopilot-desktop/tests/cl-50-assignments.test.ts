import { describe, expect, test } from "bun:test"
import { assignmentMeta } from "../src/ui/cards/assignments"
import type { AssignmentRow } from "../src/shared/rpc"

const base: AssignmentRow = {
  assignmentRef: "asgn-abcdef123456",
  leaseRef: "lease-xyz",
  goal: "Fix the login bug",
  paymentMode: "prepaid",
  expiresAt: "2026-07-01T00:00:00.000Z",
}

describe("assignmentMeta", () => {
  test("returns the goal when goal is non-empty", () => {
    const { goal } = assignmentMeta(base)
    expect(goal).toBe("Fix the login bug")
  })

  test("falls back to last 8 chars of assignmentRef when goal is empty", () => {
    const row: AssignmentRow = { ...base, goal: "" }
    const { goal } = assignmentMeta(row)
    expect(goal).toBe(base.assignmentRef.slice(-8))
  })

  test("falls back to last 8 chars of assignmentRef when goal is whitespace-only", () => {
    const row: AssignmentRow = { ...base, goal: "   " }
    const { goal } = assignmentMeta(row)
    expect(goal).toBe(base.assignmentRef.slice(-8))
  })

  test("includes paymentMode in meta", () => {
    const { meta } = assignmentMeta(base)
    expect(meta).toContain("prepaid")
  })

  test("slices expiresAt to YYYY-MM-DD", () => {
    const { meta } = assignmentMeta(base)
    expect(meta).toContain("expires 2026-07-01")
    expect(meta).not.toContain("T00:00")
  })

  test("omits expiry clause when expiresAt is empty string", () => {
    const row: AssignmentRow = { ...base, expiresAt: "" }
    const { meta } = assignmentMeta(row)
    expect(meta).not.toContain("expires")
  })

  test("includes last 6 chars of assignmentRef as suffix", () => {
    const { meta } = assignmentMeta(base)
    expect(meta).toContain(base.assignmentRef.slice(-6))
  })

  test("meta format: paymentMode · expires DATE · refSuffix", () => {
    const { meta } = assignmentMeta(base)
    expect(meta).toBe("prepaid · expires 2026-07-01 · ef123456".replace("ef123456", base.assignmentRef.slice(-6)))
  })
})
