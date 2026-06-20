import type {
  ShipAuthorizationDecision,
  ShipMode,
} from "./loop-safety.js"

export type ShipReceiptInput = {
  intentId: string
  shipMode: ShipMode
  decision: ShipAuthorizationDecision
  artifactRef?: string
  updateId?: string
  buildId?: string
  summary: string
}

export type ShipReceipt = {
  intentId: string
  shipMode: ShipMode
  decision: ShipAuthorizationDecision
  artifactRef?: string
  updateId?: string
  buildId?: string
  summary: string
}

export function buildShipReceipt(input: ShipReceiptInput): ShipReceipt {
  return {
    intentId: input.intentId,
    shipMode: input.shipMode,
    decision: input.decision,
    ...(input.artifactRef ? { artifactRef: input.artifactRef } : {}),
    ...(input.updateId ? { updateId: input.updateId } : {}),
    ...(input.buildId ? { buildId: input.buildId } : {}),
    summary: input.summary,
  }
}
