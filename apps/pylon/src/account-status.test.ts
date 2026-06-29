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
import { recordCodexAccountHealthFailure } from "./codex-account-health-ledger.js"
import { classifyCodexAccountFailure } from "./codex-account-health.js"

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
        manualResetsRemaining: 3,
      })
      expect(status.accounts[0]?.quotaPolicy).toMatchObject({
        state: "limited_unknown",
        limitScope: "unknown",
        resetAtIso: "2026-06-28T22:00:00.000Z",
        shouldWaitForReset: true,
        operatorRecovery: {
          required: false,
          manualResetAvailable: false,
          status: "not_applicable",
        },
      })
      expect(status.accounts[0]?.capacity.hourly?.remainingPercent).toBe(75)
      expect(status.accounts[0]?.capacity.weekly?.remainingPercent).toBe(50)
      expect(status.accounts[0]?.usage.totalTokens).toBe(140)
      expect(status.accounts[0]?.manualReset.manualResetsRemaining).toBe(3)
    })
  })

  test("treats Codex 5-hour exhaustion with weekly headroom as wait-only cooldown", async () => {
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
        retryAtIso: "2026-06-29T02:00:00.000Z",
        sourceDigestRef: "digest.pylon.account_quota.short_window",
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
              usedPercent: 100,
              remainingPercent: 0,
              windowMinutes: 5 * 60,
              resetsAt: 1_782_698_400,
              label: "5h",
            },
            secondary: {
              usedPercent: 64,
              remainingPercent: 36,
              windowMinutes: 7 * 24 * 60,
              resetsAt: 1_783_286_400,
              label: "weekly",
            },
            credits: null,
            planType: "pro",
            rateLimitReachedType: "primary",
          },
        ],
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

      expect(status.accounts[0]?.quotaPolicy).toMatchObject({
        state: "short_window_cooldown",
        limitScope: "short_window",
        resetAtIso: "2026-06-29T02:00:00.000Z",
        shouldWaitForReset: true,
        operatorRecovery: {
          required: false,
          manualResetAvailable: false,
          status: "not_applicable",
        },
      })
      expect(status.accounts[0]?.quotaPolicy.exhaustedWindow?.label).toBe("5h")
      expect(status.accounts[0]?.quotaPolicy.weeklyWindow?.remainingPercent).toBe(36)

      const resetAttempt = await collectPylonAccountsStatus(
        summary,
        parsePylonAccountsStatusArgs(["--account", "codex", "--reset", "--json"]),
        {
          env: { PYLON_HOME: home, CODEX_HOME: accountHome, PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings") },
          now: new Date("2026-06-28T21:46:00.000Z"),
        },
      )

      expect(resetAttempt.accounts[0]?.manualReset.performed).toBe(false)
      expect(resetAttempt.accounts[0]?.manualReset.blockerRefs).toContain(
        "blocker.pylon.accounts_status.manual_reset_not_applicable",
      )
      expect(await loadQuotaRecord(summary, accountRefHash)).not.toBeNull()
    })
  })

  test("exposes Codex weekly exhaustion as operator recovery without faking provider reset", async () => {
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
        retryAtIso: "2026-07-05T00:00:00.000Z",
        sourceDigestRef: "digest.pylon.account_quota.weekly",
        manualResetsRemaining: 2,
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
              usedPercent: 41,
              remainingPercent: 59,
              windowMinutes: 5 * 60,
              resetsAt: 1_782_698_400,
              label: "5h",
            },
            secondary: {
              usedPercent: 100,
              remainingPercent: 0,
              windowMinutes: 7 * 24 * 60,
              resetsAt: 1_783_209_600,
              label: "weekly",
            },
            credits: null,
            planType: "pro",
            rateLimitReachedType: "secondary",
          },
        ],
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

      expect(status.accounts[0]?.quotaPolicy).toMatchObject({
        state: "weekly_exhausted",
        limitScope: "weekly",
        resetAtIso: "2026-07-05T00:00:00.000Z",
        shouldWaitForReset: false,
        operatorRecovery: {
          required: true,
          manualResetAvailable: false,
          manualResetsRemaining: 3,
          command: "pylon accounts usage --account codex --refresh --json",
          status: "provider_reset_required",
        },
      })

      const reset = await collectPylonAccountsStatus(
        summary,
        parsePylonAccountsStatusArgs(["--account", "codex", "--reset", "--json"]),
        {
          env: { PYLON_HOME: home, CODEX_HOME: accountHome, PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings") },
          now: new Date("2026-06-28T21:46:00.000Z"),
        },
      )

      expect(reset.accounts[0]?.manualReset.performed).toBe(false)
      expect(reset.accounts[0]?.manualReset.blockerRefs).toContain(
        "blocker.pylon.accounts_status.manual_reset_not_applicable",
      )
      expect(reset.accounts[0]?.quota.state).toBe("limited")
      expect(reset.accounts[0]?.quotaPolicy.state).toBe("weekly_exhausted")
      expect(reset.accounts[0]?.quotaPolicy.operatorRecovery).toMatchObject({
        required: true,
        manualResetAvailable: false,
        manualResetsRemaining: 3,
        command: "pylon accounts usage --account codex --refresh --json",
        status: "provider_reset_required",
      })
      expect(await loadQuotaRecord(summary, accountRefHash)).not.toBeNull()
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

  test("accounts status manual reset clears an unknown local quota block", async () => {
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
        sourceDigestRef: "digest.pylon.account_quota.unknown",
        now: new Date("2026-06-28T21:30:00.000Z"),
      })

      const reset = await collectPylonAccountsStatus(
        summary,
        parsePylonAccountsStatusArgs(["--account", "codex", "--reset", "--json"]),
        {
          env: { PYLON_HOME: home, CODEX_HOME: accountHome, PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings") },
          now: new Date("2026-06-28T21:35:00.000Z"),
        },
      )

      expect(reset.accounts[0]?.manualReset.performed).toBe(true)
      expect(reset.accounts[0]?.manualReset.blockerRefs).toEqual([])
      expect(reset.accounts[0]?.quota.state).toBe("available")
      expect(reset.accounts[0]?.quotaPolicy.state).toBe("available")
      expect(await loadQuotaRecord(summary, accountRefHash)).toBeNull()
    })
  })

  test("projects specific Codex account auth health reasons", async () => {
    await withTempHome(async ({ home, summary }) => {
      const accountHome = join(home, "accounts", "codex", "codex-revoked")
      await mkdir(accountHome, { recursive: true })
      await writeFile(join(accountHome, "auth.json"), "{}")
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [
            { provider: "codex", ref: "codex-revoked", home: accountHome },
          ],
        },
      }))
      const accountRefHash = hashPylonAccountRef("codex", "codex-revoked")
      await recordCodexAccountHealthFailure(summary, {
        accountRefHash,
        failure: classifyCodexAccountFailure("refresh token was revoked"),
        now: new Date("2026-06-28T21:30:00.000Z"),
      })

      const status = await collectPylonAccountsStatus(
        summary,
        parsePylonAccountsStatusArgs(["--account", "codex-revoked", "--json"]),
        {
          env: { PYLON_HOME: home, CODEX_HOME: accountHome, PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings") },
          now: new Date("2026-06-28T21:45:00.000Z"),
        },
      )

      expect(status.accounts[0]?.readiness.state).toBe("credentials_revoked")
      expect(status.accounts[0]?.readiness.blockerRefs).toContain(
        "blocker.pylon.codex_account.credentials_revoked_needs_owner_reauth",
      )
    })
  })
})
