import { Effect } from "effect"
import type { BootstrapSummary } from "../bootstrap.js"
import {
  reportAppleFmBackendCapability,
  type ProbeBackendCapabilityReport,
  type ProbeRunnerIdentity,
} from "../../packages/runtime/src/index.js"
import type { AppleFmBridgeSupervisorState } from "./apple-fm-bridge-supervisor.js"
import {
  summarizeAppleFmBridgeSupervisor,
  type PylonAppleFmSupervisorStatus,
} from "./apple-fm-bridge-supervisor-status.js"

export const PYLON_APPLE_FM_STATUS_SCHEMA = "openagents.pylon.apple_fm.status.v0.1" as const

export type PylonAppleFmStatusProjection = {
  readonly schema: typeof PYLON_APPLE_FM_STATUS_SCHEMA
  readonly kind: "pylon_apple_fm_status"
  readonly runnerId: string
  readonly runnerKind: "pylon"
  readonly backendKind: ProbeBackendCapabilityReport["backendKind"]
  readonly profileId: string
  readonly model: string
  readonly capability: ProbeBackendCapabilityReport["capability"]
  readonly advertisedCapabilities: ReadonlyArray<string>
  readonly available: boolean
  readonly status: ProbeBackendCapabilityReport["status"]
  readonly baseUrl: string
  readonly platform?: string
  readonly version?: string
  readonly unavailableReason?: string
  readonly message?: string
  readonly requirements: ProbeBackendCapabilityReport["requirements"]
  readonly support: ProbeBackendCapabilityReport["support"]
  readonly blueprintSupport: ProbeBackendCapabilityReport["blueprintSupport"]
  readonly receipt: ProbeBackendCapabilityReport["receipt"]
  readonly blockerRefs: ReadonlyArray<string>
  /**
   * Public-safe summary of the local bridge restart/backoff supervisor, when a
   * supervisor is being driven for this runner. Absent when supervision is not
   * wired (e.g. hosted readiness probes that never launch a local helper).
   */
  readonly supervisor?: PylonAppleFmSupervisorStatus
  readonly observedAt: string
  readonly contentRedacted: true
}

export interface CollectPylonAppleFmStatusInput {
  readonly summary: BootstrapSummary
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly fetch?: typeof fetch
  readonly now?: Date
  /**
   * Current internal state of the local Apple FM bridge supervisor, if a
   * launcher is driving one. When provided, its public-safe summary is folded
   * into the projection and any supervision blocker (e.g. crash-loop give-up)
   * is unioned into `blockerRefs` so the readiness gate refuses to start a
   * local session while supervision itself is broken.
   */
  readonly supervisorState?: AppleFmBridgeSupervisorState
}

export async function collectPylonAppleFmStatus(
  input: CollectPylonAppleFmStatusInput,
): Promise<PylonAppleFmStatusProjection> {
  const now = input.now ?? new Date()
  const report = await Effect.runPromise(
    reportAppleFmBackendCapability({
      runner: makePylonAppleFmRunner(input.summary, now),
      env: input.env,
      fetch: input.fetch,
      now,
    }),
  )

  const supervisor =
    input.supervisorState === undefined
      ? undefined
      : summarizeAppleFmBridgeSupervisor(input.supervisorState, now.getTime())

  return pylonAppleFmStatusFromReport(report, supervisor)
}

export function pylonAppleFmStatusFromReport(
  report: ProbeBackendCapabilityReport,
  supervisor?: PylonAppleFmSupervisorStatus,
): PylonAppleFmStatusProjection {
  return {
    schema: PYLON_APPLE_FM_STATUS_SCHEMA,
    kind: "pylon_apple_fm_status",
    runnerId: report.runnerId,
    runnerKind: "pylon",
    backendKind: report.backendKind,
    profileId: report.profileId,
    model: report.model,
    capability: report.capability,
    advertisedCapabilities: report.advertisedCapabilities,
    available: report.available,
    status: report.status,
    baseUrl: report.baseUrl,
    ...(report.platform === undefined ? {} : { platform: report.platform }),
    ...(report.version === undefined ? {} : { version: report.version }),
    ...(report.unavailableReason === undefined ? {} : { unavailableReason: report.unavailableReason }),
    ...(report.message === undefined ? {} : { message: report.message }),
    requirements: report.requirements,
    support: report.support,
    blueprintSupport: report.blueprintSupport,
    receipt: report.receipt,
    blockerRefs: mergeBlockerRefs(
      appleFmBlockerRefs(report),
      supervisor?.blockerRefs ?? [],
    ),
    ...(supervisor === undefined ? {} : { supervisor }),
    observedAt: report.observedAt,
    contentRedacted: true,
  }
}

/**
 * Union the capability-derived blockers with any supervision blockers,
 * de-duplicated and stably sorted so the readiness gate and Autopilot Desktop
 * see a single deterministic set.
 */
function mergeBlockerRefs(
  capabilityRefs: ReadonlyArray<string>,
  supervisorRefs: ReadonlyArray<string>,
): ReadonlyArray<string> {
  if (supervisorRefs.length === 0) return capabilityRefs
  return [...new Set([...capabilityRefs, ...supervisorRefs])].sort()
}

function makePylonAppleFmRunner(summary: BootstrapSummary, now: Date): ProbeRunnerIdentity {
  const runnerId = summary.bootstrap.pylonRef ?? "pylon.local.loopback"
  return {
    runnerId,
    kind: "pylon",
    linkedSubject: runnerId,
    linkedAt: now.toISOString(),
    capabilities: summary.bootstrap.capabilityRefs,
  }
}

function appleFmBlockerRefs(report: ProbeBackendCapabilityReport): ReadonlyArray<string> {
  if (report.available && report.advertisedCapabilities.includes(report.capability)) {
    return []
  }

  const blockers = new Set<string>()
  const reason = report.unavailableReason ?? report.status

  if (!report.advertisedCapabilities.includes(report.capability)) {
    blockers.add("blocker.pylon.apple_fm.live_health_not_ready")
  }

  if (reason === "unsupported_hardware") {
    blockers.add("blocker.pylon.apple_fm.unsupported_hardware")
  } else if (reason === "apple_intelligence_disabled") {
    blockers.add("blocker.pylon.apple_fm.apple_intelligence_disabled")
  } else if (reason === "bridge_unreachable" || report.status === "unreachable") {
    blockers.add("blocker.pylon.apple_fm.bridge_unreachable")
  } else if (reason === "malformed_response" || report.status === "malformed") {
    blockers.add("blocker.pylon.apple_fm.malformed_health")
  } else if (report.status === "ready") {
    blockers.add("blocker.pylon.apple_fm.safe_blueprint_projection_unavailable")
  } else {
    blockers.add("blocker.pylon.apple_fm.not_ready")
  }

  return [...blockers].sort()
}
