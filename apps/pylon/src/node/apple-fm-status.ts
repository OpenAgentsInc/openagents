import { Effect } from "effect"
import type { BootstrapSummary } from "../bootstrap.js"
import {
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  reportAppleFmBackendCapability,
  type ProbeBackendCapabilityReport,
  type ProbeRunnerIdentity,
} from "../../packages/runtime/src/index.js"
import type { PylonAppleFmSupervisorStatus } from "./apple-fm-bridge-supervisor-status.js"

export const PYLON_APPLE_FM_STATUS_SCHEMA = "openagents.pylon.apple_fm.status.v0.1" as const
export const PYLON_APPLE_FM_CAPACITY_SERVICE = "apple_fm_bridge" as const

const APPLE_FM_DERIVED_CAPABILITY_REFS = new Set([
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  "adapter.probe.apple_fm.blueprint_tools.v1",
  "probe.blueprint.signature_lookup",
  "probe.blueprint.tool_menu",
  "probe.program_run.evidence.local_offline",
])

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
   * Public-safe local helper-supervision phase, present only when a supervisor
   * is driving the Foundation Models bridge helper for this runner. Absent when
   * supervision has not been wired (e.g. hosted readiness or a pre-launch
   * probe). Its blocker refs are merged into the top-level `blockerRefs`.
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

  return pylonAppleFmStatusFromReport(report)
}

export function pylonAppleFmStatusFromReport(
  report: ProbeBackendCapabilityReport,
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
    blockerRefs: appleFmBlockerRefs(report),
    observedAt: report.observedAt,
    contentRedacted: true,
  }
}

export function withAppleFmBackendCapabilities(
  capabilityRefs: ReadonlyArray<string>,
  projection: PylonAppleFmStatusProjection,
): string[] {
  const base = capabilityRefs.filter((ref) => !APPLE_FM_DERIVED_CAPABILITY_REFS.has(ref))
  if (!projection.available || !projection.advertisedCapabilities.includes(PROBE_APPLE_FM_BACKEND_CAPABILITY)) {
    return [...new Set(base)]
  }
  return [...new Set([...base, ...projection.advertisedCapabilities])].sort()
}

export function appleFmBackendCapacityRefs(
  projection: PylonAppleFmStatusProjection,
): { capacityRefs: string[]; loadRefs: string[]; healthRefs: string[] } {
  if (!projection.available || !projection.advertisedCapabilities.includes(PROBE_APPLE_FM_BACKEND_CAPABILITY)) {
    return { capacityRefs: [], healthRefs: [], loadRefs: [] }
  }

  return {
    capacityRefs: [
      `capacity.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.ready=1`,
      `capacity.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.available=1`,
    ],
    healthRefs: [
      `health.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.ready`,
      `model.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.apple_foundation_model`,
      `profile.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.apple_fm_local`,
    ],
    loadRefs: [
      `load.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.busy=0`,
      `load.inference.${PYLON_APPLE_FM_CAPACITY_SERVICE}.queued=0`,
    ],
  }
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

/**
 * Embed a local bridge-supervisor summary into the Apple FM status projection so
 * the supervision phase (running / recovering / stopped) reaches the
 * `apple_fm.status` surface and Autopilot Desktop.
 *
 * This is the surface-observability half of
 * `blocker.product_promises.local_apple_fm_helper_supervision_missing`: the pure
 * reducer (`apple-fm-bridge-supervisor.ts`) decides the lifecycle and
 * `summarizeAppleFmBridgeSupervisor(...)` projects it; this function carries that
 * summary out on the same projection the desktop already consumes, merging the
 * supervisor's blocker refs into the top-level set (deduped + sorted) so a
 * crash-looped helper is visible even when the capability probe itself reads
 * ready. It is purely additive and side-effect-free: the base projection (no
 * supervisor) is unchanged, no clock is read, and no prompts, file contents,
 * paths, tokens, URLs, or bearer material are introduced.
 */
export function withAppleFmSupervisorStatus(
  projection: PylonAppleFmStatusProjection,
  supervisor: PylonAppleFmSupervisorStatus,
): PylonAppleFmStatusProjection {
  const mergedBlockers = [
    ...new Set([...projection.blockerRefs, ...supervisor.blockerRefs]),
  ].sort()

  return {
    ...projection,
    supervisor,
    blockerRefs: mergedBlockers,
  }
}
