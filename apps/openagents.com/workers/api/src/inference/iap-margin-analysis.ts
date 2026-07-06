// MM-E3 (#8483): effective margin per IAP credit pack after the store's cut.
//
// The credit-pack rail (#8482) grants the FULL face value of a purchase as
// spendable credit (never discounted for the store's cut) — so the store's
// 15-30% cut comes entirely out of OUR margin, not the user's balance. This
// module computes exactly how much margin survives, backing the numbers in
// the compliance checklist doc rather than asserting them in prose alone.

import { DEFAULT_MARGIN } from './pricing'

export const APPLE_GOOGLE_STANDARD_STORE_CUT = 0.3
/** Apple's Small Business Program / Google's equivalent tier: developers
 * with <$1M in prior-year proceeds pay 15% instead of 30%. Requires annual
 * enrollment/re-qualification — an owner action, not automatic. */
export const APPLE_GOOGLE_SMALL_BUSINESS_STORE_CUT = 0.15

export type IapPackMarginAnalysis = Readonly<{
  packAmountUsdCents: number
  storeCutFraction: number
  /** What we actually receive after the store's cut. */
  netCashUsdCents: number
  /** Our normal compute cost to deliver the FULL face value of credit at the
   * configured inference margin (faceValue / (1 + inferenceMargin)). */
  computeCostUsdCents: number
  /** netCash - computeCost. NEGATIVE means this pack runs at a loss if the
   * user fully spends the granted credit on inference. */
  profitIfFullySpentUsdCents: number
  /** profit / netCash, as a fraction (e.g. -0.0204 = -2.04%). */
  effectiveMarginOnNetCash: number
}>

/**
 * Computes the effective margin for one credit pack. `inferenceMargin`
 * defaults to this repo's `DEFAULT_MARGIN` (pricing.ts) — the SAME margin
 * the inference gateway actually charges at, so this is not a hypothetical
 * number but the real one that applies once the granted credit is spent.
 */
export const analyzeIapPackMargin = (input: Readonly<{
  packAmountUsdCents: number
  storeCutFraction: number
  inferenceMargin?: number
}>): IapPackMarginAnalysis => {
  const inferenceMargin = input.inferenceMargin ?? DEFAULT_MARGIN
  const netCashUsdCents = input.packAmountUsdCents * (1 - input.storeCutFraction)
  const computeCostUsdCents = input.packAmountUsdCents / (1 + inferenceMargin)
  const profitIfFullySpentUsdCents = netCashUsdCents - computeCostUsdCents
  const effectiveMarginOnNetCash =
    netCashUsdCents === 0 ? 0 : profitIfFullySpentUsdCents / netCashUsdCents

  return {
    computeCostUsdCents,
    effectiveMarginOnNetCash,
    netCashUsdCents,
    packAmountUsdCents: input.packAmountUsdCents,
    profitIfFullySpentUsdCents,
    storeCutFraction: input.storeCutFraction,
  }
}
