import { describe, expect, test } from "bun:test"
import {
  applyConsistentDecisionCancellation,
  applyConsistentDecisionResolution,
  type DecisionClientView,
} from "../src/node/decision-consistency"

const requestId = "decision-1"

function pendingViews(): DecisionClientView[] {
  return [
    { clientRef: "client-web", requestId, state: "pending" },
    { clientRef: "client-desktop", requestId, state: "pending" },
    { clientRef: "client-mobile", requestId, state: "pending" },
  ]
}

describe("decision consistency", () => {
  test("first resolution wins and projects resolved_elsewhere to every other client", () => {
    const result = applyConsistentDecisionResolution(requestId, pendingViews(), {
      requestId,
      verb: "approve",
      resolvingClientRef: "client-web",
    })

    expect(result.outcome).toBe("accepted")
    expect(result.winner).toEqual({
      requestId,
      verb: "approve",
      resolvingClientRef: "client-web",
    })
    expect(result.projections).toEqual([
      {
        clientRef: "client-desktop",
        requestId,
        state: "resolved_elsewhere",
        resolvedVerb: "approve",
        resolvedByClientRef: "client-web",
      },
      {
        clientRef: "client-mobile",
        requestId,
        state: "resolved_elsewhere",
        resolvedVerb: "approve",
        resolvedByClientRef: "client-web",
      },
    ])
  })

  test("second conflicting resolution is rejected and yields resolved_elsewhere for non-winners", () => {
    const views: DecisionClientView[] = [
      {
        clientRef: "client-web",
        requestId,
        state: "resolved",
        resolvedVerb: "approve",
        resolvedByClientRef: "client-web",
      },
      { clientRef: "client-desktop", requestId, state: "pending" },
      { clientRef: "client-mobile", requestId, state: "pending" },
    ]

    const result = applyConsistentDecisionResolution(requestId, views, {
      requestId,
      verb: "deny",
      resolvingClientRef: "client-desktop",
    })

    expect(result.outcome).toBe("already_resolved")
    expect(result.winner).toEqual({
      requestId,
      verb: "approve",
      resolvingClientRef: "client-web",
    })
    expect(result.projections).toEqual([
      {
        clientRef: "client-desktop",
        requestId,
        state: "resolved_elsewhere",
        resolvedVerb: "approve",
        resolvedByClientRef: "client-web",
      },
      {
        clientRef: "client-mobile",
        requestId,
        state: "resolved_elsewhere",
        resolvedVerb: "approve",
        resolvedByClientRef: "client-web",
      },
    ])
  })

  test("cancellation propagates cancelled state to all client views", () => {
    const result = applyConsistentDecisionCancellation(requestId, pendingViews())

    expect(result.outcome).toBe("accepted")
    expect(result.projections).toEqual([
      { clientRef: "client-web", requestId, state: "cancelled" },
      { clientRef: "client-desktop", requestId, state: "cancelled" },
      { clientRef: "client-mobile", requestId, state: "cancelled" },
    ])
  })
})
