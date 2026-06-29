import { describe, expect, test } from "bun:test"

import { buildShipReceipt } from "./autonomous-ship-receipt.js"
import { createShipReceiptLedger } from "./ship-receipt-ledger.js"

const allowedReceipt = buildShipReceipt({
  mode: "ota",
  version: "v1.2.3",
  spendDecision: "allow",
  actor: "autopilot",
  shippedAt: "2026-06-13T12:00:00.000Z",
})

const deniedReceipt = buildShipReceipt({
  mode: "rebuild",
  version: "v2.0.0",
  spendDecision: "deny",
  actor: "owner",
  shippedAt: "2026-06-13T12:05:00.000Z",
})

describe("ship receipt ledger", () => {
  test("starts empty", () => {
    const ledger = createShipReceiptLedger()

    expect(ledger.list()).toEqual([])
    expect(ledger.summary()).toEqual({ count: 0, allowedCount: 0, deniedCount: 0 })
  })

  test("accepts a valid allowed receipt", () => {
    const ledger = createShipReceiptLedger()

    expect(ledger.append(allowedReceipt)).toEqual({ accepted: true })
    expect(ledger.list()).toEqual([allowedReceipt])
    expect(ledger.summary()).toEqual({ count: 1, allowedCount: 1, deniedCount: 0 })
  })

  test("accepts a valid denied receipt", () => {
    const ledger = createShipReceiptLedger()

    expect(ledger.append(deniedReceipt)).toEqual({ accepted: true })
    expect(ledger.list()).toEqual([deniedReceipt])
    expect(ledger.summary()).toEqual({ count: 1, allowedCount: 0, deniedCount: 1 })
  })

  test("preserves append order and summarizes mixed decisions", () => {
    const ledger = createShipReceiptLedger()

    ledger.append(allowedReceipt)
    ledger.append(deniedReceipt)

    expect(ledger.list()).toEqual([allowedReceipt, deniedReceipt])
    expect(ledger.summary()).toEqual({ count: 2, allowedCount: 1, deniedCount: 1 })
  })

  test("rejects invalid receipts without appending them", () => {
    const ledger = createShipReceiptLedger()

    expect(ledger.append(null)).toEqual({ accepted: false })
    expect(ledger.append(["ship_receipt"])).toEqual({ accepted: false })
    expect(ledger.append({ ...allowedReceipt, line: "changed" })).toEqual({ accepted: false })

    expect(ledger.list()).toEqual([])
    expect(ledger.summary()).toEqual({ count: 0, allowedCount: 0, deniedCount: 0 })
  })

  test("keeps ledgers isolated", () => {
    const firstLedger = createShipReceiptLedger()
    const secondLedger = createShipReceiptLedger()

    firstLedger.append(allowedReceipt)
    secondLedger.append(deniedReceipt)

    expect(firstLedger.list()).toEqual([allowedReceipt])
    expect(secondLedger.list()).toEqual([deniedReceipt])
  })

  test("returns list snapshots that cannot mutate ledger state", () => {
    const ledger = createShipReceiptLedger()
    ledger.append(allowedReceipt)

    const listed = ledger.list()
    listed.push(deniedReceipt)
    listed[0].allowed = false

    expect(ledger.list()).toEqual([allowedReceipt])
    expect(ledger.summary()).toEqual({ count: 1, allowedCount: 1, deniedCount: 0 })
  })

  test("copies accepted receipts before storing them", () => {
    const ledger = createShipReceiptLedger()
    const receipt = { ...allowedReceipt }

    expect(ledger.append(receipt)).toEqual({ accepted: true })
    receipt.allowed = false

    expect(ledger.list()).toEqual([allowedReceipt])
    expect(ledger.summary()).toEqual({ count: 1, allowedCount: 1, deniedCount: 0 })
  })
})
