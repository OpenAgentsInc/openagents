import type { DistributionTarget } from "./distribution-notarize-plan.js"

export type DistributionChannelView = {
  channels: {
    target: DistributionTarget
    latestVersion: string | null
    state: string
  }[]
  total: number
}

type RawRecord = Record<string, unknown>

const TARGETS = new Set<DistributionTarget>(["desktop", "mobile", "ota"])

const ROW_LIST_ALIASES = [
  "channels",
  "distributionChannels",
  "distribution_channels",
  "items",
  "rows",
] as const

const TARGET_ALIASES = ["target", "distributionTarget", "distribution_target", "platform"] as const

const VERSION_ALIASES = [
  "latestVersion",
  "latest_version",
  "currentVersion",
  "current_version",
  "releaseVersion",
  "release_version",
  "version",
] as const

export function projectDistributionChannels(raw: unknown): DistributionChannelView {
  const channels: DistributionChannelView["channels"] = []

  for (const row of readRows(raw)) {
    if (!isRecord(row)) continue

    const target = readTarget(row)
    if (target === null) continue

    channels.push({
      target,
      latestVersion: readVersion(row),
      state: readState(row),
    })
  }

  return {
    channels,
    total: channels.length,
  }
}

function readRows(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw
  if (!isRecord(raw)) return []

  for (const key of ROW_LIST_ALIASES) {
    if (Array.isArray(raw[key])) return raw[key]
  }

  const keyedRows: unknown[] = []
  for (const target of TARGETS) {
    const value = raw[target]
    if (isRecord(value)) keyedRows.push({ ...value, target })
  }

  return keyedRows
}

function readTarget(row: RawRecord): DistributionTarget | null {
  for (const key of TARGET_ALIASES) {
    const value = row[key]
    if (typeof value !== "string") continue

    const target = value.trim().toLowerCase()
    if (isDistributionTarget(target)) return target
  }

  return null
}

function readVersion(row: RawRecord): string | null {
  for (const key of VERSION_ALIASES) {
    const value = row[key]
    if (typeof value !== "string") continue

    const version = value.trim()
    if (version.length > 0) return version
  }

  return null
}

function readState(row: RawRecord): string {
  const value = row.state
  if (typeof value !== "string") return "unknown"

  const state = value.trim()
  return state.length > 0 ? state : "unknown"
}

function isDistributionTarget(value: string): value is DistributionTarget {
  return TARGETS.has(value as DistributionTarget)
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
