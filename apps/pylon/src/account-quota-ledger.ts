import { readFile, mkdir, unlink, writeFile } from "node:fs/promises"
import { join } from "node:path"

import type { BootstrapSummary } from "./bootstrap.js"

type QuotaSummary = Pick<BootstrapSummary, "paths">

export type QuotaRecord = {
  accountRefHash: string
  provider: string
  observedAt: string
  retryAtIso: string | null
  kind: QuotaBlockKind
  sourceDigestRef: string
  manualResetsRemaining: number | null
}

export type QuotaBlockKind = "cooldown" | "weekly_exhausted" | "unknown"

export type ManualQuotaResetRecord = {
  accountRefHash: string
  provider: string
  manualResetsRemaining: number
  updatedAt: string
  resetEvents: Array<{
    observedAt: string
    quotaDeleted: boolean
  }>
}

export const DEFAULT_MANUAL_QUOTA_RESETS = 3
const defaultQuotaCooldownMs = 3600_000
const safePathSegmentPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/

function quotaDirectory(summary: QuotaSummary) {
  return join(summary.paths.home, "account-quota")
}

function quotaRecordPath(summary: QuotaSummary, accountRefHash: string) {
  const safeRef = safePathSegmentPattern.test(accountRefHash)
    ? accountRefHash
    : encodeURIComponent(accountRefHash)
  return join(quotaDirectory(summary), `${safeRef}.json`)
}

function manualResetRecordPath(summary: QuotaSummary, accountRefHash: string) {
  const safeRef = safePathSegmentPattern.test(accountRefHash)
    ? accountRefHash
    : encodeURIComponent(accountRefHash)
  return join(quotaDirectory(summary), `${safeRef}.manual-reset.json`)
}

function publicProviderRef(provider: string) {
  const normalized = provider.trim().toLowerCase()
  if (normalized === "codex" || normalized.includes("codex")) return "codex"
  if (normalized === "claude_agent" || normalized.includes("claude")) return "claude_agent"
  return "unknown"
}

function quotaRecordFrom(value: unknown): QuotaRecord | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (typeof record.accountRefHash !== "string") return null
  if (typeof record.provider !== "string") return null
  if (typeof record.observedAt !== "string") return null
  if (record.retryAtIso !== null && typeof record.retryAtIso !== "string") return null
  if (typeof record.sourceDigestRef !== "string") return null
  const kind = quotaBlockKindFrom(record.kind)
  const manualResetsRemaining = record.manualResetsRemaining === undefined || record.manualResetsRemaining === null
    ? null
    : nonNegativeInteger(record.manualResetsRemaining)
  if (record.manualResetsRemaining !== undefined && record.manualResetsRemaining !== null && manualResetsRemaining === null) {
    return null
  }
  return {
    accountRefHash: record.accountRefHash,
    provider: record.provider,
    observedAt: record.observedAt,
    retryAtIso: record.retryAtIso,
    kind,
    sourceDigestRef: record.sourceDigestRef,
    manualResetsRemaining,
  }
}

function quotaBlockKindFrom(value: unknown): QuotaBlockKind {
  return value === "cooldown" || value === "weekly_exhausted" || value === "unknown"
    ? value
    : "unknown"
}

function nonNegativeInteger(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null
  return Math.max(0, Math.floor(value))
}

function manualResetRecordFrom(value: unknown): ManualQuotaResetRecord | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (typeof record.accountRefHash !== "string") return null
  if (typeof record.provider !== "string") return null
  if (typeof record.updatedAt !== "string") return null
  const manualResetsRemaining = nonNegativeInteger(record.manualResetsRemaining)
  if (manualResetsRemaining === null) return null
  const resetEvents = Array.isArray(record.resetEvents)
    ? record.resetEvents.flatMap((event): ManualQuotaResetRecord["resetEvents"] => {
        if (event === null || typeof event !== "object") return []
        const source = event as Record<string, unknown>
        if (typeof source.observedAt !== "string" || typeof source.quotaDeleted !== "boolean") return []
        return [{ observedAt: source.observedAt, quotaDeleted: source.quotaDeleted }]
      })
    : []
  return {
    accountRefHash: record.accountRefHash,
    provider: publicProviderRef(record.provider),
    manualResetsRemaining,
    updatedAt: record.updatedAt,
    resetEvents,
  }
}

export async function recordQuotaBlock(
  summary: QuotaSummary,
  input: {
    accountRefHash: string
    provider: string
    retryAtIso: string | null
    kind?: QuotaBlockKind
    sourceDigestRef: string
    manualResetsRemaining?: number | null
    now?: Date
  },
): Promise<void> {
  const resetRecord = input.manualResetsRemaining === undefined
    ? await loadManualQuotaResetRecord(summary, {
        accountRefHash: input.accountRefHash,
        provider: input.provider,
      })
    : null
  const explicitManualResetsRemaining = input.manualResetsRemaining === undefined || input.manualResetsRemaining === null
    ? null
    : nonNegativeInteger(input.manualResetsRemaining)
  const record: QuotaRecord = {
    accountRefHash: input.accountRefHash,
    provider: publicProviderRef(input.provider),
    observedAt: (input.now ?? new Date()).toISOString(),
    retryAtIso: input.retryAtIso,
    kind: input.kind ?? "unknown",
    sourceDigestRef: input.sourceDigestRef,
    manualResetsRemaining: explicitManualResetsRemaining ?? resetRecord?.manualResetsRemaining ?? null,
  }
  await mkdir(quotaDirectory(summary), { recursive: true })
  await writeFile(
    quotaRecordPath(summary, input.accountRefHash),
    `${JSON.stringify(record, null, 2)}\n`,
  )
}

export async function loadQuotaRecord(
  summary: QuotaSummary,
  accountRefHash: string,
): Promise<QuotaRecord | null> {
  try {
    const raw = await readFile(quotaRecordPath(summary, accountRefHash), "utf8")
    return quotaRecordFrom(JSON.parse(raw))
  } catch {
    return null
  }
}

export async function loadManualQuotaResetRecord(
  summary: QuotaSummary,
  input: { accountRefHash: string; provider: string; defaultManualResetsRemaining?: number | null },
): Promise<ManualQuotaResetRecord> {
  try {
    const raw = await readFile(manualResetRecordPath(summary, input.accountRefHash), "utf8")
    const parsed = manualResetRecordFrom(JSON.parse(raw))
    if (parsed) return parsed
  } catch {
    // Missing or malformed local manual-reset state starts from the default allowance.
  }
  const now = new Date(0).toISOString()
  const defaultRemaining =
    input.defaultManualResetsRemaining === undefined || input.defaultManualResetsRemaining === null
      ? DEFAULT_MANUAL_QUOTA_RESETS
      : nonNegativeInteger(input.defaultManualResetsRemaining) ?? DEFAULT_MANUAL_QUOTA_RESETS
  return {
    accountRefHash: input.accountRefHash,
    provider: publicProviderRef(input.provider),
    manualResetsRemaining: defaultRemaining,
    updatedAt: now,
    resetEvents: [],
  }
}

export async function consumeManualQuotaReset(
  summary: QuotaSummary,
  input: { accountRefHash: string; provider: string; defaultManualResetsRemaining?: number | null; now?: Date },
): Promise<ManualQuotaResetRecord> {
  const existing = await loadManualQuotaResetRecord(summary, input)
  if (existing.manualResetsRemaining <= 0) {
    throw new Error("no manual quota resets remaining for this account")
  }
  let quotaDeleted = false
  try {
    await unlink(quotaRecordPath(summary, input.accountRefHash))
    quotaDeleted = true
  } catch {
    quotaDeleted = false
  }
  const observedAt = (input.now ?? new Date()).toISOString()
  const updated: ManualQuotaResetRecord = {
    accountRefHash: input.accountRefHash,
    provider: publicProviderRef(input.provider),
    manualResetsRemaining: existing.manualResetsRemaining - 1,
    updatedAt: observedAt,
    resetEvents: [
      ...existing.resetEvents,
      {
        observedAt,
        quotaDeleted,
      },
    ].slice(-20),
  }
  await mkdir(quotaDirectory(summary), { recursive: true })
  await writeFile(
    manualResetRecordPath(summary, input.accountRefHash),
    `${JSON.stringify(updated, null, 2)}\n`,
  )
  return updated
}

export function isAccountAvailable(record: QuotaRecord | null, now: Date): boolean {
  if (record === null) return true
  if (record.retryAtIso !== null) return now.getTime() >= Date.parse(record.retryAtIso)

  // Default unknown retry cooldown: 3600_000 ms. A future weeklyResetHintIso hook can
  // document known periodic provider limits without changing account-routing policy here.
  return now.getTime() - Date.parse(record.observedAt) >= defaultQuotaCooldownMs
}
