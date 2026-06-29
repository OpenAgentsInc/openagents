import type { AutopilotWorkReviewAction } from './autopilot-work-routes'
import {
  validateAutopilotDecisionCloseoutReceipt,
  type AutopilotDecisionCloseoutOutcome,
  type AutopilotDecisionCloseoutReceipt,
} from './autopilot-decision-closeout'

// #5004 receipt-backed command closeout — worker-api storage + audit layer.
//
// `buildAutopilotDecisionCloseoutReceipt` (autopilot-decision-closeout.ts)
// produces ONE verifiable closeout line each time `actOnDecision` resolves a
// work-order review decision, but until now those lines had nowhere to
// accumulate: a later audit could not ask "which queued decisions were closed
// out, by whom, with what outcome?". This ledger is that place.
//
// It mirrors the protocol-side `createDecisionCloseoutLedger`
// (packages/autopilot-control-protocol) so the two surfaces stay legible to one
// audit, but it keys on the worker-api receipt's exactly-once `closeoutRef`
// rather than the remote-bridge `requestId`, and it understands the live
// review path's applied↔duplicate distinction:
//
//   - The FIRST recording of a decision builds an `applied` receipt.
//   - An idempotent replay (same Idempotency-Key) builds a `duplicate` receipt
//     with the SAME `closeoutRef` but a different `outcome`/`decidedAt`/`line`.
//
// A naive line-equality dedup (what the remote ledger does) would mis-flag that
// replay as a CONFLICT. Instead this ledger converges on the closeout's stable
// IDENTITY — the decision/work-order/action/state/actor the receipt is about —
// so a replay is recognized as the same closeout (`deduped: true`) while a
// genuinely different second closeout for the same `closeoutRef` is refused
// (`conflict`). The canonical record kept is the first (`applied`) one.
//
// Pure + in-memory, matching the worker-api ledger house style: a persistent
// store (D1/KV) can wrap this same contract later without changing callers.

export type AutopilotDecisionCloseoutAppendResult =
  // Newly recorded — the first closeout for this closeoutRef.
  | { accepted: true; deduped: false }
  // An equivalent closeout for this closeoutRef was already present (the
  // idempotent-replay / cross-observation case); the ledger did not grow.
  | { accepted: true; deduped: true }
  // The receipt failed validation (bad shape, tampered line, etc.).
  | { accepted: false; reason: 'invalid' }
  // A DIFFERENT closeout for this closeoutRef already exists. The exactly-once
  // invariant is violated; the original stands and is returned for inspection.
  | {
      accepted: false
      reason: 'conflict'
      existing: AutopilotDecisionCloseoutReceipt
    }

export type AutopilotDecisionCloseoutLedgerSummary = {
  count: number
  byOutcome: Record<AutopilotDecisionCloseoutOutcome, number>
  byAction: Record<AutopilotWorkReviewAction, number>
}

export type AutopilotDecisionCloseoutLedger = {
  // Record one closeout receipt. Idempotent per closeoutRef for receipts that
  // describe the same resolved decision; refuses a conflicting second closeout.
  append(receipt: unknown): AutopilotDecisionCloseoutAppendResult
  // Every recorded closeout, in append order (snapshot — safe to mutate).
  list(): AutopilotDecisionCloseoutReceipt[]
  // The closeout for one closeoutRef, if recorded.
  get(closeoutRef: string): AutopilotDecisionCloseoutReceipt | undefined
  // Audit slices.
  byWorkOrder(workOrderRef: string): AutopilotDecisionCloseoutReceipt[]
  byActor(actorAgentUserId: string): AutopilotDecisionCloseoutReceipt[]
  byOutcome(
    outcome: AutopilotDecisionCloseoutOutcome,
  ): AutopilotDecisionCloseoutReceipt[]
  summary(): AutopilotDecisionCloseoutLedgerSummary
}

const ALL_OUTCOMES: ReadonlyArray<AutopilotDecisionCloseoutOutcome> = [
  'applied',
  'duplicate',
]

const ALL_ACTIONS: ReadonlyArray<AutopilotWorkReviewAction> = [
  'accept',
  'reject',
  'request_changes',
]

// The stable identity of a closeout, independent of whether it was observed as
// the first `applied` recording or a later `duplicate` replay. Two receipts
// with the same identity describe the same resolved decision and converge; any
// difference is a conflict.
const closeoutIdentity = (
  receipt: AutopilotDecisionCloseoutReceipt,
): string =>
  [
    receipt.decisionRef,
    receipt.workOrderRef,
    receipt.action,
    receipt.resolvedState,
    receipt.actorAgentUserId,
    receipt.hasAnswer ? '1' : '0',
    receipt.receiptRefs.join(','),
  ].join('|')

export function createAutopilotDecisionCloseoutLedger(): AutopilotDecisionCloseoutLedger {
  // Insertion order preserved by Map; keyed by exactly-once closeoutRef.
  const byCloseoutRef = new Map<string, AutopilotDecisionCloseoutReceipt>()

  const copy = (
    receipt: AutopilotDecisionCloseoutReceipt,
  ): AutopilotDecisionCloseoutReceipt => ({
    ...receipt,
    receiptRefs: [...receipt.receiptRefs],
  })

  return {
    append(receipt: unknown): AutopilotDecisionCloseoutAppendResult {
      if (!validateAutopilotDecisionCloseoutReceipt(receipt)) {
        return { accepted: false, reason: 'invalid' }
      }

      // Validated above, so the cast is safe and `any`-free.
      const valid = receipt as AutopilotDecisionCloseoutReceipt
      const existing = byCloseoutRef.get(valid.closeoutRef)
      if (existing !== undefined) {
        if (closeoutIdentity(existing) === closeoutIdentity(valid)) {
          return { accepted: true, deduped: true }
        }
        return { accepted: false, reason: 'conflict', existing: copy(existing) }
      }

      byCloseoutRef.set(valid.closeoutRef, copy(valid))
      return { accepted: true, deduped: false }
    },

    list(): AutopilotDecisionCloseoutReceipt[] {
      return [...byCloseoutRef.values()].map(copy)
    },

    get(closeoutRef: string): AutopilotDecisionCloseoutReceipt | undefined {
      const found = byCloseoutRef.get(closeoutRef)
      return found === undefined ? undefined : copy(found)
    },

    byWorkOrder(workOrderRef: string): AutopilotDecisionCloseoutReceipt[] {
      return [...byCloseoutRef.values()]
        .filter(r => r.workOrderRef === workOrderRef)
        .map(copy)
    },

    byActor(actorAgentUserId: string): AutopilotDecisionCloseoutReceipt[] {
      return [...byCloseoutRef.values()]
        .filter(r => r.actorAgentUserId === actorAgentUserId)
        .map(copy)
    },

    byOutcome(
      outcome: AutopilotDecisionCloseoutOutcome,
    ): AutopilotDecisionCloseoutReceipt[] {
      return [...byCloseoutRef.values()]
        .filter(r => r.outcome === outcome)
        .map(copy)
    },

    summary(): AutopilotDecisionCloseoutLedgerSummary {
      const byOutcome = Object.fromEntries(
        ALL_OUTCOMES.map(o => [o, 0]),
      ) as Record<AutopilotDecisionCloseoutOutcome, number>
      const byAction = Object.fromEntries(
        ALL_ACTIONS.map(a => [a, 0]),
      ) as Record<AutopilotWorkReviewAction, number>

      for (const receipt of byCloseoutRef.values()) {
        byOutcome[receipt.outcome] += 1
        byAction[receipt.action] += 1
      }

      return { count: byCloseoutRef.size, byOutcome, byAction }
    },
  }
}
