import {
  cloudMeteringState,
  type CloudMeteringSourceReason,
} from "./cloud-metering-source-state.js"

export type CloudPanelState = {
  state: "available" | "unavailable"
  line: string
  quota: any | null
}

const QUOTA_KEYS = [
  "quota",
  "cloudQuota",
  "cloud_quota",
  "metering",
  "cloudMetering",
  "cloud_metering",
  "billing",
  "cost",
  "usage",
] as const

export function cloudPanelState(raw: unknown): CloudPanelState {
  const metering = cloudMeteringState(raw)

  if (!metering.available) {
    return {
      state: "unavailable",
      line: unavailableLine(metering.reason, metering.observedAt),
      quota: null,
    }
  }

  return {
    state: "available",
    line: `Cloud metering available; observed ${metering.observedAt}`,
    quota: readQuota(raw),
  }
}

function unavailableLine(
  reason: CloudMeteringSourceReason,
  observedAt: string | null,
): string {
  switch (reason) {
    case "no_feed":
      return "Cloud metering not available on this node"
    case "stale":
      return observedAt === null
        ? "Cloud metering unavailable: stale feed"
        : `Cloud metering unavailable: stale feed observed ${observedAt}`
    case "malformed":
      return "Cloud metering unavailable: malformed feed"
    case "ok":
      return "Cloud metering available"
  }
}

function readQuota(raw: unknown): any | null {
  if (!isPlainRecord(raw)) return null

  for (const key of QUOTA_KEYS) {
    const value = raw[key]
    if (isPlainRecord(value)) return value
  }

  return raw
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
