import { describe, expect, test } from "vite-plus/test";

import type { Issue31OwnerCommandState } from "../src/workroom/issue31-owner-private-read-model.ts";
import {
  ISSUE31_FULL_AUTO_CONTROL_SETTLEMENT_SCHEMA,
  decodeIssue31FullAutoControlSettlement,
  issue31FullAutoControlIsComplete,
  issue31FullAutoControlIsInFlight,
  issue31FullAutoControlSettlementCopy,
  settleIssue31FullAutoControl,
} from "../src/workroom/issue31-full-auto-control-settlement.ts";

const control = {
  actionRef: "action.full-auto.pause",
  kind: "pause",
  runGeneration: 7,
  idempotencyRef: "idem.run-full-auto-run-01.pause.7",
} as const;

const queued: Issue31OwnerCommandState = {
  state: "queued",
  intentEventId: "event.intent.0001",
  actionRef: control.actionRef,
  idempotencyRef: control.idempotencyRef,
  handlingRef: null,
  sourceEventId: null,
};

const accepted: Issue31OwnerCommandState = {
  state: "accepted",
  intentEventId: "event.intent.0001",
  actionRef: control.actionRef,
  idempotencyRef: control.idempotencyRef,
  handlingRef: "handling.host.0001",
  sourceEventId: "event.projection.0009",
};

const settled: Issue31OwnerCommandState = {
  state: "terminal",
  intentEventId: "event.intent.0001",
  actionRef: control.actionRef,
  idempotencyRef: control.idempotencyRef,
  handlingRef: "handling.host.0001",
  sourceEventId: "event.projection.0009",
};

describe(ISSUE31_FULL_AUTO_CONTROL_SETTLEMENT_SCHEMA, () => {
  test("an unsent control is offered, not pending and not complete", () => {
    const settlement = settleIssue31FullAutoControl(control, []);
    expect(settlement.state).toBe("offered");
    expect(issue31FullAutoControlIsComplete(settlement)).toBe(false);
    expect(issue31FullAutoControlIsInFlight(settlement)).toBe(false);
  });

  test("a published intent is requested — a relay taking it is not completion", () => {
    const settlement = settleIssue31FullAutoControl(control, [queued]);
    expect(settlement.state).toBe("requested");
    expect(issue31FullAutoControlIsComplete(settlement)).toBe(false);
    // The owner is told the truth: sent, and waiting.
    expect(issue31FullAutoControlSettlementCopy(settlement)).toContain("waiting");
  });

  test("host acceptance is not completion", () => {
    const settlement = settleIssue31FullAutoControl(control, [accepted]);
    expect(settlement.state).toBe("accepted");
    expect(issue31FullAutoControlIsComplete(settlement)).toBe(false);
    // The copy must not read as finished, or the distinction is decorative.
    expect(issue31FullAutoControlSettlementCopy(settlement)).toContain("not finished");
  });

  test("only a host-settled command completes, and it names what settled it", () => {
    const settlement = settleIssue31FullAutoControl(control, [settled]);
    expect(settlement.state).toBe("completed");
    expect(issue31FullAutoControlIsComplete(settlement)).toBe(true);
    if (settlement.state !== "completed") throw new Error("unreachable");
    expect(settlement.hostSettlementRef).toBe("event.projection.0009");
  });

  test("a refused or failed command is terminal and is never a success", () => {
    for (const state of ["refused", "failed", "unavailable"] as const) {
      const settlement = settleIssue31FullAutoControl(control, [
        {
          state,
          intentEventId: "event.intent.0001",
          actionRef: control.actionRef,
          idempotencyRef: control.idempotencyRef,
          handlingRef: "handling.host.0001",
          reasonRef: "reason.issue31.host_declined",
        },
      ]);
      expect(settlement.state).toBe(state);
      expect(issue31FullAutoControlIsComplete(settlement)).toBe(false);
    }
  });

  // ---------------------------------------------------------------------
  // Falsification: each of these is a way a control could be shown finished
  // when nothing Omega-owned finished it.
  // ---------------------------------------------------------------------

  test("a completed settlement cannot be constructed without a host settlement", () => {
    expect(() =>
      decodeIssue31FullAutoControlSettlement({
        state: "completed",
        actionRef: control.actionRef,
        idempotencyRef: control.idempotencyRef,
        intentEventId: "event.intent.0001",
        handlingRef: "handling.host.0001",
      }),
    ).toThrow();
  });

  test("an optimistic local completion is refused rather than rendered", () => {
    // The shape a hopeful caller would invent after pressing the button.
    expect(() =>
      decodeIssue31FullAutoControlSettlement({
        state: "completed",
        actionRef: control.actionRef,
        idempotencyRef: control.idempotencyRef,
        intentEventId: "event.intent.0001",
        handlingRef: "handling.host.0001",
        hostSettlementRef: "event.projection.0009",
        optimistic: true,
      }),
    ).toThrow();
  });

  test("a settlement reference that is a private path cannot be encoded", () => {
    expect(() =>
      decodeIssue31FullAutoControlSettlement({
        state: "completed",
        actionRef: control.actionRef,
        idempotencyRef: control.idempotencyRef,
        intentEventId: "event.intent.0001",
        handlingRef: "handling.host.0001",
        hostSettlementRef: "/Users/owner/.codex/auth.json",
      }),
    ).toThrow();
  });

  test("another command reusing this idempotency reference cannot complete this control", () => {
    // A ledger row under the same idempotency reference but a different action
    // is not this button's outcome. Attributing it would let one command's
    // success mark a different control finished.
    const settlement = settleIssue31FullAutoControl(control, [
      { ...settled, actionRef: "action.full-auto.stop" },
    ]);
    expect(settlement.state).toBe("unavailable");
    expect(issue31FullAutoControlIsComplete(settlement)).toBe(false);
    if (settlement.state !== "unavailable") throw new Error("unreachable");
    expect(settlement.reasonRef).toBe("reason.issue31.control_binding_mismatch");
  });

  test("a command for a different control leaves this one untouched", () => {
    const settlement = settleIssue31FullAutoControl(control, [
      { ...settled, idempotencyRef: "idem.run-full-auto-run-01.stop.7" },
    ]);
    expect(settlement.state).toBe("offered");
  });
});
