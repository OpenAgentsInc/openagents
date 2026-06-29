// Run-settlement seam tests (issue #6188). PROVES the seam stays INERT: it moves
// no sats, arming is owner-gated AND unimplemented (errors), and a fully-advanced
// 8-state machine still records every money-movement step as intent_only with
// movedSats === false.

import { describe, expect, test } from "bun:test";
import {
  advanceRunSettlement,
  arm,
  createRunSettlementMachine,
  isRunSettlementComplete,
  publicRunSettlementProjection,
  RUN_SETTLEMENT_OWNER_ARM_TOKEN,
  RUN_SETTLEMENT_STATE_ORDER,
  RunSettlementError,
  runSettlementMovedSats,
  type RunSettlementMachine,
} from "./run-settlement";

const split = () => ({
  receiptRef: "receipt:qa_runner:openagents-com:0123456789abcdef",
  authorBps: 7000,
  platformBps: 3000,
});

const advanceAll = (): RunSettlementMachine => {
  let machine = createRunSettlementMachine(split());
  for (const [i, stateId] of RUN_SETTLEMENT_STATE_ORDER.entries()) {
    machine = advanceRunSettlement(machine, stateId, {
      evidenceRef: `evidence.run_settlement.${stateId}`,
      recordedAt: `2026-06-24T12:00:0${i}.000Z`,
      sats: 100,
    });
  }
  return machine;
};

describe("createRunSettlementMachine (DEFAULT-OFF)", () => {
  test("a fresh machine is disarmed and has moved nothing", () => {
    const machine = createRunSettlementMachine(split());
    expect(machine.armed).toBe(false);
    expect(machine.state).toBeNull();
    expect(machine.transitions).toHaveLength(0);
    expect(runSettlementMovedSats(machine)).toBe(false);
  });

  test("rejects a split that does not sum to 100%", () => {
    expect(() => createRunSettlementMachine({ ...split(), authorBps: 5000 })).toThrow(RunSettlementError);
  });
});

describe("arm (OWNER-GATED / SPEC-ONLY)", () => {
  test("arming without the owner token errors", () => {
    expect(() => arm(createRunSettlementMachine(split()))).toThrow(RunSettlementError);
  });

  test("arming WITH the owner token still errors (no payout executor wired)", () => {
    expect(() =>
      arm(createRunSettlementMachine(split()), { ownerArmToken: RUN_SETTLEMENT_OWNER_ARM_TOKEN }),
    ).toThrow(/SPEC-ONLY/);
  });
});

describe("advanceRunSettlement (INERT — no sats move)", () => {
  test("a fully advanced 8-state machine moved NO sats; money-movement states are intent_only", () => {
    const machine = advanceAll();
    expect(isRunSettlementComplete(machine)).toBe(true);
    expect(runSettlementMovedSats(machine)).toBe(false);
    // every transition is honest about not moving money
    expect(machine.transitions.every(t => t.movedSats === false)).toBe(true);
    // the would-be money-movement states are recorded intent_only
    const dispatched = machine.transitions.find(t => t.stateId === "dispatched");
    const confirmed = machine.transitions.find(t => t.stateId === "confirmed");
    expect(dispatched?.evidenceKind).toBe("intent_only");
    expect(confirmed?.evidenceKind).toBe("intent_only");
    expect(machine.noSettlementImplication).toBe(true);
  });

  test("is monotonic and gap-free", () => {
    const machine = createRunSettlementMachine(split());
    expect(() =>
      advanceRunSettlement(machine, "paid", { evidenceRef: "e.paid", recordedAt: "2026-06-24T12:00:00.000Z" }),
    ).toThrow(/cannot skip states/);
  });

  test("rejects a non-public-safe evidence ref", () => {
    const machine = createRunSettlementMachine(split());
    expect(() =>
      advanceRunSettlement(machine, "authorized", {
        evidenceRef: "lnbc100n1secretinvoice",
        recordedAt: "2026-06-24T12:00:00.000Z",
      }),
    ).toThrow(RunSettlementError);
  });

  test("the public projection exposes the lifecycle without figures and confirms movedSats is false", () => {
    const projection = publicRunSettlementProjection(advanceAll());
    expect(projection.movedSats).toBe(false);
    expect(projection.complete).toBe(true);
    expect(projection.transitions.map(t => t.stateId)).toEqual([...RUN_SETTLEMENT_STATE_ORDER]);
    // no monetary FIGURE leaks into the projection transitions (only honest labels)
    expect(projection.transitions.every(t => !("sats" in t))).toBe(true);
  });
});
