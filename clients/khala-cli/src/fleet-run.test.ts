import { describe, expect, test } from "bun:test"

import {
  buildFleetRunPlan,
  parseFleetIssueList,
  runKhalaFleetSupervisor,
  validateFleetRunVerify,
} from "./fleet-run.js"
import type { KhalaFleetStatus } from "./fleet.js"

const commit = "0123456789abcdef0123456789abcdef01234567"

function status(accountRefs: readonly string[]): KhalaFleetStatus {
  return {
    accounts: accountRefs.map(accountRef => ({
      accountRef,
      email: null,
      home: `/tmp/${accountRef}`,
      lastLinkedAt: null,
      readiness: "ready",
    })),
    configPath: "/tmp/pylon/config.json",
    pylonHome: "/tmp/pylon",
    readyCount: accountRefs.length,
  }
}

describe("fleet run planning", () => {
  test("parses issue lists with hashes, commas, and whitespace", () => {
    expect(parseFleetIssueList("#6384, 6408 6410")).toEqual([6384, 6408, 6410])
  })

  test("scales slots to ready accounts times per-account, capped by max slots", () => {
    const plan = buildFleetRunPlan({
      commit,
      issues: [6384, 6408, 6410],
      maxSlots: 5,
      mode: "dry_run",
      perAccount: 2,
      pylonRef: "pylon.local",
      readyAccounts: ["codex", "codex-2", "codex-3"],
      repo: "Example/repo",
      verify: "bun test",
    })
    expect(plan.targetSlots).toBe(5)
    expect(plan.readyAccounts).toEqual(["codex", "codex-2", "codex-3"])
  })

  test("rejects local/private-looking verify commands", () => {
    expect(() => validateFleetRunVerify("OPENAGENTS_AGENT_TOKEN=x bun test")).toThrow(/public-safe/)
    expect(() => validateFleetRunVerify("bun test /Users/example/private.test.ts")).toThrow(/public-safe/)
  })
})

describe("fleet run dry-run", () => {
  test("returns planned account and issue routing without dispatching", async () => {
    const result = await runKhalaFleetSupervisor({
      commit,
      dryRun: true,
      issues: [6384, 6408, 6410],
      maxSlots: 4,
      perAccount: 2,
      pylonRef: "pylon.local",
      repo: "Example/repo",
      status: status(["codex", "codex-2"]),
      verify: "bun test",
    })
    expect(result.status).toBe("planned")
    expect(result.plan.targetSlots).toBe(4)
    expect(result.rounds.map(round => `${round.accountRef}:#${round.issue}`)).toEqual([
      "codex:#6384",
      "codex-2:#6408",
      "codex:#6410",
      "codex-2:#6384",
    ])
  })
})
