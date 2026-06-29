import { Schema as S } from 'effect'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  advanceOmniAcceptedOutcomeSettlementMachine,
  createOmniAcceptedOutcomeSettlementMachine,
  isOmniSettlementMachineComplete,
  OMNI_SETTLEMENT_STATE_ORDER,
  OmniAcceptedOutcomeSettlementMachine,
  publicOmniAcceptedOutcomeSettlementMachineProjection,
  type OmniSettlementStateId,
} from './omni-accepted-outcome-settlement-state-machine'
import {
  buildOmniContributorAccrualBundle,
  OmniContributorAccrualBundle,
  publicOmniContributorAccrualBundleProjection,
} from './omni-contributor-accrual-bundle'
import { resolveOmniContributorPartiesFromRecord } from './omni-contributor-party-sourcing'

/**
 * Settlement bundle for a single accepted outcome.
 *
 * The promise payments.accepted_outcome_economics.v1 verification text asks for
 * "one accepted outcome with separate authorized, paid, accepted, pending payout,
 * dispatched, confirmed, reconciled, and margin evidence." Those nine words map
 * onto the three subsystems behind the promise's three RED blockers:
 *
 * - the eight ordered settlement states (authorized..margin) come from the
 *   settlement STATE MACHINE
 *   (blocker.product_promises.settlement_state_machine_incomplete);
 * - the per-contributor attribution of the outcome's margin comes from the
 *   contributor accrual LEDGER, bound to the gross-margin RECEIPT via the
 *   contributor accrual bundle
 *   (blocker.product_promises.contributor_ledger_missing); and
 * - the recorded gross margin (revenue - cost) comes from the gross-margin
 *   RECEIPT (blocker.product_promises.gross_margin_receipts_missing).
 *
 * This module composes all three into ONE dereferenceable view keyed by
 * accepted-outcome id, and enforces a cross-view reconciliation so the three
 * cannot disagree about the SAME outcome's margin. It is pure, deterministic, and
 * INERT: it builds the full eight-state machine in its disarmed (intent-only)
 * form by default, so producing a "complete" bundle MOVES NO MONEY and is NOT a
 * green flip. A green flip still requires one real outcome carried through a
 * money-moving settlement path -- which this bundle, by construction, does not
 * perform.
 */

export class OmniAcceptedOutcomeSettlementBundleInvariantError extends S.TaggedErrorClass<OmniAcceptedOutcomeSettlementBundleInvariantError>()(
  'OmniAcceptedOutcomeSettlementBundleInvariantError',
  { reason: S.String },
) {}

export const OmniAcceptedOutcomeSettlementBundle = S.Struct({
  bundleKind: S.Literal('accepted_outcome_settlement_bundle'),
  // The contributor accrual bundle (ledger + gross-margin receipt, reconciled).
  contributorAccrualBundle: OmniContributorAccrualBundle,
  economicsId: S.String,
  // True once the machine has recorded all eight states. Completeness alone is
  // NOT a green flip and NOT proof money moved.
  settlementComplete: S.Boolean,
  // The eight-state settlement machine.
  settlementMachine: OmniAcceptedOutcomeSettlementMachine,
})
export type OmniAcceptedOutcomeSettlementBundle =
  typeof OmniAcceptedOutcomeSettlementBundle.Type

/**
 * Build a full settlement bundle from one economics record.
 *
 * Deterministic and pure. By default the settlement machine is built INERT
 * (dispatchArmed = false) and advanced through all eight states with intent-only
 * evidence for the money-movement states, so the resulting bundle records the
 * complete lifecycle shape without moving money. Pass dispatchArmed only from an
 * explicitly armed operator path; this subsystem never arms itself.
 *
 * Honesty rules enforced by construction (in addition to those each subsystem
 * already enforces):
 * - The machine and the contributor accrual bundle must reference the SAME
 *   accepted-outcome id.
 * - The machine's `margin` transition figure must equal the receipt's recorded
 *   gross margin -- the two views cannot carry different margins for one outcome.
 * - The machine's `pending_payout` figure must equal the ledger's distributable
 *   margin -- the payout the machine records must be the pool the ledger splits.
 * - The bundle is INERT unless explicitly armed: when disarmed, no transition
 *   may have moved money and both views must disclaim settlement implication.
 */
export const buildOmniAcceptedOutcomeSettlementBundle = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  options: Readonly<{
    dispatchArmed?: boolean
    evidenceRefFor?: (stateId: OmniSettlementStateId) => string
    recordedAtFor?: (stateId: OmniSettlementStateId) => string
  }> = {},
): OmniAcceptedOutcomeSettlementBundle => {
  const evidenceRefFor =
    options.evidenceRefFor ?? (stateId => `evidence.settlement.${stateId}`)
  const recordedAtFor =
    options.recordedAtFor ?? (() => record.updatedAt)

  const settlementMachine = OMNI_SETTLEMENT_STATE_ORDER.reduce(
    (machine, stateId) =>
      advanceOmniAcceptedOutcomeSettlementMachine(machine, record, stateId, {
        evidenceRef: evidenceRefFor(stateId),
        recordedAt: recordedAtFor(stateId),
      }),
    createOmniAcceptedOutcomeSettlementMachine(record, {
      dispatchArmed: options.dispatchArmed ?? false,
    }),
  )

  const contributorAccrualBundle = buildOmniContributorAccrualBundle(
    record,
    resolveOmniContributorPartiesFromRecord(record),
  )

  if (settlementMachine.economicsId !== contributorAccrualBundle.economicsId) {
    throw new OmniAcceptedOutcomeSettlementBundleInvariantError({
      reason: `machine economicsId ${settlementMachine.economicsId} must match accrual bundle economicsId ${contributorAccrualBundle.economicsId}.`,
    })
  }

  const marginTransition = settlementMachine.transitions.find(
    transition => transition.stateId === 'margin',
  )
  if (
    marginTransition === undefined ||
    marginTransition.amountCents !==
      contributorAccrualBundle.reconciledGrossMarginCents
  ) {
    throw new OmniAcceptedOutcomeSettlementBundleInvariantError({
      reason: `machine margin ${String(marginTransition?.amountCents)} must equal reconciled gross margin ${contributorAccrualBundle.reconciledGrossMarginCents}.`,
    })
  }

  const pendingPayoutTransition = settlementMachine.transitions.find(
    transition => transition.stateId === 'pending_payout',
  )
  const distributable = Math.max(
    0,
    contributorAccrualBundle.reconciledGrossMarginCents,
  )
  if (
    pendingPayoutTransition === undefined ||
    pendingPayoutTransition.amountCents !== distributable
  ) {
    throw new OmniAcceptedOutcomeSettlementBundleInvariantError({
      reason: `machine pending_payout ${String(pendingPayoutTransition?.amountCents)} must equal distributable margin ${distributable}.`,
    })
  }

  // INERT discipline: when disarmed, nothing may have moved money and both views
  // must still disclaim settlement implication.
  if (!settlementMachine.dispatchArmed) {
    if (settlementMachine.transitions.some(transition => transition.movedMoney)) {
      throw new OmniAcceptedOutcomeSettlementBundleInvariantError({
        reason: 'a disarmed settlement bundle cannot contain a money movement.',
      })
    }
    if (
      !settlementMachine.noSettlementImplication ||
      !contributorAccrualBundle.grossMarginReceipt.noSettlementImplication ||
      !contributorAccrualBundle.contributorAccrualLedger.noSettlementImplication
    ) {
      throw new OmniAcceptedOutcomeSettlementBundleInvariantError({
        reason: 'a disarmed settlement bundle must disclaim settlement across all views.',
      })
    }
  }

  return {
    bundleKind: 'accepted_outcome_settlement_bundle',
    contributorAccrualBundle,
    economicsId: settlementMachine.economicsId,
    settlementComplete: isOmniSettlementMachineComplete(settlementMachine),
    settlementMachine,
  }
}

/**
 * Public projection: composes the underlying public projections, which keep the
 * lifecycle and honest evidence labels visible while dropping internal monetary
 * figures and refs.
 */
export const publicOmniAcceptedOutcomeSettlementBundleProjection = (
  bundle: OmniAcceptedOutcomeSettlementBundle,
) => ({
  bundleKind: bundle.bundleKind,
  contributorAccrualBundle: publicOmniContributorAccrualBundleProjection(
    bundle.contributorAccrualBundle,
  ),
  settlementComplete: bundle.settlementComplete,
  settlementMachine: publicOmniAcceptedOutcomeSettlementMachineProjection(
    bundle.settlementMachine,
  ),
})
