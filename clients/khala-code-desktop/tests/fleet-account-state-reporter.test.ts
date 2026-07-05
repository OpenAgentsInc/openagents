import { describe, expect, test } from "bun:test"

import type {
  KhalaCodexFleetCommandInput,
  KhalaCodexFleetCommandResult,
} from "../src/bun/khala-fleet-tools"
import {
  collectLocalFleetAccountStateReports,
  fleetAccountReadinessFromPylonState,
  fleetAccountReportIntervalMs,
  fleetAccountReportRunIds,
  fleetAccountWireProvider,
  startKhalaCodeDesktopFleetAccountStateReporter,
  type FleetAccountStateReportOutcome,
  type FleetAccountStateReportSyncResult,
} from "../src/bun/fleet-account-state-reporter"

function ok(stdout: unknown): KhalaCodexFleetCommandResult {
  return {
    exitCode: 0,
    signal: null,
    stderr: "",
    stdout: typeof stdout === "string" ? stdout : JSON.stringify(stdout),
    timedOut: false,
  }
}

function pylonArgs(input: KhalaCodexFleetCommandInput): readonly string[] {
  const index = input.cmd.indexOf("src/index.ts")
  return index === -1 ? input.cmd : input.cmd.slice(index + 1)
}

const codexAccountRefHash = "account.pylon.codex.4db4cc18ebc55f39fb4da894"
const claudeAccountRefHash = "account.pylon.claude_agent.a83393092019de4dfbee9844"

/** Fake Pylon CLI covering both `codex` and `claude_agent` accounts. */
function twoProviderRunner(): (
  input: KhalaCodexFleetCommandInput,
) => Promise<KhalaCodexFleetCommandResult> {
  return async input => {
    const joined = pylonArgs(input).join(" ")
    if (joined === "provider go-online --json") {
      return ok({ ok: true, pylonRef: "pylon.local.test" })
    }
    if (joined === "accounts list --json") {
      return ok({
        accounts: [
          {
            accountRef: "codex-worker",
            accountRefHash: codexAccountRefHash,
            homeState: "present",
            provider: "codex",
          },
          {
            accountRef: "claude-worker",
            accountRefHash: claudeAccountRefHash,
            homeState: "present",
            provider: "claude_agent",
          },
        ],
        schema: "openagents.pylon.accounts_list.v0.3",
      })
    }
    if (joined === "accounts status --provider codex --json") {
      return ok({
        accounts: [{
          accountRef: "codex-worker",
          accountRefHash: codexAccountRefHash,
          provider: "codex",
          readiness: { state: "ready" },
        }],
        schema: "openagents.pylon.accounts_status.v0.1",
      })
    }
    if (joined === "accounts status --provider claude_agent --json") {
      return ok({
        accounts: [{
          accountRef: "claude-worker",
          accountRefHash: claudeAccountRefHash,
          provider: "claude_agent",
          readiness: { state: "ready" },
        }],
        schema: "openagents.pylon.accounts_status.v0.1",
      })
    }
    if (joined === "khala apm --base-url https://openagents.com --json") {
      return ok({
        active: { serverAssignmentCount: 0, serverAssignments: [] },
        counted: { completedTokenRows: 0, completedTokensPerMinute: 0 },
        schema: "openagents.pylon.khala_apm.v0.1",
      })
    }
    return {
      exitCode: 1,
      signal: null,
      stderr: `unexpected command: ${joined}`,
      stdout: "",
      timedOut: false,
    }
  }
}

describe("fleetAccountReadinessFromPylonState", () => {
  test("maps ready straight through", () => {
    expect(fleetAccountReadinessFromPylonState("ready")).toBe("ready")
  })

  test("maps throttled states to cooldown", () => {
    expect(fleetAccountReadinessFromPylonState("usage_limited")).toBe("cooldown")
    expect(fleetAccountReadinessFromPylonState("rate_limited")).toBe("cooldown")
  })

  test("maps structurally-blocked states to unavailable", () => {
    for (
      const state of [
        "credentials_missing",
        "credentials_revoked",
        "sdk_missing",
        "auth_error",
        "platform_unsupported",
        "disabled_by_config",
      ]
    ) {
      expect(fleetAccountReadinessFromPylonState(state)).toBe("unavailable")
    }
  })

  test("maps transient probe failures and unrecognized states to unknown", () => {
    expect(fleetAccountReadinessFromPylonState("network")).toBe("unknown")
    expect(fleetAccountReadinessFromPylonState("timeout")).toBe("unknown")
    expect(fleetAccountReadinessFromPylonState("something_new")).toBe("unknown")
  })
})

describe("fleetAccountWireProvider", () => {
  test("maps claude_agent to the public claude tag", () => {
    expect(fleetAccountWireProvider("claude_agent")).toBe("claude")
  })

  test("passes codex through unchanged", () => {
    expect(fleetAccountWireProvider("codex")).toBe("codex")
  })
})

describe("collectLocalFleetAccountStateReports", () => {
  test("enumerates BOTH codex and claude_agent accounts, not just codex", async () => {
    const reports = await collectLocalFleetAccountStateReports({ runner: twoProviderRunner() })
    expect(reports).toHaveLength(2)
    const byHash = new Map(reports.map(report => [report.accountRefHash, report]))
    expect(byHash.get(codexAccountRefHash)).toMatchObject({
      provider: "codex",
      readiness: "ready",
    })
    expect(byHash.get(claudeAccountRefHash)).toMatchObject({
      provider: "claude",
      readiness: "ready",
    })
  })

  test("omits capacity fields rather than fabricating a placeholder when unresolved", async () => {
    const reports = await collectLocalFleetAccountStateReports({ runner: twoProviderRunner() })
    for (const report of reports) {
      expect(report.capacityAvailable).toBeUndefined()
      expect(report.capacityBusy).toBeUndefined()
      expect(report.capacityQueued).toBeUndefined()
    }
  })

  test("reports live capacity numbers when the provider projection resolves them", async () => {
    const runner = async (
      input: KhalaCodexFleetCommandInput,
    ): Promise<KhalaCodexFleetCommandResult> => {
      const joined = pylonArgs(input).join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            codexAccounts: [{
              accountKey: "4db4cc18ebc55f39fb4da894",
              available: 5,
              busy: 1,
              queued: 0,
              ready: 5,
            }],
          },
          pylonRef: "pylon.local.test",
        })
      }
      return twoProviderRunner()(input)
    }
    const reports = await collectLocalFleetAccountStateReports({ runner })
    const codexReport = reports.find(report => report.accountRefHash === codexAccountRefHash)
    expect(codexReport).toMatchObject({
      capacityAvailable: 5,
      capacityBusy: 1,
      capacityQueued: 0,
    })
  })
})

describe("fleetAccountReportRunIds", () => {
  test("returns an empty list when unset (never guesses a run id)", () => {
    expect(fleetAccountReportRunIds({})).toEqual([])
  })

  test("splits a comma-separated list and trims whitespace", () => {
    expect(
      fleetAccountReportRunIds({
        KHALA_SYNC_FLEET_ACCOUNT_REPORT_RUN_ID: " fleet_run.a , fleet_run.b ,, ",
      }),
    ).toEqual(["fleet_run.a", "fleet_run.b"])
  })
})

describe("fleetAccountReportIntervalMs", () => {
  test("defaults to 30 seconds", () => {
    expect(fleetAccountReportIntervalMs({})).toBe(30_000)
  })

  test("honors an explicit override", () => {
    expect(fleetAccountReportIntervalMs({ KHALA_SYNC_FLEET_ACCOUNT_REPORT_INTERVAL_MS: "5000" })).toBe(5_000)
  })

  test("disable flag forces a zero interval", () => {
    expect(fleetAccountReportIntervalMs({ KHALA_SYNC_FLEET_ACCOUNT_REPORT_DISABLED: "1" })).toBe(0)
  })
})

describe("startKhalaCodeDesktopFleetAccountStateReporter", () => {
  test("is an honest no-op when no run id is configured", async () => {
    const calls: unknown[] = []
    const results: FleetAccountStateReportSyncResult[] = []
    const handle = startKhalaCodeDesktopFleetAccountStateReporter({
      env: {},
      intervalMs: 0,
      khalaSync: {
        fleetReportAccountState: async request => {
          calls.push(request)
          return { ok: true }
        },
      },
      onResult: result => results.push(result),
      toolOptions: { runner: twoProviderRunner() },
    })
    await handle.reportNow()
    expect(calls).toHaveLength(0)
    expect(results.at(-1)).toEqual({ skipped: "no_run_id_configured" })
    handle.dispose()
  })

  test("pushes one fleetReportAccountState call per account per configured run id", async () => {
    const calls: Array<{ runId: string; accountRefHash: string; provider?: string; readiness: string }> = []
    const handle = startKhalaCodeDesktopFleetAccountStateReporter({
      env: {},
      intervalMs: 0,
      khalaSync: {
        fleetReportAccountState: async request => {
          calls.push(request)
          return { ok: true }
        },
      },
      runIds: ["fleet_run.a", "fleet_run.b"],
      toolOptions: { runner: twoProviderRunner() },
    })
    const result = await handle.reportNow()
    expect(calls).toHaveLength(4)
    expect(calls.filter(call => call.runId === "fleet_run.a")).toHaveLength(2)
    expect(calls.filter(call => call.runId === "fleet_run.b")).toHaveLength(2)
    expect(calls.some(call => call.accountRefHash === claudeAccountRefHash && call.provider === "claude"))
      .toBe(true)
    expect(calls.some(call => call.accountRefHash === codexAccountRefHash && call.provider === "codex"))
      .toBe(true)
    expect(result).not.toBeNull()
    if (result !== null && !("skipped" in result)) {
      expect(result.reportCount).toBe(4)
      expect(result.failedCount).toBe(0)
    } else {
      throw new Error("expected a real report result")
    }
    handle.dispose()
  })

  test("records per-account failures without throwing, and keeps the watermark of what failed", async () => {
    const outcomes: FleetAccountStateReportOutcome[] = []
    const handle = startKhalaCodeDesktopFleetAccountStateReporter({
      env: {},
      intervalMs: 0,
      khalaSync: {
        fleetReportAccountState: async request => {
          if (request.provider === "claude") return { error: "khala_sync_fleet_disabled", ok: false }
          return { ok: true }
        },
      },
      runIds: ["fleet_run.a"],
      toolOptions: { runner: twoProviderRunner() },
    })
    const result = await handle.reportNow()
    if (result === null || "skipped" in result) throw new Error("expected a real report result")
    outcomes.push(...result.outcomes)
    expect(result.reportCount).toBe(2)
    expect(result.failedCount).toBe(1)
    expect(outcomes.find(outcome => outcome.provider === "claude")).toMatchObject({
      error: "khala_sync_fleet_disabled",
      ok: false,
    })
    expect(outcomes.find(outcome => outcome.provider === "codex")).toMatchObject({ ok: true })
    handle.dispose()
  })

  test("ticks on the configured interval using the injected timer and stops on dispose", async () => {
    const scheduledRef: { current: (() => void) | null } = { current: null }
    let cleared = false
    let tickCount = 0
    const handle = startKhalaCodeDesktopFleetAccountStateReporter({
      env: {},
      intervalMs: 1_000,
      khalaSync: {
        fleetReportAccountState: async () => ({ ok: true }),
      },
      onResult: () => {
        tickCount += 1
      },
      runIds: ["fleet_run.a"],
      setInterval: (callback, milliseconds) => {
        expect(milliseconds).toBe(1_000)
        scheduledRef.current = callback
        return "fake-timer"
      },
      clearInterval: timer => {
        expect(timer).toBe("fake-timer")
        cleared = true
      },
      toolOptions: { runner: twoProviderRunner() },
    })
    const waitUntil = async (predicate: () => boolean): Promise<void> => {
      for (let attempt = 0; attempt < 200 && !predicate(); attempt += 1) {
        await new Promise(resolve => setTimeout(resolve, 5))
      }
    }

    // The immediate first report at start.
    await waitUntil(() => tickCount >= 1)
    expect(tickCount).toBeGreaterThanOrEqual(1)
    expect(scheduledRef.current).not.toBeNull()
    const before = tickCount
    scheduledRef.current?.()
    await waitUntil(() => tickCount > before)
    expect(tickCount).toBeGreaterThan(before)
    handle.dispose()
    expect(cleared).toBe(true)
    const afterDispose = tickCount
    scheduledRef.current?.()
    await new Promise(resolve => setTimeout(resolve, 20))
    expect(tickCount).toBe(afterDispose)
  })
})
