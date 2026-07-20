import { Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { IdentityRef } from "../contract/index.ts";
import {
  RecoveryEvent,
  RecoveryState,
  applyRecoveryEvent,
  initialRecoveryState,
  isTerminalRecoveryState,
} from "./recovery-state.ts";

const identityRef = S.decodeUnknownSync(IdentityRef)("test-ref");

describe("recovery state machine", () => {
  test("the happy path discovers, reconciles, and imports an EXISTING candidate", () => {
    let state = initialRecoveryState;
    for (const event of [
      RecoveryEvent.BeginDiscovery(),
      RecoveryEvent.CandidatesFound({ count: 1 }),
      RecoveryEvent.ReconcileSucceeded({ identityRef }),
      RecoveryEvent.CustodyImportSucceeded({ identityRef, receiptRef: "receipt-1" }),
    ]) {
      const outcome = applyRecoveryEvent(state, event);
      expect(outcome.ok).toBe(true);
      if (outcome.ok) state = outcome.state;
    }
    expect(state._tag).toBe("CustodyImported");
    expect(isTerminalRecoveryState(state)).toBe(true);
  });

  test("an open path that finds no candidate stops terminal and never creates", () => {
    let state = initialRecoveryState;
    const discovering = applyRecoveryEvent(state, RecoveryEvent.BeginDiscovery());
    expect(discovering.ok).toBe(true);
    if (discovering.ok) state = discovering.state;
    const none = applyRecoveryEvent(state, RecoveryEvent.NoCandidates());
    expect(none.ok).toBe(true);
    if (none.ok) state = none.state;
    expect(state._tag).toBe("NoCandidateFound");
    expect(isTerminalRecoveryState(state)).toBe(true);

    // A custody import from a no-candidate path is rejected: no silent create.
    const create = applyRecoveryEvent(
      state,
      RecoveryEvent.CustodyImportSucceeded({ identityRef, receiptRef: "receipt-x" }),
    );
    expect(create.ok).toBe(false);
    if (!create.ok) expect(create.reason).toBe("illegal_transition");
  });

  test("a custody import is rejected `open_cannot_create` from a non-reconciled state", () => {
    const state = RecoveryState.CandidateDiscovered({ candidateCount: 2 });
    const outcome = applyRecoveryEvent(
      state,
      RecoveryEvent.CustodyImportSucceeded({ identityRef, receiptRef: "receipt-y" }),
    );
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) expect(outcome.reason).toBe("open_cannot_create");
  });

  test("a reconciliation conflict is a typed non-terminal state", () => {
    const state = RecoveryState.CandidateDiscovered({ candidateCount: 1 });
    const outcome = applyRecoveryEvent(
      state,
      RecoveryEvent.ReconcileConflicted({ conflictClass: "nostr_pubkey_mismatch" }),
    );
    expect(outcome.ok).toBe(true);
    if (outcome.ok) {
      expect(outcome.state._tag).toBe("ConflictDetected");
      expect(isTerminalRecoveryState(outcome.state)).toBe(false);
    }
  });

  test("an out-of-order transition is rejected", () => {
    const outcome = applyRecoveryEvent(
      initialRecoveryState,
      RecoveryEvent.ReconcileSucceeded({ identityRef }),
    );
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) expect(outcome.reason).toBe("illegal_transition");
  });
});
