// Monetize-any-layer + referral-on-everything seam (EPIC #5510, child #5518;
// promise marketplace.monetize_any_layer_with_referral.v1).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): anyone (or their
// agents) can monetize / sell access to ANY layer of the stack and earn
// referrals on it (e.g. refer a bulk-inference client and get a piece).
//
// SCOPE / HONESTY: this is an INERT scaffold. It is PURE — it moves no money,
// meters no spend, opens no wallet, and writes no receipt. It defines the typed
// per-layer monetization seam (a price + a referral split per layer) and PLANS
// the referral accrual a future metered-spend event WOULD produce, while
// running the existing no-resale / asset-boundary guards so the seam can never
// authorize a prohibited resale. The promise
// marketplace.monetize_any_layer_with_referral.v1 STAYS `planned`; nothing here
// flips it green and the no-resale invariant is never waived for subscription
// accounts. A green flip stays receipt-first and owner-signed per
// proof.claim_upgrade_receipts.v1.

import { Schema as S } from 'effect'

import type { AssetBoundaryAsset } from './asset-bitcoin-boundary'
import {
  type InferenceMonetizationKind,
  type ProviderAccountAuthMode,
  authorizeInferenceMonetization,
} from './inference-resale-authorization'
import type { MarketplaceComposablePrimitive } from './marketplace-product-composition'
import { currentIsoTimestamp } from './runtime-primitives'

export const MARKETPLACE_MONETIZE_ANY_LAYER_SCHEMA =
  'openagents.marketplace_monetize_any_layer.v1' as const

export const MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE =
  'marketplace.monetize_any_layer_with_referral.v1' as const

/** Referral cut basis-point bound (0..10000 BPS == 0%..100%). */
export const MONETIZE_LAYER_REFERRAL_MAX_BPS = 10000 as const

export const MONETIZE_LAYER_INVALID_PRICE_REF =
  'blocker.monetize_any_layer.invalid_price' as const
export const MONETIZE_LAYER_INVALID_REFERRAL_BPS_REF =
  'blocker.monetize_any_layer.invalid_referral_bps' as const
export const MONETIZE_LAYER_SELF_REFERRAL_REF =
  'blocker.monetize_any_layer.self_referral' as const

/**
 * The asset a layer's access price is denominated in, reusing the shared
 * credit<->Bitcoin asset-boundary vocabulary so the seam speaks one language.
 */
export type MonetizeLayerPriceAsset = AssetBoundaryAsset

/**
 * A per-layer access-selling definition: a seller exposes/resells access to ONE
 * layer at a price, with a referral split (in basis points) accruing to a
 * distinct referrer off that layer's metered spend. Definition only — INERT.
 */
export const LayerMonetizationDefinition = S.Struct({
  schema: S.Literal(MARKETPLACE_MONETIZE_ANY_LAYER_SCHEMA),
  /** Stable id for this per-layer offer. */
  offerId: S.String,
  /** Neutral seller ref (agent/user ref); no name is required. */
  sellerRef: S.String,
  /** The layer being sold (one of the composable primitives). */
  layer: S.String,
  /**
   * The monetization kind, which selects the no-resale guard branch. Only
   * `agentic_work` and `api_inference_gateway_resale` are ever authorizable;
   * `subscription_capacity_resale` is non-waivably blocked.
   */
  monetizationKind: S.Literals([
    'agentic_work',
    'api_inference_gateway_resale',
    'subscription_capacity_resale',
  ]),
  /** Access price per metered unit (msat), and its asset denomination. */
  unitPriceMsat: S.Number,
  priceAsset: S.Literals(['bitcoin', 'credit', 'free', 'usd']),
  /** Referral cut in basis points of the layer's metered spend. */
  referralBps: S.Number,
  /** Neutral referrer ref the cut accrues to (must differ from seller). */
  referrerRef: S.String,
  promiseId: S.Literal(MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE),
  createdAt: S.String,
})
export type LayerMonetizationDefinition =
  typeof LayerMonetizationDefinition.Type

export class LayerMonetizationValidationError extends S.TaggedErrorClass<LayerMonetizationValidationError>()(
  'LayerMonetizationValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

/**
 * Build a typed per-layer monetization definition. PURE and validating; pins
 * the promise id to the planned promise so no offer can over-claim.
 */
export const buildLayerMonetizationDefinition = (input: {
  offerId: string
  sellerRef: string
  layer: MarketplaceComposablePrimitive
  monetizationKind: InferenceMonetizationKind
  unitPriceMsat: number
  priceAsset: MonetizeLayerPriceAsset
  referralBps: number
  referrerRef: string
  createdAt?: string
}):
  | { ok: true; definition: LayerMonetizationDefinition }
  | { ok: false; error: LayerMonetizationValidationError } => {
  if (!isNonEmpty(input.offerId)) {
    return fail('offerId must be non-empty')
  }
  if (!isNonEmpty(input.sellerRef)) {
    return fail('sellerRef must be non-empty')
  }
  if (!isNonEmpty(input.referrerRef)) {
    return fail('referrerRef must be non-empty')
  }
  if (
    !Number.isFinite(input.unitPriceMsat) ||
    input.unitPriceMsat < 0 ||
    !Number.isInteger(input.unitPriceMsat)
  ) {
    return fail('unitPriceMsat must be a non-negative integer')
  }
  if (
    !Number.isInteger(input.referralBps) ||
    input.referralBps < 0 ||
    input.referralBps > MONETIZE_LAYER_REFERRAL_MAX_BPS
  ) {
    return fail('referralBps must be an integer in [0, 10000]')
  }

  return {
    ok: true,
    definition: {
      schema: MARKETPLACE_MONETIZE_ANY_LAYER_SCHEMA,
      offerId: input.offerId,
      sellerRef: input.sellerRef,
      layer: input.layer,
      monetizationKind: input.monetizationKind,
      unitPriceMsat: input.unitPriceMsat,
      priceAsset: input.priceAsset,
      referralBps: input.referralBps,
      referrerRef: input.referrerRef,
      promiseId: MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE,
      createdAt: input.createdAt ?? currentIsoTimestamp(),
    },
  }

  function fail(reason: string): {
    ok: false
    error: LayerMonetizationValidationError
  } {
    return { ok: false, error: new LayerMonetizationValidationError({ reason }) }
  }
}

/**
 * The planned outcome of a metered-spend event against a per-layer offer.
 * Computed PURELY — no money moves and no receipt is written. `inert: true`
 * and the planned promise state are always present so callers cannot mistake a
 * plan for a settlement.
 */
export type LayerMonetizationAccrualPlan = {
  schema: typeof MARKETPLACE_MONETIZE_ANY_LAYER_SCHEMA
  promiseId: typeof MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE
  promiseState: 'planned'
  inert: true
  /** Whether the no-resale / boundary / self-referral guards permit this plan. */
  authorized: boolean
  /** Typed blocker refs when not authorized (reused + seam-local refs). */
  blockerRefs: ReadonlyArray<string>
  /** The metered spend the plan is computed over (msat). */
  meteredSpendMsat: number
  /** The referral cut a settled event WOULD accrue (msat), 0 when blocked. */
  referralAccrualMsat: number
  referrerRef: string
  sellerRef: string
  layer: string
}

/**
 * Compute the referral accrual a metered-spend event WOULD produce against a
 * per-layer offer. PURE / INERT. Runs the existing no-resale guard
 * (authorizeInferenceMonetization) — subscription resale is non-waivably
 * blocked — plus a self-referral guard, and only then computes the BPS cut.
 * When any guard blocks, the accrual is 0 and the blockers are surfaced.
 */
export const planLayerMonetizationAccrual = (input: {
  definition: LayerMonetizationDefinition
  meteredSpendMsat: number
  /** Account auth mode for the resale guard (api_key vs subscription). */
  accountAuthMode?: ProviderAccountAuthMode
  /** The resale ref chain, when proving an api-inference resale offer. */
  resaleRefs?: Parameters<typeof authorizeInferenceMonetization>[0]['refs']
}): LayerMonetizationAccrualPlan => {
  const { definition } = input
  const blockerRefs: string[] = []

  // Reuse the canonical no-resale authorization seam. Subscription-account
  // resale is blocked unconditionally here, so the invariant is never waived.
  // Build the argument with only the defined optional fields so it satisfies
  // exactOptionalPropertyTypes.
  const resaleDecision = authorizeInferenceMonetization({
    kind: definition.monetizationKind,
    ...(input.accountAuthMode !== undefined
      ? { accountAuthMode: input.accountAuthMode }
      : {}),
    ...(input.resaleRefs !== undefined ? { refs: input.resaleRefs } : {}),
  })
  if (!resaleDecision.authorized) {
    blockerRefs.push(...resaleDecision.blockerRefs)
  }

  // Self-referral is not a referral: the referrer must be distinct.
  if (definition.referrerRef === definition.sellerRef) {
    blockerRefs.push(MONETIZE_LAYER_SELF_REFERRAL_REF)
  }

  const meteredSpendMsat =
    Number.isFinite(input.meteredSpendMsat) && input.meteredSpendMsat > 0
      ? Math.floor(input.meteredSpendMsat)
      : 0

  const authorized = blockerRefs.length === 0
  const referralAccrualMsat = authorized
    ? Math.floor((meteredSpendMsat * definition.referralBps) / 10000)
    : 0

  return {
    schema: MARKETPLACE_MONETIZE_ANY_LAYER_SCHEMA,
    promiseId: MARKETPLACE_MONETIZE_ANY_LAYER_PROMISE,
    promiseState: 'planned',
    inert: true,
    authorized,
    blockerRefs,
    meteredSpendMsat,
    referralAccrualMsat,
    referrerRef: definition.referrerRef,
    sellerRef: definition.sellerRef,
    layer: definition.layer,
  }
}
