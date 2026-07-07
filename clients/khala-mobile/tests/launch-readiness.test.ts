import { describe, expect, test } from "bun:test"

import {
  KhalaMobileLaunchReadinessSchemaId,
  isKhalaMobileLaunchReady,
  khalaMobileLaunchReadinessOwnerActions,
  khalaMobileLaunchReadinessReceipt,
} from "../src/qa/launch-readiness"

const repoPath = (ref: string): string =>
  new URL(`../../../${ref}`, import.meta.url).pathname

// Oracle for khala_mobile.qa.launch_readiness_honesty.v1
describe("Khala Mobile P0.8 launch readiness receipt", () => {
  test("launch_readiness_receipt_is_honest.unit — full launch E2E remains inconclusive until owner-gated proofs exist", () => {
    expect(khalaMobileLaunchReadinessReceipt.schema).toBe(KhalaMobileLaunchReadinessSchemaId)
    expect(khalaMobileLaunchReadinessReceipt.issue).toBe(8543)
    expect(khalaMobileLaunchReadinessReceipt.overallVerdict).toBe("INCONCLUSIVE")
    expect(isKhalaMobileLaunchReady(khalaMobileLaunchReadinessReceipt)).toBe(false)

    expect(khalaMobileLaunchReadinessReceipt.checks.map(check => check.id)).toEqual([
      "seeded_public_safe_github_account",
      "ios_android_full_straight_line_e2e",
      "promises_copy_pass",
    ])

    for (const check of khalaMobileLaunchReadinessReceipt.checks) {
      expect(check.requiredForIssue).toBe(8543)
      expect(check.verdict).not.toBe("PASS")
      expect(check.blockerRefs.length).toBeGreaterThan(0)
      expect(check.ownerActionRefs).toContain("NEEDS_OWNER.md#khala-mobile-p08-launch-readiness")
    }
  })

  test("launch_readiness_owner_gate_documented.source — owner action list and NEEDS_OWNER entry stay aligned", async () => {
    const ownerDoc = await Bun.file(repoPath("NEEDS_OWNER.md")).text()
    expect(ownerDoc).toContain("Khala Mobile P0.8 Launch Readiness")
    expect(ownerDoc).toContain("Source issue: OpenAgentsInc/openagents#8543")

    for (const action of khalaMobileLaunchReadinessOwnerActions) {
      expect(ownerDoc).toContain(action)
    }
  })
})
