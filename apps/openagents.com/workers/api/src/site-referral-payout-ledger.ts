import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import { compactRandomId } from './runtime-primitives'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

export const SITE_REFERRAL_PAYOUT_POLICY_REF =
  'policy.site_referral_payout.v1'
export const SITE_REFERRAL_PAYOUT_CAMPAIGN_REF =
  'campaign.site_referral_bitcoin_stream.v1'
export const SITE_REFERRAL_PAYOUT_PERCENT_BPS = 500
export const SITE_REFERRAL_PAYOUT_MAX_EVENT_SATS = 1000
export const SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_SATS = 5000
export const SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_COUNT = 50

export const SiteReferralPayoutState = S.Literals([
  'eligible',
  'approved',
  'dispatched',
  'settled',
  'failed',
  'refused',
  'reversed',
])
export type SiteReferralPayoutState = typeof SiteReferralPayoutState.Type

export const SiteReferralPayoutAction = S.Literals([
  'approve_dispatch',
  'mark_dispatched',
  'mark_failed',
  'mark_settled',
  'refuse',
  'reverse',
])
export type SiteReferralPayoutAction = typeof SiteReferralPayoutAction.Type

export type SiteReferralPayoutLedgerEntry = Readonly<{
  amountSats: number
  archivedAt: string | null
  caveatRefs: ReadonlyArray<string>
  createdAt: string
  evidenceRefs: ReadonlyArray<string>
  id: string
  idempotencyKey: string
  payoutRef: string
  periodKey: string
  policyRefs: ReadonlyArray<string>
  previousEntryId: string | null
  qualifyingAmountSats: number
  qualifyingEventKind: string
  qualifyingEventRef: string
  referredUserId: string | null
  referralAttributionId: string
  referralInviteId: string | null
  referralSourceId: string
  referrerUserId: string
  reversalOfEntryId: string | null
  state: SiteReferralPayoutState
  stateReasonRef: string | null
}>

type SiteReferralPayoutLedgerRow = Readonly<{
  amount_sats: number
  archived_at: string | null
  caveat_refs_json: string
  created_at: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  payout_ref: string
  period_key: string
  policy_refs_json: string
  previous_entry_id: string | null
  qualifying_amount_sats: number
  qualifying_event_kind: string
  qualifying_event_ref: string
  referred_user_id: string | null
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
  reversal_of_entry_id: string | null
  state: SiteReferralPayoutState
  state_reason_ref: string | null
}>

type ReferrerPeriodTotalsRow = Readonly<{
  payout_count: number | null
  payout_sats: number | null
}>

export type SiteReferralPayoutProjection = Readonly<{
  amountSats: number
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  currentEntryId: string
  evidenceRefs: ReadonlyArray<string>
  payoutRef: string
  policyRefs: ReadonlyArray<string>
  qualifyingEventRef: string
  referralAttributionId: string
  state: SiteReferralPayoutState
  stateReasonRef: string | null
}>

export type CreateReferralPayoutEligibilityInput = Readonly<{
  id?: string
  idempotencyKey: string
  nowIso: string
  payoutRef?: string
  periodKey: string
  qualifyingAmountSats: number
  qualifyingEventKind: string
  qualifyingEventRef: string
  referredUserId?: string | null
  referralAttributionId: string
  referralInviteId?: string | null
  referralSourceId: string
  referrerUserId: string
}>

export type TransitionReferralPayoutInput = Readonly<{
  action: SiteReferralPayoutAction
  evidenceRefs?: ReadonlyArray<string>
  id?: string
  idempotencyKey: string
  nowIso: string
  payoutRef: string
  stateReasonRef?: string | null
}>

export class SiteReferralPayoutLedgerValidationError extends S.TaggedErrorClass<SiteReferralPayoutLedgerValidationError>()(
  'SiteReferralPayoutLedgerValidationError',
  {
    reason: S.String,
  },
) {}

export class SiteReferralPayoutLedgerStorageError extends S.TaggedErrorClass<SiteReferralPayoutLedgerStorageError>()(
  'SiteReferralPayoutLedgerStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,220}$/
const PROHIBITED_REF_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|payment_secret|payment_hash|wallet_secret|private_key|webhook_secret|mdk_access_token|access_token|refresh_token|device_auth_id|code_verifier|gho_[a-z0-9_]+)/i

const requiredPolicyRefs = [SITE_REFERRAL_PAYOUT_POLICY_REF]
const requiredCaveatRefs = [
  'caveat.site_referral_payout.operator_dispatch_required',
  'caveat.site_referral_payout.settlement_evidence_required',
  'caveat.site_referral_payout.reversible_for_refund_or_abuse',
]

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_REF_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | null | undefined): void => {
  if (value === undefined || value === null) {
    return
  }

  if (!isSafeRef(value)) {
    throw new SiteReferralPayoutLedgerValidationError({
      reason: `${field} must be a public-safe ref.`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): void => values?.forEach(value => assertSafeRef(field, value))

const rowToEntry = (
  row: SiteReferralPayoutLedgerRow,
): SiteReferralPayoutLedgerEntry => ({
  amountSats: Number(row.amount_sats),
  archivedAt: row.archived_at,
  caveatRefs: parseJsonStringArray(row.caveat_refs_json),
  createdAt: row.created_at,
  evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  payoutRef: row.payout_ref,
  periodKey: row.period_key,
  policyRefs: parseJsonStringArray(row.policy_refs_json),
  previousEntryId: row.previous_entry_id,
  qualifyingAmountSats: Number(row.qualifying_amount_sats),
  qualifyingEventKind: row.qualifying_event_kind,
  qualifyingEventRef: row.qualifying_event_ref,
  referredUserId: row.referred_user_id,
  referralAttributionId: row.referral_attribution_id,
  referralInviteId: row.referral_invite_id,
  referralSourceId: row.referral_source_id,
  referrerUserId: row.referrer_user_id,
  reversalOfEntryId: row.reversal_of_entry_id,
  state: row.state,
  stateReasonRef: row.state_reason_ref,
})

export const projectSiteReferralPayout = (
  entry: SiteReferralPayoutLedgerEntry,
): SiteReferralPayoutProjection => ({
  amountSats: entry.amountSats,
  authorityBoundary:
    'Referral payout ledger state is not spendable bitcoin until operator-gated dispatch settles with public-safe evidence refs.',
  caveatRefs: entry.caveatRefs,
  currentEntryId: entry.id,
  evidenceRefs: entry.evidenceRefs,
  payoutRef: entry.payoutRef,
  policyRefs: entry.policyRefs,
  qualifyingEventRef: entry.qualifyingEventRef,
  referralAttributionId: entry.referralAttributionId,
  state: entry.state,
  stateReasonRef: entry.stateReasonRef,
})

export const calculateReferralPayoutSats = (
  qualifyingAmountSats: number,
): number => {
  if (!Number.isFinite(qualifyingAmountSats) || qualifyingAmountSats <= 0) {
    return 0
  }

  return Math.max(
    1,
    Math.min(
      SITE_REFERRAL_PAYOUT_MAX_EVENT_SATS,
      Math.floor((qualifyingAmountSats * SITE_REFERRAL_PAYOUT_PERCENT_BPS) / 10000),
    ),
  )
}

const storage = async <T>(
  operation: string,
  run: () => Promise<T>,
): Promise<T> => {
  try {
    return await run()
  } catch (error) {
    throw new SiteReferralPayoutLedgerStorageError({ error, operation })
  }
}

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<SiteReferralPayoutLedgerEntry | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM site_referral_payout_ledger_entries
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<SiteReferralPayoutLedgerRow>()

  return row === null ? null : rowToEntry(row)
}

export const readCurrentReferralPayout = async (
  db: D1Database,
  payoutRef: string,
): Promise<SiteReferralPayoutLedgerEntry | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM site_referral_payout_ledger_entries
        WHERE payout_ref = ?
          AND archived_at IS NULL
        ORDER BY created_at DESC, id DESC
        LIMIT 1`,
    )
    .bind(payoutRef)
    .first<SiteReferralPayoutLedgerRow>()

  return row === null ? null : rowToEntry(row)
}

const readReferrerPeriodTotals = async (
  db: D1Database,
  referrerUserId: string,
  periodKey: string,
): Promise<Readonly<{ count: number; sats: number }>> => {
  const row = await db
    .prepare(
      `SELECT COUNT(*) AS payout_count,
              COALESCE(SUM(amount_sats), 0) AS payout_sats
         FROM site_referral_payout_ledger_entries
        WHERE referrer_user_id = ?
          AND period_key = ?
          AND state IN ('eligible', 'approved', 'dispatched', 'settled')
          AND amount_sats > 0
          AND archived_at IS NULL`,
    )
    .bind(referrerUserId, periodKey)
    .first<ReferrerPeriodTotalsRow>()

  return {
    count: Number(row?.payout_count ?? 0),
    sats: Number(row?.payout_sats ?? 0),
  }
}

const insertEntry = async (
  database: TreasuryDatabase,
  entry: SiteReferralPayoutLedgerEntry,
): Promise<SiteReferralPayoutLedgerEntry> =>
  storage('siteReferralPayoutLedger.entry.insert', async () => {
    const db = treasuryAuthorityDb(database)
    await db
      .prepare(
        `INSERT INTO site_referral_payout_ledger_entries (
          id, payout_ref, idempotency_key, referral_attribution_id,
          referral_source_id, referral_invite_id, referrer_user_id,
          referred_user_id, qualifying_event_ref, qualifying_event_kind,
          qualifying_amount_sats, amount_sats, period_key, state,
          state_reason_ref, previous_entry_id, reversal_of_entry_id,
          evidence_refs_json, policy_refs_json, caveat_refs_json,
          created_at, archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        entry.id,
        entry.payoutRef,
        entry.idempotencyKey,
        entry.referralAttributionId,
        entry.referralSourceId,
        entry.referralInviteId,
        entry.referrerUserId,
        entry.referredUserId,
        entry.qualifyingEventRef,
        entry.qualifyingEventKind,
        entry.qualifyingAmountSats,
        entry.amountSats,
        entry.periodKey,
        entry.state,
        entry.stateReasonRef,
        entry.previousEntryId,
        entry.reversalOfEntryId,
        JSON.stringify(entry.evidenceRefs),
        JSON.stringify(entry.policyRefs),
        JSON.stringify(entry.caveatRefs),
        entry.createdAt,
      )
      .run()

    // KS-8.8 (#8319): fail-soft Postgres mirror of the appended entry.
    await mirrorTreasuryRows(
      database,
      'site_referral_payout_ledger_entries',
      'id',
      [entry.id],
    )

    return (await readByIdempotencyKey(db, entry.idempotencyKey)) ?? entry
  })

export const createReferralPayoutEligibility = async (
  database: TreasuryDatabase,
  input: CreateReferralPayoutEligibilityInput,
): Promise<SiteReferralPayoutLedgerEntry> => {
  const db = treasuryAuthorityDb(database)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('payoutRef', input.payoutRef)
  assertSafeRef('qualifyingEventRef', input.qualifyingEventRef)
  assertSafeRef('referralAttributionId', input.referralAttributionId)
  assertSafeRef('referralInviteId', input.referralInviteId)
  assertSafeRef('referralSourceId', input.referralSourceId)
  assertSafeRef('referrerUserId', input.referrerUserId)
  assertSafeRef('referredUserId', input.referredUserId)

  if (!Number.isFinite(input.qualifyingAmountSats) || input.qualifyingAmountSats < 0) {
    throw new SiteReferralPayoutLedgerValidationError({
      reason: 'qualifyingAmountSats must be finite and non-negative.',
    })
  }

  const existing = await storage(
    'siteReferralPayoutLedger.idempotency.read',
    () => readByIdempotencyKey(db, input.idempotencyKey),
  )

  if (existing !== null) {
    return existing
  }

  const totals = await storage('siteReferralPayoutLedger.referrerPeriod.read', () =>
    readReferrerPeriodTotals(db, input.referrerUserId, input.periodKey),
  )
  const calculatedAmount = calculateReferralPayoutSats(input.qualifyingAmountSats)
  const isSelfReferral = input.referredUserId === input.referrerUserId
  const cappedByCount =
    totals.count >= SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_COUNT
  const cappedByAmount =
    totals.sats + calculatedAmount > SITE_REFERRAL_PAYOUT_MAX_REFERRER_PERIOD_SATS
  const state: SiteReferralPayoutState =
    isSelfReferral || cappedByCount || cappedByAmount || calculatedAmount === 0
      ? 'refused'
      : 'eligible'
  const stateReasonRef = isSelfReferral
    ? 'reason.public.site_referral_payout.self_referral'
    : cappedByCount || cappedByAmount
      ? 'reason.public.site_referral_payout.referrer_period_cap_exceeded'
      : calculatedAmount === 0
        ? 'reason.public.site_referral_payout.no_qualifying_paid_amount'
        : null

  return insertEntry(database, {
    amountSats: state === 'eligible' ? calculatedAmount : 0,
    archivedAt: null,
    caveatRefs: requiredCaveatRefs,
    createdAt: input.nowIso,
    evidenceRefs: [input.qualifyingEventRef],
    id: input.id ?? compactRandomId('site_referral_payout_entry'),
    idempotencyKey: input.idempotencyKey,
    payoutRef:
      input.payoutRef ?? `site_referral_payout_${input.referralAttributionId}`,
    periodKey: input.periodKey,
    policyRefs: requiredPolicyRefs,
    previousEntryId: null,
    qualifyingAmountSats: Math.floor(input.qualifyingAmountSats),
    qualifyingEventKind: input.qualifyingEventKind,
    qualifyingEventRef: input.qualifyingEventRef,
    referredUserId: input.referredUserId ?? null,
    referralAttributionId: input.referralAttributionId,
    referralInviteId: input.referralInviteId ?? null,
    referralSourceId: input.referralSourceId,
    referrerUserId: input.referrerUserId,
    reversalOfEntryId: null,
    state,
    stateReasonRef,
  })
}

const transitionForAction: Record<
  SiteReferralPayoutAction,
  Readonly<{ from: ReadonlyArray<SiteReferralPayoutState>; to: SiteReferralPayoutState }>
> = {
  approve_dispatch: { from: ['eligible'], to: 'approved' },
  mark_dispatched: { from: ['approved'], to: 'dispatched' },
  mark_failed: { from: ['approved', 'dispatched'], to: 'failed' },
  mark_settled: { from: ['dispatched'], to: 'settled' },
  refuse: { from: ['eligible', 'approved'], to: 'refused' },
  reverse: {
    from: ['eligible', 'approved', 'dispatched', 'settled'],
    to: 'reversed',
  },
}

export const transitionReferralPayout = async (
  database: TreasuryDatabase,
  input: TransitionReferralPayoutInput,
): Promise<SiteReferralPayoutLedgerEntry> => {
  const db = treasuryAuthorityDb(database)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('payoutRef', input.payoutRef)
  assertSafeRef('stateReasonRef', input.stateReasonRef)
  assertSafeRefs('evidenceRefs', input.evidenceRefs)

  const existing = await storage(
    'siteReferralPayoutLedger.idempotency.read',
    () => readByIdempotencyKey(db, input.idempotencyKey),
  )

  if (existing !== null) {
    return existing
  }

  const current = await storage('siteReferralPayoutLedger.current.read', () =>
    readCurrentReferralPayout(db, input.payoutRef),
  )

  if (current === null) {
    throw new SiteReferralPayoutLedgerValidationError({
      reason: 'Unknown referral payout ref.',
    })
  }

  const transition = transitionForAction[input.action]

  if (!transition.from.includes(current.state)) {
    throw new SiteReferralPayoutLedgerValidationError({
      reason: `Payout in state ${current.state} cannot ${input.action}.`,
    })
  }

  if (transition.to === 'settled' && (input.evidenceRefs ?? []).length === 0) {
    throw new SiteReferralPayoutLedgerValidationError({
      reason: 'mark_settled requires public-safe settlement evidence refs.',
    })
  }

  const amountSats = transition.to === 'reversed'
    ? -Math.abs(current.amountSats)
    : current.amountSats

  return insertEntry(database, {
    ...current,
    amountSats,
    createdAt: input.nowIso,
    evidenceRefs: [...current.evidenceRefs, ...(input.evidenceRefs ?? [])],
    id: input.id ?? compactRandomId('site_referral_payout_entry'),
    idempotencyKey: input.idempotencyKey,
    previousEntryId: current.id,
    reversalOfEntryId: transition.to === 'reversed' ? current.id : null,
    state: transition.to,
    stateReasonRef: input.stateReasonRef ?? current.stateReasonRef,
  })
}
