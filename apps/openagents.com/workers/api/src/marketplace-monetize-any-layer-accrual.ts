// Bridge: monetize-any-layer offer + a real metered-spend event -> the ONE
// cross-category referral ledger (EPIC #5510, children #5513/#5518; promises
// marketplace.monetize_any_layer_with_referral.v1 + referral.refer_once_earn_forever.v1).
//
// THE GAP THIS CLOSES: `planLayerMonetizationAccrual` (marketplace-monetize-any-
// layer.ts) computes the referral cut a metered-spend event WOULD produce, but it
// is a pure plan with no path to the actual ledger -- it accrued into the void.
// `accrueCrossCategoryReferral` (referral-cross-category-accrual.ts) is the
// category-agnostic ledger entry point, but nothing in the monetize-any-layer
// seam called it. This module is the missing seam between them: it runs the SAME
// no-resale / asset-boundary / self-referral guards the plan runs, and ONLY when
// the plan is authorized does it feed the cut into the ONE RL-1 ledger through
// `accrueCrossCategoryReferral` (so there is never a parallel ledger or a second
// percentage). It reuses the offer's `referralBps` to compute the qualifying
// amount and lets the ledger apply its own standing 5% policy + caps.
//
// SCOPE / HONESTY: this is FLAG-GATED INERT. By default (`enabled: false`) it
// computes the plan and refuses to touch the ledger -- it returns the plan and a
// `disabled` tag, moving NO money and writing NO row. Even when armed it only
// records an ELIGIBILITY row; settlement stays on the readiness-gated, owner-armed
// dispatch rail (site-referral-payout-dispatch.ts), unchanged. The promises stay
// `planned`/`red`; nothing here flips a promise green. A green flip stays
// receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.

import {
  type AccrueCrossCategoryReferralResult,
  accrueCrossCategoryReferral,
} from './referral-cross-category-accrual'
import type { ProviderAccountAuthMode } from './inference-resale-authorization'
import {
  type LayerMonetizationAccrualPlan,
  type LayerMonetizationDefinition,
  planLayerMonetizationAccrual,
} from './marketplace-monetize-any-layer'
import type { ReferredPrincipal } from './referral-cross-category-accrual'
import type { SiteReferralRevenueAsset } from './site-referral-payout-feed'
import { authorizeInferenceMonetization } from './inference-resale-authorization'

export const MONETIZE_LAYER_ACCRUAL_DISABLED_REF =
  'blocker.monetize_any_layer.accrual_flag_disabled' as const

// 1 sat == 1000 msat. The cross-category ledger is sat-denominated; the offer
// plan is msat-denominated. Convert the qualifying spend DOWN to whole sats so a
// referral cut is never inflated past the spend it is computed from. Sub-sat
// spend yields 0 sats (the ledger then accrues nothing this event -- not an
// error), mirroring the inference accrual's below-1-sat behavior.
const MSAT_PER_SAT = 1000 as const

export const monetizeLayerSpendMsatToQualifyingSats = (msat: number): number =>
  Number.isFinite(msat) && msat > 0 ? Math.floor(msat / MSAT_PER_SAT) : 0

// Map the offer's price asset onto the ledger's rev-share asset vocabulary. The
// monetize-any-layer offer prices in the asset-boundary vocabulary
// (bitcoin/credit/free/usd); the ledger feed speaks bitcoin/credit/usd. `free`
// has no revenue to share, so it maps to `usd` (credit revshare, which the
// boundary then allows but which carries no withdrawable Bitcoin liability) and
// will accrue nothing because free offers have zero qualifying spend.
const priceAssetToRevenueAsset = (
  asset: LayerMonetizationDefinition['priceAsset'],
): SiteReferralRevenueAsset => {
  switch (asset) {
    case 'bitcoin':
      return 'bitcoin'
    case 'usd':
      return 'usd'
    case 'credit':
      return 'credit'
    case 'free':
      return 'usd'
  }
}

export type AccrueMonetizeLayerReferralInput = Readonly<{
  // The per-layer offer the spend ran against.
  definition: LayerMonetizationDefinition
  // The metered spend the referral cut is computed over (msat).
  meteredSpendMsat: number
  // Deterministic per-spend event id (idempotency anchor). One accrual per
  // (layer, event); a replay is a ledger no-op.
  eventId: string
  // The paying principal whose PERMANENT attribution decides the referrer. The
  // offer's `referrerRef` is the seller's CHOSEN referrer for self-referral
  // guarding only; the LEDGER pays the principal's actual attributed referrer,
  // never a seller-asserted one. (Refer-once-earn-forever: the binding is the
  // referee's, not the offer's.)
  principal: ReferredPrincipal
  // Account auth mode for the no-resale guard (api_key vs subscription).
  accountAuthMode?: ProviderAccountAuthMode
  // The resale ref chain, when proving an api-inference resale offer.
  resaleRefs?: Parameters<typeof authorizeInferenceMonetization>[0]['refs']
  // ISO clock override (tests). Defaults to the runtime clock (inside accrual).
  nowIso?: (() => string) | undefined
}>

export type AccrueMonetizeLayerReferralResult =
  // Flag off: computed the plan, touched NO ledger. The default path.
  | Readonly<{ _tag: 'disabled'; plan: LayerMonetizationAccrualPlan }>
  // A guard blocked the plan (resale / boundary / self-referral): no ledger row.
  | Readonly<{
      _tag: 'unauthorized'
      plan: LayerMonetizationAccrualPlan
    }>
  // The plan is authorized and the ledger accrual ran; carries BOTH the plan and
  // the ledger result (which may itself be no_attribution / zero / recorded ...).
  | Readonly<{
      _tag: 'accrued'
      plan: LayerMonetizationAccrualPlan
      accrual: AccrueCrossCategoryReferralResult
    }>

export type AccrueMonetizeLayerReferralDeps = Readonly<{
  // FLAG: the seam is INERT unless this is true. Off => `disabled`, no ledger IO.
  enabled: boolean
}>

/**
 * Accrue the referrer's cut for ONE metered-spend event against a per-layer
 * offer, into the ONE cross-category referral ledger.
 *
 * Flow:
 * 1. Compute the monetize-any-layer plan (runs the no-resale / boundary /
 *    self-referral guards). When a guard blocks, return `unauthorized` -- no
 *    ledger row.
 * 2. When the flag is off, return `disabled` -- plan only, no ledger row.
 * 3. Otherwise feed the plan's qualifying spend (converted msat -> sats) into
 *    `accrueCrossCategoryReferral` under a layer-namespaced category, and return
 *    the ledger result. The ledger applies its standing 5% policy + caps and its
 *    OWN attribution resolution + asset-boundary check, so this seam never
 *    double-cuts and never pays a seller-asserted referrer.
 *
 * INERT by default. Records eligibility only -- moves NO money. Idempotent per
 * (layer, event).
 */
export const accrueMonetizeLayerReferral = async (
  db: D1Database,
  deps: AccrueMonetizeLayerReferralDeps,
  input: AccrueMonetizeLayerReferralInput,
): Promise<AccrueMonetizeLayerReferralResult> => {
  const plan = planLayerMonetizationAccrual({
    definition: input.definition,
    meteredSpendMsat: input.meteredSpendMsat,
    ...(input.accountAuthMode !== undefined
      ? { accountAuthMode: input.accountAuthMode }
      : {}),
    ...(input.resaleRefs !== undefined ? { resaleRefs: input.resaleRefs } : {}),
  })

  // A guard blocked the plan (subscription resale, missing ref chain, self-
  // referral). Never touch the ledger.
  if (!plan.authorized) {
    return { _tag: 'unauthorized', plan }
  }

  // FLAG-GATED INERT: by default the seam plans but does not accrue.
  if (!deps.enabled) {
    return { _tag: 'disabled', plan }
  }

  // The plan's qualifying spend (msat) -> whole sats for the sat-denominated
  // ledger. The ledger applies the standing 5% policy; we feed the qualifying
  // amount, never a pre-cut. `referralBps` shaped the plan's authorization but
  // the LEDGER owns the actual percentage, so the offer cannot widen its cut.
  const qualifyingAmountSats = monetizeLayerSpendMsatToQualifyingSats(
    plan.meteredSpendMsat,
  )

  const accrual = await accrueCrossCategoryReferral(db, {
    // Layer-namespaced category so a monetize-any-layer event for the SAME
    // underlying id never collides with another category's event for that id.
    category: `monetize_${input.definition.layer}`,
    eventId: input.eventId,
    principal: input.principal,
    qualifyingAmountSats,
    qualifyingEventKind: `monetize_any_layer.${input.definition.monetizationKind}`,
    revenueAsset: priceAssetToRevenueAsset(input.definition.priceAsset),
    ...(input.nowIso !== undefined ? { nowIso: input.nowIso } : {}),
  })

  return { _tag: 'accrued', plan, accrual }
}
