import { describe, expect, test } from "bun:test"

import { createApprovalLedger } from "./approval-exactly-once.js"

describe("approval exactly-once ledger", () => {
  test("applies the first approve decision for a key", () => {
    const ledger = createApprovalLedger()

    expect(ledger.record("approval-0001", "approve")).toEqual({
      applied: true,
      decision: "approve",
      duplicate: false,
    })
  })

  test("applies the first deny decision for a key", () => {
    const ledger = createApprovalLedger()

    expect(ledger.record("approval-0002", "deny")).toEqual({
      applied: true,
      decision: "deny",
      duplicate: false,
    })
  })

  test("applies the first answer decision for a key", () => {
    const ledger = createApprovalLedger()

    expect(ledger.record("approval-0003", "answer")).toEqual({
      applied: true,
      decision: "answer",
      duplicate: false,
    })
  })

  test("suppresses duplicate records for the same key", () => {
    const ledger = createApprovalLedger()

    ledger.record("approval-0004", "approve")

    expect(ledger.record("approval-0004", "approve")).toEqual({
      applied: false,
      decision: "approve",
      duplicate: true,
    })
  })

  test("keeps the original decision when a duplicate changes decision", () => {
    const ledger = createApprovalLedger()

    ledger.record("approval-0005", "deny")

    expect(ledger.record("approval-0005", "approve")).toEqual({
      applied: false,
      decision: "deny",
      duplicate: true,
    })
  })

  test("tracks different keys independently", () => {
    const ledger = createApprovalLedger()

    expect(ledger.record("approval-0006-a", "approve")).toEqual({
      applied: true,
      decision: "approve",
      duplicate: false,
    })
    expect(ledger.record("approval-0006-b", "deny")).toEqual({
      applied: true,
      decision: "deny",
      duplicate: false,
    })
  })

  test("keeps the original decision across repeated duplicates", () => {
    const ledger = createApprovalLedger()

    ledger.record("approval-0007", "answer")
    ledger.record("approval-0007", "deny")

    expect(ledger.record("approval-0007", "approve")).toEqual({
      applied: false,
      decision: "answer",
      duplicate: true,
    })
  })
})
