import { describe, expect, test } from "bun:test"

import { createApprovalQueue } from "../src/node/approval-queue"

describe("approval queue (CL-16)", () => {
  const enqueueOne = (q: ReturnType<typeof createApprovalQueue>, ref = "approval.1") =>
    q.enqueue({ approvalRef: ref, kind: "labor_first_run", prompt: "Approve first run of code_task?", jobType: "code_task", policyRef: "policy.x", createdAt: "2026-06-13T12:00:00.000Z" })

  test("lists pending approvals (read-only view)", () => {
    const q = createApprovalQueue()
    enqueueOne(q)
    enqueueOne(q, "approval.2")
    expect(q.list().map((a) => a.approvalRef).sort()).toEqual(["approval.1", "approval.2"])
    expect(q.list()[0].kind).toBe("labor_first_run")
  })

  test("enqueue is idempotent on approvalRef", () => {
    const q = createApprovalQueue()
    enqueueOne(q)
    enqueueOne(q)
    expect(q.list()).toHaveLength(1)
  })

  test("approve applies once and removes from pending", () => {
    const q = createApprovalQueue()
    enqueueOne(q)
    const r = q.resolve("approval.1", "approve", { now: "2026-06-13T12:01:00.000Z" })
    expect(r.applied).toBe(true)
    expect(r.duplicate).toBe(false)
    expect(r.resolved?.jobType).toBe("code_task")
    expect(q.list()).toHaveLength(0)
    expect(q.history()).toHaveLength(1)
  })

  test("exactly-once: a duplicate resolve keeps the original decision and does not re-apply", () => {
    const q = createApprovalQueue()
    enqueueOne(q)
    const first = q.resolve("approval.1", "approve")
    const second = q.resolve("approval.1", "deny") // attempt to flip
    expect(first.applied).toBe(true)
    expect(second.applied).toBe(false)
    expect(second.duplicate).toBe(true)
    expect(second.decision).toBe("approve") // original wins, not deny
    expect(q.history()).toHaveLength(1)
  })

  test("deny works and is also exactly-once", () => {
    const q = createApprovalQueue()
    enqueueOne(q)
    expect(q.resolve("approval.1", "deny").applied).toBe(true)
    expect(q.resolve("approval.1", "approve").duplicate).toBe(true)
    expect(q.list()).toHaveLength(0)
  })

  test("answer requires a non-empty answer", () => {
    const q = createApprovalQueue()
    enqueueOne(q)
    const bad = q.resolve("approval.1", "answer", { answer: "  " })
    expect(bad.applied).toBe(false)
    expect(bad.error).toBe("answer_required")
    const ok = q.resolve("approval.1", "answer", { answer: "use staging" })
    expect(ok.applied).toBe(true)
    expect(q.history()[0].answer).toBe("use staging")
  })
})
