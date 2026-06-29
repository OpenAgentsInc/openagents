import { describe, expect, test } from "bun:test"
import {
  classifyActionOutcome,
  createReceiptLedger,
  receiptFor,
  recordAction,
} from "../src/node/bridge-receipts"

const allowedReceiptKeys = ["clientRequestId", "receiptRef", "status"]

describe("bridge action receipts", () => {
  test("recordAction records once then dedupes by idempotency key", () => {
    const first = recordAction(createReceiptLedger(), {
      clientRequestId: "request-1",
      idempotencyKey: "idem-1",
      receiptRef: "receipt-1",
      status: "ok",
    })

    const second = recordAction(first.ledger, {
      clientRequestId: "request-2",
      idempotencyKey: "idem-1",
      receiptRef: "receipt-2",
      status: "cancelled",
    })

    expect(first.deduped).toBe(false)
    expect(first.receipt).toEqual({
      clientRequestId: "request-1",
      receiptRef: "receipt-1",
      status: "ok",
    })
    expect(second.deduped).toBe(true)
    expect(second.receipt).toEqual({
      clientRequestId: "request-1",
      receiptRef: "receipt-1",
      status: "duplicate",
    })
    expect(second.receipt.receiptRef).toBe(first.receipt.receiptRef)
    expect(second.ledger.receiptsByIdempotencyKey.get("idem-1")).toEqual(first.receipt)
  })

  test("classifyActionOutcome applies precedence branches", () => {
    const base = {
      capabilityOk: true,
      pairingActive: true,
      decisionExpired: false,
      cancelled: false,
      supported: true,
      overloaded: false,
    }

    expect(classifyActionOutcome({ ...base, supported: false, overloaded: true })).toBe("unsupported")
    expect(classifyActionOutcome({ ...base, overloaded: true, pairingActive: false })).toBe("overloaded")
    expect(classifyActionOutcome({ ...base, pairingActive: false, capabilityOk: false })).toBe("revoked")
    expect(classifyActionOutcome({ ...base, capabilityOk: false, cancelled: true })).toBe("unauthorized")
    expect(classifyActionOutcome({ ...base, cancelled: true, decisionExpired: true })).toBe("cancelled")
    expect(classifyActionOutcome({ ...base, decisionExpired: true })).toBe("expired")
    expect(classifyActionOutcome(base)).toBe("ok")
  })

  test("receipts contain only refs and status", () => {
    const receipt = receiptFor({
      clientRequestId: "request-1",
      receiptRef: "receipt-1",
      status: "ok",
    })
    const withoutRef = receiptFor({
      clientRequestId: "request-2",
      status: "expired",
    })
    const recorded = recordAction(createReceiptLedger(), {
      clientRequestId: "request-3",
      idempotencyKey: "idem-3",
      receiptRef: "receipt-3",
      status: "cancelled",
    }).receipt

    expect(Object.keys(receipt).sort()).toEqual(allowedReceiptKeys)
    expect(Object.keys(withoutRef).sort()).toEqual(allowedReceiptKeys)
    expect(Object.keys(recorded).sort()).toEqual(allowedReceiptKeys)
    expect(withoutRef.receiptRef).toBeNull()
  })
})
