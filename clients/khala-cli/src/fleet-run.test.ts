import { describe, expect, test } from "bun:test"
import {
  KhalaFleetDelegationParameterSet,
  KhalaFleetDelegationParameterSetSchemaVersion,
} from "@openagentsinc/khala-tools"

import {
  buildFleetRunPlan,
  fleetRunCapacityEnv,
  nextFleetSupervisorDelay,
  parseFleetIssueList,
  plannedReplenishmentRounds,
  readyFleetAccountRefs,
  runKhalaFleetSupervisor,
  shouldDispatchReplenishment,
  validateFleetRunVerify,
} from "./fleet-run.js"
import type { KhalaFleetStatus } from "./fleet.js"

const commit = "0123456789abcdef0123456789abcdef01234567"

const admittedParameters = (
  overrides: Partial<KhalaFleetDelegationParameterSet> = {},
): KhalaFleetDelegationParameterSet =>
  new KhalaFleetDelegationParameterSet({
    actionSubmissionRef: "action_submission.khala_fleet_delegation.cli",
    candidateRef: "candidate.khala_fleet_delegation.cli",
    parameterSetRef: "parameter_set.khala_fleet_delegation.cli.v1",
    schemaVersion: KhalaFleetDelegationParameterSetSchemaVersion,
    source: "admitted_candidate",
    ...overrides,
  })

function status(accountRefs: readonly string[]): KhalaFleetStatus {
  return {
    accounts: accountRefs.map(accountRef => ({
      accountRef,
      email: null,
      harness: "codex",
      home: `/tmp/${accountRef}`,
      lastLinkedAt: null,
      provider: "codex",
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

  test("sets account-level capacity env from the planned per-account slots", () => {
    const plan = buildFleetRunPlan({
      commit,
      issues: [6384, 6408, 6410],
      maxSlots: 10,
      mode: "once",
      perAccount: 5,
      pylonRef: "pylon.local",
      readyAccounts: ["codex", "codex-2", "codex-3"],
      repo: "Example/repo",
      verify: "bun test",
    })

    const env = fleetRunCapacityEnv({ OPENAGENTS_PYLON_CODEX_CONCURRENCY: "1" }, plan)

    expect(env.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY).toBe("5")
    expect(env.OPENAGENTS_PYLON_CODEX_CONCURRENCY).toBe("10")
    expect(env.OPENAGENTS_PYLON_CODEX_BUSY).toBe("0")
    expect(env.OPENAGENTS_PYLON_CODEX_QUEUED).toBe("0")
  })

  test("uses admitted parameters for default slot sizing and objective templates", async () => {
    const parameters = admittedParameters({
      advertiseCapacity: {
        maxRequestedSlots: 6,
        perAccountConcurrency: 3,
      },
      objectiveTemplate: "GD4 tuned #{issue}: {objective} [{repo}] verify={verify}",
    })
    const tuned = buildFleetRunPlan({
      commit,
      delegationParameters: parameters,
      issues: [7736, 7737],
      mode: "dry_run",
      pylonRef: "pylon.local",
      readyAccounts: ["codex", "codex-2"],
      repo: "OpenAgentsInc/openagents",
      verify: "bun test",
    })
    const reverted = buildFleetRunPlan({
      commit,
      issues: [7736, 7737],
      mode: "dry_run",
      pylonRef: "pylon.local",
      readyAccounts: ["codex", "codex-2"],
      repo: "OpenAgentsInc/openagents",
      verify: "bun test",
    })

    expect(tuned).toMatchObject({
      delegationParameterSetRef: "parameter_set.khala_fleet_delegation.cli.v1",
      maxSlots: 6,
      perAccount: 3,
      targetSlots: 6,
    })
    expect(fleetRunCapacityEnv({}, tuned, parameters).OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY).toBe("3")
    const result = await runKhalaFleetSupervisor({
      commit,
      delegationParameters: parameters,
      dryRun: true,
      issues: [7736],
      pylonRef: "pylon.local",
      repo: "OpenAgentsInc/openagents",
      status: status(["codex", "codex-2"]),
      verify: "bun test",
    })

    expect(result.plan.delegationParameterSetRef).toBe("parameter_set.khala_fleet_delegation.cli.v1")
    expect(result.rounds[0]?.objective).toBe(
      "GD4 tuned #7736: Implement public issue #7736 and run the named verification. [OpenAgentsInc/openagents] verify=bun test",
    )
    expect(reverted).toMatchObject({
      delegationParameterSetRef: "parameter_set.khala_fleet_delegation.default.v1",
      maxSlots: 8,
      perAccount: 1,
      targetSlots: 2,
    })
  })

  test("rejects local/private-looking verify commands", () => {
    expect(() => validateFleetRunVerify("OPENAGENTS_AGENT_TOKEN=x bun test")).toThrow(/public-safe/)
    expect(() => validateFleetRunVerify("bun test /Users/example/private.test.ts")).toThrow(/public-safe/)
  })

  test("plans bounded deduped replenishment work for lockout recovery", () => {
    const plan = buildFleetRunPlan({
      commit,
      issues: [6384, 6408, 6410],
      maxSlots: 2,
      mode: "supervise",
      perAccount: 1,
      pylonRef: "pylon.local",
      readyAccounts: ["codex", "codex-2", "codex-3"],
      repo: "Example/repo",
      verify: "bun test",
    })

    const first = plannedReplenishmentRounds(plan)
    expect(first).toHaveLength(2)
    expect(first.map(round => round.workKind)).toEqual(["replenishment", "replenishment"])
    expect(first.map(round => round.dedupeKey)).toEqual(["desktop-fleet-readiness-audit", "bounded-codebase-audit"])
    expect(first[0]?.issue).toBeNull()
    expect(first[0]?.objective).toContain("desktop-fleet dispatch path")
    expect(first[1]?.issue).toBeNull()
    expect(first[1]?.objective).toContain("apps/pylon")

    const second = plannedReplenishmentRounds(plan, new Set(first.map(round => round.dedupeKey ?? "")))
    expect(second).toHaveLength(2)
    expect(second[0]?.dedupeKey).toBe("test-lint-typecheck-sweep")
    expect(second[1]?.dedupeKey).toBe("lockout-recovery-sweep-2")
  })

  test("keeps producing bounded replenishment work after fixed recovery tasks are exhausted", () => {
    const plan = buildFleetRunPlan({
      commit,
      issues: [6820, 6707],
      maxSlots: 2,
      mode: "supervise",
      perAccount: 1,
      pylonRef: "pylon.local",
      readyAccounts: ["codex", "codex-2"],
      repo: "OpenAgentsInc/openagents",
      verify: "bun test",
    })
    const dispatched = new Set([
      "desktop-fleet-readiness-audit",
      "bounded-codebase-audit",
      "test-lint-typecheck-sweep",
    ])

    const generated = plannedReplenishmentRounds(plan, dispatched)

    expect(generated).toHaveLength(2)
    expect(generated.map(round => round.workKind)).toEqual(["replenishment", "replenishment"])
    expect(generated.map(round => round.dedupeKey)).toEqual([
      "lockout-recovery-sweep-3",
      "lockout-recovery-sweep-4",
    ])
    expect(generated.map(round => round.issue)).toEqual([6707, 6820])
    expect(generated[0]?.objective).toContain("Re-audit public issue #6707")
  })

  test("keeps lockout recovery in the short supervisor cadence", () => {
    expect(shouldDispatchReplenishment(0)).toBe(false)
    expect(shouldDispatchReplenishment(1)).toBe(false)
    expect(shouldDispatchReplenishment(2)).toBe(true)

    const firstLockoutWait = nextFleetSupervisorDelay({
      anyRefused: true,
      lockout: true,
      refusedBackoffMs: 15_000,
    })
    expect(firstLockoutWait.delayMs).toBe(2_000)
    expect(firstLockoutWait.refusedBackoffMs).toBe(15_000)

    const partialRefusalWait = nextFleetSupervisorDelay({
      anyRefused: true,
      lockout: false,
      refusedBackoffMs: 15_000,
    })
    expect(partialRefusalWait.delayMs).toBe(15_000)
    expect(partialRefusalWait.refusedBackoffMs).toBe(30_000)
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
    expect(result.rounds.every(round => round.workKind === "issue")).toBe(true)
  })
})

describe("fleet run dispatch isolation", () => {
  test("one locked-out account's dispatch failure does not abort dispatch for other ready accounts", async () => {
    // Fake `pylon` binary: answers `provider go-online` / `presence heartbeat`
    // successfully for any account, but for account "codex-locked" the
    // `request` subcommand emits the NEEDS-OWNER reauth signature and a
    // non-zero exit — the same shape that makes real `dispatchFleetSlot`
    // throw. Regression coverage for the #8282 Promise.all landmine audit:
    // that throw must fail ONLY that one account's round entry, not abort
    // dispatch for every other ready account in the same round.
    const fakePylonScript = `
      const args = process.argv.slice(1);
      if (args[0] === "provider" && args[1] === "go-online") {
        console.log(JSON.stringify({ pylonRef: "pylon.fake" }));
        process.exit(0);
      }
      if (args[0] === "presence" && args[1] === "heartbeat") {
        console.log(JSON.stringify({ ok: true }));
        process.exit(0);
      }
      if (args.includes("request")) {
        const refIndex = args.indexOf("--account-ref");
        const accountRef = refIndex >= 0 ? args[refIndex + 1] : undefined;
        if (accountRef === "codex-locked") {
          console.error("please sign in again");
          process.exit(1);
        }
        console.log(JSON.stringify({ assignmentRef: "assignment." + accountRef }));
        process.exit(0);
      }
      process.exit(0);
    `

    const result = await runKhalaFleetSupervisor({
      commit,
      issues: [6384],
      maxSlots: 2,
      once: true,
      perAccount: 1,
      pylonCommand: ["node", "-e", fakePylonScript],
      pylonRef: "pylon.fake",
      repo: "Example/repo",
      status: status(["codex-locked", "codex-2"]),
      verify: "bun test",
    })

    expect(result.status).toBe("completed")
    expect(result.rounds).toHaveLength(2)

    const lockedRound = result.rounds.find(round => round.accountRef === "codex-locked")
    const healthyRound = result.rounds.find(round => round.accountRef === "codex-2")

    expect(lockedRound?.ok).toBe(false)
    expect(lockedRound?.status).toBe("failed")
    // The healthy sibling account must still be dispatched and accepted —
    // the locked-out account's thrown error must not have aborted the round.
    expect(healthyRound?.ok).toBe(true)
    expect(healthyRound?.status).toBe("accepted")
    expect(healthyRound?.assignmentRef).toBe("assignment.codex-2")
  })
})

describe("fleet run harness filtering", () => {
  test("a ready Claude account contributes no Codex slot", () => {
    const mixed: KhalaFleetStatus = {
      accounts: [
        {
          accountRef: "codex-2",
          email: null,
          harness: "codex",
          home: "/tmp/codex-2",
          lastLinkedAt: null,
          provider: "codex",
          readiness: "ready",
        },
        {
          accountRef: "claude",
          email: null,
          harness: "claude",
          home: "/tmp/claude",
          lastLinkedAt: null,
          provider: "claude_agent",
          readiness: "ready",
        },
      ],
      configPath: "/tmp/pylon/config.json",
      pylonHome: "/tmp/pylon",
      readyCount: 2,
    }
    expect(readyFleetAccountRefs(mixed)).toEqual(["codex-2"])
    const plan = buildFleetRunPlan({
      commit,
      issues: [6384, 6408],
      mode: "dry_run",
      readyAccounts: readyFleetAccountRefs(mixed),
      repo: "OpenAgentsInc/openagents",
      verify: "bun run --cwd apps/pylon test",
    })
    expect(plan.readyAccounts).toEqual(["codex-2"])
    const codexOnly = buildFleetRunPlan({
      commit,
      issues: [6384, 6408],
      mode: "dry_run",
      readyAccounts: ["codex-2"],
      repo: "OpenAgentsInc/openagents",
      verify: "bun run --cwd apps/pylon test",
    })
    expect(plan.targetSlots).toBe(codexOnly.targetSlots)
    expect(fleetRunCapacityEnv({}, plan).OPENAGENTS_PYLON_CODEX_CONCURRENCY).toBe(String(plan.targetSlots))
  })
})
