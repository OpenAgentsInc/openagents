/**
 * The §9 mapping table, signer map, and transition edges, pinned.
 */
import { describe, expect, test } from "vite-plus/test";

import { analyzeStatusSequence } from "@openagentsinc/nip-mkt";
import {
  admittedSignerFor,
  allowedSuccessors,
  classifySwpState,
  HAPPY_PATH,
  knownStates,
  LOCAL_ONLY_PROJECTIONS,
} from "./states.js";
import { claim, TEST_ORDER_ID, TEST_SESSION_ID } from "./testkit.js";

describe("§9 base-state derivation", () => {
  test("every known state of every flow classifies to a base state", () => {
    for (const flow of ["submarine", "reverse", "chain"] as const) {
      for (const state of knownStates(flow)) {
        const classified = classifySwpState(state);
        expect(classified.ok, `${flow}:${state} must map to a §9 row`).toBe(true);
      }
    }
  });

  test("pins the exact §9 table rows", () => {
    const expectations: Record<string, string> = {
      accepted: "accepted",
      rejected: "rejected",
      lock_terms_ready: "awaiting_input",
      requester_verification_passed: "awaiting_input",
      hold_invoice_ready: "awaiting_input",
      requester_invoice_verified: "awaiting_input",
      funding_required: "funding_required",
      source_funding_required: "funding_required",
      requester_funding_broadcast: "funding_observed",
      provider_funding_broadcast: "funding_observed",
      requester_source_broadcast: "funding_observed",
      provider_destination_broadcast: "funding_observed",
      funding_observed: "funding_observed",
      destination_funding_observed: "funding_observed",
      funding_final: "executing",
      source_funding_final: "executing",
      lightning_payment_pending: "executing",
      lightning_htlcs_held: "executing",
      provider_claim_pending: "executing",
      requester_destination_claim_pending: "executing",
      provider_claimed: "executing",
      requester_claimed: "executing",
      lightning_paid: "executing",
      lightning_settlement_pending: "settlement_pending",
      provider_source_claim_pending: "settlement_pending",
      completed: "completed",
      refund_prepared: "refund_pending",
      provider_refund_pending: "refund_pending",
      invoice_cancel_pending: "refund_pending",
      refunded: "refunded",
      provider_refunded: "refunded",
      invoice_cancelled: "refunded",
      disputed: "disputed",
      failed: "failed",
      unresolved: "failed",
    };
    for (const [state, base] of Object.entries(expectations)) {
      const classified = classifySwpState(state);
      expect(classified.ok, state).toBe(true);
      if (classified.ok) expect(classified.base, state).toBe(base);
    }
  });

  test("chain broadcast names map explicitly to funding_observed", () => {
    expect(classifySwpState("requester_source_broadcast")).toEqual({
      ok: true,
      base: "funding_observed",
    });
    expect(classifySwpState("provider_destination_broadcast")).toEqual({
      ok: true,
      base: "funding_observed",
    });
  });

  test("contract_pending and contract_bound are local projections a claim can never establish", () => {
    for (const local of LOCAL_ONLY_PROJECTIONS) {
      expect(classifySwpState(local)).toEqual({
        ok: false,
        error: "swp_status_transition_invalid",
      });
    }
  });

  test("a value matching no row is swp_status_transition_invalid", () => {
    for (const junk of ["", "banana", "settled", "claiming", "refunding", "transaction.claimed"]) {
      expect(classifySwpState(junk).ok, junk).toBe(false);
    }
  });
});

describe("signer map", () => {
  test("submarine action claims admit exactly the §9.2 signer", () => {
    expect(admittedSignerFor("submarine", "accepted")).toBe("provider");
    expect(admittedSignerFor("submarine", "lightning_paid")).toBe("provider");
    expect(admittedSignerFor("submarine", "provider_claimed")).toBe("provider");
    expect(admittedSignerFor("submarine", "requester_funding_broadcast")).toBe("requester");
    expect(admittedSignerFor("submarine", "refund_pending")).toBe("requester");
    expect(admittedSignerFor("submarine", "funding_observed")).toBe("either_observation");
    expect(admittedSignerFor("submarine", "completed")).toBe("either_observation");
  });

  test("reverse and chain claims admit exactly the §9.3/§9.4 signer", () => {
    expect(admittedSignerFor("reverse", "lightning_payment_pending")).toBe("requester");
    expect(admittedSignerFor("reverse", "provider_funding_broadcast")).toBe("provider");
    expect(admittedSignerFor("reverse", "requester_claimed")).toBe("requester");
    expect(admittedSignerFor("chain", "destination_lock_terms_ready")).toBe("provider");
    expect(admittedSignerFor("chain", "source_funding_required")).toBe("provider");
    expect(admittedSignerFor("chain", "requester_source_broadcast")).toBe("requester");
    expect(admittedSignerFor("chain", "provider_source_claimed")).toBe("provider");
  });
});

describe("transition edges", () => {
  test("the happy path is edge-connected in order for every flow", () => {
    for (const flow of ["submarine", "reverse", "chain"] as const) {
      const path = HAPPY_PATH[flow];
      for (let index = 0; index + 1 < path.length; index += 1) {
        expect(
          allowedSuccessors(flow, path[index]!),
          `${flow}: ${path[index]} -> ${path[index + 1]}`,
        ).toContain(path[index + 1]!);
      }
    }
  });

  test("recovery is reachable only from funded states", () => {
    expect(allowedSuccessors("submarine", "accepted")).not.toContain("failed");
    expect(allowedSuccessors("submarine", "requester_funding_broadcast")).toContain("failed");
    expect(allowedSuccessors("submarine", "funding_final")).toContain("refund_prepared");
    expect(allowedSuccessors("submarine", "refund_pending")).toContain("refunded");
    expect(allowedSuccessors("reverse", "provider_refunded")).toContain("invoice_cancelled");
    expect(allowedSuccessors("chain", "provider_destination_refunded")).toContain(
      "requester_source_refund_pending",
    );
  });

  test("skipping a required action is not an edge", () => {
    expect(allowedSuccessors("submarine", "accepted")).not.toContain("lightning_paid");
    expect(allowedSuccessors("submarine", "accepted")).not.toContain("completed");
    expect(allowedSuccessors("reverse", "hold_invoice_ready")).not.toContain("requester_claimed");
  });

  test("swp-v1-btc-liquid-chain-regtest: destination preflight precedes Bitcoin source funding", () => {
    expect(allowedSuccessors("chain", "requester_source_verified")).toEqual([
      "destination_lock_terms_ready",
    ]);
    expect(allowedSuccessors("chain", "destination_lock_terms_ready")).toEqual([
      "requester_destination_verified",
    ]);
    expect(allowedSuccessors("chain", "requester_destination_verified")).toEqual([
      "source_funding_required",
    ]);
    expect(allowedSuccessors("chain", "source_funding_final")).toContain(
      "provider_destination_broadcast",
    );
  });

  test("swp-v1-negative-btc-liquid-source-before-preflight: source funding fails closed", () => {
    expect(allowedSuccessors("chain", "requester_source_verified")).not.toContain(
      "source_funding_required",
    );
    expect(allowedSuccessors("chain", "destination_lock_terms_ready")).not.toContain(
      "source_funding_required",
    );
  });

  test("reverse counterparty-lock confirmation states cannot be skipped", () => {
    expect(allowedSuccessors("reverse", "provider_funding_broadcast")).toEqual([
      "funding_observed",
      "provider_refund_prepared",
      "disputed",
      "failed",
      "unresolved",
    ]);
    expect(allowedSuccessors("reverse", "funding_observed")).not.toContain(
      "requester_claim_pending",
    );
    expect(allowedSuccessors("reverse", "funding_final")).toContain("requester_claim_pending");
  });
});

const toRecord = (role: "requester" | "provider", seq: number, id?: string) => {
  const built = claim(role, seq, "accepted", id === undefined ? {} : { id });
  return {
    id: built.id,
    sessionId: TEST_SESSION_ID,
    orderId: TEST_ORDER_ID,
    author: built.author,
    seq,
  };
};

describe("agreement with the nip-mkt sequence oracle", () => {
  test("gap and fork vocabulary matches analyzeStatusSequence", () => {
    const gap = analyzeStatusSequence([
      toRecord("provider", 0),
      toRecord("provider", 1),
      toRecord("provider", 3),
    ]);
    expect(gap.decision).toBe("gap");
    if (gap.decision === "gap") {
      expect(gap.missingSequences).toEqual([2]);
      expect(gap.lastContiguousSeq).toBe(1);
    }
    const fork = analyzeStatusSequence([
      toRecord("provider", 0),
      toRecord("provider", 1, "fork-a".padEnd(64, "0")),
      toRecord("provider", 1, "fork-b".padEnd(64, "0")),
    ]);
    expect(fork.decision).toBe("fork");
    if (fork.decision === "fork") {
      expect(fork.retainIds).toHaveLength(2);
      expect(fork.advanceState).toBe(false);
    }
  });
});
