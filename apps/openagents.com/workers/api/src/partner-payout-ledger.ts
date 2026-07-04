/**
 * Generalized partner payout ledger (#4986, lane WS-E).
 *
 * This module generalizes the referral payout ledger
 * (`site-referral-payout-ledger.ts`, migration 0153) into a partner payout
 * lane that supports design-partner / agency-partner / affiliate / referral
 * payouts across multiple assets (usd | credits | sats) while preserving the
 * same eligible -> approved -> dispatched -> settled lifecycle, append-only
 * transition rows, reversal, idempotency, and public-safe ref discipline.
 *
 * It is backed by a NEW table `partner_payout_ledger_entries`
 * (migration 0184) rather than reusing the referral table 0153: the referral
 * table has NOT NULL foreign keys onto `referral_attributions` and
 * `site_referral_sources`, which a design_partner / affiliate payout cannot
 * satisfy. The new table is the minimum unavoidable schema addition; the
 * lifecycle/policy/projection logic is otherwise a direct generalization of
 * the referral lane.
 *
 * Authority boundary (mirrors INVARIANTS "Pack A Ledger And Settlement
 * Evidence"): ledger state is NOT spendable value. Settlement requires
 * operator-gated dispatch plus explicit public-safe settlement evidence refs,
 * and entries are reversible for refund/abuse.
 *
 * ============================================================================
 * COORDINATOR WIRING (deferred integration — do NOT wire from this lane)
 * ----------------------------------------------------------------------------
 * The coordinator integrating this lane must:
 *
 * 1. Apply migration `0184_partner_payout_ledger.sql` (creates
 *    `partner_payout_ledger_entries`). Confirm 0183 is owned by a sibling
 *    lane before applying so numbering does not collide.
 *
 * 2. Add an operator transition route module
 *    `partner-payout-ledger-routes.ts` modeled on
 *    `site-referral-payout-ledger-routes.ts`, exposing e.g.
 *      POST /api/operator/partners/payout-ledger/:payoutRef/transitions
 *    guarded by `requireAdminApiToken`, decoding a transition request schema
 *    (action + idempotencyKey + optional evidenceRefs + optional
 *    stateReasonRef) and calling `transitionPartnerPayout` /
 *    `projectPartnerPayout` from this module.
 *
 * 3. Register that route in the API worker router (the shared
 *    `worker-routes.ts` / `index.ts`) alongside the referral payout route.
 *    This lane intentionally does NOT touch those shared files.
 *
 * 4. Decide the eligibility producer(s): qualifying design-partner / affiliate
 *    events should call `createPartnerPayoutEligibility` with a role-specific
 *    `qualifyingEventKind`, `partnerRef`, and `asset`. The percentage/cap
 *    policy here is intentionally conservative and per-role configurable;
 *    revisit `PARTNER_PAYOUT_ROLE_POLICY` with product before launch.
 * ============================================================================
 */
import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import { compactRandomId } from './runtime-primitives'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

export const PARTNER_PAYOUT_POLICY_REF = 'policy.partner_payout.v1'

export const PartnerPayoutRole = S.Literals([
  'design_partner',
  'referral',
  'affiliate',
])
export type PartnerPayoutRole = typeof PartnerPayoutRole.Type

export const PartnerPayoutAsset = S.Literals(['usd', 'credits', 'sats'])
export type PartnerPayoutAsset = typeof PartnerPayoutAsset.Type

export const PartnerPayoutState = S.Literals([
  'eligible',
  'approved',
  'dispatched',
  'settled',
  'failed',
  'refused',
  'reversed',
])
export type PartnerPayoutState = typeof PartnerPayoutState.Type

export const PartnerPayoutAction = S.Literals([
  'approve_dispatch',
  'mark_dispatched',
  'mark_failed',
  'mark_settled',
  'refuse',
  'reverse',
])
export type PartnerPayoutAction = typeof PartnerPayoutAction.Type

/**
 * Per-role payout policy. `percentBps` is applied to the qualifying amount
 * (in the entry's asset minor unit). Caps are per partner per period_key.
 */
export type PartnerPayoutRolePolicy = Readonly<{
  maxEventAmount: number
  maxPartnerPeriodAmount: number
  maxPartnerPeriodCount: number
  percentBps: number
}>

export const PARTNER_PAYOUT_ROLE_POLICY: Record<
  PartnerPayoutRole,
  PartnerPayoutRolePolicy
> = {
  affiliate: {
    maxEventAmount: 5000,
    maxPartnerPeriodAmount: 50000,
    maxPartnerPeriodCount: 200,
    percentBps: 1000,
  },
  design_partner: {
    maxEventAmount: 100000,
    maxPartnerPeriodAmount: 1000000,
    maxPartnerPeriodCount: 100,
    percentBps: 2000,
  },
  referral: {
    maxEventAmount: 1000,
    maxPartnerPeriodAmount: 5000,
    maxPartnerPeriodCount: 50,
    percentBps: 500,
  },
}

export type PartnerPayoutLedgerEntry = Readonly<{
  amount: number
  archivedAt: string | null
  asset: PartnerPayoutAsset
  beneficiaryUserId: string | null
  caveatRefs: ReadonlyArray<string>
  createdAt: string
  evidenceRefs: ReadonlyArray<string>
  id: string
  idempotencyKey: string
  partnerRef: string
  partnerRole: PartnerPayoutRole
  partnerUserId: string
  payoutRef: string
  periodKey: string
  policyRefs: ReadonlyArray<string>
  previousEntryId: string | null
  qualifyingAmount: number
  qualifyingEventKind: string
  qualifyingEventRef: string
  reversalOfEntryId: string | null
  state: PartnerPayoutState
  stateReasonRef: string | null
}>

type PartnerPayoutLedgerRow = Readonly<{
  amount: number
  archived_at: string | null
  asset: PartnerPayoutAsset
  beneficiary_user_id: string | null
  caveat_refs_json: string
  created_at: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  partner_ref: string
  partner_role: PartnerPayoutRole
  partner_user_id: string
  payout_ref: string
  period_key: string
  policy_refs_json: string
  previous_entry_id: string | null
  qualifying_amount: number
  qualifying_event_kind: string
  qualifying_event_ref: string
  reversal_of_entry_id: string | null
  state: PartnerPayoutState
  state_reason_ref: string | null
}>

type PartnerPeriodTotalsRow = Readonly<{
  payout_amount: number | null
  payout_count: number | null
}>

export type PartnerPayoutProjection = Readonly<{
  amount: number
  asset: PartnerPayoutAsset
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  currentEntryId: string
  evidenceRefs: ReadonlyArray<string>
  partnerRef: string
  partnerRole: PartnerPayoutRole
  payoutRef: string
  policyRefs: ReadonlyArray<string>
  qualifyingEventRef: string
  state: PartnerPayoutState
  stateReasonRef: string | null
}>

export type CreatePartnerPayoutEligibilityInput = Readonly<{
  asset: PartnerPayoutAsset
  beneficiaryUserId?: string | null
  /**
   * Optional public-safe evidence refs recording the attribution basis (e.g. the
   * winning partner-agreement ref). Merged after the qualifying event ref and
   * de-duplicated; the required qualifying event ref is always present.
   */
  evidenceRefs?: ReadonlyArray<string>
  id?: string
  idempotencyKey: string
  nowIso: string
  partnerRef: string
  partnerRole: PartnerPayoutRole
  partnerUserId: string
  payoutRef?: string
  periodKey: string
  /**
   * Optional public-safe policy refs recording the attribution basis (e.g. the
   * partner-attribution policy ref). Merged after the required payout policy ref
   * and de-duplicated; `PARTNER_PAYOUT_POLICY_REF` is always present.
   */
  policyRefs?: ReadonlyArray<string>
  qualifyingAmount: number
  qualifyingEventKind: string
  qualifyingEventRef: string
}>

export type TransitionPartnerPayoutInput = Readonly<{
  action: PartnerPayoutAction
  evidenceRefs?: ReadonlyArray<string>
  id?: string
  idempotencyKey: string
  nowIso: string
  payoutRef: string
  stateReasonRef?: string | null
}>

export class PartnerPayoutLedgerValidationError extends S.TaggedErrorClass<PartnerPayoutLedgerValidationError>()(
  'PartnerPayoutLedgerValidationError',
  {
    reason: S.String,
  },
) {}

export class PartnerPayoutLedgerStorageError extends S.TaggedErrorClass<PartnerPayoutLedgerStorageError>()(
  'PartnerPayoutLedgerStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,220}$/
const PROHIBITED_REF_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|payment_secret|payment_hash|wallet_secret|private_key|webhook_secret|mdk_access_token|access_token|refresh_token|device_auth_id|code_verifier|gho_[a-z0-9_]+)/i

const requiredPolicyRefs = [PARTNER_PAYOUT_POLICY_REF]
const requiredCaveatRefs = [
  'caveat.partner_payout.operator_dispatch_required',
  'caveat.partner_payout.settlement_evidence_required',
  'caveat.partner_payout.reversible_for_refund_or_abuse',
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
    throw new PartnerPayoutLedgerValidationError({
      reason: `${field} must be a public-safe ref.`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): void => values?.forEach(value => assertSafeRef(field, value))

/** Order-preserving de-duplication for ref lists. */
const dedupeRefs = (values: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(values),
]

const rowToEntry = (row: PartnerPayoutLedgerRow): PartnerPayoutLedgerEntry => ({
  amount: Number(row.amount),
  archivedAt: row.archived_at,
  asset: row.asset,
  beneficiaryUserId: row.beneficiary_user_id,
  caveatRefs: parseJsonStringArray(row.caveat_refs_json),
  createdAt: row.created_at,
  evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  partnerRef: row.partner_ref,
  partnerRole: row.partner_role,
  partnerUserId: row.partner_user_id,
  payoutRef: row.payout_ref,
  periodKey: row.period_key,
  policyRefs: parseJsonStringArray(row.policy_refs_json),
  previousEntryId: row.previous_entry_id,
  qualifyingAmount: Number(row.qualifying_amount),
  qualifyingEventKind: row.qualifying_event_kind,
  qualifyingEventRef: row.qualifying_event_ref,
  reversalOfEntryId: row.reversal_of_entry_id,
  state: row.state,
  stateReasonRef: row.state_reason_ref,
})

export const projectPartnerPayout = (
  entry: PartnerPayoutLedgerEntry,
): PartnerPayoutProjection => ({
  amount: entry.amount,
  asset: entry.asset,
  authorityBoundary:
    'Partner payout ledger state is not spendable value until operator-gated dispatch settles with public-safe evidence refs.',
  caveatRefs: entry.caveatRefs,
  currentEntryId: entry.id,
  evidenceRefs: entry.evidenceRefs,
  partnerRef: entry.partnerRef,
  partnerRole: entry.partnerRole,
  payoutRef: entry.payoutRef,
  policyRefs: entry.policyRefs,
  qualifyingEventRef: entry.qualifyingEventRef,
  state: entry.state,
  stateReasonRef: entry.stateReasonRef,
})

export const calculatePartnerPayoutAmount = (
  role: PartnerPayoutRole,
  qualifyingAmount: number,
): number => {
  if (!Number.isFinite(qualifyingAmount) || qualifyingAmount <= 0) {
    return 0
  }

  const policy = PARTNER_PAYOUT_ROLE_POLICY[role]

  return Math.max(
    1,
    Math.min(
      policy.maxEventAmount,
      Math.floor((qualifyingAmount * policy.percentBps) / 10000),
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
    throw new PartnerPayoutLedgerStorageError({ error, operation })
  }
}

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<PartnerPayoutLedgerEntry | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM partner_payout_ledger_entries
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<PartnerPayoutLedgerRow>()

  return row === null ? null : rowToEntry(row)
}

export const readCurrentPartnerPayout = async (
  db: D1Database,
  payoutRef: string,
): Promise<PartnerPayoutLedgerEntry | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM partner_payout_ledger_entries
        WHERE payout_ref = ?
          AND archived_at IS NULL
        ORDER BY created_at DESC, id DESC
        LIMIT 1`,
    )
    .bind(payoutRef)
    .first<PartnerPayoutLedgerRow>()

  return row === null ? null : rowToEntry(row)
}

const readPartnerPeriodTotals = async (
  db: D1Database,
  partnerUserId: string,
  periodKey: string,
  asset: PartnerPayoutAsset,
): Promise<Readonly<{ amount: number; count: number }>> => {
  const row = await db
    .prepare(
      `SELECT COUNT(*) AS payout_count,
              COALESCE(SUM(amount), 0) AS payout_amount
         FROM partner_payout_ledger_entries
        WHERE partner_user_id = ?
          AND period_key = ?
          AND asset = ?
          AND state IN ('eligible', 'approved', 'dispatched', 'settled')
          AND amount > 0
          AND archived_at IS NULL`,
    )
    .bind(partnerUserId, periodKey, asset)
    .first<PartnerPeriodTotalsRow>()

  return {
    amount: Number(row?.payout_amount ?? 0),
    count: Number(row?.payout_count ?? 0),
  }
}

const insertEntry = async (
  database: TreasuryDatabase,
  entry: PartnerPayoutLedgerEntry,
): Promise<PartnerPayoutLedgerEntry> =>
  storage('partnerPayoutLedger.entry.insert', async () => {
    const db = treasuryAuthorityDb(database)
    await db
      .prepare(
        `INSERT INTO partner_payout_ledger_entries (
          id, payout_ref, idempotency_key, partner_role, partner_user_id,
          partner_ref, beneficiary_user_id, asset, qualifying_event_ref,
          qualifying_event_kind, qualifying_amount, amount, period_key,
          state, state_reason_ref, previous_entry_id, reversal_of_entry_id,
          evidence_refs_json, policy_refs_json, caveat_refs_json,
          created_at, archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        entry.id,
        entry.payoutRef,
        entry.idempotencyKey,
        entry.partnerRole,
        entry.partnerUserId,
        entry.partnerRef,
        entry.beneficiaryUserId,
        entry.asset,
        entry.qualifyingEventRef,
        entry.qualifyingEventKind,
        entry.qualifyingAmount,
        entry.amount,
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
    await mirrorTreasuryRows(database, 'partner_payout_ledger_entries', 'id', [
      entry.id,
    ])

    return (await readByIdempotencyKey(db, entry.idempotencyKey)) ?? entry
  })

export const createPartnerPayoutEligibility = async (
  database: TreasuryDatabase,
  input: CreatePartnerPayoutEligibilityInput,
): Promise<PartnerPayoutLedgerEntry> => {
  const db = treasuryAuthorityDb(database)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('payoutRef', input.payoutRef)
  assertSafeRef('qualifyingEventRef', input.qualifyingEventRef)
  assertSafeRef('partnerRef', input.partnerRef)
  assertSafeRef('partnerUserId', input.partnerUserId)
  assertSafeRef('beneficiaryUserId', input.beneficiaryUserId)
  assertSafeRefs('evidenceRefs', input.evidenceRefs)
  assertSafeRefs('policyRefs', input.policyRefs)

  if (!Number.isFinite(input.qualifyingAmount) || input.qualifyingAmount < 0) {
    throw new PartnerPayoutLedgerValidationError({
      reason: 'qualifyingAmount must be finite and non-negative.',
    })
  }

  const existing = await storage('partnerPayoutLedger.idempotency.read', () =>
    readByIdempotencyKey(db, input.idempotencyKey),
  )

  if (existing !== null) {
    return existing
  }

  const policy = PARTNER_PAYOUT_ROLE_POLICY[input.partnerRole]
  const totals = await storage('partnerPayoutLedger.partnerPeriod.read', () =>
    readPartnerPeriodTotals(
      db,
      input.partnerUserId,
      input.periodKey,
      input.asset,
    ),
  )
  const calculatedAmount = calculatePartnerPayoutAmount(
    input.partnerRole,
    input.qualifyingAmount,
  )
  const isSelfPayout =
    input.beneficiaryUserId !== undefined &&
    input.beneficiaryUserId !== null &&
    input.beneficiaryUserId === input.partnerUserId
  const cappedByCount = totals.count >= policy.maxPartnerPeriodCount
  const cappedByAmount =
    totals.amount + calculatedAmount > policy.maxPartnerPeriodAmount
  const state: PartnerPayoutState =
    isSelfPayout || cappedByCount || cappedByAmount || calculatedAmount === 0
      ? 'refused'
      : 'eligible'
  const stateReasonRef = isSelfPayout
    ? 'reason.public.partner_payout.self_payout'
    : cappedByCount || cappedByAmount
      ? 'reason.public.partner_payout.partner_period_cap_exceeded'
      : calculatedAmount === 0
        ? 'reason.public.partner_payout.no_qualifying_amount'
        : null

  return insertEntry(database, {
    amount: state === 'eligible' ? calculatedAmount : 0,
    archivedAt: null,
    asset: input.asset,
    beneficiaryUserId: input.beneficiaryUserId ?? null,
    caveatRefs: requiredCaveatRefs,
    createdAt: input.nowIso,
    evidenceRefs: dedupeRefs([
      input.qualifyingEventRef,
      ...(input.evidenceRefs ?? []),
    ]),
    id: input.id ?? compactRandomId('partner_payout_entry'),
    idempotencyKey: input.idempotencyKey,
    partnerRef: input.partnerRef,
    partnerRole: input.partnerRole,
    partnerUserId: input.partnerUserId,
    payoutRef:
      input.payoutRef ??
      `partner_payout_${input.partnerRole}_${input.partnerRef}`,
    periodKey: input.periodKey,
    policyRefs: dedupeRefs([...requiredPolicyRefs, ...(input.policyRefs ?? [])]),
    previousEntryId: null,
    qualifyingAmount: Math.floor(input.qualifyingAmount),
    qualifyingEventKind: input.qualifyingEventKind,
    qualifyingEventRef: input.qualifyingEventRef,
    reversalOfEntryId: null,
    state,
    stateReasonRef,
  })
}

const transitionForAction: Record<
  PartnerPayoutAction,
  Readonly<{ from: ReadonlyArray<PartnerPayoutState>; to: PartnerPayoutState }>
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

export const transitionPartnerPayout = async (
  database: TreasuryDatabase,
  input: TransitionPartnerPayoutInput,
): Promise<PartnerPayoutLedgerEntry> => {
  const db = treasuryAuthorityDb(database)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('payoutRef', input.payoutRef)
  assertSafeRef('stateReasonRef', input.stateReasonRef)
  assertSafeRefs('evidenceRefs', input.evidenceRefs)

  const existing = await storage('partnerPayoutLedger.idempotency.read', () =>
    readByIdempotencyKey(db, input.idempotencyKey),
  )

  if (existing !== null) {
    return existing
  }

  const current = await storage('partnerPayoutLedger.current.read', () =>
    readCurrentPartnerPayout(db, input.payoutRef),
  )

  if (current === null) {
    throw new PartnerPayoutLedgerValidationError({
      reason: 'Unknown partner payout ref.',
    })
  }

  const transition = transitionForAction[input.action]

  if (!transition.from.includes(current.state)) {
    throw new PartnerPayoutLedgerValidationError({
      reason: `Payout in state ${current.state} cannot ${input.action}.`,
    })
  }

  if (transition.to === 'settled' && (input.evidenceRefs ?? []).length === 0) {
    throw new PartnerPayoutLedgerValidationError({
      reason: 'mark_settled requires public-safe settlement evidence refs.',
    })
  }

  const amount =
    transition.to === 'reversed'
      ? -Math.abs(current.amount)
      : current.amount

  return insertEntry(database, {
    ...current,
    amount,
    createdAt: input.nowIso,
    evidenceRefs: [...current.evidenceRefs, ...(input.evidenceRefs ?? [])],
    id: input.id ?? compactRandomId('partner_payout_entry'),
    idempotencyKey: input.idempotencyKey,
    previousEntryId: current.id,
    reversalOfEntryId: transition.to === 'reversed' ? current.id : null,
    state: transition.to,
    stateReasonRef: input.stateReasonRef ?? current.stateReasonRef,
  })
}
