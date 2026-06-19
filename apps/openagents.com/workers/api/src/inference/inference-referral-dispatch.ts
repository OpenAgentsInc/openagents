// Inference referral payout dispatch (EPIC #5474 / sub-EPIC #5475, child #5490).
//
// A THIN inference-facing wrapper over the already-built RL-2 dispatch primitive
// (`site-referral-payout-dispatch.ts` -> `dispatchReferralPayoutSettlement`). It
// does NOT reimplement dispatch: it reuses the same idempotent,
// readiness-gated, asset-boundary-enforced settle path, so an inference referral
// payout and a site-checkout referral payout move money through ONE rail.
//
// MONEY-SAFETY (inherited from the primitive, restated for this entry point):
// - Idempotent: deterministic per-payout transition + adapter keys; a retried
//   dispatch settles AT MOST ONCE; an already-settled payout returns its entry.
// - Readiness-gated / OWNER-ARMED: the injected readiness gate
//   (`livePayoutClaimAllowed`) must allow live payouts. Until the owner arms the
//   live payout mode, the gate is false and dispatch refuses — so the FIRST real
//   inference referral payout is owner-armed, never auto-fired (mirrors the
//   `blocker.product_promises.referral_first_real_payout_pending` posture). Tests
//   inject a gate that is false (no live payout) or a mock adapter (no money).
// - Asset boundary (RL-3): only Bitcoin-funded inference revenue may move
//   withdrawable Bitcoin. Card/USD-funded inference accrues CREDIT revshare and
//   is refused for Bitcoin dispatch by the shared boundary guard.

import {
  type SiteReferralPayoutDispatchDependencies,
  type SiteReferralPayoutDispatchOutcome,
  dispatchReferralPayoutSettlement,
} from '../site-referral-payout-dispatch'
import { type SiteReferralRevenueAsset } from '../site-referral-payout-feed'

export type DispatchInferenceReferralPayoutInput = Readonly<{
  payoutRef: string
  // Rev-share asset of the qualifying inference revenue. Only `bitcoin` may
  // dispatch withdrawable Bitcoin; `usd`/`credit` are refused by the boundary.
  // The operator/caller supplies it from the payout's funding lineage.
  revenueAsset: SiteReferralRevenueAsset
}>

/**
 * Dispatch one accrued inference referral payout through the shared RL-2 rail,
 * idempotently and readiness-gated (owner-armed). Returns the primitive's
 * outcome unchanged (`settled` | `already_settled` | `refused`).
 */
export const dispatchInferenceReferralPayout = async (
  db: D1Database,
  dependencies: SiteReferralPayoutDispatchDependencies,
  input: DispatchInferenceReferralPayoutInput,
): Promise<SiteReferralPayoutDispatchOutcome> =>
  dispatchReferralPayoutSettlement(db, dependencies, {
    payoutRef: input.payoutRef,
    revenueAsset: input.revenueAsset,
  })
