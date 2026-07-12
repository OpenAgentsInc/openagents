import { mkdir, readFile, unlink, writeFile } from "node:fs/promises"
import { join } from "node:path"

import type { BootstrapSummary } from "../shared/bootstrap.js"

type Summary = Pick<BootstrapSummary, "paths">

export type PylonClaudeAccountHealthRecord = Readonly<{
  schema: "openagents.pylon.claude_account_health.v0.1"
  accountRefHash: string
  observedAt: string
  reason: "provider_disabled"
  sourceDigestRef: "digest.pylon.claude_account.provider_disabled"
}>

const safePathSegmentPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/

const recordPath = (summary: Summary, accountRefHash: string): string => {
  const safeRef = safePathSegmentPattern.test(accountRefHash)
    ? accountRefHash
    : encodeURIComponent(accountRefHash)
  return join(summary.paths.home, "claude-account-health", `${safeRef}.json`)
}

const decode = (value: unknown): PylonClaudeAccountHealthRecord | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  return record.schema === "openagents.pylon.claude_account_health.v0.1" &&
    typeof record.accountRefHash === "string" &&
    typeof record.observedAt === "string" &&
    record.reason === "provider_disabled" &&
    record.sourceDigestRef === "digest.pylon.claude_account.provider_disabled"
    ? record as PylonClaudeAccountHealthRecord
    : null
}

export const claudeProviderDisabledFailure = (error: unknown): boolean => {
  const message = error instanceof Error ? error.message : String(error)
  return message.toLowerCase().includes("organization has disabled claude subscription access")
}

export async function recordClaudeProviderDisabled(
  summary: Summary,
  accountRefHash: string,
  now = new Date(),
): Promise<void> {
  const record: PylonClaudeAccountHealthRecord = {
    schema: "openagents.pylon.claude_account_health.v0.1",
    accountRefHash,
    observedAt: now.toISOString(),
    reason: "provider_disabled",
    sourceDigestRef: "digest.pylon.claude_account.provider_disabled",
  }
  const directory = join(summary.paths.home, "claude-account-health")
  await mkdir(directory, { recursive: true })
  await writeFile(recordPath(summary, accountRefHash), `${JSON.stringify(record, null, 2)}\n`)
}

export async function loadClaudeAccountHealthRecord(
  summary: Summary,
  accountRefHash: string,
): Promise<PylonClaudeAccountHealthRecord | null> {
  try {
    return decode(JSON.parse(await readFile(recordPath(summary, accountRefHash), "utf8")))
  } catch {
    return null
  }
}

export async function clearClaudeAccountHealth(
  summary: Summary,
  accountRefHash: string,
): Promise<void> {
  try {
    await unlink(recordPath(summary, accountRefHash))
  } catch {
    // Missing health is already clear.
  }
}
