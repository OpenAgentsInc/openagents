import { describe, expect, test } from "bun:test"

import {
  KhalaMobileLaunchReadinessSchemaId,
  isKhalaMobileLaunchReady,
  khalaMobileLaunchReadinessOwnerActions,
  khalaMobileLaunchReadinessReceipt,
  khalaMobileLaunchReadinessRemainingOwnerAsk,
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
      expect(check.ownerActionRefs).toContain("NEEDS_OWNER.md#khala-mobile-p08-launch-readiness")
      // Honesty invariant: a non-PASS check must name its blockers; a PASS
      // check must have none left and must cite real evidence.
      if (check.verdict === "PASS") {
        expect(check.blockerRefs).toEqual([])
        expect(check.evidenceRefs.length).toBeGreaterThan(0)
      } else {
        expect(check.blockerRefs.length).toBeGreaterThan(0)
      }
    }

    // The seeded-account owner gate RESOLVED 2026-07-09 (GitHub user
    // AgentFlampy + fork AgentFlampy/openagents, recorded on #8543). The
    // other two checks stay non-PASS until the full dual-platform receipts
    // and the owner copy sign-off exist — including the CX-3 (#8547)
    // writeback wall, which must remain a named blocker until that lane runs.
    const byId = new Map(khalaMobileLaunchReadinessReceipt.checks.map(check => [check.id, check]))
    expect(byId.get("seeded_public_safe_github_account")?.verdict).toBe("PASS")
    expect(byId.get("seeded_public_safe_github_account")?.notes).toContain("AgentFlampy/openagents")
    expect(byId.get("ios_android_full_straight_line_e2e")?.verdict).not.toBe("PASS")
    expect(byId.get("ios_android_full_straight_line_e2e")?.blockerRefs.join(" ")).toContain("8547")
    expect(byId.get("promises_copy_pass")?.verdict).not.toBe("PASS")
  })

  test("launch_readiness_owner_gate_documented.source — owner action list and NEEDS_OWNER entry stay aligned", async () => {
    const ownerDoc = await Bun.file(repoPath("NEEDS_OWNER.md")).text()
    expect(ownerDoc).toContain("Khala Mobile P0.8 Launch Readiness")
    expect(ownerDoc).toContain("Source issue: OpenAgentsInc/openagents#8543")

    for (const action of khalaMobileLaunchReadinessOwnerActions) {
      expect(ownerDoc).toContain(action)
    }

    // The one remaining owner tap for the unattended harness (2026-07-09):
    // capture an AgentFlampy mobile OpenAuth session so the user-session-only
    // legs (repo list/bind, credits) can run.
    expect(ownerDoc).toContain(khalaMobileLaunchReadinessRemainingOwnerAsk)
  })
})
