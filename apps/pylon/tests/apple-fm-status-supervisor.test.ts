import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  appleFmBackendCapacityRefs,
  collectPylonAppleFmStatus,
  withAppleFmBackendCapabilities,
  withAppleFmSupervisorStatus,
  type PylonAppleFmStatusProjection,
} from "../src/node/apple-fm-status"
import {
  APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
  createAppleFmBridgeSupervisorState,
  reduceAppleFmBridgeSupervisor,
  type AppleFmBridgeSupervisorEvent,
  type AppleFmBridgeSupervisorState,
} from "../src/node/apple-fm-bridge-supervisor"
import {
  PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
  summarizeAppleFmBridgeSupervisor,
} from "../src/node/apple-fm-bridge-supervisor-status"

const env = {
  PYLON_HOME: "/tmp/pylon-apple-fm-status-supervisor-test",
  PROBE_APPLE_FM_BASE_URL: "http://127.0.0.1:11435/",
}

const readyFetch: typeof fetch = (async () =>
  Response.json({
    ready: true,
    modelId: "apple-foundation-model",
    platform: "darwin-arm64",
    version: "fake-bridge",
  })) as typeof fetch

async function readyProjection(): Promise<PylonAppleFmStatusProjection> {
  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--json", "--pylon-ref", "pylon.test.apple-fm"]),
    env,
  )
  return collectPylonAppleFmStatus({
    summary,
    env,
    fetch: readyFetch,
    now: new Date("2026-06-15T00:00:00.000Z"),
  })
}

function drive(
  state: AppleFmBridgeSupervisorState,
  events: ReadonlyArray<AppleFmBridgeSupervisorEvent>,
): AppleFmBridgeSupervisorState {
  return events.reduce(
    (acc, event) => reduceAppleFmBridgeSupervisor(acc, event).state,
    state,
  )
}

describe("withAppleFmSupervisorStatus", () => {
  test("live ready status adds Apple FM capability and inference capacity refs", async () => {
    const projection = await readyProjection()

    expect(withAppleFmBackendCapabilities([], projection)).toEqual(
      expect.arrayContaining([
        "probe.backend.apple_fm_bridge",
        "adapter.probe.apple_fm.blueprint_tools.v1",
        "probe.program_run.evidence.local_offline",
      ]),
    )
    expect(appleFmBackendCapacityRefs(projection)).toEqual({
      capacityRefs: [
        "capacity.inference.apple_fm_bridge.ready=1",
        "capacity.inference.apple_fm_bridge.available=1",
      ],
      healthRefs: [
        "health.inference.apple_fm_bridge.ready",
        "model.inference.apple_fm_bridge.apple_foundation_model",
        "profile.inference.apple_fm_bridge.apple_fm_local",
      ],
      loadRefs: [
        "load.inference.apple_fm_bridge.busy=0",
        "load.inference.apple_fm_bridge.queued=0",
      ],
    })
  })

  test("blocked status strips stale Apple FM capability and publishes no capacity", async () => {
    const summary = createBootstrapSummary(
      parseBootstrapArgs(["--json", "--pylon-ref", "pylon.test.apple-fm-blocked"]),
      env,
    )
    const blocked = await collectPylonAppleFmStatus({
      summary,
      env,
      fetch: (async () =>
        Response.json({
          ready: false,
          unavailableReason: "apple_intelligence_disabled",
        })) as typeof fetch,
      now: new Date("2026-06-15T00:00:00.000Z"),
    })

    expect(
      withAppleFmBackendCapabilities(
        ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu", "capability.pylon.local_codex"],
        blocked,
      ),
    ).toEqual(["capability.pylon.local_codex"])
    expect(appleFmBackendCapacityRefs(blocked)).toEqual({
      capacityRefs: [],
      healthRefs: [],
      loadRefs: [],
    })
  })

  test("base projection has no supervisor until one is attached", async () => {
    const base = await readyProjection()
    expect(base.available).toBe(true)
    expect(base.blockerRefs).toEqual([])
    expect(base.supervisor).toBeUndefined()
  })

  test("attaching a running supervisor surfaces health without adding blockers", async () => {
    const base = await readyProjection()
    const supervisorState = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
    ])
    const supervisor = summarizeAppleFmBridgeSupervisor(supervisorState, 1_000)

    const merged = withAppleFmSupervisorStatus(base, supervisor)
    expect(merged.supervisor?.schema).toBe(
      PYLON_APPLE_FM_SUPERVISOR_STATUS_SCHEMA,
    )
    expect(merged.supervisor?.health).toBe("running")
    expect(merged.blockerRefs).toEqual([])
    // additive: the base projection is not mutated.
    expect(base.supervisor).toBeUndefined()
  })

  test("a crash-looped supervisor merges its blocker even when health reads ready", async () => {
    const base = await readyProjection()
    let supervisorState = createAppleFmBridgeSupervisorState({
      maxRestartsInWindow: 1,
    })
    supervisorState = drive(supervisorState, [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 0 },
      { kind: "process_exited", nowMs: 10, exitCode: 1 },
      { kind: "tick", nowMs: 10_000 },
      { kind: "process_started", nowMs: 10_000 },
      { kind: "process_exited", nowMs: 10_010, exitCode: 1 },
    ])
    const supervisor = summarizeAppleFmBridgeSupervisor(supervisorState, 10_020)

    const merged = withAppleFmSupervisorStatus(base, supervisor)
    expect(merged.supervisor?.health).toBe("stopped")
    expect(merged.blockerRefs).toContain(
      APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    )
  })

  test("merged blocker refs are deduped and sorted", () => {
    const base: PylonAppleFmStatusProjection = {
      schema: "openagents.pylon.apple_fm.status.v0.1",
      kind: "pylon_apple_fm_status",
      runnerId: "pylon.test",
      runnerKind: "pylon",
      backendKind: "apple_fm_bridge",
      profileId: "p",
      model: "m",
      capability: "probe.backend.apple_fm_bridge",
      advertisedCapabilities: [],
      available: false,
      status: "ready",
      baseUrl: "http://127.0.0.1:11435",
      requirements: [],
      support: [],
      blueprintSupport: [],
      receipt: undefined,
      blockerRefs: [
        "blocker.zzz.late",
        APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
      ],
      observedAt: "2026-06-15T00:00:00.000Z",
      contentRedacted: true,
    } as PylonAppleFmStatusProjection

    const supervisor = summarizeAppleFmBridgeSupervisor(
      drive(createAppleFmBridgeSupervisorState({ maxRestartsInWindow: 1 }), [
        { kind: "start_requested", nowMs: 0 },
        { kind: "process_started", nowMs: 0 },
        { kind: "process_exited", nowMs: 10, exitCode: 1 },
        { kind: "tick", nowMs: 10_000 },
        { kind: "process_started", nowMs: 10_000 },
        { kind: "process_exited", nowMs: 10_010, exitCode: 1 },
      ]),
      10_020,
    )

    const merged = withAppleFmSupervisorStatus(base, supervisor)
    // crash-loop blocker present once, sorted ahead of the late one.
    const occurrences = merged.blockerRefs.filter(
      (ref) => ref === APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    ).length
    expect(occurrences).toBe(1)
    expect([...merged.blockerRefs]).toEqual([...merged.blockerRefs].sort())
  })

  test("the merged projection carries no sensitive content", async () => {
    const base = await readyProjection()
    const supervisor = summarizeAppleFmBridgeSupervisor(
      drive(createAppleFmBridgeSupervisorState(), [
        { kind: "start_requested", nowMs: 0 },
        { kind: "process_started", nowMs: 0 },
        { kind: "process_exited", nowMs: 1_000, exitCode: 1 },
      ]),
      1_100,
    )
    const merged = withAppleFmSupervisorStatus(base, supervisor)
    const serialized = JSON.stringify(merged).toLowerCase()
    for (const forbidden of ["token", "bearer", "prompt", "secret", "/users/"]) {
      expect(serialized.includes(forbidden)).toBe(false)
    }
    expect(merged.contentRedacted).toBe(true)
    expect(merged.supervisor?.contentRedacted).toBe(true)
  })
})
