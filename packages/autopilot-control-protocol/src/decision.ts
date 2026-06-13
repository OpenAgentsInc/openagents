// Decision exactly-once state machine. A server-originated decision request is
// a first-class object; a client resolves it with one verb, exactly once. Late,
// duplicate, cancelled, expired, or externally-resolved answers return typed
// results the UI renders without retry loops. Pure and shared across clients.

export type DecisionVerb = "approve" | "deny" | "answer"
export type DecisionState = "pending" | "resolved" | "cancelled" | "expired"

export type DecisionRecord = {
  requestId: string
  actionRef: string
  state: DecisionState
  resolvedVerb: DecisionVerb | null
  expiresAtMs: number
}

export type ResolveOutcome =
  | "accepted"
  | "duplicate"
  | "already_resolved"
  | "cancelled"
  | "expired"
  | "unknown_request"

export function pendingDecision(input: {
  requestId: string
  actionRef: string
  expiresAtMs: number
}): DecisionRecord {
  return {
    requestId: input.requestId,
    actionRef: input.actionRef,
    state: "pending",
    resolvedVerb: null,
    expiresAtMs: input.expiresAtMs,
  }
}

// Resolve a pending decision exactly once. `nowMs` decides expiry; matching
// requestId is the caller's responsibility (pass the record for that id).
export function resolveDecision(
  record: DecisionRecord,
  answer: { requestId: string; verb: DecisionVerb },
  nowMs: number,
): { record: DecisionRecord; outcome: ResolveOutcome } {
  if (answer.requestId !== record.requestId) {
    return { record, outcome: "unknown_request" }
  }
  if (record.state === "cancelled") return { record, outcome: "cancelled" }
  if (record.state === "resolved") {
    // A repeat of the same resolution is a duplicate; a different one is simply
    // rejected as already-resolved.
    return {
      record,
      outcome: record.resolvedVerb === answer.verb ? "duplicate" : "already_resolved",
    }
  }
  if (nowMs >= record.expiresAtMs) {
    return { record: { ...record, state: "expired" }, outcome: "expired" }
  }
  return {
    record: { ...record, state: "resolved", resolvedVerb: answer.verb },
    outcome: "accepted",
  }
}

// A decision resolved/cancelled elsewhere (broadcast) updates local state so the
// card disables itself.
export function applyExternalResolution(
  record: DecisionRecord,
  external: { state: "resolved" | "cancelled"; verb?: DecisionVerb },
): DecisionRecord {
  if (record.state !== "pending") return record
  return {
    ...record,
    state: external.state,
    resolvedVerb: external.state === "resolved" ? external.verb ?? record.resolvedVerb : record.resolvedVerb,
  }
}
