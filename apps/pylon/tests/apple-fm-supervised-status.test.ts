import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import type {
  CollectPylonAppleFmStatusInput,
  PylonAppleFmStatusProjection,
} from "../src/node/apple-fm-status"
import { PYLON_APPLE_FM_STATUS_SCHEMA } from "../src/node/apple-fm-status"
import {
  APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
} from "../src/node/apple-fm-bridge-supervisor"
import {
  PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
  type PylonAppleFmSupervisorStatus,
} from "../src/node/apple-fm-bridge-supervisor-status"
import { createSupervisedAppleFmStatusAction } from "../src/node/apple-fm-supervised-status"

const summary = createBootstrapSummary(
  parseBootstrapArgs([], { PYLON_HOME: "/tmp/pylon-apple-fm-supervised-status-test" }),
)

const baseInput: CollectPylonAppleFmStatusInput = { summary }

function fakeProjection(
  overrides: Partial<PylonAppleFmStatusProjection> = {},
): PylonAppleFmStatusProjection {
  return {
    schema: PYLON_APPLE_FM_STATUS_SCHEMA,
    kind: "pylon_apple_fm_status",
    runnerId: "pylon.local.loopback",
    runnerKind: "pylon",
    backendKind: "apple-fm",
    profileId: "apple-fm-default",
    model: "apple-foundation-model",
    capability: "chat",
    advertisedCapabilities: ["chat"],
    available: true,
    status: "ready",
    baseUrl: "http://127.0.0.1:11435/",
    requirements: [],
    support: { supported: true },
    blueprintSupport: { supported: true },
    receipt: null,
    blockerRefs: [],
    observedAt: "2026-06-20T00:00:00.000Z",
    contentRedacted: true,
    ...overrides,
  } as PylonAppleFmStatusProjection
}

function runningSupervisor(): PylonAppleFmSupervisorStatus {
  return {
    schema: PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
    kind: "pylon_apple_fm_supervisor_status",
    health: "running",
    phase: "running",
    supervised: true,
    consecutiveRestarts: 0,
    restartsInWindow: 0,
    backoffRemainingMs: null,
    blockerRefs: [],
    contentRedacted: true,
  }
}

function crashLoopedSupervisor(): PylonAppleFmSupervisorStatus {
  return {
    schema: PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
    kind: "pylon_apple_fm_supervisor_status",
    health: "stopped",
    phase: "given_up",
    supervised: false,
    consecutiveRestarts: 5,
    restartsInWindow: 5,
    backoffRemainingMs: null,
    blockerRefs: [APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER],
    contentRedacted: true,
  }
}

describe("createSupervisedAppleFmStatusAction", () => {
  test("no provider → base projection passes through unchanged", async () => {
    const base = fakeProjection()
    const action = createSupervisedAppleFmStatusAction(baseInput, {
      collect: async () => base,
    })
    const result = await action()
    expect(result.supervisor).toBeUndefined()
    expect(result).toEqual(base)
  })

  test("provider returning null → base projection unchanged", async () => {
    const base = fakeProjection()
    const action = createSupervisedAppleFmStatusAction(baseInput, {
      collect: async () => base,
      supervisorStatus: () => null,
    })
    const result = await action()
    expect(result.supervisor).toBeUndefined()
    expect(result).toEqual(base)
  })

  test("running supervisor → phase attached, no blockers added", async () => {
    const base = fakeProjection()
    const action = createSupervisedAppleFmStatusAction(baseInput, {
      collect: async () => base,
      supervisorStatus: runningSupervisor,
    })
    const result = await action()
    expect(result.supervisor?.health).toBe("running")
    expect(result.blockerRefs).toEqual([])
  })

  test("crash-looped supervisor → blocker merged even when capability is ready", async () => {
    const base = fakeProjection({ available: true, status: "ready", blockerRefs: [] })
    const action = createSupervisedAppleFmStatusAction(baseInput, {
      collect: async () => base,
      supervisorStatus: crashLoopedSupervisor,
    })
    const result = await action()
    expect(result.supervisor?.health).toBe("stopped")
    expect(result.blockerRefs).toContain(APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER)
  })

  test("collector receives the base input on each call", async () => {
    const seen: CollectPylonAppleFmStatusInput[] = []
    const action = createSupervisedAppleFmStatusAction(baseInput, {
      collect: async (input) => {
        seen.push(input)
        return fakeProjection()
      },
    })
    await action()
    await action()
    expect(seen).toEqual([baseInput, baseInput])
  })

  test("merged projection carries no sensitive content keys", async () => {
    const base = fakeProjection()
    const action = createSupervisedAppleFmStatusAction(baseInput, {
      collect: async () => base,
      supervisorStatus: crashLoopedSupervisor,
    })
    const result = await action()
    const serialized = JSON.stringify(result).toLowerCase()
    for (const banned of ["prompt", "token", "bearer", "authorization", "secret", "password"]) {
      expect(serialized).not.toContain(banned)
    }
  })
})
