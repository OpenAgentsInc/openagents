import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises"
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
  onFollowUp: (input: Parameters<GrokWorkerExecutorPort["runFollowUp"]>[0]) => void = () => {},
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
  runFollowUp: async input => {
    onFollowUp(input)
    return {
      ok: true,
      claimRef: input.pin.claimRef,
      stopReason: "end_turn",
      text: "private local follow-up output must not project",
      usage: {
        metering: "not_measured",
        wallClockMs: 8,
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
        marginalCostClass: "subscription",
        verification: {
          truth: "passed",
          verifierRef: expect.stringMatching(/^verifier\.public\.pylon\.grok\./),
          evidenceRefs: [expect.stringMatching(/^verification\.public\.pylon\.grok\./)],
        },
        artifactRefs: [expect.stringMatching(/^workspace\.pylon\.grok\./)],
        proofRefs: [expect.stringMatching(/^receipt\.public\.pylon\.grok\./)],
        authorityReceiptRefs: [dispatch.claim.claimRef],
        summary: "The exact named Grok claimed work completed with not_measured usage.",
        usageEvidence: {
          truth: "not_measured",
          harnessKind: "grok",
          receiptRef: expect.stringMatching(/^receipt\.public\.pylon\.grok\.[a-f0-9]{24}$/),
          tokenUsageRefs: [],
          caveatRefs: ["caveat.pylon.fleet_run.grok_usage_not_measured"],
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

  test("uses the shared prepared and prebuilt dependency caches for production Grok checkouts", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-prebuilt-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    const accountRef = "grok-prebuilt"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    const workingDirectory = join(root, "workspace")
    await mkdir(accountHome, { recursive: true })
    await mkdir(workingDirectory, { recursive: true })
    let materializeInput: Record<string, unknown> | null = null

    try {
      const result = await createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        loadRegistry: async () => [accountFor(accountHome, accountRef)],
        createExecutor: () => successfulExecutor(),
        materializeCheckout: async input => {
          materializeInput = input as unknown as Record<string, unknown>
          return {
            cleanupRef: "cleanup.public.grok.prebuilt",
            sourceRef: "source.public.grok.prebuilt",
            workingDirectory,
            workspaceRef: "workspace.public.grok.prebuilt",
          }
        },
        runVerifier: async () => ({ exitCode: 0, timedOut: false }),
      }).dispatch(dispatchFor(accountRef, 202))

      expect(result.status).toBe("completed")
      expect(materializeInput).toMatchObject({
        cacheRoot: join(summary.paths.cache, "grok-fleet-workspaces"),
        preparedWorktreeCacheRoot: join(summary.paths.cache, "workspace-prepared-cache"),
        prebuiltBaselineCacheRoot: join(summary.paths.cache, "workspace-prebuilt-baselines"),
        repositoryCacheRoot: join(summary.paths.cache, "workspace-git-cache"),
        workspaceStateRoot: join(summary.paths.cache, "workspace-leases"),
      })
      expect(materializeInput).not.toHaveProperty("checkoutRunner")
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("serializes one exact private steer onto the Grok session before verification", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-steer-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(
      parseBootstrapArgs(["--json"]),
      { PYLON_HOME: pylonHome },
    )
    const accountRef = "grok-steer"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    const workingDirectory = join(root, "workspace")
    await mkdir(accountHome, { recursive: true })
    await mkdir(workingDirectory, { recursive: true })
    const request = dispatchFor(accountRef, 203, "fixture")
    const order: string[] = []
    const privateBody = "PRIVATE: add the exact regression before closeout"
    let assignmentRef = ""
    let releaseInitial!: () => void
    const initialGate = new Promise<void>(resolve => {
      releaseInitial = resolve
    })
    let reportInitialStarted!: (
      input: Parameters<GrokWorkerExecutorPort["runClaimedWork"]>[0],
    ) => void
    const initialStarted = new Promise<
      Parameters<GrokWorkerExecutorPort["runClaimedWork"]>[0]
    >(resolve => {
      reportInitialStarted = resolve
    })
    const followUps: Array<
      Parameters<GrokWorkerExecutorPort["runFollowUp"]>[0]
    > = []
    let verifierRuns = 0
    let port!: ReturnType<typeof createPylonOwnedGrokClaimedWorkPort>
    let earlySteer: ReturnType<typeof port.applySteer> | null = null

    try {
      port = createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        loadRegistry: async () => [accountFor(accountHome, accountRef)],
        createExecutor: () => ({
          ...successfulExecutor(),
          runClaimedWork: async input => {
            order.push("initial")
            reportInitialStarted(input)
            await initialGate
            return await successfulExecutor().runClaimedWork(input)
          },
          runFollowUp: async input => {
            order.push("follow_up")
            followUps.push(input)
            return await successfulExecutor().runFollowUp(input)
          },
        }),
        materializeWorkspace: async () => ({
          checkout: null,
          verificationArgs: ["bun", "--version"],
          workingDirectory,
          workspaceRef: "workspace.public.grok.steer",
        }),
        runVerifier: async () => {
          order.push("verify")
          verifierRuns += 1
          return { exitCode: 0, timedOut: false }
        },
      })
      const exactSteerFor = (targetAssignmentRef: string) => ({
        pylonRef: "pylon.public.grok.steer",
        runRef: request.run.runRef,
        claimRef: "claim.sarah_fleet_run.grok.steer",
        workUnitRef: request.workUnit.workUnitRef,
        workClaimRef: request.claim.claimRef,
        assignmentRef: targetAssignmentRef,
        intent: {
          seq: 17,
          intentId: "intent.grok.steer.exact",
          completionContractRef:
            "contract.pylon.fleet_steering_completion.grok_exact",
        },
        body: privateBody,
        bodyRef: null,
      } as const)
      const dispatched = port.dispatch({
        ...request,
        onLifecycle: async event => {
          if (event.event === "assignment_run.runtime_started") {
            assignmentRef = event.assignmentRef ?? ""
            // Reproduce the tightest live race: the steer is accepted as soon
            // as assignment identity is projected, before the initial CLI turn
            // has started. It must queue behind that initial turn.
            earlySteer = port.applySteer(exactSteerFor(assignmentRef))
          }
        },
      })
      const initial = await initialStarted
      expect(assignmentRef).toMatch(/^assignment\.pylon\.grok\.[a-f0-9]{24}$/)
      expect(initial.sessionId).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
      )

      const exactSteer = exactSteerFor(assignmentRef)
      expect(await port.applySteer({
        ...exactSteer,
        workUnitRef: "work_unit.grok.foreign",
      })).toEqual({
        state: "failed",
        failureRef: "blocker.pylon.fleet_steering.attempt_binding_invalid",
      })
      expect(await port.applySteer({
        ...exactSteer,
        intent: {
          ...exactSteer.intent,
          intentId: "intent.grok.steer.body_ref_only",
          completionContractRef:
            "contract.pylon.fleet_steering_completion.grok_body_ref",
        },
        body: null,
        bodyRef: "private.body.ref.only",
      })).toEqual({
        state: "failed",
        failureRef: "blocker.pylon.fleet_steering.steer_body_unavailable",
      })

      expect(earlySteer).not.toBeNull()
      const first = earlySteer!
      const duplicate = port.applySteer(exactSteer)
      expect(await port.applySteer({
        ...exactSteer,
        body: "PRIVATE conflicting replay body",
      })).toEqual({
        state: "failed",
        failureRef: "blocker.pylon.fleet_steering.intent_replay_conflict",
      })
      await Bun.sleep(0)
      expect(followUps).toHaveLength(0)
      releaseInitial()
      expect(await first).toEqual({ state: "applied" })
      expect(await duplicate).toEqual({ state: "applied" })
      const result = await dispatched

      expect(followUps).toHaveLength(1)
      expect(followUps[0]).toMatchObject({
        prompt: privateBody,
        sessionId: initial.sessionId,
      })
      expect(verifierRuns).toBe(1)
      expect(order).toEqual(["initial", "follow_up", "verify"])
      expect(result.status).toBe("completed")
      expect(JSON.stringify(result)).not.toContain(privateBody)
      expect(JSON.stringify(result)).not.toContain(initial.sessionId!)
    } finally {
      releaseInitial?.()
      await rm(root, { force: true, recursive: true })
    }
  })

  test("fails the assignment and skips verification when the resumed Grok turn fails", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-steer-failure-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(
      parseBootstrapArgs(["--json"]),
      { PYLON_HOME: pylonHome },
    )
    const accountRef = "grok-steer-failure"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    const workingDirectory = join(root, "workspace")
    await mkdir(accountHome, { recursive: true })
    await mkdir(workingDirectory, { recursive: true })
    const request = dispatchFor(accountRef, 204, "fixture")
    let assignmentRef = ""
    let releaseInitial!: () => void
    const initialGate = new Promise<void>(resolve => {
      releaseInitial = resolve
    })
    let reportStarted!: () => void
    const started = new Promise<void>(resolve => {
      reportStarted = resolve
    })
    let verifierRuns = 0

    try {
      const port = createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        loadRegistry: async () => [accountFor(accountHome, accountRef)],
        createExecutor: () => ({
          ...successfulExecutor(),
          runClaimedWork: async input => {
            reportStarted()
            await initialGate
            return await successfulExecutor().runClaimedWork(input)
          },
          runFollowUp: async input => ({
            ok: false,
            claimRef: input.pin.claimRef,
            stopReason: "timeout",
            text: "PRIVATE failed follow-up output",
            failureClass: "timeout",
            usage: {
              metering: "not_measured",
              wallClockMs: 5,
              plane: "cli_session",
              marginalCostClass: "subscription",
            },
          }),
        }),
        materializeWorkspace: async () => ({
          checkout: null,
          verificationArgs: ["bun", "--version"],
          workingDirectory,
          workspaceRef: "workspace.public.grok.steer_failure",
        }),
        runVerifier: async () => {
          verifierRuns += 1
          return { exitCode: 0, timedOut: false }
        },
      })
      const dispatched = port.dispatch({
        ...request,
        onLifecycle: async event => {
          if (event.event === "assignment_run.runtime_started") {
            assignmentRef = event.assignmentRef ?? ""
          }
        },
      })
      await started
      const steered = port.applySteer({
        pylonRef: "pylon.public.grok.steer_failure",
        runRef: request.run.runRef,
        claimRef: "claim.sarah_fleet_run.grok.steer_failure",
        workUnitRef: request.workUnit.workUnitRef,
        workClaimRef: request.claim.claimRef,
        assignmentRef,
        intent: {
          seq: 18,
          intentId: "intent.grok.steer.failure",
          completionContractRef:
            "contract.pylon.fleet_steering_completion.grok_failure",
        },
        body: "PRIVATE failing steer",
        bodyRef: null,
      })
      releaseInitial()

      expect(await steered).toEqual({
        state: "failed",
        failureRef: "blocker.pylon.fleet_steering.grok_resume_failed",
      })
      const result = await dispatched
      expect(result).toMatchObject({
        status: "failed",
        lifecycle: [{
          blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionTimedOut],
        }],
      })
      expect(verifierRuns).toBe(0)
      expect(JSON.stringify(result)).not.toContain("PRIVATE")
    } finally {
      releaseInitial?.()
      await rm(root, { force: true, recursive: true })
    }
  })

  test("runs the built-in deterministic fixture verifier before promoting CLI success", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-fixture-verifier-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    const accountRef = "grok-a"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    await mkdir(accountHome, { recursive: true })

    try {
      const result = await createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        env: { PATH: process.env.PATH },
        loadRegistry: async () => [accountFor(accountHome, accountRef)],
        createExecutor: () => successfulExecutor(),
      }).dispatch(dispatchFor(accountRef, 101, "fixture"))

      expect(result).toMatchObject({
        status: "completed",
        verification: {
          truth: "passed",
          verifierRef: expect.stringMatching(/^verifier\.public\.pylon\.grok\./),
          evidenceRefs: [expect.stringMatching(/^verification\.public\.pylon\.grok\./)],
        },
      })
      expect(await readFile(
        join(
          summary.paths.cache,
          "grok-fleet-fixtures",
          result.artifactRefs[0]!,
          "grok-fixture-closeout.json",
        ),
        "utf8",
      )).toContain(dispatchFor(accountRef, 101, "fixture").claim.claimRef)
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
        materializeWorkspace: async () => {
          const workingDirectory = join(root, "workspace")
          await mkdir(workingDirectory, { recursive: true })
          return {
            checkout: null,
            verificationArgs: ["bun", "--version"],
            workingDirectory,
            workspaceRef: "workspace.public.grok.restart",
          }
        },
        runVerifier: async () => ({ exitCode: 0, timedOut: false }),
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
      const crossUnit = await fresh.reconcile({
        active: {
          ...active,
          claim: {
            ...active.claim,
            workUnitRef: "work_unit.grok.exact.foreign",
          },
        },
        now: fixedNow,
        runRef: run.runRef,
      })
      expect(crossUnit).toMatchObject({
        status: "failed",
        lifecycle: [{
          blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.receiptInvalid],
        }],
      })
      expect(await fresh.probeLiveness(one.assignmentRef!)).toBe("live")
    } finally {
      release?.()
      await rm(root, { force: true, recursive: true })
    }
  })

  test("serializes heartbeat delivery, joins it before terminal, and rejects invalid wall clock", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-grok-heartbeat-"))
    const pylonHome = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    const accountRef = "grok-a"
    const accountHome = join(pylonHome, "accounts", "grok", accountRef)
    await mkdir(accountHome, { recursive: true })
    const account = accountFor(accountHome, accountRef)
    let activeDeliveries = 0
    let maxActiveDeliveries = 0
    const delivered: string[] = []

    try {
      const port = createPylonOwnedGrokClaimedWorkPort({
        summary,
        lifecycleHeartbeatMs: 100,
        loadRegistry: async () => [account],
        createExecutor: () => ({
          ...successfulExecutor(),
          runClaimedWork: async input => {
            await Bun.sleep(250)
            return {
              ok: true,
              claimRef: input.pin.claimRef,
              stopReason: "end_turn",
              text: "",
              usage: {
                metering: "not_measured",
                wallClockMs: Number.NaN,
                plane: "cli_session",
                marginalCostClass: "subscription",
              },
            }
          },
        }),
        materializeWorkspace: async () => ({
          checkout: null,
          verificationArgs: null,
          workingDirectory: join(root, "workspace"),
          workspaceRef: "workspace.public.grok.heartbeat",
        }),
      })
      const result = await port.dispatch({
        ...dispatchFor(accountRef, 22, "fixture"),
        onLifecycle: async event => {
          activeDeliveries += 1
          maxActiveDeliveries = Math.max(maxActiveDeliveries, activeDeliveries)
          delivered.push(event.event)
          await Bun.sleep(150)
          activeDeliveries -= 1
        },
      })
      const deliveredAtTerminal = delivered.length

      expect(result).toMatchObject({
        status: "failed",
        lifecycle: [{ blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.executionFailed] }],
      })
      expect(delivered).toContain("assignment_run.runtime_started")
      expect(delivered).toContain("assignment_run.runtime_progress")
      expect(maxActiveDeliveries).toBe(1)
      await Bun.sleep(200)
      expect(delivered).toHaveLength(deliveredAtTerminal)
      const receiptFiles = await readdir(join(summary.paths.cache, "grok-fleet-receipts"))
      const terminalReceipt = JSON.parse(await readFile(
        join(summary.paths.cache, "grok-fleet-receipts", receiptFiles[0]!),
        "utf8",
      )) as { state?: unknown; wallClockMs?: unknown }
      expect(terminalReceipt).toMatchObject({ state: "failed", wallClockMs: null })
    } finally {
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
        materializeWorkspace: async request => {
          const workingDirectory = join(
            root,
            `workspace-${request.dispatch.accountRef}`,
          )
          await mkdir(workingDirectory, { recursive: true })
          return {
            checkout: null,
            verificationArgs: ["bun", "test"],
            workingDirectory,
            workspaceRef: `workspace.public.${request.dispatch.accountRef}`,
          }
        },
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

      let missingVerifierCalls = 0
      const missingVerifierWorkspace = join(root, "workspace-missing-verifier")
      await mkdir(missingVerifierWorkspace, { recursive: true })
      const missingVerifier = await createPylonOwnedGrokClaimedWorkPort({
        summary,
        now: () => fixedNow,
        loadRegistry: async () => [accountB],
        createExecutor: () => successfulExecutor(),
        materializeWorkspace: async () => ({
          checkout: null,
          verificationArgs: null,
          workingDirectory: missingVerifierWorkspace,
          workspaceRef: "workspace.public.grok.missing_verifier",
        }),
        runVerifier: async () => {
          missingVerifierCalls += 1
          return { exitCode: 0, timedOut: false }
        },
      }).dispatch(dispatchFor("grok-b", 42, "fixture"))
      expect(missingVerifier).toMatchObject({
        status: "failed",
        lifecycle: [{
          blockerRefs: [PYLON_OWNED_GROK_RUNNER_BLOCKERS.verificationFailed],
        }],
      })
      expect(missingVerifierCalls).toBe(0)

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
