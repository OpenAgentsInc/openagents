import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  countRollingWindowLocalSessionTokens,
  collectPylonAccountUsageSummary,
  collectPylonAccountsList,
  collectPylonAccountsUsage,
  loadAccountUsageStore,
  parseCodexRateLimitHeaders,
  parsePylonAccountsUsageArgs,
  providerRateLimitSnapshotsFromEvent,
  recordPylonAccountUsageObservation,
} from "../src/account-usage"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { hashPylonAccountRef } from "../src/account-registry"
import { assertPublicProjectionSafe } from "../src/state"

const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-usage-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function runPylonCli(args: string[], env: Record<string, string | undefined>) {
  const proc = Bun.spawn(["bun", INDEX, ...args], {
    cwd: CWD,
    env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const timeout = setTimeout(() => proc.kill(), 5_000)
  try {
    const [exitCode, stdout, stderr] = await Promise.all([
      proc.exited,
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ])
    return { exitCode, stdout, stderr }
  } finally {
    clearTimeout(timeout)
  }
}

describe("pylon account usage", () => {
  test("counts cumulative local-session token deltas inside a rolling window", () => {
    const now = new Date("2026-06-28T12:00:00.000Z")
    const usage = (observedAt: string, totalTokens: number, sessionRef = "session.pylon.codex.fixture") => ({
      observedAt,
      usage: {
        provider: "codex" as const,
        sessionRef,
        inputTokens: 0,
        outputTokens: totalTokens,
        totalTokens,
      },
    })

    expect(countRollingWindowLocalSessionTokens([
      usage("2026-06-28T10:30:00.000Z", 100),
      usage("2026-06-28T11:05:00.000Z", 160),
      usage("2026-06-28T11:45:00.000Z", 210),
      usage("2026-06-28T12:05:00.000Z", 300),
    ], { now, windowMinutes: 60 })).toBe(110)
  })

  test("counts first in-window totals, reset totals, and null-session observations safely", () => {
    const now = new Date("2026-06-28T12:00:00.000Z")
    expect(countRollingWindowLocalSessionTokens([
      {
        observedAt: "2026-06-28T11:15:00.000Z",
        usage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.first_seen",
          inputTokens: 20,
          outputTokens: 80,
          totalTokens: 100,
        },
      },
      {
        observedAt: "2026-06-28T11:30:00.000Z",
        usage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.first_seen",
          inputTokens: 5,
          outputTokens: 35,
          totalTokens: 40,
        },
      },
      {
        observedAt: "2026-06-28T11:45:00.000Z",
        usage: {
          provider: "codex",
          sessionRef: null,
          inputTokens: 3,
          outputTokens: 7,
          totalTokens: 10,
        },
      },
    ], { now, windowMinutes: 60 })).toBe(150)
  })

  test("codex accounts list aliases the local account inventory command", async () => {
    await withHome(async (home) => {
      const proc = await runPylonCli(["codex", "accounts", "list", "--json"], {
        ...Bun.env,
        CODEX_HOME: join(home, "codex-default"),
        CLAUDE_CONFIG_DIR: join(home, "claude-default"),
        PYLON_ACCOUNT_HOME_ROOT: join(home, "no-sibling-scan"),
        PYLON_HOME: home,
      })

      expect(proc.exitCode).toBe(0)
      expect(proc.stderr).toBe("")
      const projection = JSON.parse(proc.stdout)
      expect(projection.schema).toBe("openagents.pylon.accounts_list.v0.3")
      expect(projection.accounts.some((account: { provider: string }) => account.provider === "codex")).toBe(true)
      assertPublicProjectionSafe(projection)
    })
  })

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

  test("persists local-session usage history for rolling-window counters", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.history",
          inputTokens: 0,
          outputTokens: 100,
          totalTokens: 100,
        },
        observedAt: new Date("2026-06-28T11:00:00.000Z"),
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.history",
          inputTokens: 0,
          outputTokens: 175,
          totalTokens: 175,
        },
        observedAt: new Date("2026-06-28T11:30:00.000Z"),
      })

      const account = (await loadAccountUsageStore(summary)).accounts[hashPylonAccountRef("codex", "default")]
      expect(account?.localSessionTruth?.usage.totalTokens).toBe(175)
      expect(account?.localSessionUsageHistory?.map((entry) => entry.usage.totalTokens)).toEqual([100, 175])
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
      provider: null,
      refresh: false,
      json: true,
    })
    expect(parsePylonAccountsUsageArgs(["--all", "--refresh", "--json"])).toMatchObject({
      all: true,
      refresh: true,
      json: true,
    })
    expect(parsePylonAccountsUsageArgs(["--provider", "chatgpt", "--json"])).toMatchObject({
      provider: "codex",
      all: false,
      json: true,
    })
    expect(parsePylonAccountsUsageArgs(["--provider", "claude", "--json"])).toMatchObject({
      provider: "claude_agent",
      all: false,
      json: true,
    })
    expect(() => parsePylonAccountsUsageArgs(["--all", "--account", "codex-a", "--json"])).toThrow(
      /only one of --account, --provider, or --all/,
    )
    expect(() => parsePylonAccountsUsageArgs(["--provider", "codex", "--account", "codex-a", "--json"])).toThrow(
      /only one of --account, --provider, or --all/,
    )
  })

  test("targets default provider accounts with human-friendly account aliases", async () => {
    await withHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const projection = await collectPylonAccountsUsage(
        summary,
        parsePylonAccountsUsageArgs(["--account", "codex", "--json"]),
        {
          env: {
            CODEX_HOME: join(home, "codex-default"),
            CLAUDE_CONFIG_DIR: join(home, "claude-default"),
            PYLON_HOME: home,
            // Isolate the sibling-home scan (#4953) from the real home dir.
            PYLON_ACCOUNT_HOME_ROOT: join(home, "no-siblings"),
          },
          now: new Date("2026-06-12T12:00:00.000Z"),
        },
      )

      expect(projection.accounts).toHaveLength(1)
      expect(projection.accounts[0]).toMatchObject({
        provider: "codex",
        accountRef: null,
        accountRefHash: hashPylonAccountRef("codex", "default"),
      })
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)
    })
  })

  test("discovers sibling account homes on the machine (#4953)", async () => {
    await withHome(async (home) => {
      // A scan root holding several account homes beyond the two defaults.
      const root = join(home, "scan-root")
      await mkdir(join(root, ".codex"), { recursive: true })
      await mkdir(join(root, ".codex-pylon-b"), { recursive: true })
      await mkdir(join(root, ".claude"), { recursive: true })
      await mkdir(join(root, ".claude-work"), { recursive: true })
      await mkdir(join(root, ".claude-supervisor"), { recursive: true })
      await mkdir(join(root, ".unrelated"), { recursive: true })
      await writeFile(join(root, ".codex-not-a-dir"), "x")
      await writeFile(join(root, ".claude-work", "claude-oauth-token"), "sk-ant-oat-work\n")

      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const projection = await collectPylonAccountsList(summary, {
        env: {
          PYLON_HOME: home,
          CODEX_HOME: join(root, ".codex"),
          CLAUDE_CONFIG_DIR: join(root, ".claude"),
          PYLON_ACCOUNT_HOME_ROOT: root,
        },
        now: new Date("2026-06-12T12:00:00.000Z"),
      })

      const codexHomes = projection.accounts.filter((a) => a.provider === "codex").length
      const claudeHomes = projection.accounts.filter((a) => a.provider === "claude_agent").length
      // Both default homes + the extra sibling homes, deduped (not just 1 each).
      expect(codexHomes).toBeGreaterThanOrEqual(2)
      expect(claudeHomes).toBeGreaterThanOrEqual(2)
      // The non-account dir and the regular file are ignored.
      expect(projection.accounts.some((a) => String(a.accountRef ?? "").includes("unrelated"))).toBe(false)
      const claudeWork = projection.accounts.find((a) => a.provider === "claude_agent" && a.accountRef === "claude-work")
      const claudeSupervisor = projection.accounts.find((a) => a.provider === "claude_agent" && a.accountRef === "claude-supervisor")
      expect(claudeWork?.readiness.state).toBe("ready")
      expect(claudeSupervisor?.readiness.state).not.toBe("ready")
      assertPublicProjectionSafe(projection)
    })
  })
})
