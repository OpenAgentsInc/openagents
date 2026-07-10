import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Database } from "bun:sqlite"

import type { GrokWorkerExecutorPort } from "@openagentsinc/grok-harness/worker-executor"

import { hashPylonAccountRef, type PylonAccountRegistryEntry } from "../src/account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import {
  createPylonOwnedGrokClaimedWorkPort,
  PYLON_OWNED_GROK_RUNNER_BLOCKERS,
} from "../src/orchestration/fleet-run-owned-grok-runner.js"
import { createPylonOwnedFleetRunSupervisorRunner } from "../src/orchestration/fleet-run-owned-runner.js"
import type {
  FleetRunSupervisorActiveAssignment,
  FleetRunSupervisorDispatchInput,
} from "../src/orchestration/fleet-run-supervisor.js"
import type { FleetRun, WorkClaim } from "../src/orchestration/store.js"
import { createPylonOrchestrationStore } from "../src/orchestration/store.js"

const fixedNow = new Date("2026-07-09T23:30:00.000Z")
const commit = "418878a11e4e9c4f791d13b98e0c80b97f82df4d"

const run: FleetRun = {
  schema: "openagents.khala_code.fleet_run.v1",
  runRef: "fleet_run.grok.exact.test",
  objective: "Implement one bounded public Grok work unit.",
  workSource: "issue_list",
  targetConcurrency: 2,
  workerKind: "grok",
  refillPolicy: { maxPerAccount: 1, cooldownAware: true, stopCondition: "backlog_empty" },
  state: "running",
  dispatchKind: "supervised_dispatch",
  dagTracked: false,
  startedAt: fixedNow.toISOString(),
  counters: {
    workUnitsTotal: 2,
    activeAssignments: 0,
    completedAssignments: 0,
    failedAssignments: 0,
    blockedAssignments: 0,
  },
  createdAt: fixedNow.toISOString(),
  updatedAt: fixedNow.toISOString(),
}

const claimFor = (accountRef: string, ordinal: number, assignmentRef: string | null = null): WorkClaim => ({
  schema: "openagents.khala_code.work_claim.v1",
  claimRef: `claim.grok.exact.${ordinal}`,
  workUnitRef: `work_unit.grok.exact.${ordinal}`,
  runRef: run.runRef,
  assignmentRef,
  workerAccountRef: accountRef,
  state: "in_progress",
  ttl: 60_000,
  claimedAt: fixedNow.toISOString(),
  expiresAt: new Date(fixedNow.getTime() + 60_000).toISOString(),
  updatedAt: fixedNow.toISOString(),
})

const dispatchFor = (
  accountRef: string,
  ordinal: number,
  kind: "fixture" | "github_issue" = "github_issue",
): FleetRunSupervisorDispatchInput => {
  const claim = claimFor(accountRef, ordinal)
  return {
    accountRef,
    claim,
    run,
    taskId: `task.grok.exact.${ordinal}`,
    workerKind: "grok",
    workUnit: kind === "fixture"
      ? {
          workUnitRef: claim.workUnitRef,
          kind: "fixture",
          title: `Grok fixture ${ordinal}`,
          source: "fixture",
          status: "claimable",
        }
      : {
          workUnitRef: claim.workUnitRef,
          kind: "github_issue",
          title: `Grok issue ${ordinal}`,
          source: "issue_list",
          status: "claimable",
          body: `Implement public issue ${ordinal} with the pinned verifier.`,
          branch: "main",
          baseCommit: commit,
          repo: "OpenAgentsInc/openagents",
          number: ordinal,
          verify: `bun test apps/pylon/tests/fixture-${ordinal}.test.ts`,
        },
  }
}

const accountFor = (home: string, ref: string): PylonAccountRegistryEntry => ({
  ref,
  provider: "grok",
  home,
  openAgentsProviderAccountRef: null,
  hourlyCap: null,
  weeklyCap: null,
  manualResetsRemaining: null,
  marginalCostClass: "subscription",
})

const successfulExecutor = (
  onRun: (input: Parameters<GrokWorkerExecutorPort["runClaimedWork"]>[0]) => void = () => {},
): GrokWorkerExecutorPort => ({
  kind: "grok_cli",
  readiness: async () => ({
    ready: true,
    binary: "grok",
    plane: "cli_session",
    models: ["grok-code-fast-1"],
  }),
  runClaimedWork: async input => {
    onRun(input)
    return {
      ok: true,
      claimRef: input.pin.claimRef,
      stopReason: "end_turn",
      text: "raw local output from /Users/owner/private must not project",
      usage: {
        metering: "not_measured",
        wallClockMs: 12,
        plane: "cli_session",
        marginalCostClass: input.marginalCostClass ?? "not_measured",
      },
    }
  },
})

describe("Pylon-owned exact Grok claimed-work adapter", () => {
  test("uses one exact named GROK_HOME, pinned workspace/verifier, and not_measured closeout", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-exact-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    const accountRef = "grok-a"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    await mkdir(accountHome, { recursive: true })
    const account = accountFor(accountHome, accountRef)
    const dispatch = dispatchFor(accountRef, 1)
    let executorRuns = 0
    let verifierRuns = 0
    let materializedCwd: string | null = null

    try {
      const port = createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        env: {
          GROK_HOME: join(root, "default-grok"),
          XAI_API_KEY: "private-global-key",
          GROK_AUTH: "private-global-session",
          PATH: process.env.PATH,
        },
        loadRegistry: async () => [account],
        createExecutor: ({ env }) => {
          expect(env.GROK_HOME).toBe(accountHome)
          expect(env.XAI_API_KEY).toBeUndefined()
          expect(env.GROK_AUTH).toBeUndefined()
          return successfulExecutor(input => {
            executorRuns += 1
            expect(input.pin).toMatchObject({
              accountRefHash: hashPylonAccountRef("grok", accountRef),
              branch: "main",
              claimRef: dispatch.claim.claimRef,
              commit,
              repo: "OpenAgentsInc/openagents",
              runRef: run.runRef,
              verifyCommand: dispatch.workUnit.verify,
              workUnitRef: dispatch.workUnit.workUnitRef,
            })
            expect(input.pin.cwd).toBe(materializedCwd)
          })
        },
        checkoutRunner: async (workingDirectory, checkout) => {
          materializedCwd = workingDirectory
          expect(checkout).toMatchObject({
            kind: "git_checkout",
            repository: {
              branch: "main",
              commitSha: commit,
              fullName: "OpenAgentsInc/openagents",
            },
            verificationCommand: {
              args: ["bun", "test", "apps/pylon/tests/fixture-1.test.ts"],
            },
          })
          await mkdir(workingDirectory, { recursive: true })
        },
        runVerifier: async input => {
          verifierRuns += 1
          expect(input.args).toEqual(["bun", "test", "apps/pylon/tests/fixture-1.test.ts"])
          expect(input.cwd).toBe(materializedCwd)
          return { exitCode: 0, timedOut: false }
        },
      })

      const result = await port.dispatch(dispatch)
      expect(result).toMatchObject({
        accountRefHash: hashPylonAccountRef("grok", accountRef),
        assignmentRef: expect.stringMatching(/^assignment\.pylon\.grok\.[a-f0-9]{24}$/),
        closeoutRef: expect.stringMatching(/^closeout\.public\.pylon\.grok\.[a-f0-9]{24}$/),
        status: "completed",
        summary: "The exact named Grok claimed work completed with not_measured usage.",
        usageEvidence: {
          truth: "not_measured",
          harnessKind: "grok",
          receiptRef: expect.stringMatching(/^receipt\.public\.pylon\.grok\.[a-f0-9]{24}$/),
          tokenUsageRefs: [],
        },
        lifecycle: [{
          accountRefHash: hashPylonAccountRef("grok", accountRef),
          artifactRef: expect.stringMatching(/^workspace\.pylon\.grok\./),
          status: "closed",
        }],
      })
      expect(executorRuns).toBe(1)
      expect(verifierRuns).toBe(1)
      expect(JSON.stringify(result)).not.toContain("/Users")
      expect(JSON.stringify(result)).not.toContain("private-global")
      expect(JSON.stringify(result)).not.toContain(materializedCwd!)
      expect(JSON.stringify(result.usageEvidence)).not.toMatch(
        /inputTokens|outputTokens|reasoningTokens|cacheReadTokens|totalTokens|tokenRows/,
      )

      const replay = await port.dispatch(dispatch)
      expect(replay).toEqual(result)
      expect(executorRuns).toBe(1)
      expect(verifierRuns).toBe(1)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("coalesces duplicates and a fresh adapter reconciles the terminal receipt without rerun", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-restart-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    const accountRef = "grok-a"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    await mkdir(accountHome, { recursive: true })
    const account = accountFor(accountHome, accountRef)
    const dispatch = dispatchFor(accountRef, 2, "fixture")
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.createFleetRun({
      runRef: run.runRef,
      objective: run.objective,
      workSource: run.workSource,
      targetConcurrency: run.targetConcurrency,
      workerKind: run.workerKind,
      state: "running",
      now: fixedNow,
    })
    const storedClaim = store.tryClaimWorkUnit({
      claimRef: dispatch.claim.claimRef,
      workUnitRef: dispatch.claim.workUnitRef,
      runRef: run.runRef,
      workerAccountRef: accountRef,
      ttl: 60_000,
      now: fixedNow,
    })
    if (storedClaim === null) throw new Error("failed to seed exact Grok test claim")
    store.updateWorkClaimState(storedClaim.claimRef, "in_progress", fixedNow)
    let runs = 0
    let release!: () => void
    const gate = new Promise<void>(resolve => {
      release = resolve
    })

    try {
      const grok = createPylonOwnedGrokClaimedWorkPort({
        summary,
        store,
        now: () => fixedNow,
        loadRegistry: async () => [account],
        createExecutor: () => ({
          ...successfulExecutor(),
          runClaimedWork: async input => {
            runs += 1
            await gate
            return successfulExecutor().runClaimedWork(input)
          },
        }),
        materializeWorkspace: async () => ({
          checkout: null,
          verificationArgs: null,
          workingDirectory: join(root, "workspace"),
          workspaceRef: "workspace.public.grok.restart",
        }),
      })
      const runner = createPylonOwnedFleetRunSupervisorRunner({
        summary,
        pylonRef: "pylon.public.grok.restart",
        baseUrl: "https://openagents.test",
        grok,
      })
      const first = runner.dispatch(dispatch)
      const duplicate = runner.dispatch(dispatch)
      for (let attempt = 0; attempt < 50 && runs === 0; attempt += 1) {
        await Bun.sleep(1)
      }
      expect(runs).toBe(1)
      expect(store.getWorkClaim(storedClaim.claimRef)?.assignmentRef).toMatch(
        /^assignment\.pylon\.grok\.[a-f0-9]{24}$/,
      )
      release()
      const [one, two] = await Promise.all([first, duplicate])
      expect(one).toEqual(two)
      expect(one.status).toBe("completed")
      expect(runs).toBe(1)

      const conflicting = {
        ...dispatch,
        workUnit: { ...dispatch.workUnit, title: "Conflicting reuse of one Grok claim" },
      } satisfies FleetRunSupervisorDispatchInput
      const conflict = await runner.dispatch(conflicting)
      expect(conflict).toMatchObject({ status: "failed" })
      expect(runs).toBe(1)

      const fresh = createPylonOwnedGrokClaimedWorkPort({
        summary,
        store,
        now: () => fixedNow,
        loadRegistry: async () => [account],
        createExecutor: () => {
          throw new Error("restart reconciliation must not instantiate the CLI")
        },
      })
      const active: FleetRunSupervisorActiveAssignment = {
        accountRef,
        claim: claimFor(accountRef, 2, one.assignmentRef),
        contextId: "context.grok.exact.2",
        taskId: dispatch.taskId,
      }
      const reconciled = await fresh.reconcile({ active, now: fixedNow, runRef: run.runRef })
      expect(reconciled).toMatchObject({
        accountRefHash: one.accountRefHash,
        assignmentRef: one.assignmentRef,
        closeoutRef: one.closeoutRef,
        status: "completed",
        taskId: dispatch.taskId,
        usageEvidence: one.usageEvidence,
      })
      expect(await fresh.probeLiveness(one.assignmentRef!)).toBe("live")
    } finally {
      release?.()
      await rm(root, { force: true, recursive: true })
    }
  })

  test("fails hostile custody, timeouts, and verifier failures closed without affecting another account", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-failures-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    const accountA = accountFor(join(pylonHome, "accounts", "grok", "grok-a"), "grok-a")
    const accountB = accountFor(join(pylonHome, "accounts", "grok", "grok-b"), "grok-b")
    await mkdir(accountA.home, { recursive: true })
    await mkdir(accountB.home, { recursive: true })
    const rawFailure = "SECRET output at /Users/owner/private"
    let verifierShouldFail = false

    try {
      const port = createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        loadRegistry: async () => [accountA, accountB],
        createExecutor: ({ account }) => account.ref === "grok-a"
          ? {
              ...successfulExecutor(),
              runClaimedWork: async input => ({
                ok: false,
                claimRef: input.pin.claimRef,
                stopReason: rawFailure,
                text: rawFailure,
                failureClass: "timeout",
                usage: {
                  metering: "not_measured",
                  wallClockMs: 100,
                  plane: "cli_session",
                  marginalCostClass: "subscription",
                },
              }),
            }
          : successfulExecutor(),
        materializeWorkspace: async request => ({
          checkout: null,
          verificationArgs: request.dispatch.accountRef === "grok-b" ? ["bun", "test"] : null,
          workingDirectory: join(root, `workspace-${request.dispatch.accountRef}`),
          workspaceRef: `workspace.public.${request.dispatch.accountRef}`,
        }),
        runVerifier: async () => ({ exitCode: verifierShouldFail ? 9 : 0, timedOut: false }),
      })

      const [timedOut, unaffected] = await Promise.all([
        port.dispatch(dispatchFor("grok-a", 3, "fixture")),
        port.dispatch(dispatchFor("grok-b", 4, "fixture")),
      ])
      expect(timedOut).toMatchObject({
        status: "failed",
        lifecycle: [{ blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionTimedOut] }],
      })
      expect(unaffected).toMatchObject({ status: "completed" })
      expect(JSON.stringify([timedOut, unaffected])).not.toContain(rawFailure)

      verifierShouldFail = true
      const verifyFailed = await port.dispatch(dispatchFor("grok-b", 41, "fixture"))
      expect(verifyFailed).toMatchObject({
        status: "failed",
        lifecycle: [{ blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.verificationFailed] }],
      })

      const defaultHomeResult = await createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        loadRegistry: async () => [{ ...accountA, home: join(process.env.HOME ?? root, ".grok") }],
        createExecutor: () => {
          throw new Error("default home must be rejected before executor construction")
        },
      }).dispatch(dispatchFor("grok-a", 5, "fixture"))
      expect(defaultHomeResult).toMatchObject({
        assignmentRef: null,
        status: "blocked",
        lifecycle: [{ blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.accountUnavailable] }],
      })

      await writeFile(
        join(accountA.home, "config.toml"),
        'model.private."env_key" = "OWNER_SHARED_GROK_KEY"\n',
      )
      const configuredCredential = await createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        env: { OWNER_SHARED_GROK_KEY: "private-value", PATH: process.env.PATH },
        loadRegistry: async () => [accountA],
      }).dispatch(dispatchFor("grok-a", 6, "fixture"))
      expect(configuredCredential).toMatchObject({
        status: "blocked",
        lifecycle: [{ blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.readinessUnavailable] }],
      })
      expect(JSON.stringify(configuredCredential)).not.toContain("OWNER_SHARED_GROK_KEY")
      expect(JSON.stringify(configuredCredential)).not.toContain("private-value")
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
