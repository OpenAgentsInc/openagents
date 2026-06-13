import { readFile, mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"

import type { BootstrapSummary } from "./bootstrap"

export type QuotaRecord = {
  accountRefHash: string
  provider: string
  observedAt: string
  retryAtIso: string | null
  sourceDigestRef: string
}

const defaultQuotaCooldownMs = 3600_000
const safePathSegmentPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/

function quotaDirectory(summary: BootstrapSummary) {
  return join(summary.paths.home, "account-quota")
}

function quotaRecordPath(summary: BootstrapSummary, accountRefHash: string) {
  const safeRef = safePathSegmentPattern.test(accountRefHash)
    ? accountRefHash
    : encodeURIComponent(accountRefHash)
  return join(quotaDirectory(summary), `${safeRef}.json`)
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
  return {
    accountRefHash: record.accountRefHash,
    provider: record.provider,
    observedAt: record.observedAt,
    retryAtIso: record.retryAtIso,
    sourceDigestRef: record.sourceDigestRef,
  }
}

export async function recordQuotaBlock(
  summary: BootstrapSummary,
  input: {
    accountRefHash: string
    provider: string
    retryAtIso: string | null
    sourceDigestRef: string
    now?: Date
  },
): Promise<void> {
  const record: QuotaRecord = {
    accountRefHash: input.accountRefHash,
    provider: publicProviderRef(input.provider),
    observedAt: (input.now ?? new Date()).toISOString(),
    retryAtIso: input.retryAtIso,
    sourceDigestRef: input.sourceDigestRef,
  }
  await mkdir(quotaDirectory(summary), { recursive: true })
  await writeFile(
    quotaRecordPath(summary, input.accountRefHash),
    `${JSON.stringify(record, null, 2)}\n`,
  )
}

export async function loadQuotaRecord(
  summary: BootstrapSummary,
  accountRefHash: string,
): Promise<QuotaRecord | null> {
  try {
    const raw = await readFile(quotaRecordPath(summary, accountRefHash), "utf8")
    return quotaRecordFrom(JSON.parse(raw))
  } catch {
    return null
  }
}

export function isAccountAvailable(record: QuotaRecord | null, now: Date): boolean {
  if (record === null) return true
  if (record.retryAtIso !== null) return now.getTime() >= Date.parse(record.retryAtIso)

  // Default unknown retry cooldown: 3600_000 ms. A future weeklyResetHintIso hook can
  // document known periodic provider limits without changing account-routing policy here.
  return now.getTime() - Date.parse(record.observedAt) >= defaultQuotaCooldownMs
}
