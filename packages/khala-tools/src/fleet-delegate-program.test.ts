import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  KhalaFleetDelegateModuleError,
  prepareKhalaFleetDelegateWork,
  runKhalaFleetDelegateProgram,
  selectKhalaFleetDelegateAccount,
  type KhalaFleetDelegateAccount,
  type KhalaFleetDelegateAdvertiseResult,
  type KhalaFleetDelegateDispatchResult,
  type KhalaFleetDelegateModules,
} from "./fleet-delegate-program.js"

const readyAccount = (overrides: Partial<KhalaFleetDelegateAccount> = {}): KhalaFleetDelegateAccount => ({
  accountRef: "codex-2",
  availableSlots: 1,
  readiness: "ready",
  ...overrides,
})

const advertised = (
  available: number,
  accounts: ReadonlyArray<KhalaFleetDelegateAccount> = [readyAccount({ availableSlots: available })],
): KhalaFleetDelegateAdvertiseResult => ({
  capacity: {
    accounts,
    available,
    max: Math.max(available, 1),
  },
  heartbeatRef: `heartbeat.capacity.${available}`,
})

const completedModules = (
  overrides: Partial<KhalaFleetDelegateModules> = {},
): KhalaFleetDelegateModules => ({
  advertiseCapacity: () => Effect.succeed(advertised(1)),
  dispatch: () =>
    Effect.succeed({
      assignmentRef: "assignment.public.khala_fleet_delegate.test",
      ok: true,
    }),
  ensurePylon: () => Effect.succeed({ pylonRef: "pylon.local.test" }),
  verifyCloseout: () => Effect.succeed({ ok: true }),
  ...overrides,
})

describe("khala.fleet.delegate deterministic program", () => {
  test("reaches dispatch from a cold 0/1 start by advertising capacity first", async () => {
    const calls: string[] = []
    const modules = completedModules({
      advertiseCapacity: input => {
        calls.push(`advertise:${input.reason}`)
        return Effect.succeed(advertised(1, [readyAccount({ availableSlots: 1 })]))
      },
      dispatch: input => {
        calls.push(`dispatch:${input.account.accountRef}:${input.attempt}`)
        return Effect.succeed({
          assignmentRef: "assignment.public.khala_fleet_delegate.cold_start",
          ok: true,
        })
      },
      ensurePylon: () => {
        calls.push("ensure")
        return Effect.succeed({ pylonRef: "pylon.local.test", started: true })
      },
    })

    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run the public fixture.",
    }, modules))

    expect(result.status).toBe("completed")
    expect(calls).toEqual(["ensure", "advertise:initial", "dispatch:codex-2:1"])
    expect(result.trace.map(step => step.module)).toEqual([
      "ensure_pylon",
      "advertise_capacity",
      "select_account",
      "prepare_work",
      "dispatch",
      "verify_closeout",
    ])
    expect(result.trace[0]?.status).toBe("recovered")
    expect(result.trace[3]?.status).toBe("recovered")
  })

  test("select_account skips missing and revoked accounts before choosing a ready one", () => {
    const selected = selectKhalaFleetDelegateAccount({}, [
      readyAccount({ accountRef: "codex", readiness: "credentials_missing" }),
      readyAccount({ accountRef: "codex-3", readiness: "revoked" }),
      readyAccount({ accountRef: "codex-2", readiness: "ready" }),
    ])

    expect(selected).toMatchObject({
      account: { accountRef: "codex-2" },
      status: "selected",
    })
  })

  test("select_account returns a typed blocker when a requested account is revoked", () => {
    const selected = selectKhalaFleetDelegateAccount({ accountRef: "codex-3" }, [
      readyAccount({ accountRef: "codex-3", readiness: "revoked" }),
      readyAccount({ accountRef: "codex-2", readiness: "ready" }),
    ])

    expect(selected).toMatchObject({
      blockerCode: "revoked",
      blockerRefs: ["blocker.public.khala_fleet_delegate.revoked"],
      status: "blocked",
    })
  })

  test("prepare_work falls back to the fixture when no pins are provided", () => {
    expect(prepareKhalaFleetDelegateWork({ objective: "Run fixture." })).toEqual({
      fixture: true,
      kind: "fixture",
    })
  })

  test("prepare_work rejects partial real-work pins", () => {
    expect(() =>
      prepareKhalaFleetDelegateWork({
        objective: "Run real work.",
        repo: "OpenAgentsInc/openagents",
      }),
    ).toThrow("missing commit, verify")
  })

  test("dispatch refreshes stale heartbeat capacity and retries once", async () => {
    let dispatchCalls = 0
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      advertiseCapacity: input =>
        Effect.succeed({
          ...advertised(1),
          heartbeatRef: `heartbeat.${input.reason}`,
        }),
      dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
        dispatchCalls += 1
        return Effect.succeed(dispatchCalls === 1
          ? {
            blockerCode: "stale_heartbeat",
            message: "heartbeat stale",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.stale_heartbeat"],
          }
          : {
            assignmentRef: "assignment.public.khala_fleet_delegate.retry",
            ok: true,
          })
      },
    })))

    expect(result.status).toBe("completed")
    expect(dispatchCalls).toBe(2)
    expect(result.trace.map(step => `${step.module}:${step.status}`)).toContain("advertise_capacity:recovered")
    expect(result.trace.map(step => step.fallbackModule).filter(Boolean)).toContain("advertise_capacity")
  })

  test("dispatch backs off duplicate_active_assignment before retrying", async () => {
    let backoffs = 0
    let dispatchCalls = 0
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      backoff: () => {
        backoffs += 1
        return Effect.void
      },
      dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
        dispatchCalls += 1
        return Effect.succeed(dispatchCalls === 1
          ? {
            blockerCode: "duplicate_active_assignment",
            message: "duplicate",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.duplicate_active_assignment"],
          }
          : {
            assignmentRef: "assignment.public.khala_fleet_delegate.duplicate_retry",
            ok: true,
          })
      },
    })))

    expect(result.status).toBe("completed")
    expect(backoffs).toBe(1)
    expect(dispatchCalls).toBe(2)
    expect(result.trace.map(step => step.fallbackModule).filter(Boolean)).toContain("dispatch")
  })

  test("dispatch loops no_available_codex_capacity back through advertise_capacity", async () => {
    let advertiseCalls = 0
    let dispatchCalls = 0
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      advertiseCapacity: input => {
        advertiseCalls += 1
        return Effect.succeed({
          ...advertised(1),
          heartbeatRef: `heartbeat.${advertiseCalls}.${input.reason}`,
        })
      },
      dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
        dispatchCalls += 1
        return Effect.succeed(dispatchCalls === 1
          ? {
            blockerCode: "no_available_codex_capacity",
            message: "capacity unavailable",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.no_available_codex_capacity"],
          }
          : {
            assignmentRef: "assignment.public.khala_fleet_delegate.capacity_retry",
            ok: true,
          })
      },
    })))

    expect(result.status).toBe("completed")
    expect(advertiseCalls).toBe(2)
    expect(result.trace.map(step => step.fallbackModule).filter(Boolean)).toContain("advertise_capacity")
  })

  test("verify_closeout returns verify_failed as a typed blocker", async () => {
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      verifyCloseout: () =>
        Effect.succeed({
          blockerRefs: ["blocker.public.khala_fleet_delegate.verify_failed"],
          message: "token rows missing",
          ok: false,
        }),
    })))

    expect(result).toMatchObject({
      blockerCode: "verify_failed",
      blockerRefs: ["blocker.public.khala_fleet_delegate.verify_failed"],
      status: "blocked",
    })
    expect(result.trace.at(-1)).toMatchObject({
      module: "verify_closeout",
      precondition: "closeout_verified",
      status: "blocked",
    })
  })

  test("module failures surface through the typed taxonomy", async () => {
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      ensurePylon: () =>
        Effect.fail(new KhalaFleetDelegateModuleError({
          blockerCode: "pylon_unavailable",
          message: "Pylon is offline.",
          module: "ensure_pylon",
          refs: ["blocker.public.khala_fleet_delegate.pylon_unavailable"],
        })),
    })))

    expect(result).toMatchObject({
      blockerCode: "pylon_unavailable",
      blockerRefs: ["blocker.public.khala_fleet_delegate.pylon_unavailable"],
      status: "blocked",
    })
  })
})
