/**
 * IDR-01 recovery state machine — types and transitions only.
 *
 * This module defines the states and transitions the local recovery flow will
 * use. It does NOT implement the fail-closed open and create operations; those
 * are IDR-02. It is pure: no platform API, no secret store, no filesystem.
 *
 * FAIL-CLOSED BY CONSTRUCTION. There is no event and no transition that creates
 * a root secret. A discovery or open path can reach `NoCandidateFound`, which is
 * terminal. It can never reach a custody-import state after a no-candidate
 * result, so an open path can never silently create a root. Creation stays a
 * separate explicit operation (IDR-02).
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Data } from "effect";
import type { IdentityRef } from "../contract/index.ts";

/** The class of a reconciliation conflict (audit Phase 1/2 mismatch classes). */
export type RecoveryConflictClass =
  | "nostr_pubkey_mismatch"
  | "spark_fingerprint_mismatch"
  | "profile_mismatch"
  | "multiple_candidates";

/** The class of a fail-closed blocker. None of these creates a root. */
export type RecoveryBlocker =
  | "custody_unavailable"
  | "link_check_failed"
  | "permission_denied"
  | "owner_selection_required";

/**
 * A recovery state. The flow starts `Idle`, discovers existing candidates, and
 * either reconciles an existing identity and imports it to platform custody, or
 * reports no candidate or a blocker. No state represents a newly created root.
 */
export type RecoveryState = Data.TaggedEnum<{
  /** No recovery is in progress. */
  Idle: Record<never, never>;
  /** The flow is inspecting existence-only candidates. */
  Discovering: Record<never, never>;
  /** One or more existing candidates were discovered. */
  CandidateDiscovered: { readonly candidateCount: number };
  /** No existing candidate was found. An open path stops here; it never creates. */
  NoCandidateFound: Record<never, never>;
  /** An existing candidate reconciled to one identity. */
  Reconciled: { readonly identityRef: IdentityRef };
  /** A typed reconciliation conflict needs owner resolution. */
  ConflictDetected: { readonly conflictClass: RecoveryConflictClass };
  /** A reconciled identity was imported to platform custody with a receipt. */
  CustodyImported: { readonly identityRef: IdentityRef; readonly receiptRef: string };
  /** A fail-closed blocker stopped the flow. */
  Blocked: { readonly blocker: RecoveryBlocker };
}>;
export const RecoveryState = Data.taggedEnum<RecoveryState>();

/**
 * A recovery event. Each event is advisory input the IDR-02 flow derives from
 * existence-only discovery, reconciliation, or an attended custody import. No
 * event represents root creation.
 */
export type RecoveryEvent = Data.TaggedEnum<{
  BeginDiscovery: Record<never, never>;
  CandidatesFound: { readonly count: number };
  NoCandidates: Record<never, never>;
  ReconcileSucceeded: { readonly identityRef: IdentityRef };
  ReconcileConflicted: { readonly conflictClass: RecoveryConflictClass };
  CustodyImportSucceeded: { readonly identityRef: IdentityRef; readonly receiptRef: string };
  EncounteredBlocker: { readonly blocker: RecoveryBlocker };
}>;
export const RecoveryEvent = Data.taggedEnum<RecoveryEvent>();

/** The reason a transition was rejected. */
export type RecoveryTransitionRejection = "illegal_transition" | "open_cannot_create";

/** The outcome of folding one event into a state. */
export type RecoveryTransitionOutcome =
  | { readonly ok: true; readonly state: RecoveryState }
  | { readonly ok: false; readonly reason: RecoveryTransitionRejection };

const accept = (state: RecoveryState): RecoveryTransitionOutcome => ({ ok: true, state });
const reject = (reason: RecoveryTransitionRejection): RecoveryTransitionOutcome => ({
  ok: false,
  reason,
});

/** The initial recovery state. */
export const initialRecoveryState: RecoveryState = RecoveryState.Idle();

/** The states that accept no further transition. */
const isTerminal = (state: RecoveryState): boolean =>
  state._tag === "CustodyImported" || state._tag === "NoCandidateFound";

/** True when a state accepts no further transition. */
export const isTerminalRecoveryState = (state: RecoveryState): boolean => isTerminal(state);

/**
 * Fold one event into a recovery state. The function is total and deterministic.
 * A terminal state accepts nothing further. A custody import is only legal from
 * a reconciled candidate, so a "no candidate" path can never reach an imported
 * state and can never create a root.
 */
export const applyRecoveryEvent = (
  state: RecoveryState,
  event: RecoveryEvent,
): RecoveryTransitionOutcome => {
  if (isTerminal(state)) return reject("illegal_transition");

  return RecoveryEvent.$match(event, {
    BeginDiscovery: () =>
      state._tag === "Idle" ? accept(RecoveryState.Discovering()) : reject("illegal_transition"),
    CandidatesFound: ({ count }) =>
      state._tag === "Discovering"
        ? accept(RecoveryState.CandidateDiscovered({ candidateCount: count }))
        : reject("illegal_transition"),
    NoCandidates: () =>
      state._tag === "Discovering"
        ? accept(RecoveryState.NoCandidateFound())
        : reject("illegal_transition"),
    ReconcileSucceeded: ({ identityRef }) =>
      state._tag === "CandidateDiscovered"
        ? accept(RecoveryState.Reconciled({ identityRef }))
        : reject("illegal_transition"),
    ReconcileConflicted: ({ conflictClass }) =>
      state._tag === "CandidateDiscovered"
        ? accept(RecoveryState.ConflictDetected({ conflictClass }))
        : reject("illegal_transition"),
    CustodyImportSucceeded: ({ identityRef, receiptRef }) =>
      // A custody import is legal ONLY from a reconciled existing candidate. From
      // any other state — above all `NoCandidateFound`, which cannot be reached
      // here because it is terminal — an import would be a silent create.
      state._tag === "Reconciled"
        ? accept(RecoveryState.CustodyImported({ identityRef, receiptRef }))
        : reject("open_cannot_create"),
    EncounteredBlocker: ({ blocker }) => accept(RecoveryState.Blocked({ blocker })),
  });
};
