/**
 * The History surface view-model (issue #9320 scope item 4).
 *
 * Actionability-first ordering: sessions needing a claim, refund, or exit
 * float above chronology, and actionability is computed from chain facts and
 * tip height supplied by the host's Bitcoin observation adapter — never from
 * the stored status alone (the stored status can be stale or, per the
 * per-signer model, a claim someone made rather than a fact).
 *
 * Rendering stays in the app shell; this module emits typed
 * `@openagentsinc/swap-i18n` message keys so removed or renamed copy fails
 * typecheck here, and the terminal outcome always carries its evidence-rung
 * label — an outcome is displayed at the rung the evidence proves, never
 * upgraded by prose.
 */
import type { MessageKey } from "@openagentsinc/swap-i18n";

import type { EvidenceRung, StoredSwapSession, TerminalOutcome } from "./model.js";
import { resumeReasonOf } from "./resume.js";

/**
 * Chain facts for one session, from the host's observation adapter. `null`
 * facts (adapter unavailable) degrade honestly: nothing is claimed to be
 * claimable, and ordering falls back to resume/chronology.
 */
export interface SessionChainFacts {
  readonly tipHeight: number | null;
  /** A destination output attributable to this user is funded and claimable. */
  readonly destinationClaimable: boolean;
  /** The refund timeout has passed and the refund path is spendable. */
  readonly refundUnlocked: boolean;
}

export type HistoryAction = "claim" | "refund" | "exit" | "resume";

const ACTION_LABEL_KEYS: Readonly<Record<HistoryAction, MessageKey>> = {
  claim: "swap.history.action.claim",
  refund: "swap.history.action.refund",
  exit: "swap.history.action.exit",
  resume: "swap.history.action.resume",
};

const OUTCOME_LABEL_KEYS: Readonly<Record<TerminalOutcome, MessageKey>> = {
  completed: "swap.history.outcome.completed",
  cancelled: "swap.history.outcome.cancelled",
  expired: "swap.history.outcome.expired",
  failed: "swap.history.outcome.failed",
  refunded: "swap.history.outcome.refunded",
  disputed: "swap.history.outcome.disputed",
  unresolved: "swap.history.outcome.unresolved",
};

const RUNG_LABEL_KEYS: Readonly<Record<EvidenceRung, MessageKey>> = {
  pledged: "swap.history.rung.pledged",
  reserved: "swap.history.rung.reserved",
  measured: "swap.history.rung.measured",
  verified: "swap.history.rung.verified",
  paid: "swap.history.rung.paid",
  settled: "swap.history.rung.settled",
};

export interface HistoryRow {
  readonly sessionId: string;
  readonly createdAt: number;
  readonly swapType: StoredSwapSession["swapType"];
  readonly state: string;
  /** The single per-row action, or null when only view/delete apply. */
  readonly action: HistoryAction | null;
  readonly actionLabelKey: MessageKey | null;
  readonly outcome: TerminalOutcome | null;
  readonly outcomeLabelKey: MessageKey | null;
  /**
   * Evidence-rung label for the outcome. A terminal outcome without a
   * verified rung renders "claimed only" — the rung is never inferred up.
   */
  readonly rungLabelKey: MessageKey;
  /** Per-row delete requires confirmation with this copy. */
  readonly deleteConfirmKey: MessageKey;
}

const actionOf = (
  session: StoredSwapSession,
  facts: SessionChainFacts | null,
): HistoryAction | null => {
  if (facts?.destinationClaimable === true) return "claim";
  if (facts?.refundUnlocked === true) return "refund";
  // Crash window: an external effect was requested but never resulted —
  // the unilateral exit path must be finished before anything else.
  if (session.effectLedger.some((entry) => entry.result === null)) return "exit";
  if (resumeReasonOf(session) !== null) return "resume";
  return null;
};

const isActionable = (action: HistoryAction | null): boolean =>
  action === "claim" || action === "refund" || action === "exit";

export const historyRowOf = (
  session: StoredSwapSession,
  facts: SessionChainFacts | null,
): HistoryRow => {
  const action = actionOf(session, facts);
  const outcome = session.projection.outcome;
  const rung = session.projection.rung;
  return {
    sessionId: session.sessionId,
    createdAt: session.createdAt,
    swapType: session.swapType,
    state: session.projection.state,
    action,
    actionLabelKey: action === null ? null : ACTION_LABEL_KEYS[action],
    outcome,
    outcomeLabelKey: outcome === null ? null : OUTCOME_LABEL_KEYS[outcome],
    rungLabelKey: rung === null ? "swap.history.rung.claimed_only" : RUNG_LABEL_KEYS[rung],
    deleteConfirmKey: "swap.history.delete_confirm",
  };
};

/**
 * All rows, actionable-first then chronology descending. `factsOf` returns
 * the host adapter's current chain facts for a session, or null when the
 * adapter has no view (explorer unreachable, no wallet).
 */
export const historyRows = (
  sessions: ReadonlyArray<StoredSwapSession>,
  factsOf: (sessionId: string) => SessionChainFacts | null,
): ReadonlyArray<HistoryRow> => {
  const rows = sessions.map((session) => historyRowOf(session, factsOf(session.sessionId)));
  return [...rows].sort((a, b) => {
    const actionableDelta = Number(isActionable(b.action)) - Number(isActionable(a.action));
    if (actionableDelta !== 0) return actionableDelta;
    return b.createdAt - a.createdAt;
  });
};

/** Key for the empty-history state. */
export const HISTORY_EMPTY_KEY: MessageKey = "swap.history.empty";

/** Key for the reload/navigation guard prompt (`resume.ts` reloadGuard). */
export const RELOAD_GUARD_KEY: MessageKey = "swap.history.reload_guard";
