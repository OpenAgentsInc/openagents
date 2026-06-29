import { Schema as S } from 'effect'

import { OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema } from './omni-accepted-outcome-contracts'
import {
  type OmniAcceptedOutcomeEconomicsRecord,
  OmniAcceptedOutcomeBuyerPriceAsset,
  OmniAcceptedOutcomeFundingMode,
} from './omni-accepted-outcome-economics'

/**
 * Eight-state settlement state machine for a single accepted outcome.
 *
 * The promise payments.accepted_outcome_economics.v1 is RED on three blockers.
 * Its verification text requires that ONE accepted outcome be carried through
 * eight DISTINCT evidence states, none collapsed into another:
 *
 *   authorized -> paid -> accepted -> pending_payout -> dispatched ->
 *   confirmed -> reconciled -> margin
 *
 * The gross-margin receipt (omni-gross-margin-receipt.ts) names the lifecycle of
 * an accounting-only economics record but, because that substrate disclaims
 * settlement, it deliberately leaves every settlement-implying state
 * `not_yet_evidenced`. Nothing in the codebase produced the actual per-state
 * transitions the verification calls for. That seam is the blocker
 * blocker.product_promises.settlement_state_machine_incomplete.
 *
 * This module is that state machine. It models an accepted outcome's settlement
 * lifecycle as an ordered chain of eight typed transitions, each carrying its own
 * receipt-style evidence (a state id, an ISO timestamp, an evidence ref, and the
 * monetary figure that state asserts). It is:
 *
 * - RECEIPT-FIRST: every advance appends an immutable transition entry; the
 *   machine's state IS the receipt log, not a mutable status column.
 * - IDEMPOTENT: advancing to a state already recorded is a no-op that returns the
 *   same machine, so a retried operator step never double-records.
 * - MONOTONIC and GAP-FREE: a state can only be entered after its predecessor,
 *   so `confirmed` can never appear before `dispatched`, etc.
 * - NEVER-NEGATIVE: each asserted figure is a non-negative integer cents value;
 *   margin is derived (accepted - cost) and may be negative ONLY as a derived
 *   margin figure, never as an owed/payable balance.
 * - INERT by default: the machine MOVES NO MONEY. A transition is flag-gated by
 *   `dispatchArmed`; while disarmed (the default), the `dispatched` transition
 *   records intent-only evidence and the machine refuses to mark a transition as
 *   a real money movement. It computes and records evidence; it does not send.
 *
 * Reaching `margin` produces a machine with all eight states evidenced -- the
 * exact shape a real accepted-outcome run must populate before the promise can be
 * honestly flipped green. Building this subsystem does NOT flip the promise: a
 * green flip still requires one REAL outcome run through a money-moving path,
 * which this INERT machine does not perform.
 */

export const OmniSettlementStateId = S.Literals([
  // Buyer charge authorized (price captured as an authorization, not yet paid).
  'authorized',
  // Buyer payment captured/settled (funds received by the platform).
  'paid',
  // Outcome accepted by the buyer/reviewer; accepted value recognized.
  'accepted',
  // A payout balance is pending to the contributor(s); not yet dispatched.
  'pending_payout',
  // A payout has been dispatched (or, while INERT, intent-only recorded).
  'dispatched',
  // The dispatched payout has been confirmed by the payout rail.
  'confirmed',
  // Internal ledger reconciled against the confirmed external movement.
  'reconciled',
  // Gross margin recognized for the outcome (revenue - cost).
  'margin',
])
export type OmniSettlementStateId = typeof OmniSettlementStateId.Type

// The canonical, ordered lifecycle. The index in this array IS the state's
// position; a transition to state N is only valid when states 0..N-1 are already
// recorded. Exported so callers and tests share one source of truth.
export const OMNI_SETTLEMENT_STATE_ORDER: ReadonlyArray<OmniSettlementStateId> =
  [
    'authorized',
    'paid',
    'accepted',
    'pending_payout',
    'dispatched',
    'confirmed',
    'reconciled',
    'margin',
  ]

// States whose evidence asserts a real outbound money movement. While the
// machine is INERT (dispatchArmed = false) these may only be recorded as
// intent-only, never as a settled external movement.
const MONEY_MOVEMENT_STATES: ReadonlySet<OmniSettlementStateId> = new Set([
  'dispatched',
  'confirmed',
])

export const OmniSettlementEvidenceKind = S.Literals([
  // A figure recorded directly from the buyer/accounting side.
  'accounting_recorded',
  // A figure mechanically derived from prior recorded figures (e.g. margin).
  'derived',
  // Intent-only: recorded while the machine is INERT; asserts NO money moved.
  'intent_only',
  // A real external money movement confirmed by a payout/payment rail.
  'externally_confirmed',
])
export type OmniSettlementEvidenceKind = typeof OmniSettlementEvidenceKind.Type

export const OmniSettlementTransition = S.Struct({
  // Non-negative cents the state asserts, EXCEPT `margin` which may be negative
  // as a derived figure. Null when the state asserts no monetary figure.
  amountCents: S.NullOr(S.Number),
  asset: OmniAcceptedOutcomeBuyerPriceAsset,
  // A public-safe ref describing the evidence backing this transition. Never a
  // wallet, invoice, preimage, or raw payment string.
  evidenceRef: S.String,
  evidenceKind: OmniSettlementEvidenceKind,
  // True iff this transition asserts a real outbound money movement. ALWAYS
  // false while the machine is INERT.
  movedMoney: S.Boolean,
  recordedAt: S.String,
  stateId: OmniSettlementStateId,
})
export type OmniSettlementTransition = typeof OmniSettlementTransition.Type

export const OmniAcceptedOutcomeSettlementMachine = S.Struct({
  // Whether real money movement is armed. INERT (false) by default.
  dispatchArmed: S.Boolean,
  economicsId: S.String,
  fundingMode: OmniAcceptedOutcomeFundingMode,
  machineKind: S.Literal('accepted_outcome_settlement_state_machine'),
  // True once all eight states are recorded. Even when complete the machine has
  // moved no money unless dispatchArmed was true for the money-movement states.
  noSettlementImplication: S.Boolean,
  publicCaveatRef: S.String,
  // The current (latest) recorded state, or null before `authorized`.
  state: S.NullOr(OmniSettlementStateId),
  // The immutable, append-only receipt log. This IS the machine's state.
  transitions: S.Array(OmniSettlementTransition),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
  workroomId: S.String,
})
export type OmniAcceptedOutcomeSettlementMachine =
  typeof OmniAcceptedOutcomeSettlementMachine.Type

export class OmniSettlementStateMachineValidationError extends S.TaggedErrorClass<OmniSettlementStateMachineValidationError>()(
  'OmniSettlementStateMachineValidationError',
  { reason: S.String },
) {}

export class OmniSettlementStateMachineTransitionError extends S.TaggedErrorClass<OmniSettlementStateMachineTransitionError>()(
  'OmniSettlementStateMachineTransitionError',
  { fromState: S.NullOr(OmniSettlementStateId), reason: S.String, toState: OmniSettlementStateId },
) {}

const stateIndex = (stateId: OmniSettlementStateId): number =>
  OMNI_SETTLEMENT_STATE_ORDER.indexOf(stateId)

const SAFE_EVIDENCE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PROHIBITED_EVIDENCE_PATTERN =
  /\b(access_token|refresh_token|private_key|wallet_secret|payment_preimage|payment_secret|webhook_secret|mnemonic|xprv)\b|lnbc[0-9a-z]*|lntb[0-9a-z]*|lnbcrt[0-9a-z]*|lno1[0-9a-z]*|@/i

const assertSafeEvidenceRef = (evidenceRef: string): void => {
  if (
    !SAFE_EVIDENCE_REF_PATTERN.test(evidenceRef) ||
    PROHIBITED_EVIDENCE_PATTERN.test(evidenceRef)
  ) {
    throw new OmniSettlementStateMachineValidationError({
      reason:
        'evidenceRef must be a public-safe ref without wallet, invoice, preimage, token, or raw payment material.',
    })
  }
}

/**
 * The figure and evidence shape each lifecycle state asserts, derived from the
 * economics record. The buyer-side states reflect the recorded buyer charge; the
 * accepted/cost/margin states reflect the recorded accounting figures; the payout
 * states reflect the distributable margin (max(0, gross margin)).
 */
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

type StateFigure = Readonly<{
  amountCents: number | null
  asset: OmniAcceptedOutcomeBuyerPriceAsset
  derived: boolean
}>

const figureForState = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  stateId: OmniSettlementStateId,
): StateFigure => {
  const buyerCents = buyerAmountCents(record)
  const distributable = Math.max(0, record.grossMarginCents)
  switch (stateId) {
    case 'authorized':
    case 'paid':
      return {
        amountCents: buyerCents,
        asset: record.buyerPriceAsset,
        derived: false,
      }
    case 'accepted':
      return {
        amountCents: record.acceptedValueCents,
        asset: 'usd',
        derived: false,
      }
    case 'pending_payout':
    case 'dispatched':
    case 'confirmed':
    case 'reconciled':
      return { amountCents: distributable, asset: 'usd', derived: true }
    case 'margin':
      // Margin is the ONLY state allowed to be negative: a loss is an honest
      // derived figure, never an owed balance.
      return { amountCents: record.grossMarginCents, asset: 'usd', derived: true }
  }
}

export type OmniSettlementAdvanceInput = Readonly<{
  evidenceRef: string
  recordedAt: string
}>

/**
 * Create an empty (un-started) settlement machine for one economics record.
 *
 * Pure and deterministic. The machine starts with no transitions and state =
 * null; the first advance must be to `authorized`. INERT by default
 * (dispatchArmed = false): until armed, no transition can claim a money
 * movement. Arming is a separate, explicit operator concern; this subsystem
 * remains a recorder of evidence regardless.
 */
export const createOmniAcceptedOutcomeSettlementMachine = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  options: Readonly<{ dispatchArmed?: boolean }> = {},
): OmniAcceptedOutcomeSettlementMachine => ({
  dispatchArmed: options.dispatchArmed ?? false,
  economicsId: record.id,
  fundingMode: record.fundingMode,
  machineKind: 'accepted_outcome_settlement_state_machine',
  noSettlementImplication: true,
  publicCaveatRef: record.publicCaveatRef,
  state: null,
  transitions: [],
  workKind: record.workKind,
  workroomId: record.workroomId,
})

/**
 * Advance the machine to `toState`, appending a receipt-bearing transition.
 *
 * Honesty rules enforced by construction:
 * - IDEMPOTENT: if `toState` is already the latest recorded state, the same
 *   machine is returned unchanged (a retried operator step never double-records).
 * - MONOTONIC/GAP-FREE: `toState` must be exactly the next state after the
 *   current one; skipping or going backwards fails with a transition error.
 * - NEVER-NEGATIVE: every non-margin figure is a non-negative integer; only the
 *   derived `margin` figure may be negative.
 * - INERT: a money-movement state (dispatched/confirmed) records `intent_only`
 *   evidence with movedMoney = false while disarmed. movedMoney can only ever be
 *   true when dispatchArmed is true AND the state is a money-movement state.
 *
 * Pure: returns a new machine; never mutates the input.
 */
export const advanceOmniAcceptedOutcomeSettlementMachine = (
  machine: OmniAcceptedOutcomeSettlementMachine,
  record: OmniAcceptedOutcomeEconomicsRecord,
  toState: OmniSettlementStateId,
  input: OmniSettlementAdvanceInput,
): OmniAcceptedOutcomeSettlementMachine => {
  if (machine.economicsId !== record.id) {
    throw new OmniSettlementStateMachineValidationError({
      reason: `machine economicsId ${machine.economicsId} must match record id ${record.id}.`,
    })
  }

  assertSafeEvidenceRef(input.evidenceRef)

  // Idempotent: re-recording the current latest state is a no-op.
  if (machine.state === toState) {
    return machine
  }

  const currentIndex = machine.state === null ? -1 : stateIndex(machine.state)
  const nextIndex = stateIndex(toState)

  if (nextIndex !== currentIndex + 1) {
    throw new OmniSettlementStateMachineTransitionError({
      fromState: machine.state,
      reason:
        nextIndex <= currentIndex
          ? `cannot move backward or re-enter: ${String(machine.state)} -> ${toState}.`
          : `cannot skip states: expected ${OMNI_SETTLEMENT_STATE_ORDER[currentIndex + 1]}, got ${toState}.`,
      toState,
    })
  }

  const figure = figureForState(record, toState)

  if (
    figure.amountCents !== null &&
    toState !== 'margin' &&
    (!Number.isInteger(figure.amountCents) || figure.amountCents < 0)
  ) {
    throw new OmniSettlementStateMachineValidationError({
      reason: `state ${toState} figure must be a non-negative integer; got ${figure.amountCents}.`,
    })
  }

  if (
    toState === 'margin' &&
    figure.amountCents !== null &&
    !Number.isInteger(figure.amountCents)
  ) {
    throw new OmniSettlementStateMachineValidationError({
      reason: `margin figure must be an integer; got ${figure.amountCents}.`,
    })
  }

  const isMoneyMovementState = MONEY_MOVEMENT_STATES.has(toState)
  const movedMoney = isMoneyMovementState && machine.dispatchArmed

  let evidenceKind: OmniSettlementEvidenceKind
  if (isMoneyMovementState) {
    evidenceKind = movedMoney ? 'externally_confirmed' : 'intent_only'
  } else if (figure.derived) {
    evidenceKind = 'derived'
  } else {
    evidenceKind = 'accounting_recorded'
  }

  // Defensive INERT invariant: while the machine is disarmed, no transition may
  // claim a real money movement, and the noSettlementImplication flag must hold.
  if (!machine.dispatchArmed && movedMoney) {
    throw new OmniSettlementStateMachineValidationError({
      reason: 'an INERT settlement machine cannot record a money movement.',
    })
  }

  const transition: OmniSettlementTransition = {
    amountCents: figure.amountCents,
    asset: figure.asset,
    evidenceKind,
    evidenceRef: input.evidenceRef,
    movedMoney,
    recordedAt: input.recordedAt,
    stateId: toState,
  }

  return {
    ...machine,
    // The machine still implies no settlement unless it has actually moved money.
    noSettlementImplication:
      machine.noSettlementImplication && !movedMoney,
    state: toState,
    transitions: [...machine.transitions, transition],
  }
}

/**
 * Whether the machine has recorded all eight lifecycle states. A complete
 * machine is the shape a real accepted-outcome run must produce for the promise
 * to be honestly evaluated -- but completeness alone is NOT a green flip and is
 * NOT proof money moved (see movedMoney on the dispatched/confirmed transitions).
 */
export const isOmniSettlementMachineComplete = (
  machine: OmniAcceptedOutcomeSettlementMachine,
): boolean =>
  machine.transitions.length === OMNI_SETTLEMENT_STATE_ORDER.length &&
  machine.state === 'margin'

/**
 * Public projection: keeps the full ordered lifecycle and honest evidence labels
 * visible (so a reader can see exactly which states were money-moving vs
 * intent-only) while dropping internal monetary figures. Evidence refs are
 * retained because `assertSafeEvidenceRef` admits only public-safe refs and the
 * promise verification requires one distinct receipt/evidence anchor per state.
 */
export const publicOmniAcceptedOutcomeSettlementMachineProjection = (
  machine: OmniAcceptedOutcomeSettlementMachine,
) => ({
  complete: isOmniSettlementMachineComplete(machine),
  dispatchArmed: machine.dispatchArmed,
  fundingMode: machine.fundingMode,
  machineKind: machine.machineKind,
  noSettlementImplication: machine.noSettlementImplication,
  publicCaveatRef: machine.publicCaveatRef,
  state: machine.state,
  transitions: machine.transitions.map(transition => ({
    evidenceKind: transition.evidenceKind,
    evidenceRef: transition.evidenceRef,
    movedMoney: transition.movedMoney,
    stateId: transition.stateId,
  })),
  workKind: machine.workKind,
  workroomId: machine.workroomId,
})
