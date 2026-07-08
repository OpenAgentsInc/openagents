import { describe, expect, test } from "bun:test"
import {
  MIXED_HARNESS_FLEET_RUN_SMOKE_SCHEMA,
  runMixedHarnessFleetRunCiSmoke,
} from "../src/mixed-harness-fleet-run-smoke"
import { assertPublicProjectionSafe } from "../src/state"

// MH-2 exit receipt (#8583): a mixed two-harness FleetRun (codex + claude) run
// through ONE claim registry, proving claim uniqueness under mixed kinds and
// both closeouts receipted — the abstraction the three-harness plan depends on.
describe("mixed two-harness FleetRun (codex + claude) exit receipt", () => {
  test("claim uniqueness holds under mixed kinds and both closeouts are receipted", async () => {
    const receipt = await runMixedHarnessFleetRunCiSmoke()

    expect(receipt.schema).toBe(MIXED_HARNESS_FLEET_RUN_SMOKE_SCHEMA)
    expect(receipt.ok).toBe(true)
    expect(receipt.blockerRefs).toEqual([])

    // One FleetRun, `auto` worker kind, two DISTINCT concrete harnesses.
    expect(receipt.workerKind).toBe("auto")
    expect([...receipt.distinctHarnessKinds].sort()).toEqual(["claude", "codex"])

    // Claim uniqueness: two live claims at peak, both cross-kind steal attempts
    // rejected, zero double-live claims.
    expect(receipt.claimUniqueness.liveClaimsAtPeak).toBe(2)
    expect(receipt.claimUniqueness.crossKindCollisionsPrevented).toBe(2)
    expect(receipt.claimUniqueness.doubleLiveClaims).toBe(0)

    // Both harness legs closed out with a receipted, redacted, no-spend closeout.
    expect(receipt.legs).toHaveLength(2)
    for (const leg of receipt.legs) {
      expect(leg.ok).toBe(true)
      expect(leg.closeoutStatus).toBe("accepted")
      expect(leg.closeoutRef).not.toBeNull()
      expect(leg.paymentMode).toBe("no-spend")
      expect(leg.settlementState).toBe("not_applicable")
      expect(leg.payoutClaimAllowed).toBe(false)
      expect(leg.redacted).toBe(true)
      expect(leg.redactionViolations).toEqual([])
    }
  })

  test("the exit receipt is public-projection safe", async () => {
    const receipt = await runMixedHarnessFleetRunCiSmoke()
    expect(() => assertPublicProjectionSafe(receipt)).not.toThrow()
  })
})
