import { describe, expect, test } from "bun:test"

import {
  OPERATOR_GROUNDED_ASSERTION_EVIDENCE,
  evaluateOperatorGroundedAssertion,
} from "./operator-grounded-assertion.js"

describe("operator-grounded-assertion gate (Blueprint Signature 6)", () => {
  test("a referenced-but-unlooked-up artifact stays UNGROUNDED at REFERENCED", () => {
    const result = evaluateOperatorGroundedAssertion({
      artifactKind: "file_path",
      artifactRef: "scripts/distill_traces.ts",
      lookupTool: null,
      lookupResult: "not_looked_up",
    })
    expect(result.state).toBe("REFERENCED")
    expect(result.canAssert).toBe(false)
    expect(result.lockedAt).toBe("LOOKED_UP")
    expect(result.missingEvidence).toContain(
      OPERATOR_GROUNDED_ASSERTION_EVIDENCE.pathExists,
    )
  })

  test("a looked-up-but-negative artifact stays UNGROUNDED at LOOKED_UP", () => {
    const result = evaluateOperatorGroundedAssertion({
      artifactKind: "file_path",
      artifactRef: "scripts/distill_traces.ts",
      lookupTool: "repo_path_exists",
      lookupResult: "negative",
    })
    expect(result.state).toBe("LOOKED_UP")
    expect(result.canAssert).toBe(false)
    expect(result.lockedAt).toBe("GROUNDED")
    expect(result.blockedReason).toContain("did not confirm")
  })

  test("a positive lookup reaches GROUNDED and unlocks the assertion", () => {
    const result = evaluateOperatorGroundedAssertion({
      artifactKind: "file_path",
      artifactRef: "apps/pylon/scripts/multi-session-campaign.ts",
      lookupTool: "repo_path_exists",
      lookupResult: "positive",
    })
    expect(result.state).toBe("GROUNDED")
    expect(result.canAssert).toBe(true)
    expect(result.locked).toBe(false)
    expect(result.satisfiedEvidence).toEqual([
      OPERATOR_GROUNDED_ASSERTION_EVIDENCE.pathExists,
    ])
    expect(result.missingEvidence).toEqual([])
  })

  test("an api_endpoint grounds against the route-registered evidence", () => {
    const grounded = evaluateOperatorGroundedAssertion({
      artifactKind: "api_endpoint",
      artifactRef: "/api/v1/chat/completions",
      lookupTool: "route_exists",
      lookupResult: "positive",
    })
    expect(grounded.state).toBe("GROUNDED")
    expect(grounded.satisfiedEvidence).toEqual([
      OPERATOR_GROUNDED_ASSERTION_EVIDENCE.routeRegistered,
    ])

    const fabricated = evaluateOperatorGroundedAssertion({
      artifactKind: "api_endpoint",
      artifactRef: "/api/admin/khala/mint",
      lookupTool: "route_exists",
      lookupResult: "negative",
    })
    expect(fabricated.canAssert).toBe(false)
    expect(fabricated.missingEvidence).toContain(
      OPERATOR_GROUNDED_ASSERTION_EVIDENCE.routeRegistered,
    )
  })

  test("an empty artifact ref is not an assertion at all", () => {
    const result = evaluateOperatorGroundedAssertion({
      artifactKind: "command",
      artifactRef: "   ",
      lookupTool: "repo_grep",
      lookupResult: "positive",
    })
    expect(result.state).toBe("UNGROUNDED")
    expect(result.canAssert).toBe(false)
  })
})
