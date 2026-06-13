import type { ShipPipelinePlan } from "./ship-pipeline-plan"

export type ShipPipelineReceiptInput = {
  action: ShipPipelinePlan["action"]
  version: string
  spendDecision: "allow" | "deny"
  ranAt: string
}

export type ShipPipelineReceipt = {
  kind: "ship_pipeline_receipt"
  action: string
  version: string
  allowed: boolean
  ranAt: string
  line: string
}

export function buildPipelineReceipt(input: ShipPipelineReceiptInput): ShipPipelineReceipt {
  const allowed = input.spendDecision === "allow"

  return {
    kind: "ship_pipeline_receipt",
    action: input.action,
    version: input.version,
    allowed,
    ranAt: input.ranAt,
    line: formatPipelineReceiptLine({
      action: input.action,
      version: input.version,
      allowed,
      ranAt: input.ranAt,
    }),
  }
}

export function validate(receipt: unknown): boolean {
  if (!isReceiptRecord(receipt)) return false
  if (receipt.kind !== "ship_pipeline_receipt") return false
  if (!isPipelineAction(receipt.action)) return false
  if (typeof receipt.version !== "string") return false
  if (typeof receipt.allowed !== "boolean") return false
  if (typeof receipt.ranAt !== "string") return false
  if (typeof receipt.line !== "string") return false

  return receipt.line === formatPipelineReceiptLine(receipt)
}

function formatPipelineReceiptLine(receipt: {
  action: ShipPipelinePlan["action"]
  version: string
  allowed: boolean
  ranAt: string
}): string {
  const action = receipt.action === "ota"
    ? "OTA"
    : receipt.action === "rebuild"
      ? "Rebuild"
      : "Noop"
  const decision = receipt.allowed ? "allowed" : "denied"

  return `${action} ${receipt.version} pipeline ${decision} at ${receipt.ranAt}.`
}

function isPipelineAction(value: unknown): value is ShipPipelinePlan["action"] {
  return value === "ota" || value === "rebuild" || value === "noop"
}

function isReceiptRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
