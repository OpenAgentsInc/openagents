import { describe, expect, test } from "bun:test"

import { buildApprovalReceipt, validateApprovalReceipt } from "./approval-receipt.js"

describe("approval receipt", () => {
  test("builds approve receipts without answer state", () => {
    expect(buildApprovalReceipt({
      ref: "approval-1",
      decision: "approve",
      decidedAt: "2026-06-13T12:00:00.000Z",
      actor: "owner",
    })).toEqual({
      kind: "approval_receipt",
      ref: "approval-1",
      decision: "approve",
      hasAnswer: false,
      decidedAt: "2026-06-13T12:00:00.000Z",
      actor: "owner",
      line: "Approval approval-1 approve by owner at 2026-06-13T12:00:00.000Z.",
    })
  })

  test("builds deny receipts without answer state", () => {
    expect(buildApprovalReceipt({
      ref: "approval-2",
      decision: "deny",
      answer: "not needed",
      decidedAt: "2026-06-13T12:05:00.000Z",
      actor: "autopilot",
    })).toEqual({
      kind: "approval_receipt",
      ref: "approval-2",
      decision: "deny",
      hasAnswer: false,
      decidedAt: "2026-06-13T12:05:00.000Z",
      actor: "autopilot",
      line: "Approval approval-2 deny by autopilot at 2026-06-13T12:05:00.000Z.",
    })
  })

  test("builds answer receipts with answer state", () => {
    expect(buildApprovalReceipt({
      ref: "approval-3",
      decision: "answer",
      answer: "Use staging.",
      decidedAt: "2026-06-13T12:10:00.000Z",
      actor: "owner",
    })).toEqual({
      kind: "approval_receipt",
      ref: "approval-3",
      decision: "answer",
      hasAnswer: true,
      decidedAt: "2026-06-13T12:10:00.000Z",
      actor: "owner",
      line: "Approval approval-3 answer with answer by owner at 2026-06-13T12:10:00.000Z.",
    })
  })

  test("does not count empty answers", () => {
    expect(buildApprovalReceipt({
      ref: "approval-4",
      decision: "answer",
      answer: "",
      decidedAt: "2026-06-13T12:15:00.000Z",
      actor: "owner",
    })).toEqual({
      kind: "approval_receipt",
      ref: "approval-4",
      decision: "answer",
      hasAnswer: false,
      decidedAt: "2026-06-13T12:15:00.000Z",
      actor: "owner",
      line: "Approval approval-4 answer by owner at 2026-06-13T12:15:00.000Z.",
    })
  })

  test("validates a built receipt", () => {
    const receipt = buildApprovalReceipt({
      ref: "approval-5",
      decision: "approve",
      decidedAt: "2026-06-13T12:20:00.000Z",
      actor: "owner",
    })

    expect(validateApprovalReceipt(receipt)).toBe(true)
  })

  test("rejects non-receipt payloads", () => {
    expect(validateApprovalReceipt(null)).toBe(false)
    expect(validateApprovalReceipt(["approval_receipt"])).toBe(false)
    expect(validateApprovalReceipt({ kind: "approval_status" })).toBe(false)
  })

  test("rejects malformed scalar fields", () => {
    const receipt = buildApprovalReceipt({
      ref: "approval-6",
      decision: "deny",
      decidedAt: "2026-06-13T12:25:00.000Z",
      actor: "owner",
    })

    expect(validateApprovalReceipt({ ...receipt, ref: 6 })).toBe(false)
    expect(validateApprovalReceipt({ ...receipt, hasAnswer: "false" })).toBe(false)
    expect(validateApprovalReceipt({ ...receipt, decidedAt: 1781353500000 })).toBe(false)
    expect(validateApprovalReceipt({ ...receipt, actor: null })).toBe(false)
  })

  test("rejects invalid decisions and mismatched lines", () => {
    const receipt = buildApprovalReceipt({
      ref: "approval-7",
      decision: "answer",
      answer: "Use staging.",
      decidedAt: "2026-06-13T12:30:00.000Z",
      actor: "owner",
    })

    expect(validateApprovalReceipt({ ...receipt, decision: "allow" })).toBe(false)
    expect(validateApprovalReceipt({
      ...receipt,
      line: "Approval approval-7 answer by owner at 2026-06-13T12:30:00.000Z.",
    })).toBe(false)
  })
})
