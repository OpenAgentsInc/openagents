// Bridge action receipt core (CL-12 / issue #4914).
//
// Pure, transport-agnostic module. Live control-server/session wiring belongs
// to CL-14; this file only tracks idempotency receipts and classifies outcomes.

import {
  type BridgeActionReceipt,
  type BridgeResultStatus,
} from "@openagentsinc/autopilot-control-protocol"

export type ReceiptLedger = {
  readonly receiptsByIdempotencyKey: ReadonlyMap<string, BridgeActionReceipt>
}

export type RecordActionInput = {
  clientRequestId: string
  idempotencyKey: string
  status: BridgeResultStatus
  receiptRef?: string
}

export type ClassifyActionOutcomeInput = {
  capabilityOk: boolean
  pairingActive: boolean
  decisionExpired: boolean
  cancelled: boolean
  supported: boolean
  overloaded: boolean
}

export type ReceiptForInput = {
  clientRequestId: string
  status: BridgeResultStatus
  receiptRef?: string
}

export function createReceiptLedger(): ReceiptLedger {
  return { receiptsByIdempotencyKey: new Map() }
}

export function receiptFor(input: ReceiptForInput): BridgeActionReceipt {
  return {
    clientRequestId: input.clientRequestId,
    receiptRef: input.receiptRef ?? null,
    status: input.status,
  }
}

export function recordAction(
  ledger: ReceiptLedger,
  input: RecordActionInput,
): { ledger: ReceiptLedger; receipt: BridgeActionReceipt; deduped: boolean } {
  const prior = ledger.receiptsByIdempotencyKey.get(input.idempotencyKey)
  if (prior) {
    return {
      ledger,
      receipt: { ...prior, status: "duplicate" },
      deduped: true,
    }
  }

  const receipt = receiptFor(input)
  const receiptsByIdempotencyKey = new Map(ledger.receiptsByIdempotencyKey)
  receiptsByIdempotencyKey.set(input.idempotencyKey, receipt)

  return {
    ledger: { receiptsByIdempotencyKey },
    receipt,
    deduped: false,
  }
}

export function classifyActionOutcome(input: ClassifyActionOutcomeInput): BridgeResultStatus {
  if (!input.supported) return "unsupported"
  if (input.overloaded) return "overloaded"
  if (!input.pairingActive) return "revoked"
  if (!input.capabilityOk) return "unauthorized"
  if (input.cancelled) return "cancelled"
  if (input.decisionExpired) return "expired"
  return "ok"
}
