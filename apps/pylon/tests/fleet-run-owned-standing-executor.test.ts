import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { hashPylonAccountRef, type PylonAccountRegistryEntry } from "../src/account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import type {
  PylonKhalaCloseoutResult,
  PylonKhalaRequestInput,
} from "../src/khala-requester.js"
import {
  openPylonOwnedStandingFleetRunExecutor,
} from "../src/orchestration/fleet-run-owned-standing-executor.js"
import {
  openPylonFleetRunRuntime,
  PYLON_FLEET_RUN_DATABASE_FILENAME,
  type PylonFleetRunRuntime,
} from "../src/orchestration/fleet-run-runtime.js"
import {
  openPylonStandingFleetRunExecutor,
  PylonStandingFleetRunConstructionError,
  type OpenPylonStandingFleetRunExecutorInput,
} from "../src/orchestration/fleet-run-standing-executor.js"
import { planFixtureWork } from "../src/orchestration/work-planner.js"

const fixedNow = new Date("2026-07-09T22:00:00.000Z")
const pylonRef = "pylon.public.fc2.owned-standing"

const waitUntil = async (predicate: () => boolean): Promise<void> => {
  for (let attempt = 0; attempt < 2_000; attempt += 1) {
    if (predicate()) return
    await Bun.sleep(5)
  }
  throw new Error("timed out waiting for owned standing FleetRun closeout")
}

const registryAccount = (
  ref: string,
  home: string,
  provider: PylonAccountRegistryEntry["provider"] = "codex",
): PylonAccountRegistryEntry => ({
  ref,
  provider,
  home,
  openAgentsProviderAccountRef: `provider_account.public.${ref}`,
  hourlyCap: null,
  weeklyCap: null,
  manualResetsRemaining: null,
  marginalCostClass: "subscription",
})

const requestReceipt = (request: PylonKhalaRequestInput, assignmentRef: string) => ({
  assignmentRef,
  workflow: request.workflow ?? null,
  frames: [{
    data: "delegated",
    parsed: {
      openagents: {
        coding_delegation: {
          assignmentRef,
          pylonRef: request.targetPylonRef,
          workflowClass: request.workflow,
        },
      },
    },
  }],
})

const exactCloseoutReceipt = async (assignmentRef: string): Promise<PylonKhalaCloseoutResult> => {
  const claude = assignmentRef.includes("claude")
  const tokenUsage = {
    cacheReadTokens: 1,
    demandKind: "own_capacity" as const,
    demandSource: "khala_coding_delegation" as const,
    inputTokens: 5,
    model: claude ? "openagents/pylon-claude" as const : "openagents/pylon-codex" as const,
    outputTokens: 3,
    provider: claude ? "pylon-claude-own-capacity" as const : "pylon-codex-own-capacity" as const,
    reasoningTokens: 1,
    refs: [`token_usage_event.public.${assignmentRef}`],
    rowCount: 1,
    totalTokens: 8,
    usageTruth: "exact" as const,
  }
  const policy = {
    paymentMode: "no-spend" as const,
    payoutClaimAllowed: false,
    settlementState: "not_applicable" as const,
    source: "worker_closeout_event" as const,
  }
  const owner = { agentUserRef: "agent:owner", openauthUserRef: "user:owner" }
  const checklist = {
    blockerRefs: [],
    items: [{ ok: true, ref: "check.public.fixture" }],
    ok: true,
  }
  const workerCloseout = {
    artifactRefs: [`artifact.public.${assignmentRef}`],
    authorityReceiptRefs: [`receipt.public.${assignmentRef}`],
    closeoutRefs: [`closeout.public.${assignmentRef}`],
    eventRef: `event.public.worker_closeout.${assignmentRef}`,
    observedAt: fixedNow.toISOString(),
    projectionBlockerRefs: [],
    proofRefs: [`proof.public.${assignmentRef}`],
    resultRefs: [`result.public.${assignmentRef}`],
    source: "worker_closeout_event" as const,
    status: "closeout_submitted",
    testRefs: [`test.public.${assignmentRef}`],
    verificationRefs: [`test.public.${assignmentRef}`],
    visibility: "owner_only" as const,
  }
  return {
    assignmentRef,
    closeoutChecklist: {
      ...checklist,
      caveatRefs: [],
      schema: "openagents.pylon.khala_closeout_checklist.v0.1",
    },
    ok: true,
    proof: {
      assignmentRef,
      closeoutPolicy: policy,
      owner,
      pylonRef,
      proofChecklist: {
        ...checklist,
        schema: "openagents.pylon.khala_proof_checklist.v0.1",
      },
      tokenUsage,
      workerCloseout,
    },
    schema: "openagents.pylon.khala_closeout.v0.1",
    status: {
      assignmentRef,
      closeoutPolicy: policy,
      lifecycle: {
        closeoutRefs: [`closeout.public.${assignmentRef}`],
        proofRefs: [`proof.public.${assignmentRef}`],
      },
      owner,
      progress: {
        closeoutReady: true,
        hasTokenUsage: true,
        state: "closed_out",
      },
      pylonRef,
      tokenUsage: { ...tokenUsage, status: "recorded" },
      workerCloseout,
    },
  } as PylonKhalaCloseoutResult
}

describe("canonical Pylon-owned standing FleetRun composition", () => {
  test("dispatches one mixed standing wave across exact Codex, Claude, and Grok accounts", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-owned-standing-three-harness-"))
    const pylonHome = join(root, "pylon-home")
    const env = { PYLON_HOME: pylonHome } as NodeJS.ProcessEnv
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), env)
    const runRef = "fleet_run.fc2.three_harness"
    const accounts = [
      registryAccount("codex-a", join(pylonHome, "accounts", "codex", "codex-a"), "codex"),
      registryAccount("claude-a", join(pylonHome, "accounts", "claude_agent", "claude-a"), "claude_agent"),
      registryAccount("grok-a", join(pylonHome, "accounts", "grok", "grok-a"), "grok"),
    ]
    for (const account of accounts) await mkdir(account.home, { recursive: true })
    const requestKinds: string[] = []
    const assignmentKinds: string[] = []
    let grokRuns = 0

    try {
      const seed = await openPylonFleetRunRuntime({ bootstrap: summary, now: () => fixedNow })
      seed.store.createFleetRun({
        runRef,
        objective: "Run one deterministic fixture on every named local harness.",
        workSource: "fixture",
        workSourceDescriptor: {
          schema: "openagents.pylon.fleet_run_work_source.v1",
          kind: "fixture",
          units: [
            { ref: "three-harness.one", title: "Three harness one" },
            { ref: "three-harness.two", title: "Three harness two" },
            { ref: "three-harness.three", title: "Three harness three" },
          ],
        },
        targetConcurrency: 3,
        workerKind: "auto",
        state: "running",
        now: fixedNow,
      })
      await seed.close()

      const standing = await openPylonOwnedStandingFleetRunExecutor({
        summary,
        env,
        now: () => fixedNow,
        baseUrl: "https://openagents.test",
        pylonRef,
        runRef,
        clock: { now: () => fixedNow, sleep: () => new Promise<void>(() => {}) },
        startImmediately: false,
        options: {
          loadRegistry: async () => accounts,
          capacity: {
            advertisedSlotsForAccount: () => 1,
            loadUsage: async () => ({
              schema: "openagents.pylon.account_usage_store.v0.3",
              accounts: {},
              updatedAt: fixedNow.toISOString(),
            }),
            probeReadiness: async () => "ready",
          },
          runner: {
            readCloseout: exactCloseoutReceipt,
            request: async request => {
              requestKinds.push(request.workflow)
              return requestReceipt(request, `assignment.public.${request.workflow}`)
            },
            runAssignment: async request => {
              const provider = request.accountRef.startsWith("claude") ? "claude_agent" : "codex"
              assignmentKinds.push(provider)
              const accountRefHash = hashPylonAccountRef(provider, request.accountRef)
              return {
                accountRefHash,
                assignmentRef: request.assignmentRef,
                closeout: {
                  paymentMode: "no-spend",
                  payoutClaimAllowed: false,
                  settlementState: "not_applicable",
                  status: "accepted",
                },
                lifecycle: [{
                  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
                  event: "assignment_run.completed",
                  observedAt: fixedNow.toISOString(),
                  assignmentRef: request.assignmentRef,
                  accountRefHash,
                  status: "closed",
                }],
                ok: true,
              }
            },
          },
          grok: {
            createExecutor: ({ env: grokEnv }) => {
              expect(grokEnv.GROK_HOME).toBe(accounts[2]!.home)
              return {
                kind: "grok_cli",
                readiness: async () => ({
                  ready: true,
                  binary: "grok",
                  plane: "cli_session",
                  models: ["grok-code-fast-1"],
                }),
                runClaimedWork: async input => {
                  grokRuns += 1
                  return {
                    ok: true,
                    claimRef: input.pin.claimRef,
                    stopReason: "end_turn",
                    text: "private local Grok fixture output",
                    usage: {
                      metering: "not_measured",
                      wallClockMs: 10,
                      plane: "cli_session",
                      marginalCostClass: "subscription",
                    },
                  }
                },
              }
            },
          },
        },
      })

      try {
        await waitUntil(() =>
          standing.runtime.store.listWorkClaims({ runRef }).filter(claim => claim.state === "closeout").length === 3
        )
        const claims = standing.runtime.store.listWorkClaims({ runRef })
        expect(claims).toHaveLength(3)
        expect(new Set(claims.map(claim => claim.workerAccountRef))).toEqual(
          new Set(["codex-a", "claude-a", "grok-a"]),
        )
        expect(claims.filter(claim => claim.assignmentRef?.startsWith("assignment.pylon.grok.")).length).toBe(1)
        expect(requestKinds.sort()).toEqual(["claude_agent_task", "codex_agent_task"])
        expect(assignmentKinds.sort()).toEqual(["claude_agent", "codex"])
        expect(grokRuns).toBe(1)
        expect(standing.runtime.store.listDispatchContexts().map(context => context.runnerKind).sort()).toEqual([
          "claude_agent",
          "codex",
          "grok_cli",
        ])
      } finally {
        await standing.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  }, 20_000)

  test("reopens one persisted descriptor and dispatches it once through one named isolated account", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-owned-standing-composition-"))
    const pylonHome = join(root, "pylon-home")
    const defaultCodexHome = join(root, "default-codex-home")
    const defaultClaudeHome = join(root, "default-claude-home")
    const isolatedHome = join(root, "accounts", "codex", "codex-isolated")
    const env = {
      PYLON_HOME: pylonHome,
      CODEX_HOME: defaultCodexHome,
    } as NodeJS.ProcessEnv
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), env)
    const runRef = "fleet_run.fc2.owned_standing"
    const workUnitRef = "fixture:owned-standing.unit"
    const assignmentRef = "assignment.public.fc2.owned_standing"
    const account = registryAccount("codex-isolated", isolatedHome)

    try {
      const seed = await openPylonFleetRunRuntime({ bootstrap: summary, now: () => fixedNow })
      seed.store.createFleetRun({
        runRef,
        objective: "Run one bounded Pylon-owned standing fixture.",
        workSource: "fixture",
        workSourceDescriptor: {
          schema: "openagents.pylon.fleet_run_work_source.v1",
          kind: "fixture",
          units: [{ ref: "owned-standing.unit", title: "Owned standing unit" }],
        },
        targetConcurrency: 1,
        workerKind: "codex",
        state: "running",
        now: fixedNow,
      })
      await seed.close()

      const registryReads: PylonAccountRegistryEntry[][] = []
      let usageReads = 0
      const readinessHomes: string[] = []
      const requests: PylonKhalaRequestInput[] = []
      const assignmentRuns: Array<{ accountRef: string; assignmentRef: string }> = []
      const standing = await openPylonOwnedStandingFleetRunExecutor({
        summary,
        env,
        now: () => fixedNow,
        baseUrl: "https://openagents.test",
        pylonRef,
        runRef,
        clock: {
          now: () => fixedNow,
          sleep: () => new Promise<void>(() => {}),
        },
        startImmediately: false,
        options: {
          defaultHomes: {
            codex: defaultCodexHome,
            claudeAgent: defaultClaudeHome,
          },
          loadRegistry: async () => {
            registryReads.push([account])
            return [account]
          },
          capacity: {
            advertisedSlotsForAccount: () => 1,
            loadUsage: async () => {
              usageReads += 1
              return {
                schema: "openagents.pylon.account_usage_store.v0.3",
                accounts: {},
                updatedAt: fixedNow.toISOString(),
              }
            },
            probeReadiness: async input => {
              readinessHomes.push(input.account.home)
              return "ready"
            },
          },
          runner: {
            readCloseout: exactCloseoutReceipt,
            request: async request => {
              requests.push(request)
              return requestReceipt(request, assignmentRef)
            },
            runAssignment: async request => {
              assignmentRuns.push(request)
              const accountRefHash = hashPylonAccountRef("codex", request.accountRef)
              return {
                accountRefHash,
                assignmentRef: request.assignmentRef,
                closeout: {
                  paymentMode: "no-spend",
                  payoutClaimAllowed: false,
                  settlementState: "not_applicable",
                  status: "accepted",
                },
                lifecycle: [{
                  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
                  event: "assignment_run.runtime_started",
                  observedAt: fixedNow.toISOString(),
                  assignmentRef: request.assignmentRef,
                  accountRefHash,
                }, {
                  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
                  event: "assignment_run.completed",
                  observedAt: fixedNow.toISOString(),
                  assignmentRef: request.assignmentRef,
                  status: "accepted",
                }],
                ok: true,
              }
            },
          },
        },
      })

      try {
        await waitUntil(() =>
          standing.runtime.store.listWorkClaims({ runRef })
            .some(claim => claim.assignmentRef === assignmentRef && claim.state === "closeout")
        )
        expect(standing.runtime.databasePath).toBe(join(pylonHome, PYLON_FLEET_RUN_DATABASE_FILENAME))
        expect(standing.recovery).toMatchObject({
          inspectedAssignments: 0,
          recoveredAssignments: 0,
        })
        expect(standing.runtime.store.getFleetRun(runRef)?.workSourceDescriptor).toEqual({
          schema: "openagents.pylon.fleet_run_work_source.v1",
          kind: "fixture",
          units: [{ ref: "owned-standing.unit", title: "Owned standing unit" }],
        })
        expect(registryReads).toHaveLength(2)
        expect(usageReads).toBe(1)
        expect(readinessHomes).toEqual([isolatedHome])
        expect(requests).toHaveLength(1)
        expect(requests[0]).toMatchObject({
          targetAccountRefHash: hashPylonAccountRef("codex", account.ref),
          targetPylonRef: pylonRef,
          workflow: "codex_agent_task",
        })
        expect(requests[0]?.workspace).toBeUndefined()
        expect(assignmentRuns).toHaveLength(1)
        expect(assignmentRuns[0]).toMatchObject({ accountRef: account.ref, assignmentRef })
        expect(typeof (assignmentRuns[0] as { onLifecycle?: unknown }).onLifecycle).toBe("function")

        const claims = standing.runtime.store.listWorkClaims({ runRef })
        expect(claims).toHaveLength(1)
        expect(claims[0]).toMatchObject({
          assignmentRef,
          state: "closeout",
          workUnitRef,
          workerAccountRef: account.ref,
        })
        expect(JSON.stringify([requests, assignmentRuns, claims])).not.toContain(defaultCodexHome)
        expect(JSON.stringify([requests, assignmentRuns, claims])).not.toContain(defaultClaudeHome)
      } finally {
        await standing.close()
      }

      const reopened = await openPylonFleetRunRuntime({ bootstrap: summary, now: () => fixedNow })
      try {
        expect(reopened.store.listWorkClaims({ runRef })).toHaveLength(1)
        expect(reopened.store.getFleetRun(runRef)?.workSourceDescriptor?.kind).toBe("fixture")
      } finally {
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("rejects conflicting adapter construction before opening a runtime", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-standing-invalid-config-"))
    const pylonHome = join(root, "pylon-home")
    const env = { PYLON_HOME: pylonHome } as NodeJS.ProcessEnv
    const invalid = {
      env,
      pylonRef,
      runRef: "fleet_run.fc2.invalid_config",
      adapterFactory: () => {
        throw new Error("must not run")
      },
      capacity: { accounts: async () => [] },
      livenessProbe: () => "unknown" as const,
      planner: {
        plan: ({ now }: { now: Date }) => Promise.resolve(planFixtureWork({ kind: "fixture", count: 1 }, { now })),
      },
      runner: {
        dispatch: async () => ({ assignmentRef: null, lifecycle: [], status: "blocked" as const }),
      },
    } as unknown as OpenPylonStandingFleetRunExecutorInput

    try {
      await expect(openPylonStandingFleetRunExecutor(invalid)).rejects.toMatchObject({
        failure: "invalid_adapter_config",
      })
      expect(existsSync(join(pylonHome, PYLON_FLEET_RUN_DATABASE_FILENAME))).toBe(false)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("closes the one opened runtime when adapter construction or resume fails", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-standing-construction-close-"))
    const pylonHome = join(root, "pylon-home")
    const env = { PYLON_HOME: pylonHome } as NodeJS.ProcessEnv
    let failedFactoryRuntime: PylonFleetRunRuntime | null = null
    let resumeFailureRuntime: PylonFleetRunRuntime | null = null

    try {
      await expect(openPylonStandingFleetRunExecutor({
        env,
        pylonRef,
        runRef: "fleet_run.fc2.factory_failure",
        adapterFactory: context => {
          expect(context.store).toBe(context.runtime.store)
          failedFactoryRuntime = context.runtime
          throw new Error("raw private factory failure")
        },
      })).rejects.toMatchObject({ failure: "adapter_factory_failed" })
      expect(failedFactoryRuntime).not.toBeNull()
      await expect(failedFactoryRuntime!.manager.status()).rejects.toThrow("fleet run manager is closed")

      await expect(openPylonStandingFleetRunExecutor({
        env,
        pylonRef,
        runRef: "fleet_run.fc2.unknown_resume",
        adapterFactory: context => {
          expect(context.store).toBe(context.runtime.store)
          resumeFailureRuntime = context.runtime
          return {
            capacity: { accounts: async () => [] },
            livenessProbe: () => "unknown",
            planner: {
              plan: ({ now }) => Promise.resolve(planFixtureWork({ kind: "fixture", count: 1 }, { now })),
            },
            runner: {
              dispatch: async () => ({ assignmentRef: null, lifecycle: [], status: "blocked" }),
            },
          }
        },
      })).rejects.toThrow("unknown fleet run")
      expect(resumeFailureRuntime).not.toBeNull()
      await expect(resumeFailureRuntime!.manager.status()).rejects.toThrow("fleet run manager is closed")
    } finally {
      await failedFactoryRuntime?.close()
      await resumeFailureRuntime?.close()
      await rm(root, { force: true, recursive: true })
    }
  })

  test("validates adapter factory output before recovery", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-standing-invalid-factory-result-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    let runtime: PylonFleetRunRuntime | null = null
    try {
      await expect(openPylonStandingFleetRunExecutor({
        env,
        pylonRef,
        runRef: "fleet_run.fc2.invalid_factory_result",
        adapterFactory: context => {
          runtime = context.runtime
          return {} as never
        },
      })).rejects.toBeInstanceOf(PylonStandingFleetRunConstructionError)
      expect(runtime).not.toBeNull()
      await expect(runtime!.manager.status()).rejects.toThrow("fleet run manager is closed")
    } finally {
      await runtime?.close()
      await rm(root, { force: true, recursive: true })
    }
  })
})
