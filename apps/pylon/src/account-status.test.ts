import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { classifyQuotaSignal } from "./account-quota.js"
import {
  consumeManualQuotaReset,
  loadQuotaRecord,
  recordQuotaBlock,
} from "./account-quota-ledger.js"
import {
  collectPylonAccountsStatus,
  parsePylonAccountsStatusArgs,
  recordPylonAccountUsageObservation,
} from "./account-usage.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import { hashPylonAccountRef } from "./account-registry.js"

async function withTempHome<T>(fn: (input: { home: string; summary: ReturnType<typeof createBootstrapSummary> }) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-status-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    return await fn({ home, summary })
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("#6637 account status observability", () => {
  test("parses a concrete provider retry time from quota output", () => {
    const signal = classifyQuotaSignal(
      "Hit your usage limit. Please try again at 2026-06-28T21:41:00Z.",
      "codex",
    )

    expect(signal.exhausted).toBe(true)
    expect(signal.retryAtIso).toBe("2026-06-28T21:41:00.000Z")
  })

  test("projects quota, provider windows, usage, and manual reset state per account", async () => {
    await withTempHome(async ({ home, summary }) => {
      const accountHome = join(home, "accounts", "codex", "codex")
      await mkdir(accountHome, { recursive: true })
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [
            {
              provider: "codex",
              ref: "codex",
              home: accountHome,
            },
          ],
        },
      }))
      const accountRefHash = hashPylonAccountRef("codex", "codex")
      await recordQuotaBlock(summary, {
        accountRefHash,
        provider: "codex",
        retryAtIso: "2026-06-28T22:00:00.000Z",
        sourceDigestRef: "digest.pylon.account_quota.test",
        now: new Date("2026-06-28T21:30:00.000Z"),
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        account: {
          provider: "codex",
          selector: "registry_ref",
          accountRef: "codex",
          accountRefHash,
          home: accountHome,
        },
        providerSnapshots: [
          {
            provider: "codex",
            limitId: "codex",
            limitName: "Codex",
            primary: {
              usedPercent: 25,
              remainingPercent: 75,
              windowMinutes: 60,
              resetsAt: 1_782_682_200,
              label: "hourly",
            },
            secondary: {
              usedPercent: 50,
              remainingPercent: 50,
              windowMinutes: 7 * 24 * 60,
              resetsAt: 1_783_286_400_000,
              label: "weekly",
            },
            credits: null,
            planType: "pro",
            rateLimitReachedType: null,
          },
        ],
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.test",
          inputTokens: 100,
          outputTokens: 40,
          totalTokens: 140,
        },
        observedAt: new Date("2026-06-28T21:31:00.000Z"),
      })

      const status = await collectPylonAccountsStatus(
        summary,
        parsePylonAccountsStatusArgs(["--account", "codex", "--json"]),
        {
          env: { PYLON_HOME: home, CODEX_HOME: accountHome, PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings") },
          now: new Date("2026-06-28T21:45:00.000Z"),
        },
      )

      expect(status.schema).toBe("openagents.pylon.accounts_status.v0.1")
      expect(status.accounts).toHaveLength(1)
      expect(status.accounts[0]?.quota).toMatchObject({
        state: "limited",
        cooldownExpiresAt: "2026-06-28T22:00:00.000Z",
        cooldownSecondsRemaining: 900,
      })
      expect(status.accounts[0]?.capacity.hourly?.remainingPercent).toBe(75)
      expect(status.accounts[0]?.capacity.weekly?.remainingPercent).toBe(50)
      expect(status.accounts[0]?.usage.totalTokens).toBe(140)
      expect(status.accounts[0]?.manualReset.manualResetsRemaining).toBe(3)
    })
  })

  test("manual reset deletes the quota block and decrements the account allowance", async () => {
    await withTempHome(async ({ summary }) => {
      const accountRefHash = hashPylonAccountRef("codex", "codex")
      await recordQuotaBlock(summary, {
        accountRefHash,
        provider: "codex",
        retryAtIso: "2026-06-28T22:00:00.000Z",
        sourceDigestRef: "digest.pylon.account_quota.test",
        now: new Date("2026-06-28T21:30:00.000Z"),
      })

      const reset = await consumeManualQuotaReset(summary, {
        accountRefHash,
        provider: "codex",
        now: new Date("2026-06-28T21:35:00.000Z"),
      })

      expect(reset.manualResetsRemaining).toBe(2)
      expect(reset.resetEvents.at(-1)).toEqual({
        observedAt: "2026-06-28T21:35:00.000Z",
        quotaDeleted: true,
      })
      expect(await loadQuotaRecord(summary, accountRefHash)).toBeNull()
    })
  })
})
