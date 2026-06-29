import {
  type SiteReferralPayoutLedgerEntry,
  type SiteReferralPayoutLedgerStorageError,
  type SiteReferralPayoutLedgerValidationError,
  readCurrentReferralPayout,
  transitionReferralPayout,
} from './site-referral-payout-ledger'
import type { SiteReferralRevenueAsset } from './site-referral-payout-feed'
import { referralRevenueAssetToBoundaryAsset } from './site-referral-payout-feed'
import { validateAssetBoundary } from './asset-bitcoin-boundary'
import type { MdkPayoutModeGateProjection } from './mdk-payout-mode-gate'
import { isoTimestampAfterIso } from './runtime-primitives'

/**
 * RL-1 (openagents #5458): DISPATCH the referral payout ledger through the
 * already-built MDK/Spark payout rail.
 *
 * Previously `mark_settled` recorded a bare evidence string and called NO wallet
 * adapter, so no eligible referral row ever moved real Bitcoin. This module is
 * the missing settle->adapter wire. It drives an `eligible` row through
 * `approved -> dispatched -> settled`, invoking the injected payout adapter
 * BEFORE recording the settlement, so the public `settled` state always has a
 * confirmed, dereferenceable receipt ref behind it.
 *
 * MONEY-SAFETY (this is a real payout path):
 * - Idempotent: deterministic per-payout dispatch keys + a pre-flight state read
 *   mean a retried dispatch settles AT MOST ONCE. An already-`settled` payout
 *   returns its existing entry without re-dispatching.
 * - Readiness-gated: the injected readiness gate
 *   (`mdk-payout-mode-gate.ts` -> `livePayoutClaimAllowed`) must allow live
 *   payouts. A blocked/sandbox/unregistered target never dispatches.
 * - Rev-share invariant (`INVARIANTS.md` "Site Referral Bitcoin Withdrawal
 *   Gate"): only Bitcoin revenue may move withdrawable Bitcoin. Credit/USD
 *   (Stripe credit) revenue is refused for Bitcoin dispatch -- it is credit
 *   revshare, not a Bitcoin liability. Free/promo (zero amount) never reaches
 *   here because the ledger refuses it at eligibility time.
 * - The adapter is injected. Tests inject a MOCK that records no real payout;
 *   production injects the readiness-gated MDK/Spark rail. This module fakes no
 *   receipt: the settlement evidence ref is whatever the adapter returns.
 */

/** Minimal payout-adapter surface this dispatcher needs. The production wrapper
 * around the MDK hosted / Spark treasury adapters satisfies it; tests inject a
 * mock that records the call and returns a public-safe receipt ref WITHOUT
 * moving money. The dispatcher records `settled` only after this succeeds. */
export type ReferralPayoutAdapter = Readonly<{
  adapterKind: string
  /** Move `amountSats` to the referrer's registered payout target, keyed by
   * `idempotencyKey` so the underlying rail dedupes a retry. Returns a public-
   * safe settlement receipt ref. Throws/rejects on any failure (the dispatcher
   * then records NO settled state). */
  dispatch: (input: {
    amountSats: number
    idempotencyKey: string
    payoutRef: string
  }) => Promise<{ receiptRef: string }>
}>

export type SiteReferralPayoutDispatchDependencies = Readonly<{
  adapter: ReferralPayoutAdapter
  nowIso: () => string
  /** Live readiness for the requested payout mode. The dispatcher proceeds only
   * when `livePayoutClaimAllowed` is true (never to an unregistered/!ready
   * target). */
  readReadiness: () => Promise<MdkPayoutModeGateProjection>
}>

export type SiteReferralPayoutDispatchInput = Readonly<{
  payoutRef: string
  /** Rev-share asset of the qualifying revenue. Only `bitcoin` may dispatch. */
  revenueAsset: SiteReferralRevenueAsset
}>

export type SiteReferralPayoutDispatchOutcome =
  | Readonly<{
      _tag: 'settled'
      entry: SiteReferralPayoutLedgerEntry
      receiptRef: string
    }>
  | Readonly<{
      _tag: 'already_settled'
      entry: SiteReferralPayoutLedgerEntry
    }>
  | Readonly<{
      _tag: 'refused'
      entry: SiteReferralPayoutLedgerEntry | null
      reasonRef: string
    }>

export class SiteReferralPayoutDispatchError extends Error {
  readonly _tag = 'SiteReferralPayoutDispatchError'
  readonly reason: string

  constructor(reason: string, cause?: unknown) {
    super(reason, cause === undefined ? undefined : { cause })
    this.name = 'SiteReferralPayoutDispatchError'
    this.reason = reason
  }
}

export type SiteReferralPayoutDispatchFailure =
  | SiteReferralPayoutDispatchError
  | SiteReferralPayoutLedgerStorageError
  | SiteReferralPayoutLedgerValidationError

const SAFE_RECEIPT_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/

// Deterministic per-payout transition keys. Keying every transition by the
// payoutRef + action means a replay of the whole dispatch re-uses the same
// ledger idempotency keys; `transitionReferralPayout` short-circuits on a known
// key, so the chain advances at most once.
const approveKey = (payoutRef: string): string =>
  `site_referral_payout_dispatch.approve.${payoutRef}`
const dispatchedKey = (payoutRef: string): string =>
  `site_referral_payout_dispatch.dispatched.${payoutRef}`
const settledKey = (payoutRef: string): string =>
  `site_referral_payout_dispatch.settled.${payoutRef}`
// The adapter-level idempotency key (DebtReceiptKey-style): exactly one real
// payout per payout row, deduped by the underlying rail on retry.
const adapterIdempotencyKey = (payoutRef: string): string =>
  `site_referral_payout.adapter.${payoutRef}`

/**
 * Settle one eligible referral payout through the injected adapter, idempotently
 * and readiness-gated, enforcing the rev-share asset boundary. Returns:
 *
 * - `already_settled` when the payout was settled before (no re-dispatch).
 * - `refused` (no money moved) when the asset boundary, readiness gate, or
 *   ledger state blocks it; a public-safe reason ref explains why.
 * - `settled` when the adapter confirmed a payout and the ledger recorded the
 *   `settled` state with the adapter's receipt ref as evidence.
 */
export const dispatchReferralPayoutSettlement = async (
  db: D1Database,
  dependencies: SiteReferralPayoutDispatchDependencies,
  input: SiteReferralPayoutDispatchInput,
): Promise<SiteReferralPayoutDispatchOutcome> => {
  const current = await readCurrentReferralPayout(db, input.payoutRef)

  if (current === null) {
    return {
      _tag: 'refused',
      entry: null,
      reasonRef: 'reason.public.site_referral_payout.unknown_payout_ref',
    }
  }

  if (current.state === 'settled') {
    return { _tag: 'already_settled', entry: current }
  }

  // Only `eligible` (or an already-`approved`/`dispatched` retry) may advance.
  // refused/failed/reversed rows never dispatch.
  if (
    current.state !== 'eligible' &&
    current.state !== 'approved' &&
    current.state !== 'dispatched'
  ) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: `reason.public.site_referral_payout.not_dispatchable_state.${current.state}`,
    }
  }

  if (current.amountSats <= 0) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: 'reason.public.site_referral_payout.no_qualifying_paid_amount',
    }
  }

  // Rev-share invariant (RL-3 #5460): enforce the SHARED credit<->Bitcoin
  // asset-boundary guard on this live payout. Only Bitcoin revenue may fund a
  // withdrawable Bitcoin payout; credit/USD (Stripe credit) revenue is credit
  // revshare and must NOT move Bitcoin; free/promo never creates withdrawable
  // Bitcoin. This dispatch ALWAYS produces a withdrawable Bitcoin send, so the
  // contributor asset is `bitcoin`. Fail closed on any violation (no money
  // moves, the adapter is never called) with the boundary's reason ref.
  const boundaryViolation = validateAssetBoundary({
    contributorAsset: 'bitcoin',
    movement: 'payout',
    revenueAsset: referralRevenueAssetToBoundaryAsset(input.revenueAsset),
  })

  if (boundaryViolation !== null) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: boundaryViolation.reasonRef,
    }
  }

  // Readiness gate: never dispatch to a blocked / sandbox / unregistered target.
  const readiness = await dependencies.readReadiness()

  if (!readiness.livePayoutClaimAllowed) {
    return {
      _tag: 'refused',
      entry: current,
      reasonRef: 'reason.public.site_referral_payout.payout_target_not_ready',
    }
  }

  // The ledger reads the current entry by `created_at DESC, id DESC`, so the
  // chained transitions MUST carry strictly increasing timestamps for the read
  // to resolve to the latest state deterministically (ids are random and not
  // monotonic). Derive three increasing instants from one `nowIso()` via the
  // sanctioned runtime-primitive helper (no raw Date primitives).
  const approveIso = dependencies.nowIso()
  const dispatchedIso = isoTimestampAfterIso(approveIso, 1)
  const settledIso = isoTimestampAfterIso(approveIso, 2)

  // Advance to `approved` then `dispatched`. Both transitions are idempotency-
  // keyed by payoutRef, so a retry re-reads the same key and does not duplicate.
  let entry = current

  if (entry.state === 'eligible') {
    entry = await transitionReferralPayout(db, {
      action: 'approve_dispatch',
      idempotencyKey: approveKey(input.payoutRef),
      nowIso: approveIso,
      payoutRef: input.payoutRef,
      stateReasonRef:
        'reason.public.site_referral_payout.operator_dispatch_approved',
    })
  }

  if (entry.state === 'approved') {
    entry = await transitionReferralPayout(db, {
      action: 'mark_dispatched',
      idempotencyKey: dispatchedKey(input.payoutRef),
      nowIso: dispatchedIso,
      payoutRef: input.payoutRef,
      stateReasonRef:
        'reason.public.site_referral_payout.dispatch_requested_to_adapter',
    })
  }

  // Now in `dispatched`. Invoke the adapter BEFORE recording `settled`, so the
  // public settled state always has a confirmed receipt behind it. The adapter's
  // own idempotency key dedupes a retried real payout.
  let receiptRef: string

  try {
    const result = await dependencies.adapter.dispatch({
      amountSats: entry.amountSats,
      idempotencyKey: adapterIdempotencyKey(input.payoutRef),
      payoutRef: input.payoutRef,
    })
    receiptRef = result.receiptRef
  } catch (cause) {
    // Adapter failed: leave the row in `dispatched` (an operator/retry can
    // re-drive it). Surface a typed failure; record NO settled state.
    throw new SiteReferralPayoutDispatchError(
      'site_referral_payout_adapter_dispatch_failed',
      cause,
    )
  }

  if (!SAFE_RECEIPT_REF_PATTERN.test(receiptRef)) {
    throw new SiteReferralPayoutDispatchError(
      'site_referral_payout_adapter_returned_unsafe_receipt_ref',
    )
  }

  const settled = await transitionReferralPayout(db, {
    action: 'mark_settled',
    evidenceRefs: [
      receiptRef,
      `evidence.site_referral_payout.adapter.${dependencies.adapter.adapterKind}`,
    ],
    idempotencyKey: settledKey(input.payoutRef),
    nowIso: settledIso,
    payoutRef: input.payoutRef,
    stateReasonRef:
      'reason.public.site_referral_payout.settled_with_adapter_receipt',
  })

  return { _tag: 'settled', entry: settled, receiptRef }
}
