/**
 * Partner-payout feed: storage-backed wire from a real paid event into the
 * operator-gated partner payout ledger (#5524 follow-up).
 *
 * Advances `blocker.product_promises.partner_attribution_policy_missing` by
 * supplying the one documented remaining step the pure halves left open:
 *
 *   readActivePartnerAgreementsForCustomer()  -- loads the EXPLICIT candidate
 *     (this module, storage)                     agreements for a paying customer
 *   resolvePartnerPayoutEligibilityInput()    -- runs the attribution policy and
 *     (partner-attribution-eligibility.ts)       maps an `attributed` decision to
 *                                                a ledger-ready eligibility input
 *   createPartnerPayoutEligibility()          -- records ONE eligibility row in
 *     (partner-payout-ledger.ts)                 the operator-gated ledger
 *
 * This is the partner-rail analogue of `recordReferralPayoutForPaidEvent`
 * (`site-referral-payout-feed.ts`), and it is DISTINCT BY CONSTRUCTION: the
 * referral feed infers its earner from last-touch click attribution; this feed
 * reads only EXPLICIT, currently-active partner agreements from the
 * `partner_agreements` table (migration 0214). When a paid customer has no
 * covering agreement (the common case) it records NOTHING — there is no inferred
 * fallback. It never moves money; it only records eligibility, which stays
 * operator-gated through approve/dispatch/settle.
 *
 * The agreement reader and the ledger writer are injectable so the product rules
 * remain independently testable without a live D1; the defaults bind to D1.
 */
import { Schema as S } from 'effect'

import {
  type PartnerAgreement,
  assessPartnerAgreementSeed,
} from './partner-attribution-policy'
import {
  type PartnerPayoutEligibilityResolution,
  type PartnerQualifyingPaidEvent,
  resolvePartnerPayoutEligibilityInput,
} from './partner-attribution-eligibility'
import {
  type PartnerPayoutLedgerEntry,
  PartnerPayoutLedgerStorageError,
  type PartnerPayoutLedgerValidationError,
  type PartnerPayoutRole,
  createPartnerPayoutEligibility,
} from './partner-payout-ledger'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

/**
 * Same public-safe user-id shape the referral feed guards against
 * (`site-referral-payout-feed.ts`); a malformed id reads zero candidate
 * agreements rather than building an unbounded query.
 */
const SAFE_USER_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

/**
 * A candidate-agreement row as stored in `partner_agreements` (migration 0214).
 * The window is the agreement's own; the attribution policy decides coverage at
 * the event time, so this read returns all currently-active agreements for the
 * customer and lets `decidePartnerAttribution` apply the window precisely.
 */
type PartnerAgreementRow = Readonly<{
  agreement_ref: string
  effective_from: string
  effective_until: string | null
  partner_ref: string
  partner_user_id: string
  role: PartnerPayoutRole
}>

const rowToAgreement = (row: PartnerAgreementRow): PartnerAgreement => ({
  agreementRef: row.agreement_ref,
  effectiveFromIso: row.effective_from,
  effectiveUntilIso: row.effective_until,
  partnerRef: row.partner_ref,
  partnerUserId: row.partner_user_id,
  role: row.role,
})

/**
 * Reader contract: load the candidate partner agreements for a paying customer.
 * Injectable so the feed can be tested without a live D1.
 */
export type PartnerAgreementReader = (
  db: D1Database,
  customerUserId: string,
) => Promise<ReadonlyArray<PartnerAgreement>>

/**
 * Default D1-backed reader. Loads every currently-active, non-archived agreement
 * naming the customer; the attribution policy then applies role eligibility, the
 * active window, precedence, and self-payout exclusion. Read-only and bounded.
 */
export const readActivePartnerAgreementsForCustomer: PartnerAgreementReader =
  async (db, customerUserId) => {
    if (!SAFE_USER_ID_PATTERN.test(customerUserId)) {
      return []
    }

    const result = await db
      .prepare(
        `SELECT agreement_ref AS agreement_ref,
                partner_ref AS partner_ref,
                partner_user_id AS partner_user_id,
                role AS role,
                effective_from AS effective_from,
                effective_until AS effective_until
           FROM partner_agreements
          WHERE customer_user_id = ?
            AND archived_at IS NULL
            AND policy_state = 'active'
          ORDER BY effective_from ASC, agreement_ref ASC
          LIMIT 100`,
      )
      .bind(customerUserId)
      .all<PartnerAgreementRow>()

    return (result.results ?? []).map(rowToAgreement)
  }

/**
 * Raised when an agreement fails the attribution policy's write-boundary
 * invariants or carries a non-public-safe ref/id. Distinct from the ledger's own
 * validation error: this guards what may be STORED as an agreement, before any
 * payout exists.
 */
export class PartnerAgreementValidationError extends S.TaggedErrorClass<PartnerAgreementValidationError>()(
  'PartnerAgreementValidationError',
  {
    reason: S.String,
  },
) {}

const wrapStorage = async <T>(
  operation: string,
  run: () => Promise<T>,
): Promise<T> => {
  try {
    return await run()
  } catch (error) {
    throw new PartnerPayoutLedgerStorageError({ error, operation })
  }
}

const readAgreementByRef = async (
  db: D1Database,
  agreementRef: string,
): Promise<PartnerAgreement | null> => {
  const row = await db
    .prepare(
      `SELECT agreement_ref AS agreement_ref,
              partner_ref AS partner_ref,
              partner_user_id AS partner_user_id,
              role AS role,
              effective_from AS effective_from,
              effective_until AS effective_until
         FROM partner_agreements
        WHERE agreement_ref = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(agreementRef)
    .first<PartnerAgreementRow>()

  return row === null ? null : rowToAgreement(row)
}

/**
 * Input for seeding ONE explicit partner agreement (migration 0214). The
 * attribution policy reads these rows; without one, no partner is ever credited
 * for the named customer (the no-fallback rule). Holds only public-safe
 * refs/ids — no payout destinations, invoices, preimages, or provider payloads.
 */
export type CreatePartnerAgreementInput = Readonly<{
  agreementRef: string
  customerUserId: string
  effectiveFromIso: string
  effectiveUntilIso?: string | null
  id?: string
  nowIso: string
  partnerRef: string
  partnerUserId: string
  role: PartnerPayoutRole
}>

const SAFE_AGREEMENT_FIELDS = [
  'agreementRef',
  'partnerRef',
  'partnerUserId',
  'customerUserId',
] as const

/**
 * Seed (or idempotently return) one explicit partner agreement.
 *
 * This is the writer the feed's reader had no counterpart for: it is the only
 * sanctioned way a row enters `partner_agreements`, and it enforces the
 * attribution policy at the WRITE boundary so a policy-violating agreement can
 * never be stored and later credited. Rejects non-public-safe refs/ids, then
 * applies `assessPartnerAgreementSeed` (role attributability incl. the referral
 * exclusion, self-agreement exclusion, and window consistency). Idempotent on
 * `agreementRef`. Never moves money; it only records who MAY be attributed.
 */
export const recordPartnerAgreement = async (
  database: TreasuryDatabase,
  input: CreatePartnerAgreementInput,
): Promise<PartnerAgreement> => {
  const db = treasuryAuthorityDb(database)
  const values: Record<(typeof SAFE_AGREEMENT_FIELDS)[number], string> = {
    agreementRef: input.agreementRef,
    customerUserId: input.customerUserId,
    partnerRef: input.partnerRef,
    partnerUserId: input.partnerUserId,
  }

  for (const field of SAFE_AGREEMENT_FIELDS) {
    if (!SAFE_USER_ID_PATTERN.test(values[field])) {
      throw new PartnerAgreementValidationError({
        reason: `${field} must be a public-safe ref.`,
      })
    }
  }

  const effectiveUntilIso = input.effectiveUntilIso ?? null
  const decision = assessPartnerAgreementSeed({
    customerUserId: input.customerUserId,
    effectiveFromIso: input.effectiveFromIso,
    effectiveUntilIso,
    partnerUserId: input.partnerUserId,
    role: input.role,
  })

  if (decision._tag === 'rejected') {
    throw new PartnerAgreementValidationError({ reason: decision.reason })
  }

  const existing = await wrapStorage('partnerAgreements.byRef.read', () =>
    readAgreementByRef(db, input.agreementRef),
  )

  if (existing !== null) {
    return existing
  }

  await wrapStorage('partnerAgreements.insert', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO partner_agreements (
          id, agreement_ref, partner_ref, partner_user_id, customer_user_id,
          role, effective_from, effective_until, policy_state, created_at,
          archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, NULL)`,
      )
      .bind(
        input.id ?? `partner_agreement_${input.agreementRef}`,
        input.agreementRef,
        input.partnerRef,
        input.partnerUserId,
        input.customerUserId,
        input.role,
        input.effectiveFromIso,
        effectiveUntilIso,
        input.nowIso,
      )
      .run(),
  )

  const stored = await wrapStorage('partnerAgreements.readback', () =>
    readAgreementByRef(db, input.agreementRef),
  )

  if (stored === null) {
    throw new PartnerAgreementValidationError({
      reason: 'partner agreement was not persisted.',
    })
  }

  // KS-8.8 (#8319): fail-soft Postgres mirror of the persisted agreement.
  await mirrorTreasuryRows(database, 'partner_agreements', 'agreement_ref', [
    input.agreementRef,
  ])

  return stored
}

/**
 * Result of feeding a qualifying paid event into the partner payout ledger.
 *
 * - `recorded`            -- an explicit active agreement won; one eligibility
 *                           row was created (operator-gated from here). The
 *                           attribution basis (`agreementRef`/`policyRef`) is
 *                           surfaced for the caller to record alongside it.
 * - `no_active_agreement` -- no covering agreement (the common case); recorded
 *                           nothing. No inferred fallback.
 * - `self_attribution`    -- the only winner is the paying customer; recorded
 *                           nothing.
 */
export type RecordPartnerPayoutResult =
  | Readonly<{ _tag: 'no_active_agreement' }>
  | Readonly<{ _tag: 'self_attribution'; partnerRef: string }>
  | Readonly<{
      _tag: 'recorded'
      agreementRef: string
      entry: PartnerPayoutLedgerEntry
      policyRef: string
    }>

export type RecordPartnerPayoutError =
  | PartnerPayoutLedgerStorageError
  | PartnerPayoutLedgerValidationError

/**
 * Injectable dependencies; both default to the D1-backed implementations.
 */
export type RecordPartnerPayoutDeps = Readonly<{
  createEligibility?: typeof createPartnerPayoutEligibility
  readAgreements?: PartnerAgreementReader
}>

/**
 * Wire a real qualifying paid event into the partner payout ledger.
 *
 * Reads the customer's explicit candidate agreements, runs the attribution
 * policy via `resolvePartnerPayoutEligibilityInput`, and on an `eligible`
 * decision records exactly one eligibility row (idempotent on the event's
 * `idempotencyKey`). A `no_active_agreement` or `self_attribution` decision
 * records nothing. This module never moves money and never infers an earner.
 */
export const recordPartnerPayoutForPaidEvent = async (
  db: D1Database,
  event: PartnerQualifyingPaidEvent,
  deps: RecordPartnerPayoutDeps = {},
): Promise<RecordPartnerPayoutResult> => {
  const readAgreements = deps.readAgreements ?? readActivePartnerAgreementsForCustomer
  const createEligibility = deps.createEligibility ?? createPartnerPayoutEligibility

  const candidateAgreements = await readAgreements(db, event.customerUserId)

  const resolution: PartnerPayoutEligibilityResolution =
    resolvePartnerPayoutEligibilityInput(event, candidateAgreements)

  if (resolution._tag === 'no_active_agreement') {
    return { _tag: 'no_active_agreement' }
  }

  if (resolution._tag === 'self_attribution') {
    return { _tag: 'self_attribution', partnerRef: resolution.partnerRef }
  }

  const entry = await createEligibility(db, resolution.input)

  return {
    _tag: 'recorded',
    agreementRef: resolution.agreementRef,
    entry,
    policyRef: resolution.policyRef,
  }
}
