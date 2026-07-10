import { describe, expect, test } from "bun:test"

import type { SarahFleetOwnerProjection } from "../contracts/fleet-owner-projection.ts"
import {
  SarahFleetBrowserHostError,
  makeSarahFleetBrowserCommands,
  makeSarahFleetBrowserCoordinator,
  makeSarahFleetBrowserRuntime,
  parseSarahFleetBrowserConfig,
  type SarahFleetBrowserRuntime,
  type SarahFleetBrowserViewState,
} from "./fleet-browser-host.ts"
import type {
  SarahFleetConnectionState,
  SarahFleetLiveSession,
} from "./fleet-sync-live-session.ts"

const config = parseSarahFleetBrowserConfig(
  "https://openagents.com/sarah?fleet_run=fleet.fc3.run",
)!
const commandAttemptRef = "attempt.fc3.codex"
const commandWorkUnitRef = "unit.fc3.codex"
const commandWorkerRef = "worker.fc3.codex"
const commandAssignmentRef = "assignment.fc3.codex"
const commandEventRef = `event.pylon.fleet_run.${"a".repeat(24)}`
const commandApprovalRef = "approval.fc3.codex"

const commandProjection = (input?: Readonly<{
  latestAttemptRef?: string
  attemptState?: "running" | "failed"
}>): SarahFleetOwnerProjection => {
  const latestAttemptRef = input?.latestAttemptRef ?? commandAttemptRef
  const attemptState = input?.attemptState ?? "running"
  return {
    run: { runRef: config.runRef },
    workUnits: [
      {
        workUnitRef: commandWorkUnitRef,
        state: attemptState,
        latestAttemptRef,
        approvalRefs:
          latestAttemptRef === commandAttemptRef && attemptState === "running"
            ? [commandApprovalRef]
            : [],
        attempts: [
          {
            attemptRef: commandAttemptRef,
            workUnitRef: commandWorkUnitRef,
            assignmentRef: null,
            workerRef: commandWorkerRef,
            state: attemptState,
            progressClass: "blocked",
            capacity: { accountRefHash: null },
            approvalRefs: [commandApprovalRef],
          },
        ],
      },
    ],
    approvals: [
      {
        approvalRef: commandApprovalRef,
        status: "pending",
        bindingStatus: "exact",
        runRef: config.runRef,
        workUnitRef: commandWorkUnitRef,
        attemptRef: commandAttemptRef,
        assignmentRef: null,
        workerRef: commandWorkerRef,
        accountRefHash: null,
        requestEventRef: commandEventRef,
        availableDecisions: ["allow", "deny"],
      },
    ],
  } as unknown as SarahFleetOwnerProjection
}

describe("Sarah exact-run browser host", () => {
  test("keeps no-scope unchanged and accepts unrelated URL parameters", () => {
    expect(
      parseSarahFleetBrowserConfig(
        "https://openagents.com/sarah?panel=blueprint&campaign=sarah",
      ),
    ).toBeNull()
    const selected = parseSarahFleetBrowserConfig(
      "https://openagents.com/sarah?panel=fleet&fleet_run=fleet.fc3.run",
    )
    expect(selected?.runRef).toBe("fleet.fc3.run")
    expect(String(selected?.scope)).toBe("scope.fleet_run.fleet.fc3.run")
  })

  test("rejects duplicate, malformed, and alternate run/scope claims", () => {
    for (const url of [
      "https://openagents.com/sarah?fleet_run=fleet.a&fleet_run=fleet.b",
      "https://openagents.com/sarah?fleet_run=not%20a%20ref",
      "https://openagents.com/sarah?fleet_run=fleet.a&fleet_scope=scope.fleet_run.fleet.b",
      "https://openagents.com/sarah?runRef=fleet.a",
    ]) {
      expect(() => parseSarahFleetBrowserConfig(url)).toThrow(
        SarahFleetBrowserHostError,
      )
    }
  })

  test("submits exact monotone intents and suppresses same-cursor duplicates", async () => {
    let cursor = 7
    let serial = 0
    const pushes: Array<Record<string, unknown>> = []
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => commandProjection(),
      cursor: () => cursor,
      randomId: () => `browsercommand${++serial}`,
      now: () => "2026-07-09T20:00:00.000Z",
      client: {
        submitIntent: async (input) => {
          pushes.push(input)
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "applied",
            lastMutationId: input.mutationId,
          }
        },
      },
    })

    const first = commands.runControl({ runRef: config.runRef, action: "pause" })
    const duplicate = commands.runControl({ runRef: config.runRef, action: "pause" })
    expect(first).toBe(duplicate)
    expect(await first).toMatchObject({ mutationId: 1, status: "applied" })
    expect(await commands.runControl({ runRef: config.runRef, action: "pause" }))
      .toMatchObject({ mutationId: 1 })
    expect(pushes).toHaveLength(1)
    expect(pushes[0]).toMatchObject({
      scope: config.scope,
      mutationId: 1,
      intent: {
        kind: "fleet_run_control",
        runRef: config.runRef,
        action: "pause",
      },
    })

    cursor += 1
    expect(await commands.runControl({ runRef: config.runRef, action: "pause" }))
      .toMatchObject({ mutationId: 2 })
  })

  test("retries the same idempotent mutation and never echoes private steer bodies", async () => {
    const privateBody = "PRIVATE steer body that must never be echoed"
    let calls = 0
    let cursor = 11
    const mutationIds: number[] = []
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => commandProjection(),
      cursor: () => cursor,
      randomId: () => "browsercommandretry",
      client: {
        submitIntent: async (input) => {
          calls += 1
          mutationIds.push(input.mutationId)
          if (calls === 1) throw new Error(privateBody)
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "duplicate",
            lastMutationId: input.mutationId,
          }
        },
      },
    })

    const failure = await commands
      .steer({
        runRef: config.runRef,
        targetRef: commandAttemptRef,
        body: privateBody,
      })
      .catch((error: unknown) => error)
    expect(JSON.stringify(failure)).not.toContain(privateBody)
    cursor = 12
    const receipt = await commands.steer({
      runRef: config.runRef,
      targetRef: commandAttemptRef,
      body: privateBody,
    })
    expect(receipt).toMatchObject({ mutationId: 1, status: "duplicate" })
    expect(JSON.stringify(receipt)).not.toContain(privateBody)
    expect(mutationIds).toEqual([1, 1])
  })

  test("does not alias distinct inline steer bodies at one cursor and target", async () => {
    let serial = 0
    const bodies: string[] = []
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => commandProjection(),
      cursor: () => 14,
      randomId: () => `browsersteer${++serial}`,
      client: {
        submitIntent: async (input) => {
          if (input.intent.kind === "steer_message") {
            bodies.push(input.intent.body ?? "")
          }
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "applied",
            lastMutationId: input.mutationId,
          }
        },
      },
    })
    await commands.steer({
      runRef: config.runRef,
      targetRef: commandAttemptRef,
      body: "First private steer",
    })
    await commands.steer({
      runRef: config.runRef,
      targetRef: commandAttemptRef,
      body: "Second private steer",
    })
    expect(bodies).toEqual(["First private steer", "Second private steer"])
  })

  test("allows only the exact current pending approval, including a nullable assignment", async () => {
    const pushes: Array<Record<string, unknown>> = []
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => commandProjection(),
      cursor: () => 16,
      randomId: () => "browserapproval",
      client: {
        submitIntent: async (input) => {
          pushes.push(input)
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "applied",
            lastMutationId: input.mutationId,
          }
        },
      },
    })
    await commands.approvalDecision({
      runRef: config.runRef,
      approvalRef: "approval.fc3.codex",
      decision: "allow",
    })
    expect(pushes[0]).toMatchObject({
      intent: {
        kind: "approval_decision",
        runRef: config.runRef,
        approvalRef: "approval.fc3.codex",
        decision: "allow",
      },
    })
    expect(() =>
      commands.approvalDecision({
        runRef: "fleet.foreign.run",
        approvalRef: "approval.fc3.codex",
        decision: "deny",
      }),
    ).toThrow(SarahFleetBrowserHostError)
    expect(pushes).toHaveLength(1)
  })

  test("rejects work-unit, worker, assignment, and stale attempt steer targets before push", async () => {
    const pushes: Array<Record<string, unknown>> = []
    let projection = commandProjection()
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => projection,
      cursor: () => 20,
      randomId: () => "browserinvalidtarget",
      client: {
        submitIntent: async (input) => {
          pushes.push(input)
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "applied",
            lastMutationId: input.mutationId,
          }
        },
      },
    })
    for (const targetRef of [
      commandWorkUnitRef,
      commandWorkerRef,
      commandAssignmentRef,
    ]) {
      const failure = await commands
        .steer({ runRef: config.runRef, targetRef, body: "Private steer" })
        .catch((error: unknown) => error)
      expect(failure).toBeInstanceOf(SarahFleetBrowserHostError)
      expect((failure as SarahFleetBrowserHostError).reason).toBe(
        "invalid_command_target",
      )
    }
    projection = commandProjection({ latestAttemptRef: "attempt.fc3.retry" })
    const stale = await commands
      .steer({
        runRef: config.runRef,
        targetRef: commandAttemptRef,
        body: "Private stale steer",
      })
      .catch((error: unknown) => error)
    expect(stale).toBeInstanceOf(SarahFleetBrowserHostError)
    expect(pushes).toEqual([])
  })

  test("rechecks projection after a reconnect race before approval push", async () => {
    let projection = commandProjection()
    let pushes = 0
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => projection,
      cursor: () => {
        projection = commandProjection({
          latestAttemptRef: "attempt.fc3.retry",
        })
        return 21
      },
      randomId: () => "browserreconnectrace",
      client: {
        submitIntent: async (input) => {
          pushes += 1
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "applied",
            lastMutationId: input.mutationId,
          }
        },
      },
    })
    const failure = await commands
      .approvalDecision({
        runRef: config.runRef,
        approvalRef: commandApprovalRef,
        decision: "allow",
      })
      .catch((error: unknown) => error)
    expect(failure).toBeInstanceOf(SarahFleetBrowserHostError)
    expect((failure as SarahFleetBrowserHostError).reason).toBe(
      "invalid_command_target",
    )
    expect(pushes).toBe(0)
  })

  test("bounds settled command retention at 256 entries", async () => {
    let cursor = 0
    let serial = 0
    let pushes = 0
    const commands = makeSarahFleetBrowserCommands({
      config,
      projection: () => commandProjection(),
      cursor: () => cursor,
      randomId: () => `browserbound${++serial}`,
      client: {
        submitIntent: async (input) => {
          pushes += 1
          return {
            intentId: input.intent.intentId,
            mutationId: input.mutationId,
            status: "applied",
            lastMutationId: input.mutationId,
          }
        },
      },
    })
    for (cursor = 0; cursor < 257; cursor += 1) {
      await commands.runControl({ runRef: config.runRef, action: "pause" })
    }
    cursor = 0
    const replayAfterEviction = await commands.runControl({
      runRef: config.runRef,
      action: "pause",
    })
    expect(replayAfterEviction.mutationId).toBe(258)
    expect(pushes).toBe(258)
  })

  test("hydrates, surfaces reconnect, and fails closed on a foreign projection", () => {
    let stateListener: (state: SarahFleetConnectionState) => void = () => {}
    let projectionListener: (projection: SarahFleetOwnerProjection) => void = () => {}
    let disposed = 0
    const session: SarahFleetLiveSession = {
      start: async () => {},
      refresh: async () => {},
      dispose: () => {
        disposed += 1
      },
      snapshot: () => ({ phase: "idle" }),
      projection: () => null,
      subscribe: (listener) => {
        stateListener = listener
        listener({ phase: "idle" })
        return () => {
          stateListener = () => {}
        }
      },
      subscribeProjection: (listener) => {
        projectionListener = listener
        return () => {
          projectionListener = () => {}
        }
      },
    }
    const runtime = makeSarahFleetBrowserRuntime({
      config,
      origin: "https://openagents.com",
      fetch: async () => new Response(null, { status: 500 }),
      randomId: () => "browserruntime",
      makeSession: () => session,
    })
    const observed: SarahFleetBrowserViewState[] = []
    runtime.subscribe((state) => observed.push(state))
    const ownerProjection = {
      run: { runRef: config.runRef },
    } as SarahFleetOwnerProjection
    projectionListener(ownerProjection)
    expect(runtime.snapshot().projection).toBe(ownerProjection)
    stateListener({
      phase: "reconnecting",
      scope: config.scope,
      cursor: 4 as never,
      attempt: 1,
      retryAtMs: 100,
      mustRefetchReason: null,
      error: {
        reason: "network_unavailable",
        messageSafe: "Fleet connection is temporarily unavailable.",
        retryable: true,
      },
    })
    expect(observed.at(-1)?.connection.phase).toBe("reconnecting")

    projectionListener({
      run: { runRef: "fleet.foreign.run" },
    } as SarahFleetOwnerProjection)
    expect(runtime.snapshot().connection).toMatchObject({
      phase: "failed",
      error: { reason: "foreign_state" },
    })
    expect(runtime.snapshot().projection).toBeNull()
    expect(disposed).toBe(1)
  })

  test("coordinator disposes before exact scope replacement and on shutdown", () => {
    const events: string[] = []
    const makeRuntime = (selected: typeof config): SarahFleetBrowserRuntime => ({
      config: selected,
      start: async () => {
        events.push(`start:${selected.runRef}`)
      },
      snapshot: () => ({
        config: selected,
        connection: { phase: "idle" },
        projection: null,
      }),
      subscribe: (listener) => {
        listener({
          config: selected,
          connection: { phase: "idle" },
          projection: null,
        })
        return () => events.push(`unsubscribe:${selected.runRef}`)
      },
      commands: {} as SarahFleetBrowserRuntime["commands"],
      dispose: () => events.push(`dispose:${selected.runRef}`),
    })
    const coordinator = makeSarahFleetBrowserCoordinator({
      makeRuntime,
      onState: () => {},
    })
    const second = parseSarahFleetBrowserConfig(
      "https://openagents.com/sarah?fleet_run=fleet.fc3.second",
    )!
    coordinator.setConfig(config)
    coordinator.setConfig(second)
    coordinator.dispose()
    expect(events).toEqual([
      "start:fleet.fc3.run",
      "unsubscribe:fleet.fc3.run",
      "dispose:fleet.fc3.run",
      "start:fleet.fc3.second",
      "unsubscribe:fleet.fc3.second",
      "dispose:fleet.fc3.second",
    ])
  })

  test("coordinator factory failure clears once and retains no runtime", () => {
    const states: Array<SarahFleetBrowserViewState | null> = []
    const coordinator = makeSarahFleetBrowserCoordinator({
      makeRuntime: () => {
        throw new Error("private factory detail")
      },
      onState: (state) => states.push(state),
    })
    coordinator.setConfig(config)
    expect(states).toEqual([null])
    expect(coordinator.current()).toBeNull()
  })
})
