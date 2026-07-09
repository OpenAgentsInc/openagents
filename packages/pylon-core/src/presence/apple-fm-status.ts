/**
 * Apple FM (Foundation Models) heartbeat contribution — the presence side of
 * `apps/pylon/src/node/apple-fm-status.ts` (issue #8578, PY-1 presence
 * extraction, dependency-injection option "B" per the issue thread).
 *
 * `presence.ts`'s `sendHeartbeat` needs three things from the apple-fm
 * subtree: the `PylonAppleFmStatusProjection` type, and the two pure
 * capacity-ref helpers `appleFmBackendCapacityRefs` /
 * `withAppleFmBackendCapabilities`. All three are structurally simple (plain
 * strings/booleans/arrays/literals) — the ONLY reason they could not move
 * verbatim is that the real `PylonAppleFmStatusProjection` type in
 * `apps/pylon/src/node/apple-fm-status.ts` references
 * `ProbeBackendCapabilityReport["..."]` field types from
 * `@openagentsinc/pylon-runtime` (`apps/pylon/packages/runtime`), a NESTED
 * workspace package that is imported only by relative path everywhere in
 * apps/pylon and is never resolvable by name from a sibling workspace
 * package like `pylon-core` (confirmed in the prior PY-1 session — see the
 * issue #8578 comment thread).
 *
 * The fix is a structural mirror: `PylonAppleFmStatusProjection` below has
 * the exact same shape, with each `ProbeBackendCapabilityReport["field"]`
 * reference inlined as the literal/primitive type it actually resolves to
 * (checked directly against
 * `apps/pylon/packages/runtime/src/fleet/backend-capability.ts`'s
 * `ProbeBackendCapabilityReport` Effect Schema). Any real projection object
 * built by the app's `collectPylonAppleFmStatus` is structurally assignable
 * here with zero transformation, so the app-level shim
 * (`apps/pylon/src/presence.ts`) can pass the real value straight through the
 * existing `appleFmStatusProbe` injection seam. `presence.ts` itself never
 * imports `@openagentsinc/pylon-runtime`, directly or transitively.
 *
 * `presence.ts` never reads `.supervisor`, so that field is typed loosely
 * (a plain unknown-valued record) rather than mirroring
 * `PylonAppleFmSupervisorStatus` field-by-field.
 *
 * Drift note: if `apps/pylon/packages/runtime`'s `ProbeBackendCapabilityReport`
 * schema changes its literal `backendKind` / `capability` / `status` /
 * `requirements` / `support` shapes, this mirror needs a matching update.
 * That drift can only ever show up as a compile-time type error here (the
 * real HTTP probe and its response always stay in `apps/pylon`, so no
 * runtime value ever flows through this file) — it can never silently
 * corrupt heartbeat data.
 */

export const PYLON_APPLE_FM_STATUS_SCHEMA = "openagents.pylon.apple_fm.status.v0.1" as const
export const PYLON_APPLE_FM_CAPACITY_SERVICE = "apple_fm_bridge" as const

// Mirrors PROBE_APPLE_FM_BACKEND_CAPABILITY from
// apps/pylon/packages/runtime/src/runner/identity.ts.
const PROBE_APPLE_FM_BACKEND_CAPABILITY = "probe.backend.apple_fm_bridge" as const

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
  // Mirrors ProbeBackendCapabilityReport["backendKind"] (APPLE_FM_BACKEND_KIND).
  readonly backendKind: "apple_fm_bridge"
  readonly profileId: string
  readonly model: string
  // Mirrors ProbeBackendCapabilityReport["capability"] (PROBE_APPLE_FM_BACKEND_CAPABILITY).
  readonly capability: "probe.backend.apple_fm_bridge"
  readonly advertisedCapabilities: ReadonlyArray<string>
  readonly available: boolean
  // Mirrors ProbeBackendCapabilityReport["status"].
  readonly status: "ready" | "unavailable" | "unsupported" | "malformed" | "unreachable"
  readonly baseUrl: string
  readonly platform?: string
  readonly version?: string
  readonly unavailableReason?: string
  readonly message?: string
  // Mirrors ProbeBackendCapabilityReport["requirements"].
  readonly requirements: {
    readonly appleSilicon: "required"
    readonly appleIntelligence: "required"
    readonly liveHealth: "required"
  }
  // Mirrors ProbeBackendCapabilityReport["support"].
  readonly support: {
    readonly snapshotStreaming: boolean
    readonly toolCallbacks: boolean
  }
  // Mirrors ProbeBackendCapabilityReport["blueprintSupport"]
  // (ProbeBlueprintBackendCapabilitySupport).
  readonly blueprintSupport: {
    readonly appleFmSchemaProjection: {
      readonly maxProjectedToolCount: number
      readonly supported: boolean
      readonly supportedInputSchemaRefs: ReadonlyArray<string>
      readonly unsupportedReason?: string
    }
    readonly backendAvailability: {
      readonly api: boolean
      readonly local: boolean
      readonly swarm: boolean
    }
    readonly backendToolProjectionAdapters: ReadonlyArray<string>
    readonly localProgramRunEvidenceOffline: boolean
    readonly moduleVersionRefs: ReadonlyArray<string>
    readonly programFamilies: ReadonlyArray<string>
    readonly programSignatureRefs: ReadonlyArray<string>
    readonly programTypeRefs: ReadonlyArray<string>
    readonly registryVersionRefs: ReadonlyArray<string>
    readonly safeProjection: boolean
    readonly safeProjectionPolicyRefs: ReadonlyArray<string>
    readonly supportedBlueprintCapabilityRefs: ReadonlyArray<string>
    readonly toolRefs: ReadonlyArray<string>
    readonly warnings: ReadonlyArray<string>
  }
  // Mirrors ProbeBackendCapabilityReport["receipt"] (S.Unknown).
  readonly receipt: unknown
  readonly blockerRefs: ReadonlyArray<string>
  // presence.ts never reads this; typed loosely rather than mirroring
  // PylonAppleFmSupervisorStatus field-by-field.
  readonly supervisor?: Readonly<Record<string, unknown>>
  readonly observedAt: string
  readonly contentRedacted: true
}

/**
 * A "not probed" projection — used as presence's default when no
 * `appleFmStatusProbe` is injected. Represents "apple-fm was not evaluated
 * this heartbeat": no capacity/health/load/capability refs are contributed
 * and no blocker is raised (mirrors how an omitted `walletProbe` leaves
 * `walletReadiness: "unknown"` rather than reporting `offline`). Live Pylon
 * entry points never hit this path — `apps/pylon/src/presence.ts`'s
 * `sendHeartbeat` wrapper always injects the real probe by default.
 */
export const NOT_PROBED_APPLE_FM_STATUS: PylonAppleFmStatusProjection = {
  schema: PYLON_APPLE_FM_STATUS_SCHEMA,
  kind: "pylon_apple_fm_status",
  runnerId: "",
  runnerKind: "pylon",
  backendKind: "apple_fm_bridge",
  profileId: "",
  model: "",
  capability: PROBE_APPLE_FM_BACKEND_CAPABILITY,
  advertisedCapabilities: [],
  available: false,
  status: "unavailable",
  baseUrl: "",
  requirements: {
    appleSilicon: "required",
    appleIntelligence: "required",
    liveHealth: "required",
  },
  support: {
    snapshotStreaming: false,
    toolCallbacks: false,
  },
  blueprintSupport: {
    appleFmSchemaProjection: {
      maxProjectedToolCount: 0,
      supported: false,
      supportedInputSchemaRefs: [],
    },
    backendAvailability: {
      api: false,
      local: false,
      swarm: false,
    },
    backendToolProjectionAdapters: [],
    localProgramRunEvidenceOffline: false,
    moduleVersionRefs: [],
    programFamilies: [],
    programSignatureRefs: [],
    programTypeRefs: [],
    registryVersionRefs: [],
    safeProjection: false,
    safeProjectionPolicyRefs: [],
    supportedBlueprintCapabilityRefs: [],
    toolRefs: [],
    warnings: [],
  },
  receipt: null,
  blockerRefs: [],
  observedAt: new Date(0).toISOString(),
  contentRedacted: true,
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
