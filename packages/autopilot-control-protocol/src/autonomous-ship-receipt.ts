export type AutonomousShipReceiptInput = {
  mode: "ota" | "rebuild"
  version: string
  spendDecision: "allow" | "deny"
  actor: "autopilot" | "owner"
  shippedAt: string
}

export type AutonomousShipReceipt = {
  kind: "ship_receipt"
  mode: string
  version: string
  allowed: boolean
  actor: string
  shippedAt: string
  line: string
}

export function buildShipReceipt(input: AutonomousShipReceiptInput): AutonomousShipReceipt {
  const allowed = input.spendDecision === "allow"

  return {
    kind: "ship_receipt",
    mode: input.mode,
    version: input.version,
    allowed,
    actor: input.actor,
    shippedAt: input.shippedAt,
    line: formatShipReceiptLine({
      mode: input.mode,
      version: input.version,
      allowed,
      actor: input.actor,
      shippedAt: input.shippedAt,
    }),
  }
}

export function validateShipReceipt(receipt: unknown): boolean {
  if (!isReceiptRecord(receipt)) return false
  if (receipt.kind !== "ship_receipt") return false
  if (receipt.mode !== "ota" && receipt.mode !== "rebuild") return false
  if (typeof receipt.version !== "string") return false
  if (typeof receipt.allowed !== "boolean") return false
  if (receipt.actor !== "autopilot" && receipt.actor !== "owner") return false
  if (typeof receipt.shippedAt !== "string") return false
  if (typeof receipt.line !== "string") return false

  // Each field is validated above; build the typed shape explicitly so the
  // formatter receives the exact type rather than a broad Record.
  return receipt.line === formatShipReceiptLine({
    mode: receipt.mode,
    version: receipt.version,
    allowed: receipt.allowed,
    actor: receipt.actor,
    shippedAt: receipt.shippedAt,
  })
}

function formatShipReceiptLine(receipt: {
  mode: "ota" | "rebuild"
  version: string
  allowed: boolean
  actor: "autopilot" | "owner"
  shippedAt: string
}): string {
  const mode = receipt.mode === "ota" ? "OTA" : "Rebuild"
  const decision = receipt.allowed ? "allowed" : "denied"

  return `${mode} ${receipt.version} ship ${decision} by ${receipt.actor} at ${receipt.shippedAt}.`
}

function isReceiptRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
