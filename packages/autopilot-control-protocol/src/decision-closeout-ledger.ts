// #5004 receipt-backed command closeout — storage + audit-index layer. Where
// buildDecisionCloseoutReceipt produces ONE verifiable closeout line, this
// ledger is where those lines accumulate so a later audit can ask "what
// decisions were closed out, by which client, with what outcome?".
//
// The ledger enforces the exactly-once invariant at the audit layer: a command
// (keyed by its decision requestId) closes out exactly once. Because the same
// closeout can legitimately be observed by more than one client surface
// (desktop AND web both render the same node closeout broadcast), re-appending a
// byte-identical receipt is idempotent (cross-client CONVERGENCE), while a
// SECOND, DIFFERING closeout for the same requestId is a `conflict` and is
// refused — two clients must never disagree about how a command ended.
//
// Pure + in-memory, matching createShipReceiptLedger: a persistent store (D1/KV)
// can wrap this same contract later without changing callers.

import {
  validateDecisionCloseoutReceipt,
  type DecisionClient,
  type DecisionCloseoutReceipt,
  type TerminalDecisionOutcome,
  DECISION_CLIENTS,
  TERMINAL_DECISION_OUTCOMES,
} from "./decision-closeout-receipt.js"

export type DecisionCloseoutAppendResult =
  // Newly recorded — the first closeout for this requestId.
  | { accepted: true; deduped: false }
  // An identical closeout for this requestId was already present; the ledger did
  // not grow. This is the cross-client convergence case (two surfaces saw the
  // same node closeout) and is treated as success.
  | { accepted: true; deduped: true }
  // The receipt failed validation (bad shape, tampered line, etc.).
  | { accepted: false; reason: "invalid" }
  // A DIFFERENT closeout for this requestId already exists. The exactly-once
  // invariant is violated; the original stands and is returned for inspection.
  | { accepted: false; reason: "conflict"; existing: DecisionCloseoutReceipt }

export type DecisionCloseoutLedgerSummary = {
  count: number
  byOutcome: Record<TerminalDecisionOutcome, number>
  byClient: Record<DecisionClient, number>
}

export type DecisionCloseoutLedger = {
  // Record one closeout receipt. Idempotent per requestId for identical
  // receipts; refuses a conflicting closeout for an already-closed command.
  append(receipt: unknown): DecisionCloseoutAppendResult
  // Every recorded closeout, in append order (snapshot — safe to mutate).
  list(): DecisionCloseoutReceipt[]
  // The closeout for one decision requestId, if recorded.
  get(requestId: string): DecisionCloseoutReceipt | undefined
  // Audit slices.
  byClient(client: DecisionClient): DecisionCloseoutReceipt[]
  byActor(actor: string): DecisionCloseoutReceipt[]
  byOutcome(outcome: TerminalDecisionOutcome): DecisionCloseoutReceipt[]
  summary(): DecisionCloseoutLedgerSummary
}

export function createDecisionCloseoutLedger(): DecisionCloseoutLedger {
  // Insertion order preserved by Map; keyed by exactly-once requestId.
  const byRequestId = new Map<string, DecisionCloseoutReceipt>()

  const copy = (receipt: DecisionCloseoutReceipt): DecisionCloseoutReceipt => ({ ...receipt })

  return {
    append(receipt: unknown): DecisionCloseoutAppendResult {
      if (!validateDecisionCloseoutReceipt(receipt)) return { accepted: false, reason: "invalid" }

      // Validated above, so the cast is safe and `any`-free.
      const valid = receipt as DecisionCloseoutReceipt
      const existing = byRequestId.get(valid.requestId)
      if (existing !== undefined) {
        // The canonical `line` is a deterministic digest of every field, so line
        // equality means the two closeouts agree in full.
        if (existing.line === valid.line) return { accepted: true, deduped: true }
        return { accepted: false, reason: "conflict", existing: copy(existing) }
      }

      byRequestId.set(valid.requestId, copy(valid))
      return { accepted: true, deduped: false }
    },

    list(): DecisionCloseoutReceipt[] {
      return [...byRequestId.values()].map(copy)
    },

    get(requestId: string): DecisionCloseoutReceipt | undefined {
      const found = byRequestId.get(requestId)
      return found === undefined ? undefined : copy(found)
    },

    byClient(client: DecisionClient): DecisionCloseoutReceipt[] {
      return [...byRequestId.values()].filter((r) => r.client === client).map(copy)
    },

    byActor(actor: string): DecisionCloseoutReceipt[] {
      return [...byRequestId.values()].filter((r) => r.actor === actor).map(copy)
    },

    byOutcome(outcome: TerminalDecisionOutcome): DecisionCloseoutReceipt[] {
      return [...byRequestId.values()].filter((r) => r.outcome === outcome).map(copy)
    },

    summary(): DecisionCloseoutLedgerSummary {
      const byOutcome = Object.fromEntries(
        TERMINAL_DECISION_OUTCOMES.map((o) => [o, 0]),
      ) as Record<TerminalDecisionOutcome, number>
      const byClient = Object.fromEntries(
        DECISION_CLIENTS.map((c) => [c, 0]),
      ) as Record<DecisionClient, number>

      for (const receipt of byRequestId.values()) {
        byOutcome[receipt.outcome] += 1
        byClient[receipt.client] += 1
      }

      return { count: byRequestId.size, byOutcome, byClient }
    },
  }
}
