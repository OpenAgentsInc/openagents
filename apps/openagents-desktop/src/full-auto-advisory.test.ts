import { describe, expect, test } from "vite-plus/test"

import {
  adviseFullAutoRoute,
  decideFullAutoRoute,
  decodeFullAutoAdvisoryAnalysis,
  decodeFullAutoRouteRecommendation,
  FULL_AUTO_ADVISORY_ANALYSIS_SCHEMA,
  FULL_AUTO_ROUTE_RECOMMENDATION_SCHEMA,
  type FullAutoRouteRecommendation,
} from "./full-auto-advisory.ts"
import { type FullAutoRoutingLaneGate } from "./full-auto-routing.ts"

const AT = "2026-07-20T00:00:00Z"

const allReadyGate: FullAutoRoutingLaneGate = (lane) =>
  ["codex-local", "claude-local", "acp:grok-cli", "acp:cursor-agent"].includes(lane)
    ? { admitted: true, fullAuto: true }
    : null

const recommend = (lane: string): FullAutoRouteRecommendation => ({
  schema: FULL_AUTO_ROUTE_RECOMMENDATION_SCHEMA,
  source: "apple_fm",
  advisory: true,
  recommendedLane: lane,
  rationale: "on-device heuristic",
  at: AT,
})

const OWNER_POLICY = [{ lane: "codex-local" }, { lane: "claude-local" }]

describe("decideFullAutoRoute is deterministic and recommendation-free", () => {
  test("chooses the first candidate of the owner-ordered policy", () => {
    expect(decideFullAutoRoute(OWNER_POLICY, allReadyGate).chosenLane).toBe("codex-local")
  })

  test("refuses fail-closed when the policy does not validate", () => {
    const partialGate: FullAutoRoutingLaneGate = (lane) =>
      lane === "codex-local" ? { admitted: true, fullAuto: true } : null
    const decision = decideFullAutoRoute(OWNER_POLICY, partialGate)
    expect(decision.chosenLane).toBeNull()
    expect(decision.refusalReason).toBe("lane_unknown")
  })
})

describe("Apple FM recommends; it never decides", () => {
  test("the deterministic decision is identical regardless of the recommendation", () => {
    const base = adviseFullAutoRoute(OWNER_POLICY, allReadyGate, null).decision.chosenLane
    for (const lane of ["claude-local", "acp:grok-cli", "acp:cursor-agent", "codex-local"]) {
      const advised = adviseFullAutoRoute(OWNER_POLICY, allReadyGate, recommend(lane))
      // Apple FM recommends a different lane, but the owner order still wins.
      expect(advised.decision.chosenLane).toBe(base)
      expect(advised.decision.chosenLane).toBe("codex-local")
    }
  })

  test("a recommendation for a non-policy lane is recorded but never chosen", () => {
    const advised = adviseFullAutoRoute(OWNER_POLICY, allReadyGate, recommend("acp:cursor-agent"))
    expect(advised.recommendation?.recommendedLane).toBe("acp:cursor-agent")
    expect(advised.decision.chosenLane).toBe("codex-local")
    expect(advised.recommendationMatchedDecision).toBe(false)
  })

  test("recommendationMatchedDecision is a report, not a cause", () => {
    const matching = adviseFullAutoRoute(OWNER_POLICY, allReadyGate, recommend("codex-local"))
    expect(matching.recommendationMatchedDecision).toBe(true)
    // Even when it matches, the decision was computed the same way it is
    // computed with no recommendation at all.
    expect(matching.decision.chosenLane).toBe(
      decideFullAutoRoute(OWNER_POLICY, allReadyGate).chosenLane,
    )
  })

  test("recommendation and decision are distinct recorded facts", () => {
    const advised = adviseFullAutoRoute(OWNER_POLICY, allReadyGate, recommend("acp:grok-cli"))
    expect(advised.recommendation).not.toBeNull()
    expect(advised.decision).toBeDefined()
    // The recommendation object carries no decision/action field.
    expect(Object.keys(advised.recommendation ?? {})).not.toContain("chosenLane")
  })
})

describe("advisory types are structurally advisory (no action authority)", () => {
  test("a route recommendation must carry advisory: true", () => {
    expect(() =>
      decodeFullAutoRouteRecommendation({
        schema: FULL_AUTO_ROUTE_RECOMMENDATION_SCHEMA,
        source: "apple_fm",
        advisory: false,
        recommendedLane: "codex-local",
        rationale: "x",
        at: AT,
      }),
    ).toThrow()
  })

  test("advisory analysis carries observations and advisory: true, no verdict", () => {
    const analysis = decodeFullAutoAdvisoryAnalysis({
      schema: FULL_AUTO_ADVISORY_ANALYSIS_SCHEMA,
      source: "apple_fm",
      advisory: true,
      observations: ["no progress in the last 3 turns", "objective may have drifted"],
      at: AT,
    })
    expect(analysis.advisory).toBe(true)
    expect(analysis.observations).toHaveLength(2)
    // No verdict/decision/state field exists on an advisory analysis.
    expect(Object.keys(analysis)).not.toContain("verdict")
    expect(Object.keys(analysis)).not.toContain("decision")
  })

  test("a recommendation round-trips through decode", () => {
    const r = recommend("codex-local")
    expect(decodeFullAutoRouteRecommendation(r)).toEqual(r)
  })
})
