import { describe, expect, test } from "vite-plus/test"
import { decodeRuntimeInteraction } from "@openagentsinc/agent-runtime-schema"
import { Effect } from "effect"
import { createRuntimeInteractionHttpAuthority } from "./runtime-interaction-authority.js"

const pending = decodeRuntimeInteraction({
  schema: "openagents.runtime_interaction.v1",
  interactionRef: "interaction.tool.1",
  threadId: "thread.runtime.1",
  turnId: "turn.runtime.1",
  requestedSequence: 1,
  requestedAt: "2026-07-11T22:00:00.000Z",
  expiresAt: "2099-07-11T22:05:00.000Z",
  source: { lane: "claude_pylon", adapterKind: "claude_code", surface: "server" },
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: ["intent.start.1"],
  payload: {
    kind: "tool_approval",
    displayText: "Allow write?",
    toolCallId: "tool.call.1",
    toolName: "Write",
    authority: {
      authorityRef: "authority.1", policyRef: "policy.1", decisionRef: "decision.pending.1",
      toolRef: "tool.write", status: "operator_escalation_required", allowed: false,
      blockerRefs: ["blocker.owner_approval"],
    },
  },
  lifecycle: { status: "pending" },
})

test("HTTP authority posts the canonical request and waits for terminal owner state", async () => {
  const calls: Array<{ url: string; init?: RequestInit }> = []
  let reads = 0
  const authority = createRuntimeInteractionHttpAuthority({
    adminToken: "private-token",
    baseUrl: "https://openagents.com/",
    ownerUserId: "user.1",
    pollIntervalMs: 1,
    fetchImpl: (async (url, init) => {
      calls.push({ url: String(url), init })
      if (init?.method === "POST") return Response.json({ ok: true })
      reads += 1
      return Response.json({
        interaction: reads === 1
          ? pending
          : { ...pending, lifecycle: { status: "resolved", envelope: {
              decisionRef: "decision.owner.1", idempotencyKey: "idem.owner.1",
              decidedAt: "2026-07-11T22:01:00.000Z", surface: "mobile",
              decision: { kind: "tool_approval", outcome: "approve" },
            } } },
      })
    }) as typeof fetch,
  })
  await Effect.runPromise(authority.request(pending))
  const terminal = decodeRuntimeInteraction(await Effect.runPromise(authority.awaitTerminal(pending.interactionRef)))
  expect(terminal.lifecycle.status).toBe("resolved")
  expect(JSON.parse(String(calls[0]?.init?.body))).toEqual({ ownerUserId: "user.1", interaction: pending })
  expect(calls.every(call => (call.init?.headers as Record<string, string>).authorization === "Bearer private-token")).toBe(true)
})

test("HTTP authority asks server-clock mutator to expire an elapsed request", async () => {
  const elapsed = { ...pending, expiresAt: "2020-07-11T22:05:00.000Z" }
  let expired = false
  const bodies: Array<any> = []
  const authority = createRuntimeInteractionHttpAuthority({
    adminToken: "private-token",
    baseUrl: "https://openagents.com",
    ownerUserId: "user.1",
    fetchImpl: (async (_url, init) => {
      if (init?.method === "POST") {
        const body = JSON.parse(String(init.body)); bodies.push(body)
        if (body.action === "expire") expired = true
        return Response.json({ ok: true })
      }
      return Response.json({ interaction: expired
        ? { ...elapsed, lifecycle: { status: "expired", terminalAt: "2026-07-11T22:05:00.000Z", reasonRef: "reason.interaction_deadline_elapsed" } }
        : elapsed })
    }) as typeof fetch,
  })
  expect(decodeRuntimeInteraction(await Effect.runPromise(authority.awaitTerminal(elapsed.interactionRef))).lifecycle.status).toBe("expired")
  expect(bodies).toEqual([{ action: "expire", ownerUserId: "user.1", interactionRef: "interaction.tool.1", threadId: "thread.runtime.1", turnId: "turn.runtime.1" }])
})
