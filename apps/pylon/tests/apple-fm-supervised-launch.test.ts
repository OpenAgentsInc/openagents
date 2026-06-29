import { describe, expect, test } from "bun:test"
import type { DiscoveredAppleFmBridgeHelper } from "../src/node/apple-fm-bridge-helper"
import type { AppleFmBridgeLauncher } from "../src/node/apple-fm-bridge-launcher"
import type { DefaultAppleFmBridgeLauncher } from "../src/node/apple-fm-bridge-launcher-host"
import type { PylonAppleFmSupervisorStatus } from "../src/node/apple-fm-bridge-supervisor-status"
import { createAppleFmSupervisedLaunch } from "../src/node/apple-fm-supervised-launch"

const HELPER: DiscoveredAppleFmBridgeHelper = {
  path: "/opt/pylon/apple-fm-bridge/foundation-bridge",
  source: "packaged-resource",
}

const RUNNING_STATUS: PylonAppleFmSupervisorStatus = {
  schema: "openagents.pylon.apple_fm.supervisor.v0.1",
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

/** Recording fake launcher so the lifecycle stays deterministic (no process). */
function makeFakeLauncher() {
  const calls: string[] = []
  const launcher: AppleFmBridgeLauncher = {
    start() {
      calls.push("start")
    },
    notifyHealthy() {
      calls.push("notifyHealthy")
    },
    status() {
      calls.push("status")
      return RUNNING_STATUS
    },
    stop() {
      calls.push("stop")
    },
  }
  return { launcher, calls }
}

describe("createAppleFmSupervisedLaunch", () => {
  test("returns an inert handle when no helper is discovered", () => {
    let assembleCalls = 0
    const launch = createAppleFmSupervisedLaunch({
      assemble: () => {
        assembleCalls += 1
        return null
      },
    })

    expect(assembleCalls).toBe(1)
    expect(launch.supervised).toBe(false)
    expect(launch.helper).toBeNull()
    expect(launch.supervisorStatus).toBeUndefined()
    // inert callbacks do not throw
    launch.notifyHealthy()
    launch.stop()
  })

  test("starts supervision and exposes the launcher status as the provider", () => {
    const { launcher, calls } = makeFakeLauncher()
    const assembled: DefaultAppleFmBridgeLauncher = { helper: HELPER, launcher }

    const launch = createAppleFmSupervisedLaunch({ assemble: () => assembled })

    expect(launch.supervised).toBe(true)
    expect(launch.helper).toBe(HELPER)
    // start() runs exactly once at construction, before any status read
    expect(calls).toEqual(["start"])

    expect(launch.supervisorStatus).toBeDefined()
    expect(launch.supervisorStatus?.()).toEqual(RUNNING_STATUS)
    expect(calls).toEqual(["start", "status"])
  })

  test("notifyHealthy forwards the bridge heartbeat into the launcher", () => {
    const { launcher, calls } = makeFakeLauncher()
    const launch = createAppleFmSupervisedLaunch({
      assemble: () => ({ helper: HELPER, launcher }),
    })

    launch.notifyHealthy()
    expect(calls).toEqual(["start", "notifyHealthy"])
  })

  test("stop is idempotent and no-ops after the first call", () => {
    const { launcher, calls } = makeFakeLauncher()
    const launch = createAppleFmSupervisedLaunch({
      assemble: () => ({ helper: HELPER, launcher }),
    })

    launch.stop()
    launch.stop()
    expect(calls.filter((c) => c === "stop")).toHaveLength(1)
  })

  test("notifyHealthy is inert after stop (no late heartbeats reach a torn-down launcher)", () => {
    const { launcher, calls } = makeFakeLauncher()
    const launch = createAppleFmSupervisedLaunch({
      assemble: () => ({ helper: HELPER, launcher }),
    })

    launch.stop()
    launch.notifyHealthy()
    expect(calls).toEqual(["start", "stop"])
  })

  test("threads launcher options through to the assembly factory", () => {
    let seenPort: number | undefined
    createAppleFmSupervisedLaunch({
      port: 51234,
      assemble: (options) => {
        seenPort = options.port
        return null
      },
    })
    expect(seenPort).toBe(51234)
  })

  test("the supervisor-status provider carries no sensitive content", () => {
    const { launcher } = makeFakeLauncher()
    const launch = createAppleFmSupervisedLaunch({
      assemble: () => ({ helper: HELPER, launcher }),
    })

    const status = launch.supervisorStatus?.()
    const serialized = JSON.stringify(status)
    expect(serialized).not.toContain(HELPER.path)
    expect(serialized).not.toContain("/opt/")
    expect(serialized).not.toMatch(/token|secret|bearer|prompt/i)
  })
})
