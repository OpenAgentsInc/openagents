import { describe, expect, test } from "bun:test"

import { buildClaimReceipt, validate } from "./assignment-claim-receipt.js"

describe("assignment claim receipt", () => {
  test("builds a deterministic claim receipt", () => {
    expect(buildClaimReceipt({
      assignmentRef: "OpenAgentsInc/openagents#4928",
      claimedByHash: "agent_hash_abc123",
      claimedAt: "2026-06-13T12:00:00.000Z",
    })).toEqual({
      kind: "assignment_claim_receipt",
      assignmentRef: "OpenAgentsInc/openagents#4928",
      claimedByHash: "agent_hash_abc123",
      claimedAt: "2026-06-13T12:00:00.000Z",
      line: "Assignment OpenAgentsInc/openagents#4928 claimed by agent_hash_abc123 at 2026-06-13T12:00:00.000Z.",
    })
  })

  test("trims assignment refs through assignment claim validation", () => {
    expect(buildClaimReceipt({
      assignmentRef: "  assignment_4928  ",
      claimedByHash: "agent_hash_def456",
      claimedAt: "2026-06-13T12:05:00.000Z",
    })).toEqual({
      kind: "assignment_claim_receipt",
      assignmentRef: "assignment_4928",
      claimedByHash: "agent_hash_def456",
      claimedAt: "2026-06-13T12:05:00.000Z",
      line: "Assignment assignment_4928 claimed by agent_hash_def456 at 2026-06-13T12:05:00.000Z.",
    })
  })

  test("validates a built receipt", () => {
    const receipt = buildClaimReceipt({
      assignmentRef: "assignment_4929",
      claimedByHash: "agent_hash_ghi789",
      claimedAt: "2026-06-13T12:10:00.000Z",
    })

    expect(validate(receipt)).toBe(true)
  })

  test("rejects non-receipt payloads", () => {
    expect(validate(null)).toBe(false)
    expect(validate(["assignment_claim_receipt"])).toBe(false)
    expect(validate({ kind: "assignment_claim" })).toBe(false)
  })

  test("rejects malformed scalar fields", () => {
    const receipt = buildClaimReceipt({
      assignmentRef: "assignment_4930",
      claimedByHash: "agent_hash_jkl012",
      claimedAt: "2026-06-13T12:15:00.000Z",
    })

    expect(validate({ ...receipt, assignmentRef: "" })).toBe(false)
    expect(validate({ ...receipt, claimedByHash: 4928 })).toBe(false)
    expect(validate({ ...receipt, claimedAt: 1781352900000 })).toBe(false)
  })

  test("rejects receipts with unnormalized assignment refs", () => {
    const receipt = buildClaimReceipt({
      assignmentRef: "assignment_4931",
      claimedByHash: "agent_hash_mno345",
      claimedAt: "2026-06-13T12:20:00.000Z",
    })

    expect(validate({
      ...receipt,
      assignmentRef: " assignment_4931 ",
      line: "Assignment  assignment_4931  claimed by agent_hash_mno345 at 2026-06-13T12:20:00.000Z.",
    })).toBe(false)
  })

  test("rejects a receipt with a mismatched line", () => {
    const receipt = buildClaimReceipt({
      assignmentRef: "assignment_4932",
      claimedByHash: "agent_hash_pqr678",
      claimedAt: "2026-06-13T12:25:00.000Z",
    })

    expect(validate({
      ...receipt,
      line: "Assignment assignment_4932 claimed by agent_hash_other at 2026-06-13T12:25:00.000Z.",
    })).toBe(false)
  })
})
