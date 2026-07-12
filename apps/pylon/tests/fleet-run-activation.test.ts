import { Database } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import {
  openPylonNodeFleetRunActivationService,
  PYLON_NODE_MAX_ACTIVE_FLEET_RUNS,
  type PylonFleetRunExecutorOpener,
} from "../src/node/fleet-run-activation.js"
import {
  openPylonFleetRunRuntime,
  pylonFleetRunDatabasePath,
} from "../src/orchestration/fleet-run-runtime.js"
import type { FleetRunSupervisorObservedEvent } from "../src/orchestration/fleet-run-supervisor.js"
import type {
  PylonFleetRunExecutionBatch,
  PylonFleetRunExecutionHttpPort,
} from "../src/orchestration/fleet-run-execution-reporter.js"

const pylonRef = "pylon.public.fc2.node_activation"

const fixture = async <T>(run: (input: {
  root: string
  summary: ReturnType<typeof createBootstrapSummary>
}) => Promise<T>): Promise<T> => {
  const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-activation-"))
  const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), env)
  try {
    return await run({ root, summary })
  } finally {
    await rm(root, { force: true, recursive: true })
  }
}

const seedRuns = async (
  summary: ReturnType<typeof createBootstrapSummary>,
  runRefs: readonly string[],
): Promise<void> => {
  const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
  try {
    for (const runRef of runRefs) {
      runtime.store.createFleetRun({
        runRef,
        objective: `Bounded fixture for ${runRef}`,
        workSource: "fixture",
        targetConcurrency: 1,
        workerKind: "codex",
        state: "running",
      })
    }
  } finally {
    await runtime.close()
  }
}

const seedManagedCloudRun = async (
  summary: ReturnType<typeof createBootstrapSummary>,
  runRef: string,
): Promise<void> => {
  const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
  try {
    runtime.store.createFleetRun({
      runRef,
      objective: `Bounded managed-cloud fixture for ${runRef}`,
      workSource: "fixture",
      targetConcurrency: 1,
      workerKind: "codex",
      state: "running",
      authorityBinding: {
        schema: "openagents.pylon.fleet_run_authority_binding.v1",
        source: "sarah_authority",
        authorityFingerprint: "d".repeat(64),
        claimRef: `claim.sarah_fleet_run.${"1".repeat(24)}`,
        pylonRef,
        targetPreference: "managed_cloud",
        phase: "accepted",
      },
    })
  } finally {
    await runtime.close()
  }
}

const setRunState = async (
  summary: ReturnType<typeof createBootstrapSummary>,
  runRef: string,
  state: "running" | "paused" | "draining" | "stopped" | "completed",
): Promise<void> => {
  const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
  try {
    runtime.store.updateFleetRunState(runRef, state)
  } finally {
    await runtime.close()
  }
}

const openerFixture = () => {
  const opened: string[] = []
  const closed: string[] = []
  const opener: PylonFleetRunExecutorOpener = async (input) => {
    opened.push(input.runRef)
    return {
      close: async () => {
        closed.push(input.runRef)
      },
    }
  }
  return { closed, opened, opener }
}

describe("headless Pylon FleetRun activation", () => {
  test("blocks explicit managed-cloud authority without its opener and never substitutes owner-local execution", async () => {
    await fixture(async ({ summary }) => {
      const runRef = `fleet_run.sarah.${"3".repeat(20)}`
      await seedManagedCloudRun(summary, runRef)
      const ownerLocal = openerFixture()
      const blocked = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: ownerLocal.opener,
      })

      expect(await blocked.arm(runRef)).toEqual({
        schema: "openagents.pylon.fleet_run_activation.v1",
        pylonRef,
        runRef,
        armed: true,
        active: false,
        state: "armed_blocked",
        reason: "managed_cloud_executor_unconfigured",
        retryable: true,
      })
      expect(ownerLocal.opened).toEqual([])
      await blocked.close()

      const restartedOwnerLocal = openerFixture()
      const managedCloud = openerFixture()
      const configured = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: restartedOwnerLocal.opener,
        openManagedCloudExecutor: managedCloud.opener,
      })
      expect(restartedOwnerLocal.opened).toEqual([])
      expect(managedCloud.opened).toEqual([runRef])
      expect((await configured.status(runRef)).runs[0]).toMatchObject({
        runRef,
        armed: true,
        active: true,
        reason: null,
      })
      await configured.close()
    })
  })

  test("concurrent duplicate arm opens one handle and restart resumes only the durable armed run", async () => {
    await fixture(async ({ summary }) => {
      const armedRef = "fleet_run.fc2.armed"
      const inertRef = "fleet_run.fc2.inert"
      await seedRuns(summary, [armedRef, inertRef])
      const firstOpener = openerFixture()
      const first = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        agentToken: "must-not-project",
        openExecutor: firstOpener.opener,
      })

      const [left, right] = await Promise.all([first.arm(armedRef), first.arm(armedRef)])
      expect(left).toMatchObject({ runRef: armedRef, armed: true, active: true, state: "active" })
      expect(right).toEqual(left)
      expect(firstOpener.opened).toEqual([armedRef])
      const firstStatus = await first.status()
      expect(firstStatus).toMatchObject({
        pylonRef,
        activeRuns: 1,
        maxActiveRuns: PYLON_NODE_MAX_ACTIVE_FLEET_RUNS,
        invalidStoredRows: 0,
        blockerRefs: [],
      })
      expect(JSON.stringify(firstStatus)).not.toContain("openagents.test")
      expect(JSON.stringify(firstStatus)).not.toContain("must-not-project")
      expect(JSON.stringify(firstStatus)).not.toContain(summary.paths.home)
      await first.close()
      expect(firstOpener.closed).toEqual([armedRef])

      const secondOpener = openerFixture()
      const second = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: secondOpener.opener,
      })
      expect(secondOpener.opened).toEqual([armedRef])
      expect((await second.status()).runs).toEqual([
        expect.objectContaining({ runRef: armedRef, armed: true, active: true }),
      ])
      await second.disarm(armedRef)
      await second.close()

      const thirdOpener = openerFixture()
      const third = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: thirdOpener.opener,
      })
      expect(thirdOpener.opened).toEqual([])
      expect((await third.status(armedRef)).runs[0]).toMatchObject({
        runRef: armedRef,
        armed: false,
        active: false,
        state: "disarmed",
      })
      await third.close()
    })
  })

  test("keeps failed opens armed and retries only on another explicit arm", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.retry"
      await seedRuns(summary, [runRef])
      let fail = true
      let opens = 0
      const opener: PylonFleetRunExecutorOpener = async () => {
        opens += 1
        if (fail) throw new Error("private provider and local path details")
        return { close: () => Promise.resolve() }
      }
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test/private",
        agentToken: "private-token",
        openExecutor: opener,
      })
      const blocked = await service.arm(runRef)
      expect(blocked).toEqual({
        schema: "openagents.pylon.fleet_run_activation.v1",
        pylonRef,
        runRef,
        armed: true,
        active: false,
        state: "armed_blocked",
        reason: "executor_open_failed",
        retryable: true,
      })
      expect(JSON.stringify(await service.status())).not.toContain("private")

      fail = false
      expect(await service.arm(runRef)).toMatchObject({ armed: true, active: true, reason: null })
      expect(opens).toBe(2)
      await service.close()
    })
  })

  test("wires accepted managed-cloud lifecycle observations into the normal durable execution reporter", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.sarah.0123456789abcdefabcd"
      const claimRef = "claim.sarah_fleet_run.0123456789abcdef01234567"
      const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
      runtime.store.createFleetRun({
        runRef,
        objective: "Public-safe execution projection fixture",
        workSource: "fixture",
        targetConcurrency: 1,
        workerKind: "auto",
        state: "running",
        authorityBinding: {
          schema: "openagents.pylon.fleet_run_authority_binding.v1",
          source: "sarah_authority",
          authorityFingerprint: "a".repeat(64),
          claimRef,
          pylonRef,
          targetPreference: "managed_cloud",
          phase: "accepted",
        },
      })
      await runtime.close()

      const batches: PylonFleetRunExecutionBatch[] = []
      const executionRemote: PylonFleetRunExecutionHttpPort = {
        append: async ({ batch }) => {
          batches.push(batch)
          return {
            schema: "openagents.pylon.fleet_run_execution_ack.v1",
            runRef,
            claimRef,
            acceptedThroughSequence: batch.events.at(-1)?.sequence ?? 0,
            storedEventCount: batch.events.length,
            duplicateEventCount: 0,
            execution: {
              state: batch.events.some(event => event.kind === "run_terminal")
                ? "completed"
                : "running",
              lastSequence: batch.events.at(-1)?.sequence ?? 0,
              counters: {
                workUnitsTotal: 0,
                activeAssignments: 0,
                acceptedAssignments: 0,
                failedAssignments: 0,
                staleAssignments: 0,
              },
              startedAt: "2026-07-09T20:00:00.000Z",
              updatedAt: "2026-07-09T20:00:00.000Z",
              closeouts: [],
            },
          }
        },
      }
      let observe: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
      const opener: PylonFleetRunExecutorOpener = async input => {
        observe = input.onLifecycle
        return { close: () => Promise.resolve() }
      }
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        agentToken: "private-token",
        executionRemote,
        openExecutor: async () => {
          throw new Error("owner-local fallback must not run")
        },
        openManagedCloudExecutor: opener,
      })
      await service.arm(runRef)
      await observe?.({ kind: "completed", runRef, reason: "backlog_empty" })
      expect(batches.flatMap(batch => batch.events).map(event => event.kind)).toEqual([
        "run_started",
        "run_terminal",
      ])
      expect(JSON.stringify(batches)).not.toContain("private-token")
      await service.close()
    })
  })

  test("journals an accepted Sarah run locally when authenticated delivery is offline", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.sarah.1123456789abcdefabcd"
      const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
      runtime.store.createFleetRun({
        runRef,
        objective: "Public-safe offline execution journal fixture",
        workSource: "fixture",
        targetConcurrency: 1,
        workerKind: "auto",
        state: "running",
        authorityBinding: {
          schema: "openagents.pylon.fleet_run_authority_binding.v1",
          source: "sarah_authority",
          authorityFingerprint: "c".repeat(64),
          claimRef: "claim.sarah_fleet_run.1123456789abcdef01234567",
          pylonRef,
          targetPreference: "owner_local",
          phase: "accepted",
        },
      })
      await runtime.close()

      let observe: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: async input => {
          observe = input.onLifecycle
          return { close: () => Promise.resolve() }
        },
      })
      await service.arm(runRef)
      await observe?.({ kind: "completed", runRef, reason: "backlog_empty" })
      await service.close()

      const reopened = await openPylonFleetRunRuntime({ bootstrap: summary })
      try {
        expect(reopened.store.listFleetRunExecutionOutbox(runRef, {
          pendingOnly: true,
        }).map(entry => JSON.parse(entry.eventJson).kind)).toEqual([
          "run_started",
          "run_terminal",
        ])
      } finally {
        await reopened.close()
      }
    })
  })

  test("keeps an arm blocked instead of opening an executor without configured transport", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.no_transport"
      await seedRuns(summary, [runRef])
      const opener = openerFixture()
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        openExecutor: opener.opener,
      })
      expect(await service.arm(runRef)).toMatchObject({
        armed: true,
        active: false,
        reason: "transport_not_configured",
      })
      expect(opener.opened).toEqual([])
      await service.close()
    })
  })

  test("bounds opening plus active handles and fills an armed slot after authoritative disarm", async () => {
    await fixture(async ({ summary }) => {
      const firstRef = "fleet_run.fc2.limit_a"
      const secondRef = "fleet_run.fc2.limit_b"
      await seedRuns(summary, [firstRef, secondRef])
      const opener = openerFixture()
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        maxActiveRuns: 1,
        openExecutor: opener.opener,
      })
      expect(await service.arm(firstRef)).toMatchObject({ active: true })
      expect(await service.arm(secondRef)).toMatchObject({
        armed: true,
        active: false,
        reason: "active_limit_reached",
        retryable: true,
      })
      expect(opener.opened).toEqual([firstRef])

      expect(await service.disarm(firstRef)).toMatchObject({ armed: false, active: false })
      expect(opener.closed).toEqual([firstRef])
      expect(opener.opened).toEqual([firstRef, secondRef])
      expect((await service.status(secondRef)).runs[0]).toMatchObject({
        armed: true,
        active: true,
      })
      await service.close()
    })
  })

  test("a supervisor-terminal run is durably disarmed and frees its slot for the next armed run", async () => {
    await fixture(async ({ summary }) => {
      const firstRef = "fleet_run.fc2.terminal_a"
      const secondRef = "fleet_run.fc2.terminal_b"
      await seedRuns(summary, [firstRef, secondRef])
      const opened: string[] = []
      const closed: string[] = []
      const lifecycle = new Map<
        string,
        (event: FleetRunSupervisorObservedEvent) => void | Promise<void>
      >()
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: async input => {
          opened.push(input.runRef)
          if (input.onLifecycle !== undefined) lifecycle.set(input.runRef, input.onLifecycle)
          return {
            close: async () => {
              closed.push(input.runRef)
            },
          }
        },
      })
      expect(await service.arm(firstRef)).toMatchObject({ armed: true, active: true })
      expect(await service.arm(secondRef)).toMatchObject({
        armed: true,
        active: false,
        reason: "active_limit_reached",
      })

      await setRunState(summary, firstRef, "completed")
      await lifecycle.get(firstRef)?.({
        kind: "completed",
        runRef: firstRef,
        reason: "backlog_empty",
      })
      const status = await service.status()
      expect(opened).toEqual([firstRef, secondRef])
      expect(closed).toEqual([firstRef])
      expect(status.activeRuns).toBe(1)
      expect(status.runs).toEqual([
        expect.objectContaining({
          runRef: firstRef,
          armed: false,
          active: false,
          state: "disarmed",
          reason: null,
        }),
        expect.objectContaining({
          runRef: secondRef,
          armed: true,
          active: true,
          state: "active",
          reason: null,
        }),
      ])
      await service.close()
    })
  })

  test("a terminal lifecycle emitted during executor open reaps without deadlocking", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.terminal_during_open"
      await seedRuns(summary, [runRef])
      let closeAttempts = 0
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: async input => {
          await setRunState(summary, runRef, "completed")
          await input.onLifecycle?.({
            kind: "completed",
            runRef,
            reason: "backlog_empty",
          })
          return {
            close: async () => {
              closeAttempts += 1
            },
          }
        },
      })

      await service.arm(runRef)
      expect((await service.status(runRef)).runs[0]).toMatchObject({
        runRef,
        armed: false,
        active: false,
        state: "disarmed",
        reason: null,
      })
      expect(closeAttempts).toBe(1)
      await service.close()
    })
  })

  test("a terminal close failure stays counted but cannot resurrect after restart", async () => {
    await fixture(async ({ summary }) => {
      const failedRef = "fleet_run.fc2.terminal_close_failure"
      const waitingRef = "fleet_run.fc2.waiting_after_failure"
      await seedRuns(summary, [failedRef, waitingRef])
      let observe: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
      let closeAttempts = 0
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: async input => {
          if (input.runRef === failedRef) observe = input.onLifecycle
          return {
            close: async () => {
              if (input.runRef === failedRef) {
                closeAttempts += 1
                throw new Error("private terminal close failure")
              }
            },
          }
        },
      })
      await service.arm(failedRef)
      await service.arm(waitingRef)
      await setRunState(summary, failedRef, "stopped")
      await observe?.({
        kind: "terminal",
        runRef: failedRef,
        terminalState: "stopped",
        blockerRefs: [],
      })

      const blockedStatus = await service.status()
      expect(closeAttempts).toBeGreaterThan(0)
      expect(blockedStatus.activeRuns).toBe(1)
      expect(blockedStatus.runs).toEqual([
        expect.objectContaining({
          runRef: failedRef,
          armed: false,
          active: true,
          state: "disarmed_cleanup_blocked",
          reason: "executor_close_failed",
          retryable: false,
        }),
        expect.objectContaining({
          runRef: waitingRef,
          armed: true,
          active: false,
          reason: "active_limit_reached",
        }),
      ])
      await service.close()

      const restarted = openerFixture()
      const next = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: restarted.opener,
      })
      expect(restarted.opened).toEqual([waitingRef])
      expect((await next.status(failedRef)).runs[0]).toMatchObject({
        runRef: failedRef,
        armed: false,
        active: false,
      })
      await next.close()
    })
  })

  test("a newer terminal signal retries a failed in-flight reap once without spinning", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.terminal_signal_during_reap"
      await seedRuns(summary, [runRef])
      let observe: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
      let closeAttempts = 0
      let markFirstCloseStarted!: () => void
      let releaseFirstClose!: () => void
      const firstCloseStarted = new Promise<void>((resolve) => {
        markFirstCloseStarted = resolve
      })
      const firstCloseRelease = new Promise<void>((resolve) => {
        releaseFirstClose = resolve
      })
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: async input => {
          observe = input.onLifecycle
          return {
            close: async () => {
              closeAttempts += 1
              if (closeAttempts === 1) {
                markFirstCloseStarted()
                await firstCloseRelease
              }
              if (closeAttempts <= 2) throw new Error("private transient close failure")
            },
          }
        },
      })
      await service.arm(runRef)
      await setRunState(summary, runRef, "stopped")
      const terminalEvent: FleetRunSupervisorObservedEvent = {
        kind: "terminal",
        runRef,
        terminalState: "stopped",
        blockerRefs: [],
      }

      await observe?.(terminalEvent)
      await firstCloseStarted
      // This second authoritative signal arrives after the first scheduled
      // reap entered handle.close(). It must authorize exactly one follow-up
      // attempt even though the first close fails.
      await observe?.(terminalEvent)
      releaseFirstClose()

      for (let attempt = 0; attempt < 20 && closeAttempts < 2; attempt += 1) {
        await Bun.sleep(1)
      }
      expect(closeAttempts).toBe(2)
      // Both cleanup attempts failed. With no third lifecycle generation, the
      // scheduler must leave the failed handle counted instead of spinning.
      await Bun.sleep(5)
      expect(closeAttempts).toBe(2)
      await service.close()
    })
  })

  test("does not infer finality or remove restart intent from the durable run state alone", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.store_terminal_only"
      await seedRuns(summary, [runRef])
      const opener = openerFixture()
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: opener.opener,
      })
      await service.arm(runRef)
      await setRunState(summary, runRef, "stopped")
      expect((await service.status(runRef)).runs[0]).toMatchObject({
        runRef,
        armed: true,
        active: true,
        state: "active",
      })
      expect(opener.closed).toEqual([])
      await service.close()
    })
  })

  test("a stale terminal observation never disarms a run that is live again", async () => {
    for (const state of ["running", "paused", "draining"] as const) {
      await fixture(async ({ summary }) => {
        const runRef = `fleet_run.fc2.stale_terminal_${state}`
        await seedRuns(summary, [runRef])
        const opener = openerFixture()
        let observe: ((event: FleetRunSupervisorObservedEvent) => void | Promise<void>) | undefined
        const service = await openPylonNodeFleetRunActivationService({
          summary,
          pylonRef,
          baseUrl: "https://openagents.test",
          openExecutor: async input => {
            observe = input.onLifecycle
            return await opener.opener(input)
          },
        })
        await service.arm(runRef)
        await setRunState(summary, runRef, state)
        await observe?.({
          kind: "terminal",
          runRef,
          terminalState: "stopped",
          blockerRefs: [],
        })
        expect((await service.status(runRef)).runs[0]).toMatchObject({
          runRef,
          armed: true,
          active: true,
          state: "active",
        })
        expect(opener.closed).toEqual([])
        await service.close()
      })
    }
  })

  test("rejects a second node-level supervisor handle instead of overstating production capacity", async () => {
    await fixture(async ({ summary }) => {
      await expect(openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        maxActiveRuns: 2,
        openExecutor: openerFixture().opener,
      })).rejects.toThrow(
        "exactly one active run; parallelism belongs inside that run",
      )
    })
  })

  test("serializes arm then disarm and preserves disarm when handle cleanup fails", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.serial"
      await seedRuns(summary, [runRef])
      let releaseOpen!: () => void
      const opening = new Promise<void>((resolve) => {
        releaseOpen = resolve
      })
      let closeAttempts = 0
      const opener: PylonFleetRunExecutorOpener = async () => {
        await opening
        return {
          close: async () => {
            closeAttempts += 1
            throw new Error("private close failure")
          },
        }
      }
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: opener,
      })
      const arm = service.arm(runRef)
      const disarm = service.disarm(runRef)
      releaseOpen()
      expect(await arm).toMatchObject({ armed: true, active: true })
      expect(await disarm).toMatchObject({
        armed: false,
        active: true,
        state: "disarmed_cleanup_blocked",
        reason: "executor_close_failed",
      })
      expect(closeAttempts).toBe(1)
      await service.close()

      const restarted = openerFixture()
      const next = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: restarted.opener,
      })
      expect(restarted.opened).toEqual([])
      await next.close()
    })
  })

  test("migration creates activation authority and tampered or foreign rows fail closed", async () => {
    await fixture(async ({ summary }) => {
      await mkdir(summary.paths.home, { recursive: true })
      const database = new Database(pylonFleetRunDatabasePath(summary), { create: true })
      database.exec(`
        CREATE TABLE pylon_orchestration_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO pylon_orchestration_meta (key, value) VALUES ('schema_version', '3');
      `)
      database.close()

      const knownRef = "fleet_run.fc2.known"
      await seedRuns(summary, [knownRef])
      const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
      expect(() => runtime.store.setFleetRunActivation({
        pylonRef,
        runRef: "../../private token",
        armed: true,
      })).toThrow("bounded public-safe refs")
      runtime.store.setFleetRunActivation({ pylonRef, runRef: "fleet_run.fc2.missing", armed: true })
      runtime.store.setFleetRunActivation({
        pylonRef: "pylon.public.fc2.foreign",
        runRef: knownRef,
        armed: true,
      })
      expect(runtime.store.getFleetRunActivation(pylonRef, "fleet_run.fc2.missing")).toEqual({
        pylonRef,
        runRef: "fleet_run.fc2.missing",
        armed: true,
      })
      await runtime.close()
      const tamper = new Database(pylonFleetRunDatabasePath(summary))
      tamper.query(`
        INSERT INTO pylon_orchestration_fleet_run_activations
          (pylon_ref, run_ref, armed)
        VALUES
          ($pylonRef, $runRef, 1)
      `).run({ $pylonRef: pylonRef, $runRef: "../../private token" })
      tamper.close()

      const opener = openerFixture()
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        baseUrl: "https://openagents.test",
        openExecutor: opener.opener,
      })
      expect(opener.opened).toEqual([])
      const status = await service.status()
      expect(status.runs).toEqual([
        expect.objectContaining({
          runRef: "fleet_run.fc2.missing",
          armed: true,
          active: false,
          reason: "unknown_stored_run",
        }),
      ])
      expect(JSON.stringify(status)).not.toContain("private token")
      expect(JSON.stringify(status)).not.toContain("foreign")
      expect(status.invalidStoredRows).toBe(1)
      expect(status.blockerRefs).toEqual([
        "blocker.pylon.fleet_run_activation.invalid_stored_ref",
      ])
      await expect(service.arm("fleet_run.fc2.unknown")).rejects.toMatchObject({
        reason: "fleet_run_unknown",
      })
      await expect(service.arm("../../unsafe secret")).rejects.toMatchObject({
        reason: "fleet_run_ref_invalid",
      })
      await service.close()
    })
  })

  test("wraps canonical store failures in a fixed typed public-safe operational error", async () => {
    await fixture(async ({ summary }) => {
      const runRef = "fleet_run.fc2.store_failure"
      await seedRuns(summary, [runRef])
      const runtime = await openPylonFleetRunRuntime({ bootstrap: summary })
      const service = await openPylonNodeFleetRunActivationService({
        summary,
        pylonRef,
        openExecutor: openerFixture().opener,
        openRuntime: async () => runtime,
      })
      runtime.store.getFleetRun = () => {
        throw new Error(`private sqlite failure at ${summary.paths.home}`)
      }
      await expect(service.arm(runRef)).rejects.toMatchObject({
        message: "fleet run activation authority unavailable",
        reason: "fleet_run_activation_authority_unavailable",
      })
      await service.close()
    })
  })
})
