import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { hashPylonAccountRef } from "../src/account-registry"
import { consumeManualQuotaReset, recordQuotaBlock } from "../src/account-quota-ledger"
import { collectPylonOperatorAccountStatus } from "../src/account-status"
import { recordPylonAccountUsageObservation } from "../src/account-usage"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { assertPublicProjectionSafe } from "../src/state"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-status-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("pylon operator account status", () => {
  test("aggregates registered account quota, capacity, manual resets, and usage", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const codexHome = join(home, "codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(
        summary.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                {
                  ref: "codex-a",
                  provider: "codex",
                  home: codexHome,
                  hourlyCap: 1_000,
                  weeklyCap: 10_000,
                  manualResetsRemaining: 2,
                  marginalCostClass: "subscription",
                },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )

      const accountRefHash = hashPylonAccountRef("codex", "codex-a")
      await recordQuotaBlock(summary, {
        accountRefHash,
        provider: "codex",
        retryAtIso: "2026-06-28T02:00:00.000Z",
        sourceDigestRef: "digest.pylon.account_quota.test",
        now: new Date("2026-06-28T01:00:00.000Z"),
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        account: {
          provider: "codex",
          selector: "registry_ref",
          accountRef: "codex-a",
          accountRefHash,
          home: codexHome,
        },
        providerSnapshots: [
          {
            provider: "codex",
            limitId: "codex",
            limitName: null,
            primary: {
              usedPercent: 25,
              remainingPercent: 75,
              windowMinutes: 60,
              resetsAt: null,
              label: "usage",
            },
            secondary: {
              usedPercent: 40,
              remainingPercent: 60,
              windowMinutes: 10_080,
              resetsAt: null,
              label: "weekly",
            },
            credits: null,
            planType: null,
            rateLimitReachedType: null,
          },
        ],
        observedAt: new Date("2026-06-28T01:05:00.000Z"),
      })

      const projection = await collectPylonOperatorAccountStatus(summary, {
        now: new Date("2026-06-28T01:10:00.000Z"),
      })

      expect(projection.accounts).toEqual([
        {
          accountRefHash,
          provider: "codex",
          isRateLimited: true,
          quotaState: "limited",
          cooldownExpiresAt: "2026-06-28T02:00:00.000Z",
          hourlyCap: 1_000,
          hourlyUsage: 250,
          weeklyCap: 10_000,
          weeklyUsage: 4_000,
          manualResetsRemaining: 2,
          resetAllowed: false,
          marginalCostClass: "subscription",
        },
      ])
      expect(JSON.stringify(projection)).not.toContain(codexHome)
      assertPublicProjectionSafe(projection)
    })
  })

  test("defaults marginalCostClass to not_measured when the registry entry omits it (MH-8, #8587)", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const codexHome = join(home, "codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          dev: {
            accounts: [{ ref: "codex-a", provider: "codex", home: codexHome }],
          },
        }),
      )

      const projection = await collectPylonOperatorAccountStatus(summary, {
        now: new Date("2026-06-28T01:10:00.000Z"),
      })
      expect(projection.accounts[0]?.marginalCostClass).toBe("not_measured")
      assertPublicProjectionSafe(projection)
    })
  })

  test("rejects an unknown marginalCostClass value and falls back to not_measured, never invents free", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const codexHome = join(home, "codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          dev: {
            accounts: [{
              ref: "codex-a",
              provider: "codex",
              home: codexHome,
              marginalCostClass: "totally_free_trust_me",
            }],
          },
        }),
      )

      const projection = await collectPylonOperatorAccountStatus(summary, {
        now: new Date("2026-06-28T01:10:00.000Z"),
      })
      expect(projection.accounts[0]?.marginalCostClass).toBe("not_measured")
    })
  })

  test("uses rolling local-session token deltas when provider windows are missing", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const codexHome = join(home, "codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(
        summary.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                {
                  ref: "codex-a",
                  provider: "codex",
                  home: codexHome,
                  hourlyCap: 1_000,
                  weeklyCap: 10_000,
                },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )

      const accountRefHash = hashPylonAccountRef("codex", "codex-a")
      const account = {
        provider: "codex" as const,
        selector: "registry_ref" as const,
        accountRef: "codex-a",
        accountRefHash,
        home: codexHome,
      }
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        account,
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.status",
          inputTokens: 0,
          outputTokens: 100,
          totalTokens: 100,
        },
        observedAt: new Date("2026-06-28T10:30:00.000Z"),
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        account,
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.status",
          inputTokens: 0,
          outputTokens: 160,
          totalTokens: 160,
        },
        observedAt: new Date("2026-06-28T11:05:00.000Z"),
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        account,
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.status",
          inputTokens: 0,
          outputTokens: 210,
          totalTokens: 210,
        },
        observedAt: new Date("2026-06-28T11:45:00.000Z"),
      })

      const projection = await collectPylonOperatorAccountStatus(summary, {
        now: new Date("2026-06-28T12:00:00.000Z"),
      })

      expect(projection.accounts[0]?.hourlyUsage).toBe(110)
      expect(projection.accounts[0]?.weeklyUsage).toBe(null)
      assertPublicProjectionSafe(projection)
    })
  })

  test("reports consumed manual resets from the quota ledger", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const codexHome = join(home, "codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(
        summary.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                {
                  ref: "codex-a",
                  provider: "codex",
                  home: codexHome,
                  manualResetsRemaining: 2,
                },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )

      const accountRefHash = hashPylonAccountRef("codex", "codex-a")
      await recordQuotaBlock(summary, {
        accountRefHash,
        provider: "codex",
        retryAtIso: "2026-06-28T02:00:00.000Z",
        sourceDigestRef: "digest.pylon.account_quota.test",
        manualResetsRemaining: 2,
        now: new Date("2026-06-28T01:00:00.000Z"),
      })
      await consumeManualQuotaReset(summary, {
        accountRefHash,
        provider: "codex",
        defaultManualResetsRemaining: 2,
        now: new Date("2026-06-28T01:10:00.000Z"),
      })

      const projection = await collectPylonOperatorAccountStatus(summary, {
        now: new Date("2026-06-28T01:15:00.000Z"),
      })

      expect(projection.accounts[0]?.manualResetsRemaining).toBe(1)
      expect(projection.accounts[0]?.isRateLimited).toBe(false)
      assertPublicProjectionSafe(projection)
    })
  })
})
