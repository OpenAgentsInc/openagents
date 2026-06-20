import { describe, expect, test } from "bun:test"

import {
  buildDecisionResolveEnvelope,
  canResolveDecision,
} from "./bridge-decision-client.js"
import type { Capability } from "./bridge.js"

const baseInput = {
  pairingRef: "pairing.fixture.0001",
  capabilityRef: "capability.fixture.answer_decision",
  requestId: "decision.request.fixture.0001",
  clientRequestId: "client.request.fixture.0001",
} as const

describe("bridge decision client", () => {
  test("buildDecisionResolveEnvelope builds an approve resolve request", () => {
    expect(buildDecisionResolveEnvelope({ ...baseInput, verb: "approve" })).toEqual({
      verb: "decision.resolve",
      clientRequestId: "client.request.fixture.0001",
      idempotencyKey: "client.request.fixture.0001",
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "capability.fixture.answer_decision",
      requestId: "decision.request.fixture.0001",
      decisionVerb: "approve",
    })
  })

  test("buildDecisionResolveEnvelope carries deny decisions", () => {
    expect(buildDecisionResolveEnvelope({ ...baseInput, verb: "deny" }).decisionVerb).toBe("deny")
  })

  test("buildDecisionResolveEnvelope carries answer decisions", () => {
    expect(buildDecisionResolveEnvelope({ ...baseInput, verb: "answer" }).decisionVerb).toBe("answer")
  })

  test("buildDecisionResolveEnvelope uses clientRequestId as idempotencyKey", () => {
    const envelope = buildDecisionResolveEnvelope({
      ...baseInput,
      verb: "approve",
      clientRequestId: "client.request.retry-safe.0001",
    })

    expect(envelope.idempotencyKey).toBe("client.request.retry-safe.0001")
  })

  test("canResolveDecision is true when answer_decision is present", () => {
    const capabilities: Capability[] = ["observe_public", "answer_decision"]

    expect(canResolveDecision(capabilities)).toBe(true)
  })

  test("canResolveDecision is false for read-only capabilities", () => {
    const capabilities: Capability[] = ["observe_public", "read_artifact"]

    expect(canResolveDecision(capabilities)).toBe(false)
  })

  test("canResolveDecision is false for empty capabilities", () => {
    expect(canResolveDecision([])).toBe(false)
  })
})
