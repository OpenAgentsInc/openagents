import { describe, expect, test } from "bun:test"

import { buildDecisionResolveEnvelope } from "@openagentsinc/autopilot-control-protocol"
import { createApprovalQueue } from "../src/node/approval-queue"

// #5004: a decision is exactly-once ACROSS surfaces (phone over the bridge vs
// desktop/web over /command) because every surface resolves through the SAME
// node approval queue. This test ties the #5002 bridge decision envelope
// (buildDecisionResolveEnvelope → requestId/decisionVerb) to the node queue via
// the control-server's mapping (requestId → approvalRef, decisionVerb →
// decision) and proves the second surface's resolve is a no-op duplicate that
// preserves the first decision.

describe("cross-surface decision exactly-once (#5004)", () => {
  const enqueue = (q: ReturnType<typeof createApprovalQueue>) =>
    q.enqueue({
      approvalRef: "approval.1",
      kind: "labor_first_run",
      prompt: "Approve first run?",
      jobType: "code_task",
      policyRef: "policy.x",
      createdAt: "2026-06-14T12:00:00.000Z",
    })

  test("phone (bridge envelope) resolves; desktop (/command) repeat is a duplicate, original preserved", () => {
    const q = createApprovalQueue()
    enqueue(q)

    // Phone resolves over the bridge: build the canonical decision envelope, then
    // apply the SAME mapping the control-server /bridge handler uses.
    const phoneEnv = buildDecisionResolveEnvelope({
      requestId: "approval.1",
      verb: "approve",
      pairingRef: "pair.phone",
      capabilityRef: "answer_decision",
      clientRequestId: "req.phone.1",
    })
    const first = q.resolve(phoneEnv.requestId, phoneEnv.decisionVerb)
    expect(first.applied).toBe(true)
    expect(first.duplicate).toBe(false)

    // Desktop tries the same approval over /command with a DIFFERENT verb.
    // Exactly-once: rejected as duplicate; the original "approve" stands.
    const second = q.resolve("approval.1", "deny")
    expect(second.applied).toBe(false)
    expect(second.duplicate).toBe(true)
    expect(second.decision).toBe("approve")

    // Single spine: one history entry, nothing left pending.
    expect(q.list()).toHaveLength(0)
    expect(q.history()).toHaveLength(1)
  })
})
