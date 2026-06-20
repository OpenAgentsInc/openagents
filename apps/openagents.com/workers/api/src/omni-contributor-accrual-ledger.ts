import { Schema as S } from 'effect'

import { OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema } from './omni-accepted-outcome-contracts'
import {
  type OmniAcceptedOutcomeEconomicsRecord,
  OmniAcceptedOutcomeFundingMode,
} from './omni-accepted-outcome-economics'

/**
 * Contributor accrual ledger for a single accepted outcome.
 *
 * The promise payments.accepted_outcome_economics.v1 requires that every
 * accepted outcome distinguish buyer payment, accepted value, pending balance
 * adjustment, payout intent, settlement attempt, reconciliation, and gross
 * margin -- and forbids collapsing paid, accepted, payable, dispatched,
 * confirmed, reconciled, settled, and gross-margin states into one claim.
 *
 * The gross-margin receipt names those lifecycle states for the OUTCOME as a
 * whole. This builder takes the next, distinct step the registry calls out as a
 * blocker (blocker.product_promises.contributor_ledger_missing): it attributes
 * the OUTCOME's *derived* gross margin to the contributors who produced it, by
 * basis-points share, as ACCOUNTING-ONLY ACCRUALS.
 *
 * An accrual is not a payable balance, a dispatched payout, or a settled
 * payment. While the source economics record disclaims settlement implication
 * (noSettlementImplication = true), every contributor's payable and settlement
 * lifecycle states are forced to not_yet_evidenced, and a defensive invariant
 * (OmniContributorAccrualLedgerInvariantError) rejects any attempt to present a
 * contributor's accrual as a payable or settled balance. The accrued cents are
 * always derived from the recorded gross margin, never collapsed with payout
 * or settlement evidence.
 */

export const OmniContributorAccrualRole = S.Literals([
  // The agent/operator that executed the work.
  'runner',
  // A human or agent that reviewed/accepted the outcome.
  'reviewer',
  // The party that supplied the source data, prompt, or brief.
  'originator',
  // The party that referred the buyer or the contributor.
  'referrer',
  // The platform's retained share of gross margin.
  'platform',
])
export type OmniContributorAccrualRole = typeof OmniContributorAccrualRole.Type

// An accrued share is DERIVED from the recorded gross margin. A payable or
// settled state has no evidence on this accounting-only substrate.
export const OmniContributorAccrualEvidenceState = S.Literals([
  'accrual_derived',
  'not_yet_evidenced',
])
export type OmniContributorAccrualEvidenceState =
  typeof OmniContributorAccrualEvidenceState.Type

export const OmniContributorAccrualShare = S.Struct({
  contributorId: S.String,
  role: OmniContributorAccrualRole,
  // Share of the distributable gross-margin pool, in basis points (0..10000).
  shareBasisPoints: S.Number,
})
export type OmniContributorAccrualShare = typeof OmniContributorAccrualShare.Type

export const OmniContributorAccrualEntry = S.Struct({
  // Cents attributed to this contributor, derived from gross margin by share.
  accruedMarginCents: S.Number,
  // Always 'accrual_derived': this is a recorded accrual, nothing more.
  accrualEvidenceState: OmniContributorAccrualEvidenceState,
  contributorId: S.String,
  // True because claiming a payable/settled balance here would imply a payout
  // or settlement this accounting substrate cannot prove.
  impliesSettlement: S.Boolean,
  // Whether a payable balance has been evidenced for this contributor.
  payableEvidenceState: OmniContributorAccrualEvidenceState,
  role: OmniContributorAccrualRole,
  // Whether a settled (paid + reconciled) balance has been evidenced.
  settlementEvidenceState: OmniContributorAccrualEvidenceState,
  shareBasisPoints: S.Number,
})
export type OmniContributorAccrualEntry =
  typeof OmniContributorAccrualEntry.Type

export const OmniContributorAccrualLedger = S.Struct({
  distributableMarginCents: S.Number,
  economicsId: S.String,
  entries: S.Array(OmniContributorAccrualEntry),
  fundingMode: OmniAcceptedOutcomeFundingMode,
  grossMarginCents: S.Number,
  ledgerKind: S.Literal('accepted_outcome_contributor_accrual'),
  noSettlementImplication: S.Boolean,
  publicCaveatRef: S.String,
  // Count of entries with any payable/settled evidence. Must be 0 while the
  // economics record disclaims settlement implication.
  settlementEvidencedEntryCount: S.Number,
  totalAccruedCents: S.Number,
  totalShareBasisPoints: S.Number,
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniContributorAccrualLedger =
  typeof OmniContributorAccrualLedger.Type

export class OmniContributorAccrualLedgerValidationError extends S.TaggedErrorClass<OmniContributorAccrualLedgerValidationError>()(
  'OmniContributorAccrualLedgerValidationError',
  { reason: S.String },
) {}

export class OmniContributorAccrualLedgerInvariantError extends S.TaggedErrorClass<OmniContributorAccrualLedgerInvariantError>()(
  'OmniContributorAccrualLedgerInvariantError',
  { reason: S.String },
) {}

const TOTAL_BASIS_POINTS = 10_000

const CONTRIBUTOR_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,128}$/

const assertValidShares = (
  shares: ReadonlyArray<OmniContributorAccrualShare>,
): void => {
  if (shares.length === 0) {
    throw new OmniContributorAccrualLedgerValidationError({
      reason: 'a contributor accrual ledger requires at least one share.',
    })
  }

  const seen = new Set<string>()
  let totalBasisPoints = 0

  for (const share of shares) {
    if (!CONTRIBUTOR_ID_PATTERN.test(share.contributorId)) {
      throw new OmniContributorAccrualLedgerValidationError({
        reason: `contributorId ${JSON.stringify(share.contributorId)} must be a safe ref.`,
      })
    }
    if (seen.has(share.contributorId)) {
      throw new OmniContributorAccrualLedgerValidationError({
        reason: `contributorId ${share.contributorId} appears more than once.`,
      })
    }
    seen.add(share.contributorId)

    if (
      !Number.isInteger(share.shareBasisPoints) ||
      share.shareBasisPoints < 0 ||
      share.shareBasisPoints > TOTAL_BASIS_POINTS
    ) {
      throw new OmniContributorAccrualLedgerValidationError({
        reason: `shareBasisPoints for ${share.contributorId} must be an integer in [0, ${TOTAL_BASIS_POINTS}].`,
      })
    }
    totalBasisPoints += share.shareBasisPoints
  }

  if (totalBasisPoints !== TOTAL_BASIS_POINTS) {
    throw new OmniContributorAccrualLedgerValidationError({
      reason: `contributor shares must sum to ${TOTAL_BASIS_POINTS} basis points, got ${totalBasisPoints}.`,
    })
  }
}

/**
 * Distribute an integer cents pool across shares (basis points) so the parts
 * sum EXACTLY to the pool. Uses the largest-remainder method with a stable
 * tie-break by input order, so the result is fully deterministic.
 */
const distributePool = (
  poolCents: number,
  shares: ReadonlyArray<OmniContributorAccrualShare>,
): ReadonlyArray<number> => {
  const exact = shares.map(
    share => (poolCents * share.shareBasisPoints) / TOTAL_BASIS_POINTS,
  )
  const floors = exact.map(value => Math.floor(value))
  const assigned = floors.reduce((sum, value) => sum + value, 0)
  let remainder = poolCents - assigned

  // Order indices by descending fractional remainder, then by input order.
  const order = shares
    .map((_, index) => index)
    .sort((a, b) => {
      const fractionalA = exact[a]! - floors[a]!
      const fractionalB = exact[b]! - floors[b]!
      if (fractionalB !== fractionalA) {
        return fractionalB - fractionalA
      }
      return a - b
    })

  const result = [...floors]
  let cursor = 0
  while (remainder > 0 && cursor < order.length) {
    const index = order[cursor]!
    result[index] = result[index]! + 1
    remainder -= 1
    cursor += 1
  }

  return result
}

/**
 * Build a contributor accrual ledger from one economics record and a set of
 * contributor shares.
 *
 * Deterministic and pure: the same inputs always yield the same ledger.
 *
 * Honesty rules enforced by construction:
 * - Shares must sum to exactly 10000 basis points (100%).
 * - The distributable pool is max(0, grossMargin): a loss accrues nothing,
 *   never a negative "owed" balance to a contributor.
 * - Accrued cents are 'accrual_derived' from gross margin and always sum
 *   exactly to the distributable pool (largest-remainder, no rounding leak).
 * - While the record disclaims settlement, every payable/settlement state is
 *   not_yet_evidenced and the defensive invariant rejects any other value.
 */
export const buildOmniContributorAccrualLedger = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  shares: ReadonlyArray<OmniContributorAccrualShare>,
): OmniContributorAccrualLedger => {
  assertValidShares(shares)

  const settlementDisclaimed = record.noSettlementImplication
  const distributableMarginCents = Math.max(0, record.grossMarginCents)
  const accruals = distributePool(distributableMarginCents, shares)

  const entries: ReadonlyArray<OmniContributorAccrualEntry> = shares.map(
    (share, index) => ({
      accruedMarginCents: accruals[index]!,
      accrualEvidenceState: 'accrual_derived',
      contributorId: share.contributorId,
      impliesSettlement: true,
      payableEvidenceState: 'not_yet_evidenced',
      role: share.role,
      settlementEvidenceState: 'not_yet_evidenced',
      shareBasisPoints: share.shareBasisPoints,
    }),
  )

  // Defensive invariant: while settlement is disclaimed, no contributor accrual
  // may be presented as a payable or settled balance, and an accrual must
  // remain a derived figure (never recorded as standalone evidence of payment).
  if (settlementDisclaimed) {
    for (const entry of entries) {
      if (
        entry.payableEvidenceState !== 'not_yet_evidenced' ||
        entry.settlementEvidenceState !== 'not_yet_evidenced'
      ) {
        throw new OmniContributorAccrualLedgerInvariantError({
          reason: `contributor ${entry.contributorId} cannot have a payable or settled balance while the economics record disclaims settlement implication.`,
        })
      }
      if (entry.accrualEvidenceState !== 'accrual_derived') {
        throw new OmniContributorAccrualLedgerInvariantError({
          reason: `contributor ${entry.contributorId} accrual must be derived from gross margin, not recorded as standalone payment evidence.`,
        })
      }
    }
  }

  const totalAccruedCents = entries.reduce(
    (sum, entry) => sum + entry.accruedMarginCents,
    0,
  )

  // The whole point of the largest-remainder distribution: parts sum exactly to
  // the distributable pool, so no margin is invented or lost in attribution.
  if (totalAccruedCents !== distributableMarginCents) {
    throw new OmniContributorAccrualLedgerInvariantError({
      reason: `accrued total ${totalAccruedCents} must equal distributable margin ${distributableMarginCents}.`,
    })
  }

  const settlementEvidencedEntryCount = entries.filter(
    entry =>
      entry.payableEvidenceState !== 'not_yet_evidenced' ||
      entry.settlementEvidenceState !== 'not_yet_evidenced',
  ).length

  return {
    distributableMarginCents,
    economicsId: record.id,
    entries,
    fundingMode: record.fundingMode,
    grossMarginCents: record.grossMarginCents,
    ledgerKind: 'accepted_outcome_contributor_accrual',
    noSettlementImplication: record.noSettlementImplication,
    publicCaveatRef: record.publicCaveatRef,
    settlementEvidencedEntryCount,
    totalAccruedCents,
    totalShareBasisPoints: TOTAL_BASIS_POINTS,
    workKind: record.workKind,
    workroomId: record.workroomId,
  }
}

/**
 * Public projection: keeps each contributor's role, share, and the honest
 * evidence labels (so a reader can see accruals are not payable/settled) while
 * dropping internal monetary figures.
 */
export const publicOmniContributorAccrualLedgerProjection = (
  ledger: OmniContributorAccrualLedger,
) => ({
  entries: ledger.entries.map(entry => ({
    accrualEvidenceState: entry.accrualEvidenceState,
    contributorId: entry.contributorId,
    impliesSettlement: entry.impliesSettlement,
    payableEvidenceState: entry.payableEvidenceState,
    role: entry.role,
    settlementEvidenceState: entry.settlementEvidenceState,
    shareBasisPoints: entry.shareBasisPoints,
  })),
  fundingMode: ledger.fundingMode,
  ledgerKind: ledger.ledgerKind,
  noSettlementImplication: ledger.noSettlementImplication,
  publicCaveatRef: ledger.publicCaveatRef,
  settlementEvidencedEntryCount: ledger.settlementEvidencedEntryCount,
  totalShareBasisPoints: ledger.totalShareBasisPoints,
  workKind: ledger.workKind,
})
