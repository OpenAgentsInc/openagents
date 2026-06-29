// RL-1 settlement wire (openagents #5511): the PRODUCTION referral payout
// adapter that the readiness-gated dispatcher (`site-referral-payout-dispatch.ts`
// -> `dispatchReferralPayoutSettlement`) invokes to move real Bitcoin.
//
// Before this module the Worker injected a THROWING placeholder adapter
// (`inference_referral_owner_armed_placeholder`): even with the readiness gate
// armed, no referral row could ever settle a real payout, so `mark_settled`
// effectively recorded nothing real. This module is the missing settle->adapter
// leg: it wraps the SAME hosted-MDK programmatic-payout rail the agent-claim and
// treasury payout paths use into the dispatcher's minimal `ReferralPayoutAdapter`
// shape, returning a public-safe, redacted receipt ref the ledger records as the
// dereferenceable settlement evidence.
//
// READINESS-GATED / OWNER-ARMED (the #5512 boundary): the dispatcher's injected
// readiness gate (`mdk-payout-mode-gate.ts` -> `livePayoutClaimAllowed`) is the
// arming switch. On the not-yet-armed production path the gate refuses BEFORE the
// dispatcher reaches this adapter, so the adapter is constructed (ready) but
// never called until the owner arms a live payout mode AND configures a funded
// hosted-MDK client + a registered referrer destination. If this adapter is ever
// reached without a configured client/destination, it FAILS CLOSED (throws): the
// dispatcher records NO settled state and no money moves. It fakes no receipt --
// the settlement evidence is whatever the real rail returns, redacted.
//
// MONEY-SAFETY:
// - Never returns a fabricated receipt. A receipt ref only exists after the rail
//   confirms the payout (status SUCCESS / a real payment id).
// - Redacts all payment material (payment id / hash) to a stable sha256-derived
//   ref before it crosses any boundary, so no preimage/hash/invoice leaks into
//   the public ledger projection.
// - Idempotent at the rail: the dispatcher passes a deterministic per-payout
//   adapter idempotency key, which the hosted-MDK rail dedupes on retry.

import { Schema as S } from 'effect'

import type { ReferralPayoutAdapter } from './site-referral-payout-dispatch'

/**
 * Tagged failure thrown by the production referral payout adapter. The
 * dispatcher's `ReferralPayoutAdapter.dispatch` contract rejects on any failure
 * (the dispatcher then records NO settled state); this tagged error preserves
 * the failure reason without a raw `new Error` and carries no payment material.
 */
export class SiteReferralPayoutAdapterError extends S.TaggedErrorClass<SiteReferralPayoutAdapterError>()(
  'SiteReferralPayoutAdapterError',
  {
    reason: S.String,
  },
) {}

/**
 * The minimal hosted-MDK programmatic-payout surface this adapter needs. It is
 * intentionally the SAME shape as `AgentClaimRewardHostedMdkClient.programmaticPayout`
 * and `treasury-payment-hosted-mdk-payout-adapter`'s RPC: one call that moves
 * `amountSats` to `destination`, deduped by `idempotencyKey`. The production
 * Worker supplies the funded hosted-MDK route client; tests supply a mock that
 * records the call and returns a deterministic payment id WITHOUT moving money.
 */
export type ReferralPayoutProgrammaticClient = Readonly<{
  programmaticPayout: (
    input: Readonly<{
      amountSats: number
      destination: string
      idempotencyKey: string
    }>,
  ) => Promise<
    Readonly<{
      paymentHash?: string | undefined
      paymentId: string
      status: 'REQUESTED' | 'SUCCESS' | 'FAILED'
    }>
  >
}>

/**
 * Resolve the registered payout destination (a reusable BOLT12 offer / LN
 * address) for a referral payout. The raw destination is sensitive payout-target
 * material and never leaves this function: it flows straight into the rail call
 * and is never recorded. Returns `null` when the referrer has no registered,
 * approved destination -- the adapter then fails closed (no money moves).
 */
export type ReferralPayoutDestinationResolver = (
  payoutRef: string,
) => Promise<string | null>

export type SiteReferralPayoutAdapterConfig = Readonly<{
  adapterKind?: string | undefined
  client: ReferralPayoutProgrammaticClient | null
  resolveDestination: ReferralPayoutDestinationResolver
}>

const textEncoder = new TextEncoder()
const controlCharacterPattern = /[\u0000-\u001f\u007f]/
const reusableDestinationPattern = /(^lno1|^lnurl|\S+@\S+)/i

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', textEncoder.encode(value))

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * Redact raw payment material (payment hash / id) to a stable, public-safe ref.
 * The same material always maps to the same ref (so a settled receipt is
 * dereferenceable + idempotent), but the raw value never crosses the boundary.
 */
const redactedReceiptRef = async (rawMaterial: string): Promise<string> =>
  `receipt.site_referral_payout.hosted_mdk.${(await sha256Hex(rawMaterial)).slice(0, 32)}`

/**
 * Build the production referral payout adapter over the hosted-MDK programmatic
 * payout rail. The returned adapter satisfies the dispatcher's
 * `ReferralPayoutAdapter`: `dispatch` moves `amountSats` to the resolved
 * destination and returns a redacted receipt ref, throwing (so NO settled state
 * is recorded) on any failure.
 *
 * INERT posture: when `client` is null (no funded hosted-MDK client configured)
 * the adapter throws on dispatch. Combined with the readiness gate refusing
 * first on the not-yet-armed path, this means the production wiring is SAFE to
 * install while the owner-armed gate is still off (#5512): the placeholder that
 * could never pay is replaced by a real-but-gated rail.
 */
export const makeSiteReferralPayoutAdapter = (
  config: SiteReferralPayoutAdapterConfig,
): ReferralPayoutAdapter => ({
  adapterKind: config.adapterKind ?? 'hosted_mdk',
  dispatch: async input => {
    if (config.client === null) {
      // Fail closed: no funded payout client configured. Money-safety: the
      // dispatcher records NO settled state when this throws.
      throw new SiteReferralPayoutAdapterError({
        reason:
          'site_referral_payout_adapter_unconfigured: hosted MDK payout client not armed',
      })
    }

    const destination = await config.resolveDestination(input.payoutRef)

    if (destination === null) {
      throw new SiteReferralPayoutAdapterError({
        reason:
          'site_referral_payout_adapter_destination_unavailable: referrer has no registered payout destination',
      })
    }

    const trimmed = destination.trim()

    if (
      trimmed === '' ||
      trimmed.length > 4096 ||
      controlCharacterPattern.test(trimmed) ||
      // Referral payouts move to the referrer's REUSABLE destination (BOLT12
      // offer / LN address); a single-use bolt11 invoice is not a valid payout
      // target for an idempotent retry, so require a reusable form.
      !reusableDestinationPattern.test(trimmed)
    ) {
      throw new SiteReferralPayoutAdapterError({
        reason:
          'site_referral_payout_adapter_destination_invalid: destination must be a reusable payout target',
      })
    }

    const result = await config.client.programmaticPayout({
      amountSats: input.amountSats,
      destination: trimmed,
      idempotencyKey: input.idempotencyKey,
    })

    if (result.status === 'FAILED') {
      throw new SiteReferralPayoutAdapterError({
        reason: 'site_referral_payout_adapter_rail_failed',
      })
    }

    // Receipt material is the rail's payment hash (preferred) or payment id,
    // redacted before it crosses the boundary. The ledger records this as the
    // settled state's dereferenceable evidence ref.
    return {
      receiptRef: await redactedReceiptRef(result.paymentHash ?? result.paymentId),
    }
  },
})
