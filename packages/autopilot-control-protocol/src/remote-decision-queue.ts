// #5000 / #5004 remote decision-queue transport. The composing layer that turns
// the capability-scoped bridge (BridgeTransport) into a cross-client,
// exactly-once decision queue: it ingests the node's decision events (delivered
// over session.subscribe / session.history) into a live queue of DecisionRecords,
// resolves a decision over the bridge (decision.resolve relay), classifies the
// result into one typed receipt outcome, and offline-queues resolutions taken
// while disconnected for oldest-first drain when the bridge returns.
//
// This is the capability that lets command APIs (decision resolve) flow over the
// capability-scoped bridge to a remote node, shared one-implementation across
// desktop / web / Expo. It adds NO new authority: it relays decision.resolve
// through the existing BridgeTransport, which carries the answer_decision
// capability and is enforced node-side against the STORED pairing claims. A
// read-only credential cannot resolve — `resolveDecision` throws on the node's
// 403 and the queue records it as an `unauthorized` receipt.
//
// Pure + transport-agnostic (the BridgeTransport injects fetch; the caller
// injects `nowMs`/idgen), matching the rest of this package.

import { classifyActionOutcome, createActionQueue, isRetryableOutcome, type ActionOutcome } from "./action-receipt.js"
import {
  applyExternalResolution,
  pendingDecision,
  resolveDecision as resolveDecisionRecord,
  type DecisionRecord,
  type DecisionVerb,
} from "./decision.js"
import type { BridgeTransport } from "./bridge-transport.js"

// A node decision event as projected over session.subscribe / session.history.
// We accept the loose record shape the bridge `events()` projection emits and
// pick out the fields we need; unknown phases are ignored.
export type DecisionEvent = {
  // The node's decision/approval requestId — the exactly-once key. The node's
  // approvalRef IS the decision requestId (see control-server decision.resolve).
  requestId: string
  // What the decision is about (rendered as the prompt actionRef).
  actionRef?: string
  // Absolute expiry; if omitted the caller's default window applies.
  expiresAtMs?: number
  // Lifecycle: "requested" opens it; "resolved"/"cancelled" close it remotely
  // (e.g. another client answered first) so this queue disables the card.
  phase: "requested" | "resolved" | "cancelled"
  // The verb a remote resolution applied (only meaningful when phase=resolved).
  resolvedVerb?: DecisionVerb
}

// The local, receipt-backed projection of one decision in the queue.
export type DecisionQueueEntry = {
  record: DecisionRecord
  // The last receipt outcome from a local resolve attempt, if any. `null` until
  // this client tries to resolve it; set to the classified outcome afterwards.
  receipt: ActionOutcome | null
  // True while a resolution is queued offline awaiting drain.
  pendingOffline: boolean
}

// The result of attempting to resolve a decision through the queue.
export type ResolveResult = {
  outcome: ActionOutcome | "rejected" | "queued"
  // The entry after the attempt (closed-out on a terminal outcome).
  entry: DecisionQueueEntry
  // Set when outcome === "rejected": why the local state machine refused
  // (already resolved, cancelled, expired, unknown) before any network call.
  reason?: "already_resolved" | "cancelled" | "expired" | "unknown_request" | "duplicate"
}

export type RemoteDecisionQueue = {
  // Fold a node decision event into the queue (idempotent per requestId).
  ingest: (event: DecisionEvent, nowMs: number) => void
  // Fold a batch (e.g. a session.history catch-up), oldest-first.
  ingestMany: (events: ReadonlyArray<DecisionEvent>, nowMs: number) => void
  // The pending (still-answerable) decisions, oldest-first.
  pending: () => DecisionQueueEntry[]
  // Every tracked decision, including closed-out ones (for receipt history).
  all: () => DecisionQueueEntry[]
  get: (requestId: string) => DecisionQueueEntry | undefined
  // Resolve a decision exactly once over the bridge. The local state machine
  // gates first (duplicate/already-resolved/expired/cancelled never hit the
  // wire); on a network failure the resolution is offline-queued and `drain`
  // replays it. Returns the classified receipt.
  resolve: (input: { requestId: string; verb: DecisionVerb; answer?: string; nowMs: number }) => Promise<ResolveResult>
  // Replay offline-queued resolutions oldest-first when connectivity returns.
  // Live entries are retried; expired queue entries are dropped and surfaced as
  // an `expired` receipt. Returns the per-decision outcomes applied.
  drain: (nowMs: number) => Promise<Array<{ requestId: string; outcome: ActionOutcome }>>
  offlineSize: () => number
}

type OfflineResolution = { requestId: string; verb: DecisionVerb; answer?: string }

export function createRemoteDecisionQueue(input: {
  transport: Pick<BridgeTransport, "resolveDecision">
  // Default decision window when an event omits expiresAtMs.
  defaultTtlMs?: number
  // Offline-resolution queue TTL (a resolution stranded longer is dropped as
  // expired rather than replayed stale). Defaults to 10 minutes.
  offlineTtlMs?: number
}): RemoteDecisionQueue {
  const defaultTtlMs = input.defaultTtlMs ?? 300_000
  const entries = new Map<string, DecisionQueueEntry>()
  const offline = createActionQueue<OfflineResolution>({ ttlMs: input.offlineTtlMs ?? 600_000 })

  const ingest = (event: DecisionEvent, nowMs: number): void => {
    const existing = entries.get(event.requestId)
    if (event.phase === "requested") {
      // Opening event: create the pending record once; re-delivery is a no-op
      // (the exactly-once requestId key dedups subscribe/history overlap).
      if (existing !== undefined) return
      entries.set(event.requestId, {
        record: pendingDecision({
          requestId: event.requestId,
          actionRef: event.actionRef ?? event.requestId,
          expiresAtMs: event.expiresAtMs ?? nowMs + defaultTtlMs,
        }),
        receipt: null,
        pendingOffline: false,
      })
      return
    }
    // resolved / cancelled: an external close (another client answered, or the
    // node cancelled/expired it). Disable the local card via the shared merge.
    if (existing === undefined) return
    entries.set(event.requestId, {
      ...existing,
      record: applyExternalResolution(existing.record, {
        state: event.phase,
        ...(event.resolvedVerb === undefined ? {} : { verb: event.resolvedVerb }),
      }),
    })
  }

  const performResolve = async (
    res: OfflineResolution,
    nowMs: number,
  ): Promise<{ outcome: ActionOutcome; entry: DecisionQueueEntry }> => {
    const current = entries.get(res.requestId)
    // Should not happen (callers gate first), but stay total.
    if (current === undefined) {
      const synthetic: DecisionQueueEntry = {
        record: pendingDecision({ requestId: res.requestId, actionRef: res.requestId, expiresAtMs: nowMs }),
        receipt: "unsupported",
        pendingOffline: false,
      }
      return { outcome: "unsupported", entry: synthetic }
    }
    try {
      const body = (await input.transport.resolveDecision({
        requestId: res.requestId,
        verb: res.verb,
        ...(res.answer === undefined ? {} : { answer: res.answer }),
      })) as { duplicate?: unknown; revoked?: unknown; stale?: unknown; applied?: unknown } | null
      // The transport throws on a non-ok envelope, so a returned body is a 2xx
      // ok result; body signals (duplicate/revoked/stale) still win.
      const outcome = classifyActionOutcome({ ok: true, status: 200, body })
      const updated: DecisionQueueEntry = {
        ...current,
        // A successful relay closes the local record exactly once.
        record:
          outcome === "applied" || outcome === "duplicate"
            ? resolveDecisionRecord(current.record, { requestId: res.requestId, verb: res.verb }, nowMs).record
            : current.record,
        receipt: outcome,
        pendingOffline: false,
      }
      entries.set(res.requestId, updated)
      return { outcome, entry: updated }
    } catch (error) {
      // A thrown error is either a network failure (offline, retryable) or the
      // node's typed rejection surfaced as a message. We can't read the status
      // off a thrown Error reliably, so classify a thrown error as offline only
      // when it looks like a transport/network failure; otherwise it is a
      // terminal error receipt. Callers re-enqueue on a retryable outcome.
      const networkError = isLikelyNetworkError(error)
      const outcome = classifyActionOutcome({ networkError, ok: false })
      const updated: DecisionQueueEntry = {
        ...current,
        receipt: outcome,
        pendingOffline: isRetryableOutcome(outcome),
      }
      entries.set(res.requestId, updated)
      return { outcome, entry: updated }
    }
  }

  return {
    ingest,
    ingestMany(events, nowMs) {
      for (const event of events) ingest(event, nowMs)
    },
    pending() {
      return [...entries.values()].filter((e) => e.record.state === "pending")
    },
    all() {
      return [...entries.values()]
    },
    get(requestId) {
      return entries.get(requestId)
    },
    async resolve({ requestId, verb, answer, nowMs }) {
      const current = entries.get(requestId)
      if (current === undefined) {
        return {
          outcome: "rejected",
          reason: "unknown_request",
          entry: {
            record: pendingDecision({ requestId, actionRef: requestId, expiresAtMs: nowMs }),
            receipt: null,
            pendingOffline: false,
          },
        }
      }
      // Local exactly-once gate FIRST — duplicate/already-resolved/cancelled/
      // expired never reach the wire. This is what keeps the queue exactly-once
      // across clients even before the node's own exactly-once relay.
      const local = resolveDecisionRecord(current.record, { requestId, verb }, nowMs)
      if (local.outcome === "duplicate") {
        const entry = { ...current, record: local.record, receipt: "duplicate" as ActionOutcome }
        entries.set(requestId, entry)
        return { outcome: "duplicate", reason: "duplicate", entry }
      }
      if (local.outcome !== "accepted") {
        // already_resolved / cancelled / expired / unknown_request: reflect any
        // state transition (e.g. pending→expired) but do not call the node.
        const entry = { ...current, record: local.record }
        entries.set(requestId, entry)
        return { outcome: "rejected", reason: local.outcome, entry }
      }
      // Accepted locally → relay over the bridge.
      const res: OfflineResolution = { requestId, verb, ...(answer === undefined ? {} : { answer }) }
      const { outcome, entry } = await performResolve(res, nowMs)
      if (isRetryableOutcome(outcome)) {
        // Offline / overloaded: queue for drain. The local record stays pending
        // so a later drain can still apply it exactly once.
        offline.enqueue({ id: requestId, action: res, nowMs })
        return { outcome: "queued", entry }
      }
      return { outcome, entry }
    },
    async drain(nowMs) {
      const { ready, expired } = offline.drain(nowMs)
      const results: Array<{ requestId: string; outcome: ActionOutcome }> = []
      for (const dropped of expired) {
        const current = entries.get(dropped.action.requestId)
        if (current !== undefined) {
          entries.set(dropped.action.requestId, { ...current, receipt: "expired", pendingOffline: false })
        }
        results.push({ requestId: dropped.action.requestId, outcome: "expired" })
      }
      for (const queued of ready) {
        const { outcome } = await performResolve(queued.action, nowMs)
        if (isRetryableOutcome(outcome)) {
          // Still unreachable — re-enqueue for the next drain.
          offline.enqueue({ id: queued.action.requestId, action: queued.action, nowMs })
        }
        results.push({ requestId: queued.action.requestId, outcome })
      }
      return results
    },
    offlineSize: () => offline.size(),
  }
}

// A thrown value from the injected fetch transport that looks like a network /
// connectivity failure (vs. the node's typed non-ok rejection). Conservative:
// only well-known offline signals count, so a node rejection is never masked as
// retryable-offline.
function isLikelyNetworkError(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false
  const name = (error as { name?: unknown }).name
  const message = (error as { message?: unknown }).message
  const text = `${typeof name === "string" ? name : ""} ${typeof message === "string" ? message : ""}`.toLowerCase()
  return (
    text.includes("failed to fetch") ||
    text.includes("network") ||
    text.includes("fetch failed") ||
    text.includes("econnrefused") ||
    text.includes("timeout") ||
    text.includes("offline") ||
    name === "TypeError" // fetch throws TypeError on network failure in browsers
  )
}
