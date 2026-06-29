// Settlement seam for a verified run — INERT / SPEC-ONLY / OWNER-GATED.
//
// Issue #6188, spec §E.1 / §F.6: "one trace-decomposed split over Lightning
// behind the 8-state INERT machine (pay-per-run / author rev-share). No money
// until armed."
//
// This module is that seam — and ONLY the seam. It defines the typed 8-state
// settlement lifecycle for a run-receipt → author rev-share split and a pure,
// deterministic machine that records INTENT-ONLY evidence. It is:
//
//   - DEFAULT-OFF: `armed` is false by default. There is NO code path here that
//     moves sats, opens a wallet, builds an invoice, or contacts a payout rail.
//   - OWNER-GATED: arming requires an explicit owner arming token AND a runtime
//     capability that this module does not provide. `arm()` without both fails
//     closed; even when "armed", the dispatch step still records intent_only,
//     because the actual money-movement executor is intentionally absent.
//   - HONEST: a money-movement state can only ever be labeled `intent_only`
//     here; `movedSats` is hard-wired false. A test proves no sats move and that
//     arming without the owner gate errors.
//
// The 8 states mirror the platform's accepted-outcome settlement lifecycle so a
// future real implementation can map onto the same vocabulary, but this is a
// distinct seam for the RUN/skill rev-share economy, not the accepted-outcome
// path. It moves nothing.

import { Schema as S } from "effect";

export const RUN_SETTLEMENT_SCHEMA_VERSION = "openagents.qa_runner.run_settlement.v1";

/** The canonical 8-state run-settlement lifecycle (authorized..margin). */
export const RunSettlementStateId = S.Literals([
  // The run-receipt's split is authorized (intent captured; nothing charged).
  "authorized",
  // The buyer side is recorded as paid (pay-per-run); intent-only while INERT.
  "paid",
  // The run is accepted as verified work (the receipt's verification class).
  "accepted",
  // An author rev-share payout is pending (computed split; not dispatched).
  "pending_payout",
  // The payout is dispatched — INERT: intent-only, NO sats move.
  "dispatched",
  // The dispatched payout is confirmed by a rail — INERT: never reached live.
  "confirmed",
  // Internal ledger reconciled against the (absent) movement.
  "reconciled",
  // Margin recognized for the run economy.
  "margin",
]);
export type RunSettlementStateId = typeof RunSettlementStateId.Type;

export const RUN_SETTLEMENT_STATE_ORDER: ReadonlyArray<RunSettlementStateId> = [
  "authorized",
  "paid",
  "accepted",
  "pending_payout",
  "dispatched",
  "confirmed",
  "reconciled",
  "margin",
];

/** States that would assert a real outbound money movement in a live system. */
const MONEY_MOVEMENT_STATES: ReadonlySet<RunSettlementStateId> = new Set([
  "dispatched",
  "confirmed",
]);

export const RunSettlementEvidenceKind = S.Literals([
  "intent_only", // recorded while INERT; asserts NO sats moved
  "derived", // mechanically derived from prior figures
  "accounting_recorded", // a non-money-movement figure recorded
]);
export type RunSettlementEvidenceKind = typeof RunSettlementEvidenceKind.Type;

export const RunSettlementTransition = S.Struct({
  stateId: RunSettlementStateId,
  evidenceKind: RunSettlementEvidenceKind,
  /** A public-safe ref backing this transition. Never a wallet/invoice/preimage. */
  evidenceRef: S.String,
  /** Non-negative integer sats this state asserts; null when no figure. */
  sats: S.NullOr(S.Number),
  /** ALWAYS false in this INERT seam — no transition ever moves sats. */
  movedSats: S.Literal(false),
  recordedAt: S.String,
});
export type RunSettlementTransition = typeof RunSettlementTransition.Type;

/**
 * The run-settlement split: the receipt being settled and the bounded set of
 * author/platform share basis points. PURE DATA — defining a split moves nothing.
 */
export const RunSettlementSplit = S.Struct({
  /** The dereferenceable run receipt ref this split settles. */
  receiptRef: S.String,
  /** The author's rev-share in basis points (0..10000). */
  authorBps: S.Number,
  /** The platform's share in basis points (0..10000); authorBps + platformBps = 10000. */
  platformBps: S.Number,
});
export type RunSettlementSplit = typeof RunSettlementSplit.Type;

export const RunSettlementMachine = S.Struct({
  schemaVersion: S.Literal(RUN_SETTLEMENT_SCHEMA_VERSION),
  /** INERT by default. Even when armed, this seam moves no sats (no executor). */
  armed: S.Boolean,
  /** True while no transition has moved sats (always true in this seam). */
  noSettlementImplication: S.Boolean,
  split: RunSettlementSplit,
  state: S.NullOr(RunSettlementStateId),
  transitions: S.Array(RunSettlementTransition),
});
export type RunSettlementMachine = typeof RunSettlementMachine.Type;

export class RunSettlementError extends Error {
  constructor(reason: string) {
    super(`run_settlement_error: ${reason}`);
    this.name = "RunSettlementError";
  }
}

/** The owner arming token expected to even CONSIDER arming. Absent by default. */
export const RUN_SETTLEMENT_OWNER_ARM_TOKEN = "owner.arm.run_settlement.v1";

const stateIndex = (stateId: RunSettlementStateId): number =>
  RUN_SETTLEMENT_STATE_ORDER.indexOf(stateId);

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/;
const PROHIBITED_REF_PATTERN =
  /\b(access_token|refresh_token|private_key|wallet_secret|payment_preimage|payment_secret|mnemonic|xprv)\b|lnbc[0-9a-z]*|lntb[0-9a-z]*|lno1[0-9a-z]*|@/i;

const assertSafeRef = (ref: string): void => {
  if (!SAFE_REF_PATTERN.test(ref) || PROHIBITED_REF_PATTERN.test(ref)) {
    throw new RunSettlementError(
      "evidenceRef must be a public-safe ref (no wallet, invoice, preimage, token, or raw payment material).",
    );
  }
};

/**
 * Create the INERT run-settlement machine for a split. DEFAULT-OFF: `armed` is
 * false; nothing here moves sats regardless. Validates the split sums to 100%.
 */
export function createRunSettlementMachine(split: RunSettlementSplit): RunSettlementMachine {
  if (
    !Number.isInteger(split.authorBps) ||
    !Number.isInteger(split.platformBps) ||
    split.authorBps < 0 ||
    split.platformBps < 0
  ) {
    throw new RunSettlementError("split basis points must be non-negative integers.");
  }
  if (split.authorBps + split.platformBps !== 10000) {
    throw new RunSettlementError("authorBps + platformBps must equal 10000 (100%).");
  }
  assertSafeRef(split.receiptRef);
  return {
    schemaVersion: RUN_SETTLEMENT_SCHEMA_VERSION,
    armed: false,
    noSettlementImplication: true,
    split,
    state: null,
    transitions: [],
  };
}

/**
 * Attempt to arm the machine. OWNER-GATED and INTENTIONALLY INCAPABLE: arming
 * requires BOTH the owner arming token AND a live payout executor capability.
 * This seam never provides the executor, so arming ALWAYS fails closed — there
 * is no path in this module that produces an armed, money-moving machine.
 */
export function arm(
  _machine: RunSettlementMachine,
  options: Readonly<{ ownerArmToken?: string; payoutExecutor?: unknown }> = {},
): never {
  if (options.ownerArmToken !== RUN_SETTLEMENT_OWNER_ARM_TOKEN) {
    throw new RunSettlementError(
      "arming requires the owner arming token; settlement stays INERT / OWNER-GATED.",
    );
  }
  // Even WITH the owner token, this seam has no payout executor. Arming is a
  // spec-only stub: it cannot move sats and refuses to pretend it can.
  throw new RunSettlementError(
    "run settlement is SPEC-ONLY: no payout executor is wired. Arming is owner-gated and not implemented; no sats can move from this seam.",
  );
}

export type RunSettlementAdvanceInput = Readonly<{
  evidenceRef: string;
  recordedAt: string;
  /** The figure (sats) this state asserts; null when none. */
  sats?: number | null;
}>;

/**
 * Advance the machine one state, appending an INTENT-ONLY transition. MONOTONIC,
 * GAP-FREE, idempotent on the current state. A money-movement state is ALWAYS
 * recorded `intent_only` with `movedSats: false`; this seam can never move sats.
 */
export function advanceRunSettlement(
  machine: RunSettlementMachine,
  toState: RunSettlementStateId,
  input: RunSettlementAdvanceInput,
): RunSettlementMachine {
  assertSafeRef(input.evidenceRef);

  if (machine.state === toState) return machine;

  const currentIndex = machine.state === null ? -1 : stateIndex(machine.state);
  const nextIndex = stateIndex(toState);
  if (nextIndex !== currentIndex + 1) {
    throw new RunSettlementError(
      nextIndex <= currentIndex
        ? `cannot move backward or re-enter: ${String(machine.state)} -> ${toState}.`
        : `cannot skip states: expected ${RUN_SETTLEMENT_STATE_ORDER[currentIndex + 1]}, got ${toState}.`,
    );
  }

  const sats = input.sats ?? null;
  if (sats !== null && (!Number.isInteger(sats) || sats < 0)) {
    throw new RunSettlementError(`state ${toState} sats must be a non-negative integer; got ${sats}.`);
  }

  const isMoneyMovementState = MONEY_MOVEMENT_STATES.has(toState);
  // INERT invariant: NEVER label a transition as moving sats.
  const evidenceKind: RunSettlementEvidenceKind = isMoneyMovementState
    ? "intent_only"
    : toState === "margin" || toState === "pending_payout" || toState === "reconciled"
      ? "derived"
      : "accounting_recorded";

  const transition: RunSettlementTransition = {
    stateId: toState,
    evidenceKind,
    evidenceRef: input.evidenceRef,
    sats,
    movedSats: false,
    recordedAt: input.recordedAt,
  };

  return {
    ...machine,
    noSettlementImplication: true,
    state: toState,
    transitions: [...machine.transitions, transition],
  };
}

/** Whether all eight lifecycle states are recorded (still: no sats moved). */
export function isRunSettlementComplete(machine: RunSettlementMachine): boolean {
  return (
    machine.transitions.length === RUN_SETTLEMENT_STATE_ORDER.length &&
    machine.state === "margin"
  );
}

/** True iff the machine has moved any sats. In this seam, ALWAYS false. */
export function runSettlementMovedSats(machine: RunSettlementMachine): boolean {
  return machine.transitions.some(transition => transition.movedSats);
}

/** Public projection: ordered lifecycle + honest labels; no figures/refs. */
export function publicRunSettlementProjection(machine: RunSettlementMachine) {
  return {
    schemaVersion: machine.schemaVersion,
    armed: machine.armed,
    complete: isRunSettlementComplete(machine),
    movedSats: runSettlementMovedSats(machine),
    noSettlementImplication: machine.noSettlementImplication,
    state: machine.state,
    transitions: machine.transitions.map(transition => ({
      stateId: transition.stateId,
      evidenceKind: transition.evidenceKind,
      movedSats: transition.movedSats,
    })),
  };
}
