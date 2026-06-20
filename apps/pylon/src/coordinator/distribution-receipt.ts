import type { DistributionTarget } from "./distribution-checklist.js"

export type DistributionReceiptInput = {
  target: DistributionTarget
  artifactRef: string
  version: string
  distributedAt: string
}

export type DistributionReceipt = {
  readonly target: DistributionTarget
  readonly artifactRef: string
  readonly version: string
  readonly distributedAt: string
}

const RECEIPT_KEYS = ["artifactRef", "distributedAt", "target", "version"].sort()
const TARGETS = new Set<DistributionTarget>(["desktop", "mobile", "ota"])

export function buildDistributionReceipt(input: DistributionReceiptInput): DistributionReceipt {
  return {
    target: input.target,
    artifactRef: input.artifactRef.trim(),
    version: input.version.trim(),
    distributedAt: input.distributedAt.trim(),
  }
}

export function validateDistributionReceipt(receipt: unknown): receipt is DistributionReceipt {
  if (receipt === null || typeof receipt !== "object" || Array.isArray(receipt)) {
    return false
  }

  const keys = Object.keys(receipt).sort()
  if (keys.length !== RECEIPT_KEYS.length || keys.some((key, index) => key !== RECEIPT_KEYS[index])) {
    return false
  }

  const record = receipt as Record<string, unknown>
  return (
    isDistributionTarget(record.target) &&
    isNonEmptyString(record.artifactRef) &&
    isNonEmptyString(record.version) &&
    isNonEmptyString(record.distributedAt)
  )
}

function isDistributionTarget(value: unknown): value is DistributionTarget {
  return typeof value === "string" && TARGETS.has(value as DistributionTarget)
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}
