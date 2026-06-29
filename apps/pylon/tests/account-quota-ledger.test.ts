import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  isAccountAvailable,
  loadQuotaRecord,
  recordQuotaBlock,
} from "../src/account-quota-ledger"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-quota-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("account quota ledger", () => {
  test("records account quota blocks without persisting local account material", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const now = new Date("2026-06-13T12:00:00.000Z")
      const retryAt = new Date(now.getTime() + 3600_000)
      const accountRefHash = "account.pylon.codex.0123456789abcdef01234567"
      const rawProviderSentence = `codex quota exhausted for account at ${home}`

      await recordQuotaBlock(summary, {
        accountRefHash,
        provider: rawProviderSentence,
        retryAtIso: retryAt.toISOString(),
        sourceDigestRef: "digest.pylon.quota.source.test",
        now,
      })

      const record = await loadQuotaRecord(summary, accountRefHash)
      expect(isAccountAvailable(record, now)).toBe(false)
      expect(isAccountAvailable(record, new Date(retryAt.getTime()))).toBe(true)
      expect(isAccountAvailable(await loadQuotaRecord(summary, "missing-account"), now)).toBe(true)

      const persisted = await readFile(
        join(summary.paths.home, "account-quota", `${accountRefHash}.json`),
        "utf8",
      )
      expect(persisted).not.toContain(home)
      expect(persisted).not.toContain(rawProviderSentence)
      expect(JSON.parse(persisted)).toEqual({
        accountRefHash,
        provider: "codex",
        observedAt: now.toISOString(),
        retryAtIso: retryAt.toISOString(),
        kind: "unknown",
        sourceDigestRef: "digest.pylon.quota.source.test",
        manualResetsRemaining: 3,
      })
    })
  })
})
