// Agentic labor-product flow — the typed, end-to-end shape of ONE agentic
// labor PRODUCT being sold: an agent POSTS a reusable labor product, a buyer
// ORDERS it, the work is DISPATCHED to a worker, a RESULT is DELIVERED, and the
// order SETTLES under one receipt seam (promise
// autopilot.agentic_labor_products.v1, yellow; docs/transcripts/239.md "Let's
// Make Money": OpenAgents sells agentic labor/products, not dumb base-inference
// resale).
//
// THE GAP THIS CLOSES: the promise's claim is "OpenAgents sells agentic
// labor/products". The lane-c fanout proved a single OPERATOR-staged work order
// can fan to the open labor market and settle (control_center_fanout_marketplace
// .v1). The NIP-90 LBR rails (packages/nip90) prove the request -> quote ->
// acceptance -> result event chain. But there was no typed flow that models a
// labor PRODUCT a buyer can ORDER off a listing and carry through to a settled
// receipt — the customer-facing post -> order -> dispatch -> deliver -> settle
// lifecycle the yellow promise's verification asks for ("order, review,
// artifact, acceptance, billing, and handoff evidence"). This module is that
// flow.
//
// SCOPE / HONESTY: this is FLAG-GATED INERT where it touches real settlement.
// The flow model is PURE: it provisions nothing, opens no wallet, reads no real
// balance, and writes no real receipt — it assembles a typed flow PLAN with the
// public-safe receipt ref the order WOULD settle under (derived from the same
// cloud-metering receipt-ref helper every other priced primitive uses). The
// settlement seam (`settleLaborProductOrder`) is INERT by default (`enabled:
// false` => `disabled`, no ledger IO, no money moved); only when armed AND given
// an owner-sign-off ref does it feed a receipt-first charge through the shared
// credit ledger. The promise STAYS yellow: a typed flow + an inert seam is not
// proof of a real labor product sold. A green flip stays receipt-first and
// owner-signed per proof.claim_upgrade_receipts.v1 with a dereferenceable
// settlement receipt and demand provenance per proof.demand_provenance.v1.

import { Effect, Schema as S } from 'effect'

import {
  type CloudMeteringDeps,
  type CloudMeteringOutcome,
  cloudChargeReceiptRef,
  settleCloudPrimitiveCharge,
} from './cloud/cloud-metering'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const AGENTIC_LABOR_PRODUCT_SCHEMA =
  'openagents.agentic_labor_product.v1' as const

// The yellow promise this flow sits under. It STAYS yellow; the flow makes no
// live-sale claim.
export const AGENTIC_LABOR_PRODUCTS_PROMISE =
  'autopilot.agentic_labor_products.v1' as const

// The public-safe primitive tag the order's settlement receipt and idempotency
// key are namespaced under (the cloud-metering helpers key off this).
export const AGENTIC_LABOR_PRODUCT_PRIMITIVE =
  'autopilot.agentic_labor_product.order' as const

// The NIP-90 market stream a labor product settles on (compute | data | labor).
export const AGENTIC_LABOR_PRODUCT_STREAM_KIND = 'labor' as const

// The self-serve blocker, now CLEARED by the deployed self-serve order-planning
// path (POST /api/public/autopilot/labor-products + planSelfServeLaborProductOrder):
// a buyer/agent can plan a labor-product order with no operator staging. Kept as
// an exported constant for historical clarity; it is NO LONGER listed in any
// uncleared-blocker projection.
export const LABOR_PRODUCT_NOT_ALL_FLOWS_SELF_SERVE_REF =
  'blocker.product_promises.not_all_labor_flows_self_serve' as const
// The remaining uncleared blocker this flow documents and does NOT clear: a
// typed self-serve flow + an inert settlement seam is not a real labor product
// sold with a dereferenceable settlement receipt and owner sign-off.
export const LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF =
  'blocker.product_promises.agentic_labor_product_real_sale_receipt_missing' as const

/**
 * The lifecycle stage of one labor-product order. The flow advances strictly
 * forward: a listing is `posted`, a buyer `ordered` it, a worker is
 * `dispatched`, a result is `delivered`, and the order is `settled`.
 */
export const LaborProductStage = S.Literals([
  'posted',
  'ordered',
  'dispatched',
  'delivered',
  'settled',
])
export type LaborProductStage = typeof LaborProductStage.Type

const STAGE_ORDER: ReadonlyArray<LaborProductStage> = [
  'posted',
  'ordered',
  'dispatched',
  'delivered',
  'settled',
]

/** Index of a stage in the forward lifecycle (for monotonicity checks). */
export const laborProductStageIndex = (stage: LaborProductStage): number =>
  STAGE_ORDER.indexOf(stage)

/**
 * A labor PRODUCT an agent posts: a reusable, orderable unit of agentic labor.
 * Neutral refs only — no name is required and no raw price/payment material is
 * carried (the price is a public-safe sats figure used only to derive the
 * order's would-be charge).
 */
export const LaborProductListing = S.Struct({
  /** Stable listing id. */
  listingId: S.String,
  /** Neutral seller ref (the agent posting the product; e.g. "agent:<id>"). */
  sellerRef: S.String,
  title: S.String,
  summary: S.String,
  /** Neutral capability ref the product delivers (promise/capability id). */
  capabilityRef: S.String,
  /** Public-safe list price in whole sats (>= 0). */
  priceSats: S.Number,
})
export type LaborProductListing = typeof LaborProductListing.Type

/**
 * The settlement seam shape for a labor-product order: the public-safe receipt
 * ref the order WOULD settle under and the account that would be debited.
 * INERT in the flow plan — nothing is written here.
 */
export const LaborProductSettlement = S.Struct({
  /** Neutral buyer/account ref the order debits (e.g. "agent:<id>"). */
  accountRef: S.String,
  /** Public-safe receipt ref the order settles under (dereferenceable shape). */
  receiptRef: S.String,
  /** The NIP-90 market stream this settles on. */
  streamKind: S.Literal(AGENTIC_LABOR_PRODUCT_STREAM_KIND),
})
export type LaborProductSettlement = typeof LaborProductSettlement.Type

/**
 * A typed end-to-end labor-product flow: the listing, the buyer, the order's
 * current stage, the worker the order is dispatched to, the delivered artifact
 * ref, and the settlement seam. PLAN only — no provisioning, dispatch, delivery
 * effect, metering, debit, or settlement is performed.
 */
export const LaborProductFlowPlan = S.Struct({
  schema: S.Literal(AGENTIC_LABOR_PRODUCT_SCHEMA),
  /** Stable order id. */
  orderId: S.String,
  listing: LaborProductListing,
  /** Neutral buyer ref (account/agent/customer ref). */
  buyerRef: S.String,
  /** The order's current lifecycle stage. */
  stage: LaborProductStage,
  /** Neutral worker ref the order is dispatched to (null until dispatched). */
  workerRef: S.NullOr(S.String),
  /** Public-safe delivered artifact ref (null until delivered). */
  artifactRef: S.NullOr(S.String),
  settlement: LaborProductSettlement,
  /** Always the yellow promise — the flow over-claims nothing. */
  promiseIds: S.Tuple([S.Literal(AGENTIC_LABOR_PRODUCTS_PROMISE)]),
  /** Always yellow — a typed flow is not a live sale. */
  promiseState: S.Literal('yellow'),
  /** Always true — the flow plan is INERT (moves no money, dispatches nothing). */
  inert: S.Literal(true),
  /** The blockers this flow documents and does NOT clear. */
  unclearedBlockerRefs: S.Array(S.String),
  createdAt: S.String,
})
export type LaborProductFlowPlan = typeof LaborProductFlowPlan.Type

export class LaborProductValidationError extends S.TaggedErrorClass<LaborProductValidationError>()(
  'LaborProductValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const isWholeNonNegative = (value: number): boolean =>
  Number.isInteger(value) && value >= 0

/**
 * The public-safe receipt ref a labor-product order settles under, derived from
 * the shared cloud-metering receipt-ref helper so the labor product speaks the
 * SAME receipt vocabulary as every other priced primitive. Never a raw amount,
 * destination, or payment material.
 */
export const laborProductOrderReceiptRef = (orderId: string): string =>
  cloudChargeReceiptRef(AGENTIC_LABOR_PRODUCT_PRIMITIVE, orderId)

/**
 * Build a typed labor-product flow plan from raw input. PURE and validating:
 *   - requires non-empty order id / buyer ref;
 *   - requires a well-formed listing (non-empty listing id / seller ref / title
 *     / capability ref, and a whole non-negative price in sats);
 *   - requires `workerRef` once the stage is at/after `dispatched` and
 *     `artifactRef` once the stage is at/after `delivered` (the lifecycle is
 *     coherent: you cannot be delivered without an artifact, nor dispatched
 *     without a worker);
 *   - derives the order's public-safe settlement receipt ref from the shared
 *     cloud-metering helper and pins the labor stream;
 *   - pins the yellow promise and records the uncleared blockers so the plan can
 *     never over-claim.
 */
export const buildLaborProductFlowPlan = (input: {
  orderId: string
  buyerRef: string
  listing: LaborProductListing
  stage: LaborProductStage
  workerRef?: string | null
  artifactRef?: string | null
  createdAt?: string
}):
  | { ok: true; plan: LaborProductFlowPlan }
  | { ok: false; error: LaborProductValidationError } => {
  if (!isNonEmpty(input.orderId)) {
    return fail('orderId must be non-empty')
  }
  if (!isNonEmpty(input.buyerRef)) {
    return fail('buyerRef must be non-empty')
  }
  if (!isNonEmpty(input.listing.listingId)) {
    return fail('listing.listingId must be non-empty')
  }
  if (!isNonEmpty(input.listing.sellerRef)) {
    return fail('listing.sellerRef must be non-empty')
  }
  if (!isNonEmpty(input.listing.title)) {
    return fail('listing.title must be non-empty')
  }
  if (!isNonEmpty(input.listing.capabilityRef)) {
    return fail('listing.capabilityRef must be non-empty')
  }
  if (!isWholeNonNegative(input.listing.priceSats)) {
    return fail('listing.priceSats must be a whole non-negative number of sats')
  }

  const stageIndex = laborProductStageIndex(input.stage)
  const workerRef = input.workerRef ?? null
  const artifactRef = input.artifactRef ?? null

  // Lifecycle coherence: a dispatched-or-later order must name a worker; a
  // delivered-or-later order must carry an artifact ref.
  if (stageIndex >= laborProductStageIndex('dispatched') && workerRef === null) {
    return fail('a dispatched order must name a workerRef')
  }
  if (stageIndex >= laborProductStageIndex('delivered') && artifactRef === null) {
    return fail('a delivered order must carry an artifactRef')
  }

  return {
    ok: true,
    plan: {
      schema: AGENTIC_LABOR_PRODUCT_SCHEMA,
      orderId: input.orderId,
      listing: input.listing,
      buyerRef: input.buyerRef,
      stage: input.stage,
      workerRef,
      artifactRef,
      settlement: {
        accountRef: input.buyerRef,
        receiptRef: laborProductOrderReceiptRef(input.orderId),
        streamKind: AGENTIC_LABOR_PRODUCT_STREAM_KIND,
      },
      promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
      promiseState: 'yellow',
      inert: true,
      // The self-serve blocker is cleared (deployed self-serve order path); only
      // the real-sale-receipt blocker remains until a real ordered+settled sale.
      unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
      createdAt: input.createdAt ?? currentIsoTimestamp(),
    },
  }

  function fail(reason: string): {
    ok: false
    error: LaborProductValidationError
  } {
    return { ok: false, error: new LaborProductValidationError({ reason }) }
  }
}

// ---------------------------------------------------------------------------
// Settlement seam (FLAG-GATED INERT)
// ---------------------------------------------------------------------------

export const LABOR_PRODUCT_SETTLEMENT_DISABLED_REF =
  'blocker.agentic_labor_product.settlement_flag_disabled' as const
export const LABOR_PRODUCT_SETTLEMENT_NOT_DELIVERED_REF =
  'blocker.agentic_labor_product.order_not_delivered' as const
export const LABOR_PRODUCT_SETTLEMENT_NO_OWNER_SIGN_OFF_REF =
  'blocker.agentic_labor_product.owner_sign_off_missing' as const

export type SettleLaborProductOrderInput = Readonly<{
  /** The flow plan whose order is being settled. Must be `delivered`. */
  plan: LaborProductFlowPlan
  /**
   * Owner sign-off ref authorizing this order's settlement. Required to arm the
   * real debit: a flow with no owner sign-off is never settled (green is
   * owner-gated per proof.claim_upgrade_receipts.v1). Absent => not_authorized.
   */
  ownerSignOffRef?: string | undefined
  /** Public-safe adapter/runtime id that produced the delivery (attribution). */
  adapterId: string
  /** ISO clock override (tests). Defaults to the runtime clock. */
  nowIso?: (() => string) | undefined
}>

export type SettleLaborProductOrderResult =
  // Flag off: planned the settlement, touched NO ledger. The default path.
  | Readonly<{ _tag: 'disabled'; receiptRef: string }>
  // The order is not `delivered`, or no owner sign-off was supplied: no debit.
  | Readonly<{ _tag: 'not_authorized'; receiptRef: string; reason: string }>
  // Armed + authorized + delivered: the receipt-first charge ran on the shared
  // ledger; carries the metering outcome (which may itself be zero-charge).
  | Readonly<{ _tag: 'settled'; receiptRef: string; outcome: CloudMeteringOutcome }>

export type SettleLaborProductOrderDeps = Readonly<{
  // FLAG: the seam is INERT unless this is true. Off => `disabled`, no ledger IO.
  enabled: boolean
}>

/**
 * Settle ONE delivered labor-product order against the shared credit ledger.
 *
 * Flow:
 * 1. When the flag is off, return `disabled` — plan only, no ledger row. (Even
 *    before checking authorization, so an inert seam never inspects gates it
 *    will not act on.)
 * 2. When the order is not `delivered`, or no owner sign-off ref is supplied,
 *    return `not_authorized` — no debit. Green is delivery-and-owner-gated.
 * 3. Otherwise settle a receipt-first charge through `settleCloudPrimitiveCharge`
 *    (idempotent per order, never goes negative) and return the outcome.
 *
 * INERT by default. Receipt-first, idempotent per order, owner-gated. Never
 * throws.
 */
export const settleLaborProductOrder = (
  deps: CloudMeteringDeps & SettleLaborProductOrderDeps,
  input: SettleLaborProductOrderInput,
): Effect.Effect<SettleLaborProductOrderResult> => {
  const receiptRef = input.plan.settlement.receiptRef

  // FLAG-GATED INERT: by default the seam plans but does not settle.
  if (!deps.enabled) {
    return Effect.succeed({ _tag: 'disabled', receiptRef } as const)
  }

  if (input.plan.stage !== 'settled' && input.plan.stage !== 'delivered') {
    return Effect.succeed({
      _tag: 'not_authorized',
      receiptRef,
      reason: 'order must be delivered before it can settle',
    } as const)
  }

  const ownerSignOffRef = input.ownerSignOffRef?.trim()
  if (ownerSignOffRef === undefined || ownerSignOffRef.length === 0) {
    return Effect.succeed({
      _tag: 'not_authorized',
      receiptRef,
      reason: 'owner sign-off is required to settle a labor-product order',
    } as const)
  }

  return settleCloudPrimitiveCharge(
    { ledgerDb: deps.ledgerDb, ...(deps.nowIso !== undefined ? { nowIso: deps.nowIso } : {}) },
    {
      accountRef: input.plan.settlement.accountRef,
      chargeId: input.plan.orderId,
      // Receipt-first charge in integer msat from the public-safe sats price.
      chargeMsat: input.plan.listing.priceSats * 1000,
      primitive: AGENTIC_LABOR_PRODUCT_PRIMITIVE,
      adapterId: input.adapterId,
    },
  ).pipe(
    Effect.map(
      outcome => ({ _tag: 'settled', receiptRef, outcome }) as const,
    ),
  )
}

// ---------------------------------------------------------------------------
// Settled-receipt recording (PURE)
// ---------------------------------------------------------------------------
//
// Advances the real-sale-receipt blocker
// (blocker.product_promises.agentic_labor_product_real_sale_receipt_missing) by
// supplying the LAST missing piece of the carry-through: the settlement seam
// (`settleLaborProductOrder`) returns a `settled` RESULT with a public-safe
// receipt ref, but nothing turned that result into (a) a `settled`-stage flow
// plan — the lifecycle's terminal stage was never produced anywhere — or (b) a
// typed, dereferenceable SETTLEMENT RECEIPT artifact. Without that, a real sale
// could settle on the ledger yet never surface as a settled order with a receipt
// a claim-upgrade review could dereference. This PURE function closes that loop.
//
// It does NOT clear the blocker: it only records a settlement that already
// happened. Green still needs a REAL external sale (demand provenance) settled
// under an armed, owner-signed seam — this just makes that settlement
// expressible as a settled order + receipt.

export const AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA =
  'openagents.agentic_labor_product.settlement_receipt.v1' as const

/**
 * A public-safe, dereferenceable settlement receipt for ONE labor-product order
 * that genuinely settled on the ledger. Neutral refs only — no amount,
 * destination, or payment material; the price is intentionally absent (it lives
 * on the listing/ledger, never on the public receipt). Unlike a flow PLAN this
 * is NOT inert: it records that money moved, so it carries no `inert` field. The
 * promise still reports yellow — one settled order does not flip the promise.
 */
export const LaborProductSettlementReceipt = S.Struct({
  schema: S.Literal(AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA),
  /** The order this receipt settles. */
  orderId: S.String,
  /** The listing the order was placed against. */
  listingId: S.String,
  /** Neutral seller ref (the agent that posted the product). */
  sellerRef: S.String,
  /** Neutral buyer ref (the ordering account/agent). */
  buyerRef: S.String,
  /** Neutral account ref that was debited. */
  accountRef: S.String,
  /** The NIP-90 market stream the order settled on. */
  streamKind: S.Literal(AGENTIC_LABOR_PRODUCT_STREAM_KIND),
  /** Dereferenceable public-safe receipt ref the order settled under. */
  receiptRef: S.String,
  /** Always true — a receipt is only minted for a genuine settlement. */
  settled: S.Literal(true),
  promiseIds: S.Tuple([S.Literal(AGENTIC_LABOR_PRODUCTS_PROMISE)]),
  /** Always yellow — one settled order is not a green promise. */
  promiseState: S.Literal('yellow'),
  /** ISO timestamp the receipt was recorded. */
  settledAt: S.String,
})
export type LaborProductSettlementReceipt =
  typeof LaborProductSettlementReceipt.Type

/**
 * Record a genuinely settled labor-product order: turn the settlement seam's
 * `settled` result into (a) a `settled`-stage flow plan and (b) a public-safe,
 * dereferenceable settlement receipt. PURE — it moves no money and reads no
 * ledger; it only transforms a settlement that ALREADY happened.
 *
 * Rejects anything that is not a genuine, metered settlement:
 *   - the seam result must be `settled` (not `disabled`/`not_authorized`);
 *   - the outcome must be `metered` with a non-null receipt ref (a zero-charge
 *     or under-funded outcome moved no money, so it mints no receipt);
 *   - that receipt ref must match the order's own settlement receipt ref;
 *   - the order must be `delivered` (or already `settled`, for idempotent
 *     re-recording).
 * The settled flow plan is rebuilt through `buildLaborProductFlowPlan`, so every
 * lifecycle-coherence guarantee still holds.
 */
export const recordLaborProductSettlement = (
  plan: LaborProductFlowPlan,
  result: SettleLaborProductOrderResult,
  options?: { settledAt?: string },
):
  | {
      ok: true
      plan: LaborProductFlowPlan
      receipt: LaborProductSettlementReceipt
    }
  | { ok: false; error: LaborProductValidationError } => {
  const fail = (
    reason: string,
  ): { ok: false; error: LaborProductValidationError } => ({
    ok: false,
    error: new LaborProductValidationError({ reason }),
  })

  if (result._tag !== 'settled') {
    return fail(
      `only a settled order mints a receipt; settlement was ${result._tag}`,
    )
  }
  if (!result.outcome.metered || result.outcome.receiptRef === null) {
    return fail(
      'only a metered settlement (money moved) mints a dereferenceable receipt',
    )
  }
  if (result.outcome.receiptRef !== plan.settlement.receiptRef) {
    return fail('settlement receipt ref does not match the order receipt ref')
  }
  if (plan.stage !== 'delivered' && plan.stage !== 'settled') {
    return fail(`only a delivered order can settle; order is ${plan.stage}`)
  }

  const built = buildLaborProductFlowPlan({
    orderId: plan.orderId,
    buyerRef: plan.buyerRef,
    listing: plan.listing,
    stage: 'settled',
    workerRef: plan.workerRef,
    artifactRef: plan.artifactRef,
    createdAt: plan.createdAt,
  })
  if (!built.ok) {
    return built
  }

  return {
    ok: true,
    plan: built.plan,
    receipt: {
      schema: AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
      orderId: plan.orderId,
      listingId: plan.listing.listingId,
      sellerRef: plan.listing.sellerRef,
      buyerRef: plan.buyerRef,
      accountRef: plan.settlement.accountRef,
      streamKind: AGENTIC_LABOR_PRODUCT_STREAM_KIND,
      receiptRef: result.outcome.receiptRef,
      settled: true,
      promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
      promiseState: 'yellow',
      settledAt: options?.settledAt ?? currentIsoTimestamp(),
    },
  }
}

// ---------------------------------------------------------------------------
// Self-serve order planning (PURE, INERT)
// ---------------------------------------------------------------------------
//
// Closes blocker.product_promises.not_all_labor_flows_self_serve: before this,
// a labor-product flow plan could only be assembled OPERATOR-side (staged by
// hand, like the lane-c fanout). This is the SELF-SERVE path — a buyer/agent
// posts a listing and orders it in one request and gets back the typed flow
// plan, with NO operator in the loop. It is still INERT: it provisions nothing,
// dispatches nothing, debits nothing, and writes no receipt. The plan it returns
// is pinned to the `ordered` stage (a self-serve buyer can place an order; only
// a worker/operator dispatch+deliver and only an armed+owner-signed seam can
// advance and settle it). The real-sale-receipt blocker stays uncleared.

/** A typed self-serve labor-product order request a buyer submits. */
export const LaborProductOrderRequest = S.Struct({
  /** Stable order id the buyer assigns (idempotency-friendly). */
  orderId: S.String,
  /** Neutral buyer ref (the ordering account/agent). */
  buyerRef: S.String,
  /** The listing being ordered. */
  listing: LaborProductListing,
})
export type LaborProductOrderRequest = typeof LaborProductOrderRequest.Type

/**
 * Decode an untrusted JSON body into a `LaborProductOrderRequest`, returning a
 * validation error rather than throwing. Bounded, neutral fields only.
 */
export const decodeLaborProductOrderRequest = (
  body: unknown,
):
  | { ok: true; request: LaborProductOrderRequest }
  | { ok: false; error: LaborProductValidationError } => {
  if (typeof body !== 'object' || body === null) {
    return {
      ok: false,
      error: new LaborProductValidationError({
        reason: 'request body must be a JSON object',
      }),
    }
  }
  const result = S.decodeUnknownOption(LaborProductOrderRequest)(body)
  if (result._tag === 'None') {
    return {
      ok: false,
      error: new LaborProductValidationError({
        reason: 'request body is not a valid labor-product order',
      }),
    }
  }
  return { ok: true, request: result.value }
}

/**
 * SELF-SERVE: plan a labor-product order from a buyer's own request, with no
 * operator staging. Builds a typed `ordered`-stage flow plan via the same pure
 * `buildLaborProductFlowPlan` validator. INERT: it dispatches nothing, debits
 * nothing, and writes no receipt — it returns the coherent flow plan (including
 * the public-safe receipt ref the order WOULD settle under) the buyer can then
 * see carried forward by a worker dispatch/delivery and an armed, owner-signed
 * settlement seam. The promise stays yellow and the real-sale-receipt blocker
 * stays uncleared.
 */
export const planSelfServeLaborProductOrder = (
  request: LaborProductOrderRequest,
  options?: { createdAt?: string },
):
  | { ok: true; plan: LaborProductFlowPlan }
  | { ok: false; error: LaborProductValidationError } =>
  buildLaborProductFlowPlan({
    orderId: request.orderId,
    buyerRef: request.buyerRef,
    listing: request.listing,
    stage: 'ordered',
    ...(options?.createdAt !== undefined ? { createdAt: options.createdAt } : {}),
  })

// ---------------------------------------------------------------------------
// Forward-only flow transition (PURE, INERT)
// ---------------------------------------------------------------------------
//
// Advances the real-sale-receipt blocker
// (blocker.product_promises.agentic_labor_product_real_sale_receipt_missing) by
// supplying the MISSING CONNECTIVE TISSUE between the self-serve order path
// (which yields an `ordered`-stage plan) and the settlement seam (which only
// acts on a `delivered` plan). Before this there was no typed way to carry an
// ordered order forward through `dispatch` -> `deliver`; the seam could never be
// reached from a self-serve order. This transition is PURE and INERT: it
// dispatches no worker, performs no delivery effect, moves no money, and writes
// no receipt — it only computes the next coherent flow plan, preserving the
// order's identity (orderId, listing, buyer, and the public-safe would-be
// receipt ref). It does NOT clear the blocker: a real external sale carried to a
// settled receipt under an armed, owner-signed seam is still required for green.

/**
 * A forward-only transition step for a labor-product flow:
 *   - `dispatch` assigns the worker an `ordered` order is dispatched to;
 *   - `deliver` attaches the delivered artifact ref to a `dispatched` order.
 * Each step advances the lifecycle by exactly one stage; there is no step that
 * skips, reverses, or settles (settlement is the owner-gated seam, not a plan
 * transition).
 */
export type LaborProductFlowTransition =
  | Readonly<{ kind: 'dispatch'; workerRef: string }>
  | Readonly<{ kind: 'deliver'; artifactRef: string }>

/**
 * Advance a labor-product flow plan by exactly one lifecycle stage. PURE and
 * INERT: nothing is dispatched, delivered, metered, or settled — it returns the
 * next coherent flow plan (rebuilt through the same `buildLaborProductFlowPlan`
 * validator so every coherence guarantee holds) or a validation error.
 *
 * Forward-only:
 *   - `dispatch` requires the flow to be `ordered` and a non-empty workerRef;
 *     it yields a `dispatched` plan naming that worker.
 *   - `deliver` requires the flow to be `dispatched` and a non-empty artifactRef;
 *     it yields a `delivered` plan carrying that artifact (the only stage the
 *     settlement seam acts on), preserving the dispatched worker.
 * The order's identity (orderId, listing, buyerRef, createdAt, and the derived
 * settlement receipt ref) is carried unchanged across the transition.
 */
export const advanceLaborProductFlow = (
  plan: LaborProductFlowPlan,
  transition: LaborProductFlowTransition,
):
  | { ok: true; plan: LaborProductFlowPlan }
  | { ok: false; error: LaborProductValidationError } => {
  if (transition.kind === 'dispatch') {
    if (plan.stage !== 'ordered') {
      return {
        ok: false,
        error: new LaborProductValidationError({
          reason: `dispatch requires an ordered flow; flow is ${plan.stage}`,
        }),
      }
    }
    if (!isNonEmpty(transition.workerRef)) {
      return {
        ok: false,
        error: new LaborProductValidationError({
          reason: 'dispatch requires a non-empty workerRef',
        }),
      }
    }
    return buildLaborProductFlowPlan({
      orderId: plan.orderId,
      buyerRef: plan.buyerRef,
      listing: plan.listing,
      stage: 'dispatched',
      workerRef: transition.workerRef,
      artifactRef: null,
      createdAt: plan.createdAt,
    })
  }

  if (plan.stage !== 'dispatched') {
    return {
      ok: false,
      error: new LaborProductValidationError({
        reason: `deliver requires a dispatched flow; flow is ${plan.stage}`,
      }),
    }
  }
  if (!isNonEmpty(transition.artifactRef)) {
    return {
      ok: false,
      error: new LaborProductValidationError({
        reason: 'deliver requires a non-empty artifactRef',
      }),
    }
  }
  return buildLaborProductFlowPlan({
    orderId: plan.orderId,
    buyerRef: plan.buyerRef,
    listing: plan.listing,
    stage: 'delivered',
    workerRef: plan.workerRef,
    artifactRef: transition.artifactRef,
    createdAt: plan.createdAt,
  })
}

// ---------------------------------------------------------------------------
// End-to-end sale carry-through (COMPOSED, settlement-effectful)
// ---------------------------------------------------------------------------
//
// Advances the real-sale-receipt blocker
// (blocker.product_promises.agentic_labor_product_real_sale_receipt_missing) by
// supplying the ONE composed entry point a real sale actually flows through.
// Every step already existed in isolation — plan the self-serve order, dispatch,
// deliver, settle on the ledger, record the receipt — but a caller had to
// hand-thread FIVE functions and re-feed the seam's `settled` result back into
// `recordLaborProductSettlement` by hand. Nothing carried an order from a buyer's
// order request all the way to a recorded, dereferenceable settlement receipt in
// one call, so there was no single thing a real-sale path (or a claim-upgrade
// review) could invoke and point at. This composes that path.
//
// It does NOT clear the blocker: it changes no defaults. The settlement seam is
// still FLAG-GATED INERT (`disabled` unless armed) and owner-gated
// (`not_authorized` without an owner sign-off ref), so this composition settles
// nothing until those gates are deliberately opened. Green still needs a REAL
// external sale (demand provenance per proof.demand_provenance.v1) carried
// through an armed, owner-signed seam with a published receipt.

/** A request to carry ONE labor-product sale end to end, order -> receipt. */
export type CarryLaborProductSaleInput = Readonly<{
  /** The self-serve order request the buyer submits. */
  request: LaborProductOrderRequest
  /** Neutral worker ref the order is dispatched to. */
  workerRef: string
  /** Public-safe delivered artifact ref. */
  artifactRef: string
  /** Public-safe adapter/runtime id that produced the delivery (attribution). */
  adapterId: string
  /**
   * Owner sign-off ref authorizing settlement. Absent => the seam returns
   * `not_authorized` and no money moves (green is owner-gated).
   */
  ownerSignOffRef?: string | undefined
  /** ISO clock override for the order's createdAt (tests). */
  createdAt?: string | undefined
  /** ISO clock override for the receipt's settledAt (tests). */
  settledAt?: string | undefined
}>

/**
 * The outcome of carrying a sale end to end:
 *   - `recorded`: the order settled on the ledger AND a dereferenceable receipt
 *     was minted (the only success — money moved and is provable);
 *   - `rejected`: a PURE step failed (bad order request, or a coherence guard on
 *     dispatch/deliver), carrying the failing stage;
 *   - `disabled`: the settlement flag is off (the default INERT path);
 *   - `not_authorized`: armed but no owner sign-off (or order not delivered);
 *   - `not_settled`: the seam ran but moved no money (zero-charge / under-funded),
 *     so no receipt is minted — honest about a settlement that did not happen.
 */
export type CarryLaborProductSaleResult =
  | Readonly<{
      _tag: 'recorded'
      plan: LaborProductFlowPlan
      receipt: LaborProductSettlementReceipt
      outcome: CloudMeteringOutcome
    }>
  | Readonly<{ _tag: 'rejected'; stage: LaborProductStage; reason: string }>
  | Readonly<{ _tag: 'disabled'; receiptRef: string }>
  | Readonly<{ _tag: 'not_authorized'; receiptRef: string; reason: string }>
  | Readonly<{ _tag: 'not_settled'; receiptRef: string; reason: string }>

/**
 * Carry ONE labor-product sale end to end: plan the self-serve order, dispatch
 * it to a worker, deliver the artifact, settle the order on the shared ledger,
 * and record the settled-stage flow plan + dereferenceable receipt — in a single
 * call. The PURE steps are composed exactly as the unit functions define them
 * (so every coherence and receipt-matching guard still holds); the only
 * side-effecting step is the FLAG-GATED, owner-gated `settleLaborProductOrder`.
 *
 * Honest by construction: it returns `disabled`/`not_authorized`/`not_settled`
 * rather than fabricating a receipt, so a receipt is minted ONLY when money
 * genuinely moved on the ledger. The promise stays yellow.
 */
export const carryLaborProductOrderToSettlement = (
  deps: CloudMeteringDeps & SettleLaborProductOrderDeps,
  input: CarryLaborProductSaleInput,
): Effect.Effect<CarryLaborProductSaleResult> =>
  Effect.gen(function* () {
    const ordered = planSelfServeLaborProductOrder(
      input.request,
      input.createdAt !== undefined ? { createdAt: input.createdAt } : undefined,
    )
    if (!ordered.ok) {
      return { _tag: 'rejected', stage: 'ordered', reason: ordered.error.reason }
    }

    const dispatched = advanceLaborProductFlow(ordered.plan, {
      kind: 'dispatch',
      workerRef: input.workerRef,
    })
    if (!dispatched.ok) {
      return {
        _tag: 'rejected',
        stage: 'dispatched',
        reason: dispatched.error.reason,
      }
    }

    const delivered = advanceLaborProductFlow(dispatched.plan, {
      kind: 'deliver',
      artifactRef: input.artifactRef,
    })
    if (!delivered.ok) {
      return {
        _tag: 'rejected',
        stage: 'delivered',
        reason: delivered.error.reason,
      }
    }

    const settlement = yield* settleLaborProductOrder(deps, {
      plan: delivered.plan,
      adapterId: input.adapterId,
      ...(input.ownerSignOffRef !== undefined
        ? { ownerSignOffRef: input.ownerSignOffRef }
        : {}),
    })
    if (settlement._tag === 'disabled') {
      return { _tag: 'disabled', receiptRef: settlement.receiptRef }
    }
    if (settlement._tag === 'not_authorized') {
      return {
        _tag: 'not_authorized',
        receiptRef: settlement.receiptRef,
        reason: settlement.reason,
      }
    }

    const recorded = recordLaborProductSettlement(
      delivered.plan,
      settlement,
      input.settledAt !== undefined ? { settledAt: input.settledAt } : undefined,
    )
    if (!recorded.ok) {
      // Settled on the ledger but moved no money (zero-charge / under-funded):
      // no receipt is minted. Honest, not a failure of the carry-through.
      return {
        _tag: 'not_settled',
        receiptRef: settlement.receiptRef,
        reason: recorded.error.reason,
      }
    }

    return {
      _tag: 'recorded',
      plan: recorded.plan,
      receipt: recorded.receipt,
      outcome: settlement.outcome,
    }
  })

// ---------------------------------------------------------------------------
// Read-only store + public projection
// ---------------------------------------------------------------------------

/**
 * A read-only labor-product flow store. Injected so the surface stays pure and
 * testable; the live Worker passes an empty store while the flow is INERT.
 */
export type LaborProductFlowStore = {
  list: () => ReadonlyArray<LaborProductFlowPlan>
}

export const emptyLaborProductFlowStore: LaborProductFlowStore = {
  list: () => [],
}

export const makeInMemoryLaborProductFlowStore = (
  plans: ReadonlyArray<LaborProductFlowPlan>,
): LaborProductFlowStore => ({
  list: () => plans,
})

/**
 * Staleness contract for the labor-product projection. Built fresh from the
 * injected store on every request, so it is `live_at_read` (maxStaleness 0).
 */
export const LaborProductFlowStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['agentic_labor_product_flow_changed'])

/**
 * Public-safe labor-product flow listing projection. Honest: the surface is a
 * scaffold; the promise stays yellow and every flow is inert.
 */
export const listLaborProductFlows = (
  store: LaborProductFlowStore,
): {
  schema: typeof AGENTIC_LABOR_PRODUCT_SCHEMA
  promiseIds: readonly [typeof AGENTIC_LABOR_PRODUCTS_PROMISE]
  promiseState: 'yellow'
  inert: true
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  unclearedBlockerRefs: ReadonlyArray<string>
  flows: ReadonlyArray<LaborProductFlowPlan>
} => ({
  schema: AGENTIC_LABOR_PRODUCT_SCHEMA,
  promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
  promiseState: 'yellow',
  inert: true,
  generatedAt: currentIsoTimestamp(),
  maxStalenessSeconds: LaborProductFlowStaleness.maxStalenessSeconds,
  staleness: LaborProductFlowStaleness,
  // Self-serve blocker cleared (deployed self-serve order path); only the
  // real-sale-receipt blocker remains.
  unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
  flows: store.list(),
})

/** Read one labor-product flow by order id, or null when absent. */
export const readLaborProductFlow = (
  store: LaborProductFlowStore,
  orderId: string,
): LaborProductFlowPlan | null =>
  store.list().find(flow => flow.orderId === orderId) ?? null

// ---------------------------------------------------------------------------
// Settlement-receipt dereference (read-only, INERT)
// ---------------------------------------------------------------------------
//
// Advances the real-sale-receipt blocker
// (blocker.product_promises.agentic_labor_product_real_sale_receipt_missing) by
// supplying the missing READ PATH that makes a recorded settlement receipt
// actually DEREFERENCEABLE. `recordLaborProductSettlement` /
// `carryLaborProductOrderToSettlement` MINT a typed `LaborProductSettlementReceipt`
// when money genuinely moves, but nothing could resolve one back from its
// public-safe `receiptRef`: the flow store reads flows by `orderId`, and a
// receipt's whole point is to be looked up by its receipt ref (the value a
// claim-upgrade review under proof.claim_upgrade_receipts.v1 is handed). Without
// this, a real sale could settle and mint a receipt that no public surface could
// resolve — "dereferenceable" in name only.
//
// It does NOT clear the blocker: it adds the read seam, but the store is EMPTY in
// production (no real receipt has ever been published), so this dereferences
// nothing live. Green still needs a REAL external sale (demand provenance)
// settled under an armed, owner-signed seam whose minted receipt is published
// into a store this read path can then resolve.

/**
 * A read-only settlement-receipt store. Injected so the surface stays pure and
 * testable; the live Worker passes the empty store while the flow is INERT (no
 * real settled receipt has ever been published).
 */
export type LaborProductReceiptStore = {
  list: () => ReadonlyArray<LaborProductSettlementReceipt>
}

export const emptyLaborProductReceiptStore: LaborProductReceiptStore = {
  list: () => [],
}

export const makeInMemoryLaborProductReceiptStore = (
  receipts: ReadonlyArray<LaborProductSettlementReceipt>,
): LaborProductReceiptStore => ({
  list: () => receipts,
})

/**
 * Dereference ONE settlement receipt by its public-safe `receiptRef`, or null
 * when no settled receipt with that ref exists. This is the resolution a
 * claim-upgrade review performs against a published receipt ref. Read-only and
 * INERT: it moves no money and reads no ledger.
 */
export const readLaborProductSettlementReceipt = (
  store: LaborProductReceiptStore,
  receiptRef: string,
): LaborProductSettlementReceipt | null =>
  store.list().find(receipt => receipt.receiptRef === receiptRef) ?? null

/**
 * Public-safe settlement-receipt listing projection. Honest: in production the
 * store is empty (no real settled receipt has been published), so `receipts` is
 * `[]`, the promise stays yellow, and the uncleared real-sale-receipt blocker is
 * surfaced. `live_at_read` (rebuilt from the store each request).
 */
export const listLaborProductSettlementReceipts = (
  store: LaborProductReceiptStore,
): {
  schema: typeof AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA
  promiseIds: readonly [typeof AGENTIC_LABOR_PRODUCTS_PROMISE]
  promiseState: 'yellow'
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  unclearedBlockerRefs: ReadonlyArray<string>
  receipts: ReadonlyArray<LaborProductSettlementReceipt>
} => ({
  schema: AGENTIC_LABOR_PRODUCT_SETTLEMENT_RECEIPT_SCHEMA,
  promiseIds: [AGENTIC_LABOR_PRODUCTS_PROMISE],
  promiseState: 'yellow',
  generatedAt: currentIsoTimestamp(),
  maxStalenessSeconds: LaborProductFlowStaleness.maxStalenessSeconds,
  staleness: LaborProductFlowStaleness,
  unclearedBlockerRefs: [LABOR_PRODUCT_NO_REAL_SALE_RECEIPT_REF],
  receipts: store.list(),
})
