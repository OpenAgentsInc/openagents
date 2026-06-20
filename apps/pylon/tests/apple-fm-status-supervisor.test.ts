import { describe, expect, test } from "bun:test"
import {
  pylonAppleFmStatusFromReport,
  PYLON_APPLE_FM_STATUS_SCHEMA,
} from "../src/node/apple-fm-status"
import {
  APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
  createAppleFmBridgeSupervisorState,
  reduceAppleFmBridgeSupervisor,
  type AppleFmBridgeSupervisorEvent,
  type AppleFmBridgeSupervisorState,
} from "../src/node/apple-fm-bridge-supervisor"
import { summarizeAppleFmBridgeSupervisor } from "../src/node/apple-fm-bridge-supervisor-status"
import {
  APPLE_FM_BACKEND_KIND,
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  type ProbeBackendCapabilityReport,
} from "../packages/runtime/src/index"

const BLUEPRINT_SUPPORT: ProbeBackendCapabilityReport["blueprintSupport"] = {
  appleFmSchemaProjection: {
    maxProjectedToolCount: 1,
    supported: true,
    supportedInputSchemaRefs: [],
  },
  backendAvailability: { api: false, local: true, swarm: false },
  backendToolProjectionAdapters: [],
  localProgramRunEvidenceOffline: false,
  moduleVersionRefs: [],
  programFamilies: [],
  programSignatureRefs: [],
  programTypeRefs: [],
  registryVersionRefs: [],
  safeProjection: true,
  safeProjectionPolicyRefs: [],
  supportedBlueprintCapabilityRefs: [],
  toolRefs: [],
  warnings: [],
}

function readyReport(): ProbeBackendCapabilityReport {
  return {
    kind: "probe_backend_capability_report",
    runnerId: "pylon.local.loopback",
    runnerKind: "pylon",
    backendKind: APPLE_FM_BACKEND_KIND,
    profileId: "apple-fm-local",
    model: "apple-fm",
    capability: PROBE_APPLE_FM_BACKEND_CAPABILITY,
    advertisedCapabilities: [PROBE_APPLE_FM_BACKEND_CAPABILITY],
    available: true,
    status: "ready",
    baseUrl: "http://127.0.0.1:0",
    requirements: {
      appleSilicon: "required",
      appleIntelligence: "required",
      liveHealth: "required",
    },
    support: { snapshotStreaming: true, toolCallbacks: true },
    blueprintSupport: BLUEPRINT_SUPPORT,
    receipt: {},
    observedAt: "2026-06-20T00:00:00.000Z",
    contentRedacted: true,
  }
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

describe("apple_fm.status supervisor wiring", () => {
  test("omits supervisor and leaves blockers untouched when none provided", () => {
    const projection = pylonAppleFmStatusFromReport(readyReport())
    expect(projection.schema).toBe(PYLON_APPLE_FM_STATUS_SCHEMA)
    expect(projection.supervisor).toBeUndefined()
    expect(projection.blockerRefs).toEqual([])
  })

  test("a healthy running supervisor surfaces without adding blockers", () => {
    const state = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 10 },
    ])
    const supervisor = summarizeAppleFmBridgeSupervisor(state, 20)
    const projection = pylonAppleFmStatusFromReport(readyReport(), supervisor)
    expect(projection.supervisor?.health).toBe("running")
    expect(projection.supervisor?.supervised).toBe(true)
    expect(projection.blockerRefs).toEqual([])
  })

  test("crash-loop give-up unions the supervision blocker into blockerRefs", () => {
    let state = createAppleFmBridgeSupervisorState({
      maxRestartsInWindow: 2,
      restartWindowMs: 60_000,
    })
    state = drive(state, [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 1 },
      { kind: "process_exited", nowMs: 2, exitCode: 1 },
      { kind: "process_started", nowMs: 3 },
      { kind: "process_exited", nowMs: 4, exitCode: 1 },
      { kind: "process_started", nowMs: 5 },
      { kind: "process_exited", nowMs: 6, exitCode: 1 },
    ])
    const supervisor = summarizeAppleFmBridgeSupervisor(state, 7)
    expect(supervisor.health).toBe("stopped")

    const projection = pylonAppleFmStatusFromReport(readyReport(), supervisor)
    expect(projection.blockerRefs).toContain(
      APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    )
    expect(projection.blockerRefs.length).toBeGreaterThan(0)
  })

  test("blockerRefs are de-duplicated and sorted when both sources contribute", () => {
    const unreachable: ProbeBackendCapabilityReport = {
      ...readyReport(),
      advertisedCapabilities: [],
      available: false,
      status: "unreachable",
      unavailableReason: "bridge_unreachable",
    }
    const crashed = drive(
      createAppleFmBridgeSupervisorState({ maxRestartsInWindow: 1 }),
      [
        { kind: "start_requested", nowMs: 0 },
        { kind: "process_started", nowMs: 1 },
        { kind: "process_exited", nowMs: 2, exitCode: 1 },
        { kind: "process_started", nowMs: 3 },
        { kind: "process_exited", nowMs: 4, exitCode: 1 },
      ],
    )
    const supervisor = summarizeAppleFmBridgeSupervisor(crashed, 5)
    const projection = pylonAppleFmStatusFromReport(unreachable, supervisor)
    const sorted = [...projection.blockerRefs].sort()
    expect(projection.blockerRefs).toEqual(sorted)
    expect(new Set(projection.blockerRefs).size).toBe(
      projection.blockerRefs.length,
    )
    expect(projection.blockerRefs).toContain(
      APPLE_FM_BRIDGE_SUPERVISOR_CRASH_LOOP_BLOCKER,
    )
  })

  test("the projection carries no sensitive keys beyond redacted facts", () => {
    const state = drive(createAppleFmBridgeSupervisorState(), [
      { kind: "start_requested", nowMs: 0 },
      { kind: "process_started", nowMs: 1 },
    ])
    const supervisor = summarizeAppleFmBridgeSupervisor(state, 2)
    const projection = pylonAppleFmStatusFromReport(readyReport(), supervisor)
    const serialized = JSON.stringify(projection).toLowerCase()
    expect(serialized).not.toContain("token")
    expect(serialized).not.toContain("secret")
    expect(serialized).not.toContain("bearer")
    expect(projection.contentRedacted).toBe(true)
    expect(projection.supervisor?.contentRedacted).toBe(true)
  })
})
