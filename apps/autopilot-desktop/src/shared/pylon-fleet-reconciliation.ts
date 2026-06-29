import type {
  PylonFleetAssignmentRow,
  PylonFleetCapacityState,
  PylonFleetReconciliation,
} from "./rpc.js"

export type ActiveAssignmentMarker = {
  readonly accountRefHash?: string
  readonly assignmentRef: string
  readonly leaseRef: string
  readonly refreshedAt: string
  readonly service: "codex" | "claude" | string
}

export type PresenceSnapshot = {
  readonly blockerRefs: readonly string[]
  readonly lastHeartbeatAt: string | null
  readonly pylonRef: string | null
}

export type RecentAssignmentLogState =
  | "accepted"
  | "rejected"
  | "running_or_unknown"
  | "failed_before_accept"
  | "pending_output"
  | "empty"

export type RecentAssignmentLogSummary = Record<RecentAssignmentLogState, number>

const LOG_STATES: readonly RecentAssignmentLogState[] = [
  "accepted",
  "rejected",
  "running_or_unknown",
  "failed_before_accept",
  "pending_output",
  "empty",
]

export const emptyAssignmentLogSummary = (): RecentAssignmentLogSummary =>
  Object.fromEntries(LOG_STATES.map((state) => [state, 0])) as RecentAssignmentLogSummary

export const classifyAssignmentLogText = (
  text: string,
): RecentAssignmentLogState => {
  if (text.trim() === "") return "empty"
  if (
    text.includes('"event":"assignment_run.completed"') &&
    text.includes('"status":"accepted"')
  ) {
    return "accepted"
  }
  if (
    text.includes('"event":"assignment_run.completed"') &&
    text.includes('"status":"rejected"')
  ) {
    return "rejected"
  }
  if (text.includes('"event":"assignment_run.accepted"')) {
    return "running_or_unknown"
  }
  if (text.includes('"ok": false') || text.includes('"error":')) {
    return "failed_before_accept"
  }
  return "pending_output"
}

export const summarizeAssignmentLogTexts = (
  texts: readonly string[],
): RecentAssignmentLogSummary => {
  const summary = emptyAssignmentLogSummary()
  for (const text of texts) {
    const state = classifyAssignmentLogText(text)
    summary[state] = summary[state] + 1
  }
  return summary
}

const parseDateMs = (value: string | null): number | null => {
  if (value === null) return null
  const ms = Date.parse(value)
  return Number.isFinite(ms) ? ms : null
}

const capacityState = (input: {
  lastHeartbeatAt: string | null
  blockerRefs: readonly string[]
  nowMs: number
  freshAfterMs: number
}): { state: PylonFleetCapacityState; ageSeconds: number | null } => {
  const lastHeartbeatMs = parseDateMs(input.lastHeartbeatAt)
  const ageSeconds =
    lastHeartbeatMs === null
      ? null
      : Math.max(0, Math.floor((input.nowMs - lastHeartbeatMs) / 1000))
  if (input.blockerRefs.length > 0) return { state: "blocked", ageSeconds }
  if (lastHeartbeatMs === null) return { state: "unknown", ageSeconds }
  return {
    state: input.nowMs - lastHeartbeatMs <= input.freshAfterMs ? "verified" : "stale",
    ageSeconds,
  }
}

const safeService = (
  service: ActiveAssignmentMarker["service"],
): PylonFleetAssignmentRow["service"] =>
  service === "codex" || service === "claude" ? service : "unknown"

export const reconcilePylonFleet = (input: {
  readonly availableCodexSlots?: number | null
  readonly fetchedAt: string
  readonly freshAfterMs?: number
  readonly khalaRequestWrappers: number
  readonly liveCodexExecCount: number
  readonly logs: RecentAssignmentLogSummary
  readonly markers: readonly ActiveAssignmentMarker[]
  readonly presences: readonly PresenceSnapshot[]
  readonly tokenFailureCount: number
}): PylonFleetReconciliation => {
  const freshAfterMs = input.freshAfterMs ?? 5 * 60 * 1000
  const nowMs = parseDateMs(input.fetchedAt) ?? Date.now()
  const codexMarkers = input.markers.filter((marker) => marker.service === "codex")
  const executingCodexAssignments = Math.min(
    input.liveCodexExecCount,
    codexMarkers.length,
  )
  const sortedMarkers = [...input.markers].sort((a, b) => {
    const aMs = parseDateMs(a.refreshedAt) ?? 0
    const bMs = parseDateMs(b.refreshedAt) ?? 0
    return bMs - aMs
  })
  let codexExecutingSlots = executingCodexAssignments
  const assignments = sortedMarkers.slice(0, 12).map((marker): PylonFleetAssignmentRow => {
    const refreshedMs = parseDateMs(marker.refreshedAt)
    const isCodex = marker.service === "codex"
    const state =
      isCodex && codexExecutingSlots > 0
        ? "executing"
        : isCodex
          ? "stale_unknown"
          : "marker_only"
    if (isCodex && codexExecutingSlots > 0) codexExecutingSlots -= 1
    return {
      accountRefHash:
        typeof marker.accountRefHash === "string" && marker.accountRefHash.trim() !== ""
          ? marker.accountRefHash
          : null,
      ageSeconds:
        refreshedMs === null
          ? null
          : Math.max(0, Math.floor((nowMs - refreshedMs) / 1000)),
      assignmentRef: marker.assignmentRef,
      leaseRef: marker.leaseRef,
      service: safeService(marker.service),
      state,
    }
  })

  const newestPresence = [...input.presences]
    .filter((presence) => presence.pylonRef !== null)
    .sort((a, b) => {
      const aMs = parseDateMs(a.lastHeartbeatAt) ?? 0
      const bMs = parseDateMs(b.lastHeartbeatAt) ?? 0
      return bMs - aMs
    })[0]
  const pylonRefs = [
    ...new Set(
      input.presences
        .map((presence) => presence.pylonRef)
        .filter((ref): ref is string => ref !== null && ref.trim() !== ""),
    ),
  ]
  const blockers = [
    ...new Set(input.presences.flatMap((presence) => presence.blockerRefs)),
  ]
  const capacity = capacityState({
    blockerRefs: blockers,
    freshAfterMs,
    lastHeartbeatAt: newestPresence?.lastHeartbeatAt ?? null,
    nowMs,
  })
  const stale = Math.max(0, codexMarkers.length - executingCodexAssignments)

  return {
    assignments,
    capacity: {
      ageSeconds: capacity.ageSeconds,
      availableCodexSlots: input.availableCodexSlots ?? null,
      blockerRefs: blockers,
      lastHeartbeatAt: newestPresence?.lastHeartbeatAt ?? null,
      sourceRefs: [
        "source.local.pylon.presence_state",
        "source.local.pylon.active_assignment_markers",
        "source.local.process_table",
      ],
      state: capacity.state,
    },
    counts: {
      accepted: input.logs.accepted,
      assigned: input.markers.length,
      executing: input.liveCodexExecCount,
      khalaRequestWrappers: input.khalaRequestWrappers,
      pylons: pylonRefs.length,
      rejected: input.logs.rejected,
      stale,
      tokenFailures: input.tokenFailureCount,
    },
    fetchedAt: input.fetchedAt,
    pylonRefs,
  }
}
