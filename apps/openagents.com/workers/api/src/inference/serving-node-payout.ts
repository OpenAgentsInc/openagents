// Serving-node Bitcoin revshare payout for the inference gateway
// (EPIC #5474, child #5484; design: docs/inference/2026-06-19-decentralized-
// serving-shard-wan.md §3).
//
// When a request is served by the OpenAgents serving fabric (#5483), the serving
// Pylon(s) earn a revshare cut of THAT request's margin, paid in Bitcoin, settled
// through the revenue-loop spine (EPIC #5457, RL-1/2/3). This module owns the
// PRODUCT-layer decision + split, consuming the typed `ServingReceipt` the fabric
// adapter returns. It does NOT itself dispatch a Lightning send — it produces a
// typed, idempotent, readiness-gated, owner-armed decision and the PayIn-shaped
// payout legs the existing ledger settles, so there is no parallel money path.
//
// The five gates, in order (doc §3a/§3d, §7 "Honest gaps"), each fail CLOSED:
//   1. PARITY GATE (born-verified, doc §3a): pay only against a CHECKABLE outcome.
//      A run with no exact-greedy parity / unverified parity does NOT pay by
//      default. This is Psionic's non-negotiable acceptance gate.
//   2. RL-3 NO-RESALE (inference-resale-authorization.ts): the serving cut is
//      `api_inference_gateway_resale` — the ALLOWED lane under the no-resale
//      invariant (subscription-seat resale is the forbidden lane). Invariant-clean
//      by construction (doc §3d).
//   3. RL-3 ASSET BOUNDARY (asset-bitcoin-boundary.ts): only BITCOIN revenue funds
//      a withdrawable Bitcoin share. A credit/USD/free-funded request produces NO
//      withdrawable Bitcoin serving payout (doc §3d).
//   4. OWNER-ARMED GATE (mdk-payout-mode-gate.ts): the FIRST real dispatched payout
//      is owner-armed (JUNE19_ROADMAP / doc §3d). Default DISABLED => decision is
//      armed=false and dispatches nothing live.
//   5. POSITIVE-AMOUNT: a zero/negative contributor cut produces no legs.
//
// The split (doc §3c): for a whole-model Pylon, one node earns the whole cut; for
// a shard-WAN run, the cut splits ACROSS stages, each Pylon paid for its
// layer-block contribution. The published default weighting is PER-LAYER-BLOCK
// (a stage holding more layers, doing more compute per token, earns
// proportionally more), with coordinator/draft roles paid a configurable flat
// weight for their distinct work. The receipt carries enough to recompute the
// split deterministically and reproducibly.
//
// PURITY: the split computation + decision are PURE (no IO, no clock, no env). The
// caller (route/metering integration, #5474) supplies the contributor-cut msat
// (derived from the receipt-first priced margin) and the owner-armed gate state;
// this module never moves money and never logs.

import {
  type AssetBoundaryAsset,
  type AssetBoundaryViolation,
  validateAssetBoundary,
} from '../asset-bitcoin-boundary'
import {
  authorizeInferenceMonetization,
  type InferenceResaleRefs,
} from '../inference-resale-authorization'
import { type MdkPayoutModeGateProjection } from '../mdk-payout-mode-gate'
import { type PayInLegPlan, type PayInPlan } from '../payments-ledger'
import {
  type ServingReceipt,
  type ServingStage,
} from './openagents-network-adapter'

// ----------------------------------------------------------------------------
// Split policy (the published weighting rule — doc §3c)
// ----------------------------------------------------------------------------

// The flat weight (in "layer-equivalent units") credited to a coordinator/draft
// role for its distinct non-layer work (token selection / draft proposal). A
// `stage` role is weighted by its layer-block size; these roles are weighted by
// this constant so they earn a legible, published share without inventing a
// layer range. Tunable; the receipt carries enough to recompute any rule.
export const COORDINATOR_ROLE_WEIGHT = 1
export const DRAFT_ROLE_WEIGHT = 1

// The published per-request serving-fabric CONTRIBUTOR SHARE of the margin: the
// fraction of the request's margin that fans out to the serving node(s). The
// house retains the remainder (doc §3 / gateway business doc §5: the gateway's
// per-request economics reserve a contributor share out of margin). This is the
// legible knob; the actual contributor-cut msat handed to this module is computed
// upstream from the receipt-first priced margin, so this constant documents the
// intended share and is used by `servingContributorCutMsat` helper below.
export const SERVING_CONTRIBUTOR_SHARE = 0.5

// Public-safe policy ref stamped on every serving payout decision.
export const SERVING_PAYOUT_POLICY_REF = 'policy.serving_node_payout.v1'

// Public-safe blocker reason refs (neutral; never payment material).
export const SERVING_PAYOUT_PARITY_UNVERIFIED_REF =
  'blocker.serving_payout.parity_unverified'
export const SERVING_PAYOUT_NOT_OWNER_ARMED_REF =
  'blocker.serving_payout.not_owner_armed'
export const SERVING_PAYOUT_AMOUNT_NOT_POSITIVE_REF =
  'blocker.serving_payout.amount_not_positive'
export const SERVING_PAYOUT_NO_STAGES_REF =
  'blocker.serving_payout.no_serving_stages'

// ----------------------------------------------------------------------------
// Per-stage split computation (PURE, deterministic)
// ----------------------------------------------------------------------------

// The integer "weight" a stage earns under the per-layer-block rule. `stage`
// roles are weighted by their contiguous layer-block size (end - start); the
// coordinator/draft roles take the configured flat weight. A degenerate
// non-positive layer span falls back to weight 1 so a malformed stage never
// silently drops to zero weight (defensive).
export const stageWeight = (stage: ServingStage): number => {
  if (stage.role === 'coordinator') return COORDINATOR_ROLE_WEIGHT
  if (stage.role === 'draft') return DRAFT_ROLE_WEIGHT
  const span = stage.layerEnd - stage.layerStart
  return Number.isInteger(span) && span > 0 ? span : 1
}

// One recipient's share of the contributor cut, in msat.
export type ServingPayoutShare = Readonly<{
  // The serving node (payout recipient party) — public-safe attribution ref.
  nodeRef: string
  // The stage's weight under the published rule (layer-block size or role flat).
  weight: number
  // This recipient's payout, in integer msat.
  amountMsat: number
}>

// The full split of a contributor cut across the serving stages.
export type ServingPayoutSplit = Readonly<{
  // Total contributor cut distributed (sum of shares' amountMsat). Equals the
  // input cut when it splits cleanly; the largest-weight stage absorbs any
  // integer remainder so the total is conserved to the msat (no dust lost/minted).
  totalMsat: number
  shares: ReadonlyArray<ServingPayoutShare>
}>

// Compute the per-stage payout split of `contributorCutMsat` across the receipt's
// stages, weighted by the published per-layer-block rule (doc §3c). PURE and
// deterministic: largest-remainder is assigned to the highest-weight stage (ties
// broken by earliest pipeline position) so the shares sum EXACTLY to the cut and
// the split is reproducible from the receipt alone.
//
// A whole-model single-stage receipt yields one share = the whole cut. A
// non-positive cut or an empty stage list yields an empty split (totalMsat 0).
export const computeServingPayoutSplit = (
  receipt: ServingReceipt,
  contributorCutMsat: number,
): ServingPayoutSplit => {
  const cut =
    Number.isFinite(contributorCutMsat) && contributorCutMsat > 0
      ? Math.floor(contributorCutMsat)
      : 0
  const stages = receipt.stages
  if (cut <= 0 || stages.length === 0) {
    return { shares: [], totalMsat: 0 }
  }

  const weights = stages.map(stageWeight)
  const totalWeight = weights.reduce((sum, w) => sum + w, 0)

  // Single stage (whole-model) or degenerate zero total weight: the first/only
  // stage takes the whole cut.
  if (stages.length === 1 || totalWeight <= 0) {
    return {
      shares: [
        { amountMsat: cut, nodeRef: stages[0]!.nodeRef, weight: weights[0]! },
      ],
      totalMsat: cut,
    }
  }

  // Floor each share by weight, then distribute the integer remainder to the
  // highest-weight stages (ties broken by earliest pipeline position) so the
  // total is conserved exactly and the result is deterministic.
  const floored = stages.map((stage, index) => {
    const weight = weights[index]!
    return {
      amountMsat: Math.floor((cut * weight) / totalWeight),
      index,
      nodeRef: stage.nodeRef,
      weight,
    }
  })
  const distributed = floored.reduce((sum, s) => sum + s.amountMsat, 0)
  let remainder = cut - distributed

  // Order indices by descending weight, then ascending pipeline index, for a
  // stable remainder assignment.
  const order = [...floored]
    .sort((a, b) => (b.weight - a.weight) || (a.index - b.index))
    .map(s => s.index)

  const amounts = floored.map(s => s.amountMsat)
  let cursor = 0
  while (remainder > 0 && order.length > 0) {
    const target = order[cursor % order.length]!
    amounts[target] = (amounts[target] ?? 0) + 1
    remainder -= 1
    cursor += 1
  }

  const shares: ReadonlyArray<ServingPayoutShare> = floored.map((s, index) => ({
    amountMsat: amounts[index]!,
    nodeRef: s.nodeRef,
    weight: s.weight,
  }))

  return {
    shares,
    totalMsat: shares.reduce((sum, s) => sum + s.amountMsat, 0),
  }
}

// Convenience: the intended contributor cut (msat) for a request, from the
// request's margin in msat and the published contributor share. PURE. The caller
// derives `marginMsat` from the receipt-first priced margin (#5478); this keeps
// the share knob in one legible place. Rounds DOWN so the house never overpays.
export const servingContributorCutMsat = (
  marginMsat: number,
  share: number = SERVING_CONTRIBUTOR_SHARE,
): number => {
  if (!Number.isFinite(marginMsat) || marginMsat <= 0) return 0
  if (!Number.isFinite(share) || share <= 0) return 0
  return Math.floor(marginMsat * Math.min(share, 1))
}

// ----------------------------------------------------------------------------
// Idempotency keys + receipt refs (public-safe, neutral)
// ----------------------------------------------------------------------------

// Stable idempotency key for the WHOLE serving payout of one serving run, so a
// replayed settle for the same run never double-pays. Neutral, no payment
// material.
export const servingPayoutIdempotencyKey = (servingRunRef: string): string =>
  `serving:payout:${servingRunRef}`

// Stable idempotency key for ONE stage's leg within a serving run, so a per-stage
// retry is individually idempotent (multi-stage shard runs split payout per
// stage, doc §3c).
export const servingPayoutStageLegId = (
  servingRunRef: string,
  nodeRef: string,
): string => `${servingRunRef}:stage:${nodeRef}`

// Public-safe receipt ref for a serving payout, resolvable without exposing the
// idempotency key, amount, destination, or payment material.
export const servingPayoutReceiptRef = (servingRunRef: string): string =>
  `receipt.serving.payout.${servingRunRef}`

// ----------------------------------------------------------------------------
// The gated payout decision
// ----------------------------------------------------------------------------

// How the served request was FUNDED (the revenue asset crossing the RL-3
// boundary). Only `bitcoin` revenue funds a withdrawable Bitcoin serving share.
export type ServingRevenueAsset = AssetBoundaryAsset

export type ServingNodePayoutInput = Readonly<{
  // The fabric serving receipt (#5483) — the apportionment + parity input.
  receipt: ServingReceipt
  // The contributor cut to split, in integer msat (derived upstream from the
  // receipt-first priced margin via `servingContributorCutMsat`).
  contributorCutMsat: number
  // The asset the served request's REVENUE was sourced in (RL-3 boundary).
  revenueAsset: ServingRevenueAsset
  // The owner-armed payout-mode gate projection (mdk-payout-mode-gate.ts).
  // Default-DISABLED keeps the decision inert (armed=false) — no live payout.
  payoutGate: MdkPayoutModeGateProjection
  // RL-3 resale-authorization refs for the api_inference_gateway_resale lane.
  // Optional: when omitted, the resale decision still classifies the lane as the
  // ALLOWED kind but is not "authorized" (missing ref chain) — the decision then
  // records the blockers without dispatching, never faking authorization. The
  // gateway serves from our own API-key/commercial accounts, so the resale
  // account-auth mode is pinned to `api_key` in the decision below (doc §3d).
  resaleRefs?: Partial<InferenceResaleRefs> | undefined
}>

// The typed, public-safe serving-payout decision. `armed` is true ONLY when every
// gate passes AND the owner has armed live payout; even then this module produces
// the PayIn-shaped legs and leaves the actual ledger write/dispatch to the caller
// (so tests and the default path never move money).
export type ServingNodePayoutDecision = Readonly<{
  schema: 'openagents.serving_node_payout.v1'
  servingRunRef: string
  // Whether a live payout is authorized to dispatch (all gates pass + owner-armed).
  armed: boolean
  // The computed split (always present so callers/tests can inspect the
  // apportionment even when not armed).
  split: ServingPayoutSplit
  // Public-safe ref the caller stamps on the settled payout.
  receiptRef: string
  // Idempotency key for the whole serving-run payout.
  idempotencyKey: string
  // Public-safe blocker refs (empty iff armed). Neutral.
  blockerRefs: ReadonlyArray<string>
  // Public-safe policy refs.
  policyRefs: ReadonlyArray<string>
}>

// Decide the serving-node payout for a served request. PURE. Runs the five gates
// in order and computes the split; returns a typed decision that is `armed` only
// when ALL gates pass and the owner has armed live payout. Never throws, never
// dispatches, never logs.
export const decideServingNodePayout = (
  input: ServingNodePayoutInput,
): ServingNodePayoutDecision => {
  const { receipt } = input
  const servingRunRef = receipt.servingRunRef
  const split = computeServingPayoutSplit(receipt, input.contributorCutMsat)

  const blockerRefs: string[] = []

  // GATE 1 — parity (born-verified). Pay only against a checkable outcome.
  if (!(receipt.parityMode === 'exact_greedy_parity' && receipt.parityVerified)) {
    blockerRefs.push(SERVING_PAYOUT_PARITY_UNVERIFIED_REF)
  }

  // GATE 2 — RL-3 no-resale. The serving cut is api_inference_gateway_resale
  // (the allowed lane); subscription-seat resale is the forbidden lane. Surface
  // any missing-ref-chain blockers (neutral, public-safe).
  const resale = authorizeInferenceMonetization({
    accountAuthMode: 'api_key',
    kind: 'api_inference_gateway_resale',
    ...(input.resaleRefs === undefined ? {} : { refs: input.resaleRefs }),
  })
  if (!resale.authorized) {
    blockerRefs.push(...resale.blockerRefs)
  }

  // GATE 3 — RL-3 asset boundary. Only Bitcoin revenue funds a withdrawable
  // Bitcoin serving share.
  const boundary: AssetBoundaryViolation | null = validateAssetBoundary({
    contributorAsset: 'bitcoin',
    movement: 'payout',
    revenueAsset: input.revenueAsset,
  })
  if (boundary !== null) {
    blockerRefs.push(boundary.reasonRef)
  }

  // GATE 4 — owner-armed. The first real dispatched payout is owner-armed.
  if (!input.payoutGate.livePayoutClaimAllowed) {
    blockerRefs.push(SERVING_PAYOUT_NOT_OWNER_ARMED_REF)
  }

  // GATE 5 — positive amount + at least one serving stage.
  if (receipt.stages.length === 0) {
    blockerRefs.push(SERVING_PAYOUT_NO_STAGES_REF)
  }
  if (split.totalMsat <= 0) {
    blockerRefs.push(SERVING_PAYOUT_AMOUNT_NOT_POSITIVE_REF)
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    idempotencyKey: servingPayoutIdempotencyKey(servingRunRef),
    policyRefs: [SERVING_PAYOUT_POLICY_REF, resale.schema],
    receiptRef: servingPayoutReceiptRef(servingRunRef),
    schema: 'openagents.serving_node_payout.v1',
    servingRunRef,
    split,
  }
}

// ----------------------------------------------------------------------------
// PayIn-shaped payout plan (reuses the revenue-loop ledger, no parallel path)
// ----------------------------------------------------------------------------

// Build the PayIn-shaped payout plan for an ARMED serving payout: one `reward`
// pay-in funded by a single `in` leg from the house margin account, with one
// `out` payout leg PER serving stage (the per-stage split, doc §3c). This reuses
// the exact atomic credit-ledger discipline the rest of the Worker uses
// (`createPayInStatements`): one D1 batch = one transaction; the funding leg
// covers the cost exactly; payout legs settle on `markPayInPaidStatements`.
//
// Returns `undefined` when the decision is NOT armed (no live payout) so the
// caller can never accidentally write a ledger row for a gated-off payout. The
// caller passes the house margin-account ref that funds the contributor cut.
export const buildServingPayoutPayInPlan = (
  decision: ServingNodePayoutDecision,
  houseMarginAccountRef: string,
): PayInPlan | undefined => {
  if (!decision.armed || decision.split.totalMsat <= 0) {
    return undefined
  }

  const cost = decision.split.totalMsat
  const payoutLegs: ReadonlyArray<PayInLegPlan> = decision.split.shares.map(
    share => ({
      amountMsat: share.amountMsat,
      direction: 'out',
      externalRef: 'serving_node_payout',
      kind: 'balance',
      legId: servingPayoutStageLegId(decision.servingRunRef, share.nodeRef),
      partyRef: share.nodeRef,
    }),
  )

  return {
    contextRef: `inference:serving:${decision.servingRunRef}`,
    costMsat: cost,
    genesisId: null,
    idempotencyKey: decision.idempotencyKey,
    // The house margin account funds the contributor cut (one `in` leg covering
    // the cost exactly), and the per-stage `out` legs distribute it.
    legs: [
      {
        amountMsat: cost,
        direction: 'in',
        externalRef: 'serving_node_payout_margin',
        kind: 'balance',
        legId: `${decision.servingRunRef}:margin`,
        partyRef: houseMarginAccountRef,
      },
      ...payoutLegs,
    ],
    payInId: `serving:payin:${decision.servingRunRef}`,
    payInType: 'reward',
    payerRef: houseMarginAccountRef,
    publicReceiptRef: decision.receiptRef,
    rung: null,
  }
}
