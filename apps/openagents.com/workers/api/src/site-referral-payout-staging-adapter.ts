// RL-1 staging-test settlement adapter (#5524 / DE-1).
//
// This is the staging/test-mode sibling of the PRODUCTION hosted-MDK referral
// payout adapter (`site-referral-payout-adapter.ts`). It satisfies the exact
// same `ReferralPayoutAdapter` contract the readiness-gated dispatcher
// (`site-referral-payout-dispatch.ts` -> `dispatchReferralPayoutSettlement`)
// invokes, so a staging-test settlement walks the SAME idempotent,
// readiness-gated, asset-boundary-enforced `approved -> dispatched -> settled`
// path and lands in the SAME ledger as a real payout would.
//
// PURPOSE: the production hosted-MDK adapter can only ever produce a settled,
// dereferenceable receipt once the owner arms a funded payout client and a
// registered referrer destination (#5512). Until then, the public settled
// referral-payout receipt route (`/api/public/site-referral-payout-receipts/...`)
// has never resolved a single real settled row, because no settled row exists.
// This adapter closes that loop in STAGING/TEST MODE: it produces a real-shaped,
// public-safe `receipt.site_referral_payout.staging_test.*` evidence ref that the
// SAME public receipt store dereferences as the `staging_test` settlement rail,
// proving the settlement-receipt surface works end to end BEFORE any real
// Bitcoin moves.
//
// MONEY-SAFETY (this adapter NEVER moves money, by construction):
// - It has no wallet client, no destination resolver, and no rail call. It
//   cannot send Bitcoin even if reached on a production path.
// - It is gated behind an explicit `enabled` flag that defaults OFF. When not
//   enabled it FAILS CLOSED (throws), so the dispatcher records NO settled state.
//   Production wiring keeps it disabled; staging/tests opt in explicitly.
// - It produces a receipt ref derived deterministically from the payout ref +
//   amount, so a retried dispatch maps to the SAME ref (idempotent, and the
//   already-settled short-circuit in the ledger means it settles at most once).
// - The receipt ref carries the `staging_test` rail marker so the public
//   projection labels it honestly as a staging-test settlement, never as a
//   real hosted-MDK Bitcoin payout.

import type { ReferralPayoutAdapter } from './site-referral-payout-dispatch'

export class SiteReferralPayoutStagingAdapterError extends Error {
  readonly _tag = 'SiteReferralPayoutStagingAdapterError'
  readonly reason: string

  constructor(reason: string) {
    super(reason)
    this.name = 'SiteReferralPayoutStagingAdapterError'
    this.reason = reason
  }
}

export type SiteReferralPayoutStagingAdapterConfig = Readonly<{
  /**
   * Explicit opt-in. Defaults OFF in production wiring; staging/tests set it
   * true. When false the adapter fails closed (throws) so no settled state is
   * recorded — exactly the same fail-closed posture as the unconfigured
   * production adapter.
   */
  enabled: boolean
}>

const textEncoder = new TextEncoder()

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    textEncoder.encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * Deterministic, public-safe staging-test receipt ref. The same payout ref +
 * amount always maps to the same ref, so a retried dispatch is idempotent and
 * the settled receipt is stably dereferenceable. The `staging_test` rail marker
 * makes the public projection label it honestly (never as a real Bitcoin
 * payout).
 */
export const stagingTestReceiptRef = async (
  payoutRef: string,
  amountSats: number,
): Promise<string> =>
  `receipt.site_referral_payout.staging_test.${(
    await sha256Hex(`${payoutRef}:${amountSats}`)
  ).slice(0, 32)}`

/**
 * Build the staging-test referral payout adapter. It satisfies the dispatcher's
 * `ReferralPayoutAdapter` shape and produces a deterministic, public-safe
 * `staging_test` receipt ref WITHOUT moving any money. When `enabled` is false
 * it fails closed (throws), so the dispatcher records NO settled state.
 */
export const makeSiteReferralPayoutStagingAdapter = (
  config: SiteReferralPayoutStagingAdapterConfig,
): ReferralPayoutAdapter => ({
  adapterKind: 'staging_test',
  dispatch: async input => {
    if (!config.enabled) {
      // Fail closed: staging-test settlement is not armed. Money-safety: the
      // dispatcher records NO settled state when this throws.
      throw new SiteReferralPayoutStagingAdapterError(
        'site_referral_payout_staging_adapter_not_enabled',
      )
    }

    return {
      receiptRef: await stagingTestReceiptRef(input.payoutRef, input.amountSats),
    }
  },
})
