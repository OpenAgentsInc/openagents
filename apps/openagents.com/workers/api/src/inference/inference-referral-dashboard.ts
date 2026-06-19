// Referrer inference-revshare dashboard read surface (EPIC #5474 / sub-EPIC
// #5475, child #5491).
//
// A public-safe READ over the existing RL-1 referral payout ledger, scoped to
// the inference qualifying-event kind, for one referrer. It answers the
// referrer's three questions:
//   - which of my referred accounts are spending on inference,
//   - my ongoing earnings (accrued + settled, in sats), and
//   - my settled payout receipts.
//
// PUBLIC-SAFE: it emits only refs and sat amounts (the ledger already validates
// every stored ref as public-safe and rejects payment material on write). It
// never returns tokens, seeds, destinations, idempotency keys, or raw payment
// material. It moves no money and writes nothing — read-only.
//
// REUSE: it queries the SAME `site_referral_payout_ledger_entries` table the
// site spine writes; it does not maintain a parallel store. It selects the
// LATEST entry per `payout_ref` (the ledger is append-only via `previous_entry_
// id`, so the most recent row per payout is its current state) and filters to
// the inference event kind so the inference dashboard shows inference earnings
// without mixing in site-checkout referral rows.

import { parseJsonStringArray } from '../json-boundary'
import { INFERENCE_REFERRAL_QUALIFYING_EVENT_KIND } from './inference-referral-accrual'

// Per-referred-account rollup of a referrer's inference earnings.
export type ReferredAccountEarnings = Readonly<{
  // Public-safe referred-account ref (the user/agent id the ledger stored as
  // `referred_user_id`; null when the ledger row carried none).
  referredUserId: string | null
  // Count of paid inference requests that accrued for this referred account.
  paidRequestCount: number
  // Total sats accrued across all not-reversed/not-refused states (the ongoing
  // earned amount, whether or not yet settled).
  accruedSats: number
  // Subset of `accruedSats` that has SETTLED (real payout dispatched + receipt
  // recorded).
  settledSats: number
}>

// One settled inference-referral payout receipt (public-safe).
export type InferenceReferralSettledReceipt = Readonly<{
  payoutRef: string
  amountSats: number
  // Public-safe settlement evidence refs the dispatcher recorded (the adapter
  // receipt ref + the adapter-kind evidence ref). Never payment material.
  evidenceRefs: ReadonlyArray<string>
  settledAt: string
}>

export type InferenceReferralDashboard = Readonly<{
  referrerUserId: string
  // Distinct referred accounts that have spent on inference under this referrer.
  referredAccountCount: number
  // Total ongoing earnings (sats) accrued across all referred accounts in
  // not-reversed/not-refused states.
  totalAccruedSats: number
  // Total settled (paid-out) earnings (sats).
  totalSettledSats: number
  // Total accrued but not-yet-settled (pending dispatch).
  totalPendingSats: number
  perReferredAccount: ReadonlyArray<ReferredAccountEarnings>
  settledReceipts: ReadonlyArray<InferenceReferralSettledReceipt>
  // The authority boundary statement (ledger state is not spendable Bitcoin
  // until operator-gated dispatch settles).
  authorityBoundary: string
}>

const SAFE_USER_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

// States that count toward EARNED (not reversed/refused/failed). The ledger
// uses these for its own period totals, so the dashboard's "accrued" matches the
// ledger's cap accounting.
const EARNED_STATES = ['eligible', 'approved', 'dispatched', 'settled'] as const

type LatestEntryRow = Readonly<{
  payout_ref: string
  referred_user_id: string | null
  amount_sats: number
  state: string
  evidence_refs_json: string
  created_at: string
}>

// Read the LATEST ledger entry per payout_ref for one referrer, scoped to the
// inference qualifying-event kind. The correlated subquery picks the most recent
// row per payout (append-only chain => latest = current state).
const readLatestInferenceEntries = async (
  db: D1Database,
  referrerUserId: string,
): Promise<ReadonlyArray<LatestEntryRow>> => {
  const result = await db
    .prepare(
      `SELECT e.payout_ref AS payout_ref,
              e.referred_user_id AS referred_user_id,
              e.amount_sats AS amount_sats,
              e.state AS state,
              e.evidence_refs_json AS evidence_refs_json,
              e.created_at AS created_at
         FROM site_referral_payout_ledger_entries AS e
        WHERE e.referrer_user_id = ?
          AND e.qualifying_event_kind = ?
          AND e.archived_at IS NULL
          AND e.created_at = (
            SELECT MAX(inner.created_at)
              FROM site_referral_payout_ledger_entries AS inner
             WHERE inner.payout_ref = e.payout_ref
               AND inner.archived_at IS NULL
          )
        ORDER BY e.created_at DESC
        LIMIT 1000`,
    )
    .bind(referrerUserId, INFERENCE_REFERRAL_QUALIFYING_EVENT_KIND)
    .all<LatestEntryRow>()

  return result.results ?? []
}


/**
 * Project a referrer's inference-revshare dashboard. Read-only, public-safe.
 * Returns empty totals (not an error) when the referrer id is malformed or has
 * no inference referral rows.
 */
export const readInferenceReferralDashboard = async (
  db: D1Database,
  referrerUserId: string,
): Promise<InferenceReferralDashboard> => {
  const authorityBoundary =
    'Inference referral earnings are ledger state, not spendable Bitcoin, until operator-gated dispatch settles each payout with public-safe evidence refs.'

  if (!SAFE_USER_ID_PATTERN.test(referrerUserId)) {
    return {
      authorityBoundary,
      perReferredAccount: [],
      referredAccountCount: 0,
      referrerUserId,
      settledReceipts: [],
      totalAccruedSats: 0,
      totalPendingSats: 0,
      totalSettledSats: 0,
    }
  }

  const rows = await readLatestInferenceEntries(db, referrerUserId)

  const perAccount = new Map<string, ReferredAccountEarnings>()
  const settledReceipts: InferenceReferralSettledReceipt[] = []
  let totalAccruedSats = 0
  let totalSettledSats = 0

  for (const row of rows) {
    const earned = (EARNED_STATES as ReadonlyArray<string>).includes(row.state)
    const settled = row.state === 'settled'
    const sats = Number(row.amount_sats)
    const safeSats = Number.isFinite(sats) && sats > 0 ? sats : 0

    if (earned) {
      totalAccruedSats += safeSats
    }
    if (settled) {
      totalSettledSats += safeSats
      settledReceipts.push({
        amountSats: safeSats,
        evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
        payoutRef: row.payout_ref,
        settledAt: row.created_at,
      })
    }

    const key = row.referred_user_id ?? '(unattributed)'
    const existing = perAccount.get(key) ?? {
      accruedSats: 0,
      paidRequestCount: 0,
      referredUserId: row.referred_user_id,
      settledSats: 0,
    }
    perAccount.set(key, {
      accruedSats: existing.accruedSats + (earned ? safeSats : 0),
      paidRequestCount: existing.paidRequestCount + (earned ? 1 : 0),
      referredUserId: existing.referredUserId,
      settledSats: existing.settledSats + (settled ? safeSats : 0),
    })
  }

  const perReferredAccount = [...perAccount.values()].filter(
    account => account.paidRequestCount > 0,
  )

  return {
    authorityBoundary,
    perReferredAccount,
    referredAccountCount: perReferredAccount.length,
    referrerUserId,
    settledReceipts,
    totalAccruedSats,
    totalPendingSats: Math.max(0, totalAccruedSats - totalSettledSats),
    totalSettledSats,
  }
}
