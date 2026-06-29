import {
  estimateCloudCost,
  type CloudCostEstimateInput,
} from "./cloud-cost-estimate.js"
import { cloudMeteringState } from "./cloud-metering-source-state.js"
import {
  projectCloudQuota,
  type CloudQuotaView,
} from "./cloud-quota-view.js"

export type CloudPanelViewModel = {
  available: boolean
  line: string
  quota: CloudQuotaView | null
  costSats: number | null
}

export function buildCloudPanel(raw: unknown): CloudPanelViewModel {
  const metering = cloudMeteringState(raw)
  if (!metering.available) {
    return {
      available: false,
      line: "cloud metering unavailable",
      quota: null,
      costSats: null,
    }
  }

  const quota = projectCloudQuota(raw)
  const costInput = readCostInput(raw)
  const costSats = costInput === null
    ? null
    : estimateCloudCost(costInput).costSats

  return {
    available: true,
    line: panelLine(quota, costSats),
    quota,
    costSats,
  }
}

function panelLine(quota: CloudQuotaView, costSats: number | null): string {
  return [
    "cloud metering available",
    `quota: ${quotaLine(quota)}`,
    `est. cost: ${costSats === null ? "unknown" : `${costSats} sats`}`,
  ].join("; ")
}

function quotaLine(quota: CloudQuotaView): string {
  if (quota.usedSats === null || quota.capSats === null) return "unknown"

  const percent = quota.percentUsed === null
    ? "unknown"
    : `${Math.round(quota.percentUsed)}%`
  const remaining = quota.remainingSats === null
    ? "unknown"
    : `${quota.remainingSats} sats`

  return `${quota.usedSats}/${quota.capSats} sats (${percent} used, ${remaining} remaining)`
}

function readCostInput(raw: unknown): CloudCostEstimateInput | null {
  for (const record of candidateRecords(raw)) {
    const input = parseCostInput(record)
    if (input !== null) return input
  }

  return null
}

function candidateRecords(raw: unknown): Record<string, unknown>[] {
  if (!isRecord(raw)) return []

  const records = [raw]
  for (const key of ["usage", "cost", "billing", "estimate", "cloudCost", "cloud_cost"]) {
    const value = raw[key]
    if (isRecord(value)) records.push(value)
  }
  return records
}

function parseCostInput(record: Record<string, unknown>): CloudCostEstimateInput | null {
  const tokensIn = readFiniteNumber(record, ["tokensIn", "tokens_in", "inputTokens", "input_tokens"])
  const tokensOut = readFiniteNumber(record, ["tokensOut", "tokens_out", "outputTokens", "output_tokens"])
  const ratePerMTokIn = readFiniteNumber(record, [
    "ratePerMTokIn",
    "rate_per_mtok_in",
    "inputRatePerMTok",
    "input_rate_per_mtok",
  ])
  const ratePerMTokOut = readFiniteNumber(record, [
    "ratePerMTokOut",
    "rate_per_mtok_out",
    "outputRatePerMTok",
    "output_rate_per_mtok",
  ])

  if (
    tokensIn === null ||
    tokensOut === null ||
    ratePerMTokIn === null ||
    ratePerMTokOut === null
  ) {
    return null
  }

  return {
    tokensIn,
    tokensOut,
    ratePerMTokIn,
    ratePerMTokOut,
  }
}

function readFiniteNumber(
  record: Record<string, unknown>,
  keys: readonly string[],
): number | null {
  for (const key of keys) {
    if (!Object.hasOwn(record, key)) continue

    const value = record[key]
    return typeof value === "number" && Number.isFinite(value) ? value : null
  }

  return null
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
