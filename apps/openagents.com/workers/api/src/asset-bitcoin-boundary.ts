import { Schema as S } from 'effect'

/**
 * RL-3 (openagents #5460, EPIC #5457 / §2H "Rev-share invariant"): the single,
 * shared credit<->Bitcoin asset-boundary guard.
 *
 * The governing invariant the Video-2 plan states on camera and that this guard
 * makes literally true in execution:
 *
 *   > "Bitcoin revenue can create Bitcoin revenue share. Credit spend creates
 *   >  credit revenue share. Free or promotional spend does not create
 *   >  withdrawable Bitcoin revenue share."
 *
 * Before this module, the boundary lived in two disconnected places: a private
 * `validateAssetBoundary` inside `site-commerce-revenue-share.ts` (a read-only
 * projection imported only by its test) and ad-hoc inline `revenueAsset !==
 * 'bitcoin'` checks on the live referral-dispatch and firm-up-settlement paths.
 * This module is the ONE primitive both the projection and every live
 * value-movement path call, so the invariant is enforced identically wherever
 * value crosses the credit<->Bitcoin line (purchase, spend, revshare, payout).
 *
 * It is PURE. It moves no money, reads no wallet, writes no receipt. It returns
 * a typed, public-safe, reason-qualified denial when a movement would violate
 * the boundary, and `null` when the movement is allowed. Callers fail closed on
 * a denial.
 */

/**
 * The asset a unit of value is denominated in as it crosses a boundary.
 * - `bitcoin`: Bitcoin/Lightning-sourced revenue or a withdrawable Bitcoin
 *   payout. The ONLY asset that may fund a withdrawable Bitcoin liability.
 * - `credit`: internal OpenAgents credit-balance spend -> credit revshare.
 * - `usd`: fiat credit purchase (Stripe top-up) -> credit revshare. Chargeback-
 *   aware; never a Bitcoin liability.
 * - `free`: free / promotional grant. Creates NO withdrawable revenue share at
 *   all (neither credit nor Bitcoin).
 */
export const AssetBoundaryAsset = S.Literals(['bitcoin', 'credit', 'free', 'usd'])
export type AssetBoundaryAsset = typeof AssetBoundaryAsset.Type

/**
 * The class of value movement being checked. Each names a distinct point where
 * value crosses the credit<->Bitcoin boundary so a denial reason is precise.
 */
export const AssetBoundaryMovement = S.Literals([
  'purchase',
  'spend',
  'revshare',
  'payout',
])
export type AssetBoundaryMovement = typeof AssetBoundaryMovement.Type

/** The public-safe denial reason refs this guard can emit. */
export const ASSET_BOUNDARY_CREDIT_REVENUE_NO_BITCOIN_REF =
  'reason.public.asset_boundary.credit_revenue_no_bitcoin_share' as const
export const ASSET_BOUNDARY_FREE_NO_WITHDRAWABLE_BITCOIN_REF =
  'reason.public.asset_boundary.free_or_promo_no_withdrawable_bitcoin' as const

export class AssetBoundaryViolation extends S.TaggedErrorClass<AssetBoundaryViolation>()(
  'AssetBoundaryViolation',
  {
    contributorAsset: AssetBoundaryAsset,
    movement: AssetBoundaryMovement,
    reason: S.String,
    reasonRef: S.String,
    revenueAsset: AssetBoundaryAsset,
  },
) {}

export type AssetBoundaryCheckInput = Readonly<{
  /** The asset the qualifying REVENUE was sourced in. */
  revenueAsset: AssetBoundaryAsset
  /**
   * The asset the resulting share / payout would be denominated in (the
   * contributor/referrer/worker side). For a `payout` movement this is always
   * effectively `bitcoin` (a withdrawable send); callers may still pass it
   * explicitly.
   */
  contributorAsset: AssetBoundaryAsset
  /** Which boundary crossing is being checked (for a precise denial reason). */
  movement: AssetBoundaryMovement
}>

/**
 * Decide whether a value movement respects the credit<->Bitcoin boundary.
 * Returns `null` when allowed, or a typed `AssetBoundaryViolation` (fail-closed)
 * when it would:
 *   - turn credit/USD (Stripe-credit) revenue into a withdrawable Bitcoin share
 *     (credit revenue funds credit revshare only), or
 *   - turn a free / promotional grant into any withdrawable Bitcoin share.
 *
 * The allowed crossings are exactly: Bitcoin revenue -> Bitcoin share; credit
 * revenue -> credit share; USD revenue -> credit share; free -> (no Bitcoin).
 */
export const validateAssetBoundary = (
  input: AssetBoundaryCheckInput,
): AssetBoundaryViolation | null => {
  const contributorWantsBitcoin = input.contributorAsset === 'bitcoin'

  if (!contributorWantsBitcoin) {
    // A credit/USD/free-denominated share never crosses into withdrawable
    // Bitcoin liability, so any revenue source is fine for it.
    return null
  }

  // Free / promotional revenue can NEVER create a withdrawable Bitcoin share.
  if (input.revenueAsset === 'free') {
    return new AssetBoundaryViolation({
      contributorAsset: input.contributorAsset,
      movement: input.movement,
      reason:
        'free or promotional spend does not create withdrawable Bitcoin revenue share.',
      reasonRef: ASSET_BOUNDARY_FREE_NO_WITHDRAWABLE_BITCOIN_REF,
      revenueAsset: input.revenueAsset,
    })
  }

  // Credit/USD revenue funds credit revshare only; it must not silently create
  // an immediate Bitcoin withdrawal liability.
  if (input.revenueAsset === 'credit' || input.revenueAsset === 'usd') {
    return new AssetBoundaryViolation({
      contributorAsset: input.contributorAsset,
      movement: input.movement,
      reason:
        'credit or fiat-credit revenue may not create withdrawable Bitcoin revenue share; only Bitcoin revenue funds withdrawable Bitcoin.',
      reasonRef: ASSET_BOUNDARY_CREDIT_REVENUE_NO_BITCOIN_REF,
      revenueAsset: input.revenueAsset,
    })
  }

  // Bitcoin revenue -> Bitcoin share: the only allowed Bitcoin crossing.
  return null
}

/** Convenience boolean for callers that only need allow/deny. */
export const assetBoundaryAllows = (input: AssetBoundaryCheckInput): boolean =>
  validateAssetBoundary(input) === null
