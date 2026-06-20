import {
  estimateCloudCost,
  type CloudCostEstimateInput,
} from "./cloud-cost-estimate.js"
import {
  projectFailover,
  type ProviderFailoverAccount,
} from "./provider-failover-state.js"

export type CloudQuotaSummaryInput = {
  accounts: ProviderFailoverAccount[]
  usage: CloudCostEstimateInput
}

export type CloudQuotaSummary = {
  active: string | null
  standby: string[]
  failedOver: boolean
  costSats: number
  line: string
}

export function buildCloudSummary(
  input: CloudQuotaSummaryInput,
): CloudQuotaSummary {
  const record: Record<string, unknown> = isRecord(input) ? input : {}
  const failover = projectFailover(record.accounts as ProviderFailoverAccount[])
  const estimate = estimateCloudCost(readUsage(record.usage))

  return {
    active: failover.active,
    standby: failover.standby,
    failedOver: failover.failedOver,
    costSats: estimate.costSats,
    line: summaryLine(failover.active, failover.standby, failover.failedOver, estimate.costSats),
  }
}

function readUsage(value: unknown): CloudCostEstimateInput {
  return (isRecord(value) ? value : {}) as CloudCostEstimateInput
}

function summaryLine(
  active: string | null,
  standby: readonly string[],
  failedOver: boolean,
  costSats: number,
): string {
  const activeLabel = active === null
    ? "none"
    : `${oneLine(active)}${failedOver ? " (failover)" : ""}`
  const standbyLabel = standby.length > 0
    ? standby.map(oneLine).join(", ")
    : "none"

  return `active: ${activeLabel}; standby: ${standbyLabel}; est. cost: ${costSats} sats`
}

function oneLine(value: string): string {
  return value.replace(/\s+/g, " ").trim()
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
