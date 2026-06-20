import { describe, expect, test } from "bun:test"

import { buildDecisionCloseoutReceipt } from "./decision-closeout-receipt.js"
import { createDecisionCloseoutLedger } from "./decision-closeout-ledger.js"

const desktopApplied = buildDecisionCloseoutReceipt({
  requestId: "dec-1",
  actionRef: "approve_pr_draft",
  verb: "approve",
  outcome: "applied",
  client: "desktop",
  actor: "owner",
  decidedAt: "2026-06-20T12:00:00.000Z",
})

const webAnswer = buildDecisionCloseoutReceipt({
  requestId: "dec-2",
  actionRef: "provide_context",
  verb: "answer",
  outcome: "applied",
  client: "web",
  actor: "owner",
  decidedAt: "2026-06-20T12:01:00.000Z",
  answer: "ship it",
})

const expoExpired = buildDecisionCloseoutReceipt({
  requestId: "dec-3",
  actionRef: "rerun_tests",
  verb: "approve",
  outcome: "expired",
  client: "expo",
  actor: "autopilot",
  decidedAt: "2026-06-20T12:02:00.000Z",
})

describe("decision closeout ledger", () => {
  test("starts empty", () => {
    const ledger = createDecisionCloseoutLedger()

    expect(ledger.list()).toEqual([])
    expect(ledger.summary().count).toBe(0)
    expect(ledger.summary().byOutcome.applied).toBe(0)
    expect(ledger.summary().byClient.desktop).toBe(0)
  })

  test("records a valid closeout", () => {
    const ledger = createDecisionCloseoutLedger()

    expect(ledger.append(desktopApplied)).toEqual({ accepted: true, deduped: false })
    expect(ledger.list()).toEqual([desktopApplied])
    expect(ledger.get("dec-1")).toEqual(desktopApplied)
  })

  test("rejects invalid receipts without recording them", () => {
    const ledger = createDecisionCloseoutLedger()

    expect(ledger.append(null)).toEqual({ accepted: false, reason: "invalid" })
    expect(ledger.append(["decision_closeout_receipt"])).toEqual({ accepted: false, reason: "invalid" })
    expect(ledger.append({ ...desktopApplied, line: "tampered" })).toEqual({ accepted: false, reason: "invalid" })

    expect(ledger.list()).toEqual([])
  })

  test("idempotently converges an identical cross-client re-append", () => {
    const ledger = createDecisionCloseoutLedger()

    expect(ledger.append(desktopApplied)).toEqual({ accepted: true, deduped: false })
    // Same closeout seen again (e.g. a second client surface observed the same
    // node broadcast) — the ledger does not grow.
    expect(ledger.append({ ...desktopApplied })).toEqual({ accepted: true, deduped: true })

    expect(ledger.list()).toEqual([desktopApplied])
    expect(ledger.summary().count).toBe(1)
  })

  test("refuses a conflicting closeout for an already-closed command", () => {
    const ledger = createDecisionCloseoutLedger()
    ledger.append(desktopApplied)

    // Same requestId, different outcome/client — exactly-once violation.
    const conflicting = buildDecisionCloseoutReceipt({
      requestId: "dec-1",
      actionRef: "approve_pr_draft",
      verb: "deny",
      outcome: "duplicate",
      client: "web",
      actor: "autopilot",
      decidedAt: "2026-06-20T12:09:00.000Z",
    })

    expect(ledger.append(conflicting)).toEqual({
      accepted: false,
      reason: "conflict",
      existing: desktopApplied,
    })
    // The original closeout stands.
    expect(ledger.get("dec-1")).toEqual(desktopApplied)
    expect(ledger.summary().count).toBe(1)
  })

  test("indexes audit slices by client, actor, and outcome", () => {
    const ledger = createDecisionCloseoutLedger()
    ledger.append(desktopApplied)
    ledger.append(webAnswer)
    ledger.append(expoExpired)

    expect(ledger.byClient("desktop")).toEqual([desktopApplied])
    expect(ledger.byClient("web")).toEqual([webAnswer])
    expect(ledger.byClient("expo")).toEqual([expoExpired])
    expect(ledger.byActor("owner")).toEqual([desktopApplied, webAnswer])
    expect(ledger.byActor("autopilot")).toEqual([expoExpired])
    expect(ledger.byOutcome("applied")).toEqual([desktopApplied, webAnswer])
    expect(ledger.byOutcome("expired")).toEqual([expoExpired])
  })

  test("summarizes counts across the closeout vocabulary", () => {
    const ledger = createDecisionCloseoutLedger()
    ledger.append(desktopApplied)
    ledger.append(webAnswer)
    ledger.append(expoExpired)

    const summary = ledger.summary()
    expect(summary.count).toBe(3)
    expect(summary.byOutcome.applied).toBe(2)
    expect(summary.byOutcome.expired).toBe(1)
    expect(summary.byOutcome.revoked).toBe(0)
    expect(summary.byClient).toEqual({ desktop: 1, web: 1, expo: 1 })
  })

  test("preserves append order and returns mutation-safe snapshots", () => {
    const ledger = createDecisionCloseoutLedger()
    ledger.append(desktopApplied)
    ledger.append(webAnswer)

    const listed = ledger.list()
    expect(listed).toEqual([desktopApplied, webAnswer])

    listed.push(expoExpired)
    listed[0].outcome = "error"
    expect(ledger.list()).toEqual([desktopApplied, webAnswer])
    expect(ledger.summary().count).toBe(2)
  })

  test("copies receipts on input so later caller mutation cannot corrupt the ledger", () => {
    const ledger = createDecisionCloseoutLedger()
    const receipt = { ...desktopApplied }

    expect(ledger.append(receipt)).toEqual({ accepted: true, deduped: false })
    receipt.outcome = "error"

    expect(ledger.get("dec-1")).toEqual(desktopApplied)
  })

  test("keeps ledgers isolated", () => {
    const first = createDecisionCloseoutLedger()
    const second = createDecisionCloseoutLedger()

    first.append(desktopApplied)
    second.append(webAnswer)

    expect(first.list()).toEqual([desktopApplied])
    expect(second.list()).toEqual([webAnswer])
  })
})
