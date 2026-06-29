// Khala M3 — VERIFIED ACCEPTED-OUTCOME -> Bitcoin/Spark settlement to the serving
// WORKER + the VALIDATOR (EPIC #6017, #6011).
//
// THE TRIGGER (issue #6011). The `khala-code` verifier (`khala-code-verifier.ts`)
// produces `verified:true` + `scalarReward` + an `accepted_outcome` handoff ref ONLY
// from an EXECUTED acceptance verdict — a crossy-road build that actually passed the
// headless acceptance suite (not a regex over source). That EXECUTED accepted outcome
// is the money trigger: the worker that served it and the validator that independently
// verified it both get paid in Bitcoin, with a public settlement receipt, and the
// `openagents` response block flips `settled:true`.
//
// REUSE — DO NOT FORK THE MONEY PATH. This module does NOT build a new payout authority.
// It maps the accepted outcome onto the SAME proven primitives the parity-serving M3
// path already uses:
//   - the accepted-outcome PRICE (`pricing.ts` `KHALA_CODE_ACCEPTED_OUTCOME_PRICE`)
//     splits worker/validator (`acceptedOutcomeSettlementShares`);
//   - the two shares become a synthetic `ServingNodePayoutDecision.split` (one share
//     per party), so `settleVerifiedServingPayout` (`khala-verified-work-settlement.ts`)
//     runs its EXACT fail-closed gate stack + Spark dispatch + `realBitcoinMoved`-shaped
//     receipt chain over each party — no parallel money path, no new receipt shape;
//   - the serving-run ref is DERIVED from the verification receipt ref, so idempotency
//     is keyed on the ACCEPTANCE/RECEIPT ref: a re-verify or a redelivered callback for
//     the same accepted outcome settles AT MOST ONCE per party (the settlement receipt
//     refs are deterministic; the dispatch is receipt-first and short-circuits).
//
// SAFETY (real money — conservative; the same discipline as the parity path):
//   - FAIL-CLOSED gates, in order, each falling back to a skip: the outcome must be
//     VERIFIED + EXECUTED (an `unverified`/`failed`/un-executed outcome NEVER pays); the
//     RL-3 asset boundary (only Bitcoin revenue funds a withdrawable Bitcoin share); the
//     owner-armed real-settlement gate (default OFF everywhere -> every leg is
//     `gate_not_authorized`); the per-payout cap; the cumulative daily budget; and a
//     registered Spark destination per party.
//   - IDEMPOTENT on the acceptance/receipt ref: a replay pays AT MOST ONCE per party.
//   - FAIL-SOFT: never throws into the caller (the verdict callback forwards this
//     fire-and-forget; a settlement error never regresses the backfilled receipt).
//   - INERT BY DEFAULT: with the owner real-settlement gate OFF this module is fully
//     inert (no real sats move); arming real Bitcoin beyond the bounded guinea-pig test
//     path is a NEEDS-OWNER step, never an agent workaround. Tests mock the dispatch.

import { Effect } from 'effect'

import {
  type KhalaSettlementDeps,
  type KhalaSettlementOutcome,
  settleVerifiedServingPayout,
} from './khala-verified-work-settlement'
import {
  type AcceptedOutcomePrice,
  acceptedOutcomeSettlementShares,
  lookupAcceptedOutcomePrice,
} from './pricing'
import {
  type ServingNodePayoutDecision,
  type ServingPayoutShare,
} from './serving-node-payout'

// The two settlement parties for a verified accepted outcome.
export type AcceptedOutcomeSettlementParty = 'serving_worker' | 'validator'

// Public-safe policy ref stamped on an accepted-outcome settlement.
export const KHALA_ACCEPTED_OUTCOME_SETTLEMENT_POLICY_REF =
  'policy.khala_accepted_outcome_settlement.v1'

// The minimal, public-safe accepted-outcome the settlement consumes. It is the
// EXECUTED verifier verdict reduced to exactly what the money path needs — no
// prompts, no artifact bytes, no chain-of-thought. `verified` + `executed` are the
// gate; `verificationReceiptRef` is the idempotency anchor; the two refs name the
// parties paid.
export type KhalaAcceptedOutcome = Readonly<{
  // The inference response id the accepted outcome belongs to (diagnostics only).
  requestId: string
  // The model the accepted outcome was produced for (must have an accepted-outcome
  // price; otherwise no settlement). Canonical id.
  servedModel: string
  // Whether the outcome was VERIFIED by an EXECUTED acceptance run. Both must be
  // true to settle — an `unverified`/`failed`/un-executed outcome never pays.
  verified: boolean
  executed: boolean
  // The verifier's scalar reward (diagnostics only; the PRICE, not the scalar, sets
  // the payout — the scalar is the training/quality signal, not the amount).
  scalarReward: number
  // The verification receipt ref — the public, dereferenceable anchor the settlement
  // idempotency keys on (so a re-verify never double-pays).
  verificationReceiptRef: string
  // The serving WORKER that produced the accepted artifact (payout recipient).
  workerRef: string
  // The VALIDATOR that independently verified it (payout recipient).
  validatorRef: string
}>

// A neutral, public-safe serving-run ref derived from the verification receipt ref.
// This is what makes the settlement idempotency key on the ACCEPTANCE/RECEIPT ref:
// `settleVerifiedServingPayout` derives every per-party settlement receipt ref from
// this run ref + the party node ref, so a replay produces the SAME refs and the
// receipt-first dispatch short-circuits.
export const acceptedOutcomeServingRunRef = (
  verificationReceiptRef: string,
): string => {
  const suffix = verificationReceiptRef
    .replace(/[^A-Za-z0-9_.:/-]/g, '_')
    .slice(0, 180)
  return `accepted_outcome.${suffix}`
}

// Build the synthetic, two-party serving-payout DECISION for a verified accepted
// outcome. PURE. The worker + validator shares (from the accepted-outcome price) become
// the `split.shares`, so the proven settlement engine settles each party identically to
// a serving stage. The decision is ALWAYS marked `armed` here because the real
// fail-closed authority lives DOWNSTREAM in `settleVerifiedServingPayout`'s own gate
// stack (owner gate, caps, destination) — this decision is just the apportionment the
// engine consumes, never an authorization. A non-positive/zero split yields no shares.
//
// NOTE: `settleVerifiedServingPayout` treats `share.amountMsat` as a SAT-denominated
// share (it floors msat->sat at its boundary). The accepted-outcome price is msat; we
// hand the engine sat-denominated shares directly so the on-wire amount is the intended
// few-sat accepted-outcome payout.
export const buildAcceptedOutcomeDecision = (
  input: Readonly<{
    outcome: KhalaAcceptedOutcome
    price: AcceptedOutcomePrice
  }>,
): ServingNodePayoutDecision => {
  const { outcome, price } = input
  const servingRunRef = acceptedOutcomeServingRunRef(
    outcome.verificationReceiptRef,
  )
  const split = acceptedOutcomeSettlementShares(price)

  // Convert msat shares to whole-sat shares for the engine (which floors at its
  // boundary anyway; doing it here keeps the on-wire amount legible + conserved).
  const workerSats = Math.floor(split.workerMsat / 1000)
  const validatorSats = Math.floor(split.validatorMsat / 1000)

  const shares: ReadonlyArray<ServingPayoutShare> = [
    // Worker FIRST (the guinea-pig serving Pylon is paid first, per owner direction).
    { amountMsat: workerSats * 1000, nodeRef: outcome.workerRef, weight: 1 },
    { amountMsat: validatorSats * 1000, nodeRef: outcome.validatorRef, weight: 1 },
  ]

  const totalMsat = shares.reduce((sum, s) => sum + s.amountMsat, 0)

  return {
    armed: true,
    blockerRefs: [],
    idempotencyKey: `serving:payout:${servingRunRef}`,
    policyRefs: [KHALA_ACCEPTED_OUTCOME_SETTLEMENT_POLICY_REF],
    receiptRef: `receipt.serving.payout.${servingRunRef}`,
    schema: 'openagents.serving_node_payout.v1',
    servingRunRef,
    split: { shares, totalMsat },
  }
}

export type KhalaAcceptedOutcomeSettlementResult = Readonly<{
  // Whether the outcome was even ELIGIBLE to settle (verified + executed + has an
  // accepted-outcome price). False => no settlement attempted (honest, not a payout).
  eligible: boolean
  // The party->settlement outcome from the proven engine, present only when eligible.
  // Each leg carries `settled` + `settlementReceiptRef` exactly like the parity path.
  settlement: KhalaSettlementOutcome | null
}>

// Settle a VERIFIED accepted outcome to the serving worker + validator in Bitcoin over
// Spark, reusing the proven engine. FAIL-CLOSED + FAIL-SOFT + IDEMPOTENT.
//
//   1. ELIGIBILITY (pure, fail-closed): the outcome must be `verified` AND `executed`,
//      and the model must have an accepted-outcome price. Any miss => `{ eligible:false,
//      settlement:null }` — no settlement attempted, no money moved.
//   2. The eligible outcome becomes a two-party decision (worker + validator) and runs
//      through `settleVerifiedServingPayout`, which applies its OWN fail-closed gates
//      (owner real-settlement gate default OFF, per-payout cap, daily budget, registered
//      destination) per party and dispatches the SAME receipt-first idempotent Spark
//      settlement. `parityVerified` is passed `true` because the accepted-outcome
//      EXECUTION verdict IS the checkable-outcome gate this path stands on (the verifier
//      ran the artifact); the engine's gate stack still independently fail-closes
//      everything money-related.
//
// Returns the structured result; never throws into the caller.
export const settleVerifiedAcceptedOutcome = (
  deps: KhalaSettlementDeps,
  outcome: KhalaAcceptedOutcome,
): Effect.Effect<KhalaAcceptedOutcomeSettlementResult> =>
  Effect.gen(function* () {
    // GATE 1 — verified + executed accepted outcome (fail-closed). Only an EXECUTED,
    // verified outcome is a payable accepted outcome.
    if (!(outcome.verified && outcome.executed)) {
      return { eligible: false, settlement: null }
    }

    // GATE 2 — the model must have an accepted-outcome price.
    const price = lookupAcceptedOutcomePrice(outcome.servedModel)
    if (price === undefined) {
      return { eligible: false, settlement: null }
    }

    const decision = buildAcceptedOutcomeDecision({ outcome, price })

    // The proven engine: per-party gate stack + Spark dispatch + receipt chain. The
    // accepted-outcome EXECUTION verdict is the checkable outcome, so parity is true;
    // every money gate downstream still fail-closes by default.
    const settlement = yield* settleVerifiedServingPayout(deps, {
      decision,
      parityVerified: true,
      servedModel: outcome.servedModel,
    })

    return { eligible: true, settlement }
  })

// The settled summary the route surfaces into the `openagents` receipt block. PURE.
// Derived from the engine's leg outcomes: `settled` is true iff at least one party leg
// actually settled (real Bitcoin moved); `settlementReceiptRefs` lists the
// dereferenceable per-party settlement receipts; `parties` names which parties settled.
// With every gate OFF (the default) this is `{ settled:false, refs:[], parties:[] }` —
// the honest inert default the receipt shows alongside `verified:true`.
export type AcceptedOutcomeSettledSummary = Readonly<{
  settled: boolean
  settlementReceiptRefs: ReadonlyArray<string>
  settledParties: ReadonlyArray<AcceptedOutcomeSettlementParty>
}>

export const summarizeAcceptedOutcomeSettlement = (
  outcome: KhalaAcceptedOutcome,
  result: KhalaAcceptedOutcomeSettlementResult,
): AcceptedOutcomeSettledSummary => {
  if (!result.eligible || result.settlement === null) {
    return { settled: false, settledParties: [], settlementReceiptRefs: [] }
  }

  const refs: string[] = []
  const parties: AcceptedOutcomeSettlementParty[] = []
  for (const leg of result.settlement.legs) {
    if (leg.settled && leg.settlementReceiptRef !== null) {
      refs.push(leg.settlementReceiptRef)
      // Map the leg's contributor ref back to its party label.
      parties.push(
        leg.contributorRef === outcome.workerRef
          ? 'serving_worker'
          : 'validator',
      )
    }
  }

  return {
    settled: refs.length > 0,
    settledParties: parties,
    settlementReceiptRefs: refs,
  }
}
