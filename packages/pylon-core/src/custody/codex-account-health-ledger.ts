import { mkdir, readFile, unlink, writeFile } from "node:fs/promises"
import { join } from "node:path"

import type { BootstrapSummary } from "../shared/bootstrap.js"
import {
  type PylonCodexAccountFailure,
  type PylonCodexAccountHealthReason,
} from "./codex-account-health.js"

type Summary = Pick<BootstrapSummary, "paths">

export type PylonCodexAccountHealthRecord = {
  schema: "openagents.pylon.codex_account_health.v0.1"
  accountRefHash: string
  observedAt: string
  reason: PylonCodexAccountHealthReason
  sourceDigestRef: string
  publicMessage: string
}

const safePathSegmentPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/

function healthDirectory(summary: Summary) {
  return join(summary.paths.home, "codex-account-health")
}

function healthRecordPath(summary: Summary, accountRefHash: string) {
  const safeRef = safePathSegmentPattern.test(accountRefHash)
    ? accountRefHash
    : encodeURIComponent(accountRefHash)
  return join(healthDirectory(summary), `${safeRef}.json`)
}

function healthRecordFrom(value: unknown): PylonCodexAccountHealthRecord | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (record.schema !== "openagents.pylon.codex_account_health.v0.1") return null
  if (typeof record.accountRefHash !== "string") return null
  if (typeof record.observedAt !== "string") return null
  if (
    record.reason !== "credentials_revoked" &&
    record.reason !== "usage_limited" &&
    record.reason !== "rate_limited" &&
    record.reason !== "network" &&
    record.reason !== "timeout" &&
    record.reason !== "other"
  ) return null
  if (typeof record.sourceDigestRef !== "string") return null
  if (typeof record.publicMessage !== "string") return null
  return record as PylonCodexAccountHealthRecord
}

export async function recordCodexAccountHealthFailure(
  summary: Summary,
  input: {
    accountRefHash: string
    failure: PylonCodexAccountFailure
    now?: Date
  },
): Promise<void> {
  const record: PylonCodexAccountHealthRecord = {
    schema: "openagents.pylon.codex_account_health.v0.1",
    accountRefHash: input.accountRefHash,
    observedAt: (input.now ?? new Date()).toISOString(),
    reason: input.failure.reason,
    sourceDigestRef: input.failure.sourceDigestRef,
    publicMessage: input.failure.publicMessage,
  }
  await mkdir(healthDirectory(summary), { recursive: true })
  await writeFile(healthRecordPath(summary, input.accountRefHash), `${JSON.stringify(record, null, 2)}\n`)
}

export async function loadCodexAccountHealthRecord(
  summary: Summary,
  accountRefHash: string,
): Promise<PylonCodexAccountHealthRecord | null> {
  try {
    return healthRecordFrom(JSON.parse(await readFile(healthRecordPath(summary, accountRefHash), "utf8")))
  } catch {
    return null
  }
}

export async function clearCodexAccountHealthFailure(
  summary: Summary,
  accountRefHash: string,
): Promise<void> {
  try {
    await unlink(healthRecordPath(summary, accountRefHash))
  } catch {
    // Missing health records are already clear.
  }
}

export function codexAccountHealthBlocksReadiness(
  record: PylonCodexAccountHealthRecord | null,
): record is PylonCodexAccountHealthRecord & { reason: "credentials_revoked" | "usage_limited" | "rate_limited" } {
  return (
    record?.reason === "credentials_revoked" ||
    record?.reason === "usage_limited" ||
    record?.reason === "rate_limited"
  )
}
