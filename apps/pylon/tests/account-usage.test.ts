import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  collectPylonAccountUsageSummary,
  collectPylonAccountsList,
  collectPylonAccountsUsage,
  parseCodexRateLimitHeaders,
  parsePylonAccountsUsageArgs,
  providerRateLimitSnapshotsFromEvent,
  recordPylonAccountUsageObservation,
} from "../src/account-usage"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { hashPylonAccountRef } from "../src/account-registry"
import { assertPublicProjectionSafe } from "../src/state"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-usage-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("pylon account usage", () => {
  test("parses Codex rate-limit header families into labeled snapshots", () => {
    const snapshots = parseCodexRateLimitHeaders({
      "x-codex-primary-used-percent": "35",
      "x-codex-primary-window-minutes": "43200",
      "x-codex-secondary-used-percent": "50",
      "x-codex-secondary-window-minutes": "10080",
      "x-codex-credits-has-credits": "true",
      "x-codex-credits-unlimited": "false",
      "x-codex-credits-balance": "12.4",
      "x-codex-other-primary-used-percent": "20",
      "x-codex-other-primary-window-minutes": "300",
      "x-codex-other-limit-name": "codex other",
    })

    expect(snapshots).toHaveLength(2)
    expect(snapshots[0]).toMatchObject({
      limitId: "codex",
      primary: { label: "monthly", remainingPercent: 65 },
      secondary: { label: "weekly", remainingPercent: 50 },
      credits: { hasCredits: true, unlimited: false, balance: "12.4" },
    })
    expect(snapshots[1]).toMatchObject({
      limitId: "codex_other",
      limitName: "codex other",
      primary: { label: "5h", remainingPercent: 80 },
    })
  })

  test("extracts Codex websocket/app-server rate-limit event shapes", () => {
    const snapshots = providerRateLimitSnapshotsFromEvent("codex", {
      type: "codex.rate_limits",
      metered_limit_name: "codex",
      rate_limits: {
        primary: { used_percent: 9, window_minutes: 10080, reset_at: 1_780_000_000 },
      },
      credits: { has_credits: true, unlimited: false, balance: "5" },
      plan_type: "pro",
    })

    expect(snapshots).toEqual([
      expect.objectContaining({
        provider: "codex",
        limitId: "codex",
        planType: "pro",
        primary: expect.objectContaining({
          label: "weekly",
          resetsAt: 1_780_000_000,
          remainingPercent: 91,
        }),
      }),
    ])
  })

  test("stores provider and local session truth with stale labeling", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const snapshots = parseCodexRateLimitHeaders({
        "x-codex-primary-used-percent": "35",
        "x-codex-primary-window-minutes": "43200",
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        providerSnapshots: snapshots,
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex_composer.test",
          inputTokens: 10,
          outputTokens: 5,
          totalTokens: 15,
        },
        observedAt: new Date("2026-06-12T12:00:00.000Z"),
      })

      const projection = await collectPylonAccountsUsage(
        summary,
        parsePylonAccountsUsageArgs(["--json"]),
        {
          env: {
            CODEX_HOME: join(home, "codex-default"),
            CLAUDE_CONFIG_DIR: join(home, "claude-default"),
            PYLON_HOME: home,
          },
          now: new Date("2026-06-12T12:20:00.000Z"),
        },
      )
      const codex = projection.accounts.find((account) => account.provider === "codex")
      expect(codex?.truth.provider.state).toBe("stale")
      expect(codex?.truth.localSession.state).toBe("available")
      expect(codex?.truth.localSession.usage?.totalTokens).toBe(15)
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)

      const summaryProjection = await collectPylonAccountUsageSummary(summary, {
        now: new Date("2026-06-12T12:20:00.000Z"),
      })
      expect(summaryProjection?.staleProviderTruthCount).toBe(1)
    })
  })

  test("lists registered and default accounts without raw credential homes", async () => {
    await withHome(async (home) => {
      const codexHome = join(home, "codex-a")
      const claudeHome = join(home, "claude-a")
      await mkdir(codexHome, { recursive: true })
      await mkdir(claudeHome, { recursive: true })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await writeFile(
        summary.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                { ref: "codex-a", provider: "codex", home: codexHome },
                { ref: "claude-a", provider: "claude_agent", home: claudeHome },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )

      const projection = await collectPylonAccountsList(summary, {
        env: {
          CODEX_HOME: join(home, "codex-default"),
          CLAUDE_CONFIG_DIR: join(home, "claude-default"),
          PYLON_HOME: home,
        },
        now: new Date("2026-06-12T12:00:00.000Z"),
      })
      expect(projection.accounts.map((account) => `${account.provider}:${account.accountRef ?? "default"}`)).toContain(
        "codex:codex-a",
      )
      expect(projection.accounts.map((account) => account.accountRefHash)).toContain(
        hashPylonAccountRef("codex", "codex-a"),
      )
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)
    })
  })

  test("parses refresh gates explicitly", () => {
    expect(parsePylonAccountsUsageArgs(["--json"])).toMatchObject({
      all: false,
      refresh: false,
      json: true,
    })
    expect(parsePylonAccountsUsageArgs(["--all", "--refresh", "--json"])).toMatchObject({
      all: true,
      refresh: true,
      json: true,
    })
    expect(() => parsePylonAccountsUsageArgs(["--all", "--account", "codex-a", "--json"])).toThrow(
      /either --account or --all/,
    )
  })
})
