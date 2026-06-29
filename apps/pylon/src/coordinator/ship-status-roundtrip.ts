import type { ShipMode } from "./ship-mode.js"
import type { ShipReceipt } from "./ship-receipt.js"

export type ShipStatusRoundtripShipMode = Extract<ShipMode, "ota" | "rebuild">

export type ShipStatusRoundtripState = "shipping" | "shipped" | "failed"

export type ShipStatusRoundtripInput = {
  intentRef?: string | null
  receipt: ShipReceipt
  receiptRef?: string | null
  state?: ShipStatusRoundtripState
  summary?: string | null
  updatedAt: string
}

export type ShipStatusRoundtrip = {
  intentRef: string
  shipMode: ShipStatusRoundtripShipMode
  state: ShipStatusRoundtripState
  summary: string
  receiptRef: string
  updatedAt: string
}

const UNKNOWN_INTENT_REF = "intent.unknown"

export function projectShipStatusRoundtrip(
  input: ShipStatusRoundtripInput,
): ShipStatusRoundtrip {
  return {
    intentRef: resolveIntentRef(input.intentRef, input.receipt.intentId),
    shipMode: resolveShipMode(input.receipt.shipMode),
    state: input.state ?? "shipped",
    summary: input.summary ?? input.receipt.summary,
    receiptRef: resolveReceiptRef(input),
    updatedAt: input.updatedAt,
  }
}

function resolveIntentRef(
  intentRef: string | null | undefined,
  receiptIntentId: string,
): string {
  return firstNonEmpty(intentRef, receiptIntentId) ?? UNKNOWN_INTENT_REF
}

function resolveShipMode(shipMode: ShipMode): ShipStatusRoundtripShipMode {
  if (shipMode === "ota" || shipMode === "rebuild") {
    return shipMode
  }

  throw new Error(`Cannot project ship status for non-shipping mode: ${shipMode}`)
}

function resolveReceiptRef(input: ShipStatusRoundtripInput): string {
  const intentRef = resolveIntentRef(input.intentRef, input.receipt.intentId)

  return (
    firstNonEmpty(
      input.receiptRef,
      input.receipt.updateId,
      input.receipt.buildId,
      input.receipt.artifactRef,
    ) ?? `ship-receipt:${intentRef}`
  )
}

function firstNonEmpty(
  ...values: ReadonlyArray<string | null | undefined>
): string | undefined {
  return values
    .map((value) => value?.trim())
    .find((value): value is string => Boolean(value))
}
