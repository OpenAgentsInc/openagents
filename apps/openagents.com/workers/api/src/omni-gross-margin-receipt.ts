import { Schema as S } from 'effect'

import { OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema } from './omni-accepted-outcome-contracts'
import {
  type OmniAcceptedOutcomeEconomicsRecord,
  OmniAcceptedOutcomeBuyerPriceAsset,
  OmniAcceptedOutcomeFundingMode,
} from './omni-accepted-outcome-economics'

/**
 * Gross-margin receipt for a single accepted outcome.
 *
 * The promise payments.accepted_outcome_economics.v1 requires that every
 * accepted outcome distinguish buyer payment, accepted value, pending balance
 * adjustment, payout intent, settlement attempt, reconciliation, and gross
 * margin -- and explicitly forbids collapsing paid, accepted, payable,
 * dispatched, confirmed, reconciled, settled, and gross-margin states into one
 * claim.
 *
 * The v1 economics record (omni_accepted_outcome_economics) is an
 * accounting-only substrate: it does not charge a buyer, dispatch a payout, or
 * settle a provider, and it always carries noSettlementImplication = true.
 * This builder turns one such record into a dereferenceable receipt that names
 * the full lifecycle of distinct economic states and HONESTLY labels which
 * states have evidence and which do not, so that no settlement-implying state
 * can be presented as evidenced while the record disclaims settlement.
 */

export const OmniGrossMarginReceiptEvidenceState = S.Literals([
  // An accounting figure is recorded on the economics ledger row.
  'accounting_recorded',
  // A figure mechanically derived from recorded figures (e.g. gross margin).
  'derived',
  // No evidence exists for this lifecycle state on this record.
  'not_yet_evidenced',
])
export type OmniGrossMarginReceiptEvidenceState =
  typeof OmniGrossMarginReceiptEvidenceState.Type

export const OmniGrossMarginReceiptStateId = S.Literals([
  'buyer_authorized',
  'buyer_paid',
  'accepted_value',
  'cost_basis',
  'gross_margin',
  'pending_balance_adjustment',
  'payout_intent',
  'settlement_attempt',
  'reconciliation',
])
export type OmniGrossMarginReceiptStateId =
  typeof OmniGrossMarginReceiptStateId.Type

export const OmniGrossMarginReceiptLine = S.Struct({
  amountCents: S.NullOr(S.Number),
  asset: OmniAcceptedOutcomeBuyerPriceAsset,
  evidenceState: OmniGrossMarginReceiptEvidenceState,
  // True when claiming this state as evidenced would imply a buyer payment
  // capture, payout, or settlement that this accounting substrate cannot prove.
  impliesSettlement: S.Boolean,
  stateId: OmniGrossMarginReceiptStateId,
})
export type OmniGrossMarginReceiptLine = typeof OmniGrossMarginReceiptLine.Type

export const OmniGrossMarginReceipt = S.Struct({
  economicsId: S.String,
  evidencedStateCount: S.Number,
  fundingMode: OmniAcceptedOutcomeFundingMode,
  grossMarginCents: S.Number,
  lines: S.Array(OmniGrossMarginReceiptLine),
  noSettlementImplication: S.Boolean,
  publicCaveatRef: S.String,
  receiptKind: S.Literal('accepted_outcome_gross_margin'),
  unevidencedStateIds: S.Array(OmniGrossMarginReceiptStateId),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniGrossMarginReceipt = typeof OmniGrossMarginReceipt.Type

export class OmniGrossMarginReceiptInvariantError extends S.TaggedErrorClass<OmniGrossMarginReceiptInvariantError>()(
  'OmniGrossMarginReceiptInvariantError',
  { reason: S.String },
) {}

// Lifecycle states whose evidence would imply a real money movement (buyer
// capture, balance adjustment, payout, settlement, or reconciliation) that the
// accounting-only economics substrate deliberately does not perform.
const SETTLEMENT_IMPLYING_STATES: ReadonlySet<OmniGrossMarginReceiptStateId> =
  new Set([
    'buyer_paid',
    'pending_balance_adjustment',
    'payout_intent',
    'settlement_attempt',
    'reconciliation',
  ])

const buyerAmountCents = (
  record: OmniAcceptedOutcomeEconomicsRecord,
): number => {
  switch (record.buyerPriceAsset) {
    case 'credits':
      return record.creditsCharged
    case 'sats':
      return record.satsCharged
    case 'usd':
      return record.buyerPriceCents
    case 'none':
      return 0
  }
}

/**
 * Build a gross-margin receipt from a single economics record.
 *
 * Deterministic and pure: the same record always yields the same receipt.
 *
 * Honesty rules enforced by construction:
 * - buyer_authorized reflects only the recorded buyer price/charge figure.
 * - buyer_paid and every payout/settlement/reconciliation state are
 *   not_yet_evidenced whenever the record disclaims settlement, because this
 *   substrate does not capture payment, dispatch payout, or settle a provider.
 * - gross_margin is always derived, never collapsed with settlement evidence.
 */
export const buildOmniGrossMarginReceipt = (
  record: OmniAcceptedOutcomeEconomicsRecord,
): OmniGrossMarginReceipt => {
  const buyerCents = buyerAmountCents(record)
  const settlementDisclaimed = record.noSettlementImplication

  const settlementLine = (
    stateId: OmniGrossMarginReceiptStateId,
  ): OmniGrossMarginReceiptLine => ({
    amountCents: null,
    asset: 'none',
    evidenceState: 'not_yet_evidenced',
    impliesSettlement: true,
    stateId,
  })

  const lines: ReadonlyArray<OmniGrossMarginReceiptLine> = [
    {
      amountCents: buyerCents > 0 ? buyerCents : null,
      asset: record.buyerPriceAsset,
      evidenceState:
        buyerCents > 0 ? 'accounting_recorded' : 'not_yet_evidenced',
      impliesSettlement: false,
      stateId: 'buyer_authorized',
    },
    // The accounting substrate does not capture buyer payment; while settlement
    // is disclaimed this stays unevidenced rather than reusing the price figure.
    settlementLine('buyer_paid'),
    {
      amountCents: record.acceptedValueCents,
      asset: 'usd',
      evidenceState: 'accounting_recorded',
      impliesSettlement: false,
      stateId: 'accepted_value',
    },
    {
      amountCents: record.totalCostCents,
      asset: 'usd',
      evidenceState: 'accounting_recorded',
      impliesSettlement: false,
      stateId: 'cost_basis',
    },
    {
      amountCents: record.grossMarginCents,
      asset: 'usd',
      evidenceState: 'derived',
      impliesSettlement: false,
      stateId: 'gross_margin',
    },
    settlementLine('pending_balance_adjustment'),
    settlementLine('payout_intent'),
    settlementLine('settlement_attempt'),
    settlementLine('reconciliation'),
  ]

  // Defensive invariant: a settlement-implying state must never be presented as
  // evidenced while the record disclaims settlement implication.
  if (settlementDisclaimed) {
    for (const line of lines) {
      if (
        SETTLEMENT_IMPLYING_STATES.has(line.stateId) &&
        line.evidenceState !== 'not_yet_evidenced'
      ) {
        throw new OmniGrossMarginReceiptInvariantError({
          reason: `state ${line.stateId} cannot be evidenced while the economics record disclaims settlement implication.`,
        })
      }
    }
  }

  const unevidencedStateIds = lines
    .filter(line => line.evidenceState === 'not_yet_evidenced')
    .map(line => line.stateId)

  return {
    economicsId: record.id,
    evidencedStateCount: lines.length - unevidencedStateIds.length,
    fundingMode: record.fundingMode,
    grossMarginCents: record.grossMarginCents,
    lines,
    noSettlementImplication: record.noSettlementImplication,
    publicCaveatRef: record.publicCaveatRef,
    receiptKind: 'accepted_outcome_gross_margin',
    unevidencedStateIds,
    workKind: record.workKind,
    workroomId: record.workroomId,
  }
}

/**
 * Public projection: keeps the full lifecycle and evidence labels visible (so a
 * reader can see exactly which states are unevidenced) but drops internal
 * monetary figures and internal refs.
 */
export const publicOmniGrossMarginReceiptProjection = (
  receipt: OmniGrossMarginReceipt,
) => ({
  evidencedStateCount: receipt.evidencedStateCount,
  fundingMode: receipt.fundingMode,
  lines: receipt.lines.map(line => ({
    evidenceState: line.evidenceState,
    impliesSettlement: line.impliesSettlement,
    stateId: line.stateId,
  })),
  noSettlementImplication: receipt.noSettlementImplication,
  publicCaveatRef: receipt.publicCaveatRef,
  receiptKind: receipt.receiptKind,
  unevidencedStateIds: receipt.unevidencedStateIds,
  workKind: receipt.workKind,
})
