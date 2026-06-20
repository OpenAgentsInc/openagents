// #5004 cross-client exactly-once decision coordinator. This is the composing
// layer that turns N per-surface RemoteDecisionQueues (desktop / web / Expo) and
// ONE shared DecisionCloseoutLedger into a coherent, cross-client exactly-once
// decision queue:
//
//   1. A node decision event (delivered over session.subscribe / session.history)
//      is broadcast to EVERY paired client surface — they all render the same
//      pending card.
//   2. Whichever surface the owner resolves on relays decision.resolve over its
//      own bridge. On a TERMINAL outcome the coordinator builds ONE canonical
//      closeout receipt and appends it to the shared ledger.
//   3. The coordinator then broadcasts that resolution back to the OTHER surfaces
//      as a `resolved` decision event, so their cards disable themselves. A
//      later attempt to resolve the same decision on another surface hits the
//      local exactly-once gate (duplicate / already_resolved) and produces NO
//      second closeout — the ledger already holds the one canonical receipt.
//
// This is the dereferenceable cross-client proof: a decision resolved on one
// client is seen as closed on the others, and the audit ledger holds exactly one
// closeout for it. It adds NO new authority — each surface relays through its own
// capability-scoped BridgeTransport. Pure + transport-agnostic, matching the
// rest of this package: the caller injects transports, `nowMs`, and timestamps.

import type { ActionOutcome } from "./action-receipt.js"
import type { BridgeTransport } from "./bridge-transport.js"
import type { DecisionVerb } from "./decision.js"
import {
  createRemoteDecisionQueue,
  type DecisionEvent,
  type RemoteDecisionQueue,
  type ResolveResult,
} from "./remote-decision-queue.js"
import {
  createDecisionCloseoutLedger,
  type DecisionCloseoutAppendResult,
  type DecisionCloseoutLedger,
} from "./decision-closeout-ledger.js"
import {
  buildDecisionCloseoutReceipt,
  isTerminalDecisionOutcome,
  type DecisionClient,
  type DecisionCloseoutReceipt,
} from "./decision-closeout-receipt.js"

// One paired client surface and the bridge it relays decision.resolve over. Each
// surface has its OWN transport (its own capability-scoped credential); the
// coordinator never shares credentials across surfaces.
export type CoordinatedClient = {
  client: DecisionClient
  transport: Pick<BridgeTransport, "resolveDecision">
}

export type CoordinatorResolveInput = {
  // Which surface the owner is resolving on.
  client: DecisionClient
  requestId: string
  verb: DecisionVerb
  // Who triggered the resolution (owner / autopilot / an agent ref).
  actor: string
  // ISO timestamp stamped onto the closeout receipt.
  decidedAt: string
  nowMs: number
  // Free-text answer, only meaningful when verb === "answer".
  answer?: string
}

export type CoordinatorResolveResult = {
  client: DecisionClient
  // The underlying queue result for the surface that attempted the resolution.
  result: ResolveResult
  // The canonical closeout receipt — present ONLY on the first terminal
  // resolution that genuinely closes the command.
  receipt?: DecisionCloseoutReceipt
  // The shared-ledger append result, present whenever a receipt was built.
  append?: DecisionCloseoutAppendResult
  // True when this decision was already closed out (by this or another surface)
  // before the attempt — no second receipt is produced. This is the cross-client
  // exactly-once outcome in action.
  alreadyClosed: boolean
}

export type DecisionCloseoutCoordinator = {
  // Fan a node decision event out to every paired client surface (the node
  // broadcasts the same subscribe/history stream to all of them).
  ingest(event: DecisionEvent, nowMs: number): void
  ingestMany(events: ReadonlyArray<DecisionEvent>, nowMs: number): void
  // Resolve a decision on one surface. On a terminal outcome the canonical
  // closeout receipt is recorded once and the resolution is broadcast to the
  // other surfaces so their cards disable.
  resolve(input: CoordinatorResolveInput): Promise<CoordinatorResolveResult>
  // The per-surface queue (for rendering each client's pending list).
  queueFor(client: DecisionClient): RemoteDecisionQueue | undefined
  // The shared closeout ledger (the cross-client audit authority).
  ledger(): DecisionCloseoutLedger
}

export function createDecisionCloseoutCoordinator(input: {
  clients: ReadonlyArray<CoordinatedClient>
  // Inject an existing ledger (e.g. one wrapping a persistent store) or let the
  // coordinator own a fresh in-memory one.
  ledger?: DecisionCloseoutLedger
  defaultTtlMs?: number
  offlineTtlMs?: number
}): DecisionCloseoutCoordinator {
  if (input.clients.length === 0) {
    throw new Error("decision closeout coordinator requires at least one client surface")
  }

  const ledger = input.ledger ?? createDecisionCloseoutLedger()
  const queues = new Map<DecisionClient, RemoteDecisionQueue>()
  for (const c of input.clients) {
    if (queues.has(c.client)) {
      throw new Error(`duplicate client surface: ${c.client}`)
    }
    queues.set(
      c.client,
      createRemoteDecisionQueue({
        transport: c.transport,
        ...(input.defaultTtlMs === undefined ? {} : { defaultTtlMs: input.defaultTtlMs }),
        ...(input.offlineTtlMs === undefined ? {} : { offlineTtlMs: input.offlineTtlMs }),
      }),
    )
  }

  // Broadcast a `resolved` event to every surface EXCEPT the one that resolved
  // it, so their pending cards close out via the shared external-resolution merge.
  const broadcastResolved = (
    origin: DecisionClient,
    event: { requestId: string; actionRef?: string; resolvedVerb?: DecisionVerb },
    nowMs: number,
  ): void => {
    for (const [client, queue] of queues) {
      if (client === origin) continue
      queue.ingest(
        {
          requestId: event.requestId,
          ...(event.actionRef === undefined ? {} : { actionRef: event.actionRef }),
          phase: "resolved",
          ...(event.resolvedVerb === undefined ? {} : { resolvedVerb: event.resolvedVerb }),
        },
        nowMs,
      )
    }
  }

  return {
    ingest(event, nowMs) {
      for (const queue of queues.values()) queue.ingest(event, nowMs)
    },

    ingestMany(events, nowMs) {
      for (const queue of queues.values()) queue.ingestMany(events, nowMs)
    },

    async resolve(req): Promise<CoordinatorResolveResult> {
      const queue = queues.get(req.client)
      if (queue === undefined) {
        throw new Error(`unknown client surface: ${req.client}`)
      }

      // If the command was already closed out (by any surface), do not relay or
      // record again — the single canonical receipt stands.
      const alreadyClosed = ledger.get(req.requestId) !== undefined

      const result = await queue.resolve({
        requestId: req.requestId,
        verb: req.verb,
        nowMs: req.nowMs,
        ...(req.answer === undefined ? {} : { answer: req.answer }),
      })

      const outcome = result.outcome
      // Only a terminal transport outcome (not "rejected"/"queued", not the
      // transient offline/overloaded) closes a command. And only the FIRST such
      // closeout for a requestId is recorded — that is the exactly-once guard.
      if (isActionOutcome(outcome) && isTerminalDecisionOutcome(outcome) && !alreadyClosed) {
        const receipt = buildDecisionCloseoutReceipt({
          requestId: req.requestId,
          actionRef: result.entry.record.actionRef,
          verb: req.verb,
          outcome,
          client: req.client,
          actor: req.actor,
          decidedAt: req.decidedAt,
          ...(req.answer === undefined ? {} : { answer: req.answer }),
        })
        const append = ledger.append(receipt)

        // A genuine apply/duplicate resolved the local record; tell the others.
        if (result.entry.record.state === "resolved") {
          broadcastResolved(
            req.client,
            {
              requestId: req.requestId,
              actionRef: result.entry.record.actionRef,
              ...(result.entry.record.resolvedVerb === null
                ? {}
                : { resolvedVerb: result.entry.record.resolvedVerb }),
            },
            req.nowMs,
          )
        }

        return { client: req.client, result, receipt, append, alreadyClosed: false }
      }

      return { client: req.client, result, alreadyClosed }
    },

    queueFor(client) {
      return queues.get(client)
    },

    ledger() {
      return ledger
    },
  }
}

// Narrow the ResolveResult union's outcome down to a transport ActionOutcome,
// excluding the queue-only "rejected" / "queued" sentinels.
function isActionOutcome(outcome: ResolveResult["outcome"]): outcome is ActionOutcome {
  return outcome !== "rejected" && outcome !== "queued"
}
