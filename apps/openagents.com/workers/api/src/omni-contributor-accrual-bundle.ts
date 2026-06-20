import { Schema as S } from 'effect'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniContributorAccrualLedger,
  OmniContributorAccrualLedger,
  publicOmniContributorAccrualLedgerProjection,
} from './omni-contributor-accrual-ledger'
import {
  resolveOmniContributorShares,
  type OmniContributorSharePolicyInput,
} from './omni-contributor-share-policy'
import {
  buildOmniGrossMarginReceipt,
  OmniGrossMarginReceipt,
  publicOmniGrossMarginReceiptProjection,
} from './omni-gross-margin-receipt'

/**
 * Contributor accrual bundle for a single accepted outcome.
 *
 * The promise payments.accepted_outcome_economics.v1 keeps two distinct views of
 * one outcome's economics:
 *
 * - the gross-margin RECEIPT (omni-gross-margin-receipt.ts), which names the
 *   outcome's lifecycle states (buyer authorized/paid, accepted value, cost
 *   basis, gross margin, pending balance, payout, settlement, reconciliation)
 *   and honestly labels which are evidenced; and
 * - the contributor accrual LEDGER (omni-contributor-accrual-ledger.ts), which
 *   attributes the same outcome's derived gross margin to its contributors.
 *
 * Both are built independently from the same economics record, and the share
 * split itself is produced upstream by the share policy
 * (omni-contributor-share-policy.ts). Nothing tied the three together or proved
 * that the two views agree, which is exactly the seam the registry blocker
 * blocker.product_promises.contributor_ledger_missing calls out: a single
 * dereference point for an outcome's accruals, bound to the receipt so neither
 * view can silently invent or lose margin relative to the other.
 *
 * This module is that composition root. Given one economics record and the
 * identified parties, it resolves the share split, builds the ledger and the
 * receipt, and enforces a cross-view RECONCILIATION invariant before returning a
 * single bundle keyed by accepted-outcome id. It performs no money movement and
 * preserves the promise's no-collapse discipline: an accrual is still not a
 * payable balance, and a recorded gross margin is still not settlement evidence.
 */

export class OmniContributorAccrualBundleInvariantError extends S.TaggedErrorClass<OmniContributorAccrualBundleInvariantError>()(
  'OmniContributorAccrualBundleInvariantError',
  { reason: S.String },
) {}

export const OmniContributorAccrualBundle = S.Struct({
  bundleKind: S.Literal('accepted_outcome_accrual_bundle'),
  contributorAccrualLedger: OmniContributorAccrualLedger,
  economicsId: S.String,
  grossMarginReceipt: OmniGrossMarginReceipt,
  // The single derived gross margin both views agree on, after reconciliation.
  reconciledGrossMarginCents: S.Number,
})
export type OmniContributorAccrualBundle =
  typeof OmniContributorAccrualBundle.Type

/**
 * Build a contributor accrual bundle from one economics record and the
 * identified parties for the outcome.
 *
 * Deterministic and pure: the same inputs always yield the same bundle.
 *
 * Honesty rules enforced by construction (in addition to those each underlying
 * builder already enforces):
 * - The receipt and the ledger must reference the SAME accepted-outcome id.
 * - The receipt's recorded gross margin, its gross_margin lifecycle line, and
 *   the ledger's gross margin must be the same figure -- neither view may carry
 *   a different margin for the same outcome.
 * - The ledger's distributable pool must equal max(0, that gross margin), and
 *   the contributors' accruals must sum to it exactly, so no margin is invented
 *   or lost between the receipt and the attribution.
 * - The receipt and the ledger must agree on whether settlement is disclaimed,
 *   so a bundle can never present one half as settled while the other is not.
 */
export const buildOmniContributorAccrualBundle = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  parties: OmniContributorSharePolicyInput,
): OmniContributorAccrualBundle => {
  const shares = resolveOmniContributorShares(parties)
  const contributorAccrualLedger = buildOmniContributorAccrualLedger(
    record,
    shares,
  )
  const grossMarginReceipt = buildOmniGrossMarginReceipt(record)

  if (grossMarginReceipt.economicsId !== contributorAccrualLedger.economicsId) {
    throw new OmniContributorAccrualBundleInvariantError({
      reason: `receipt economicsId ${grossMarginReceipt.economicsId} must match ledger economicsId ${contributorAccrualLedger.economicsId}.`,
    })
  }

  if (
    grossMarginReceipt.grossMarginCents !==
    contributorAccrualLedger.grossMarginCents
  ) {
    throw new OmniContributorAccrualBundleInvariantError({
      reason: `receipt gross margin ${grossMarginReceipt.grossMarginCents} must match ledger gross margin ${contributorAccrualLedger.grossMarginCents}.`,
    })
  }

  const grossMarginLine = grossMarginReceipt.lines.find(
    line => line.stateId === 'gross_margin',
  )
  if (
    grossMarginLine === undefined ||
    grossMarginLine.amountCents !== grossMarginReceipt.grossMarginCents
  ) {
    throw new OmniContributorAccrualBundleInvariantError({
      reason: `receipt gross_margin line must carry the receipt gross margin ${grossMarginReceipt.grossMarginCents}.`,
    })
  }

  const expectedDistributable = Math.max(0, grossMarginReceipt.grossMarginCents)
  if (
    contributorAccrualLedger.distributableMarginCents !== expectedDistributable
  ) {
    throw new OmniContributorAccrualBundleInvariantError({
      reason: `ledger distributable margin ${contributorAccrualLedger.distributableMarginCents} must equal max(0, gross margin) ${expectedDistributable}.`,
    })
  }

  if (
    contributorAccrualLedger.totalAccruedCents !==
    contributorAccrualLedger.distributableMarginCents
  ) {
    throw new OmniContributorAccrualBundleInvariantError({
      reason: `ledger accrued total ${contributorAccrualLedger.totalAccruedCents} must equal distributable margin ${contributorAccrualLedger.distributableMarginCents}; margin must not be invented or lost in attribution.`,
    })
  }

  if (
    grossMarginReceipt.noSettlementImplication !==
    contributorAccrualLedger.noSettlementImplication
  ) {
    throw new OmniContributorAccrualBundleInvariantError({
      reason: 'receipt and ledger must agree on whether settlement is disclaimed.',
    })
  }

  return {
    bundleKind: 'accepted_outcome_accrual_bundle',
    contributorAccrualLedger,
    economicsId: grossMarginReceipt.economicsId,
    grossMarginReceipt,
    reconciledGrossMarginCents: grossMarginReceipt.grossMarginCents,
  }
}

/**
 * Public projection: composes the two underlying public projections (which keep
 * lifecycle and evidence labels visible while dropping internal monetary
 * figures) and likewise drops the reconciled gross-margin figure.
 */
export const publicOmniContributorAccrualBundleProjection = (
  bundle: OmniContributorAccrualBundle,
) => ({
  bundleKind: bundle.bundleKind,
  contributorAccrualLedger: publicOmniContributorAccrualLedgerProjection(
    bundle.contributorAccrualLedger,
  ),
  grossMarginReceipt: publicOmniGrossMarginReceiptProjection(
    bundle.grossMarginReceipt,
  ),
})
