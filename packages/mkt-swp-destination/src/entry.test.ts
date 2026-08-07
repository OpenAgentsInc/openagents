/**
 * Behaviour-contract oracles for destination entry (issue #9317).
 *
 * Enforced contracts (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-destination):
 * - openagents_web.swap_destination.amountless_invoice_refused.v1
 * - openagents_web.swap_destination.invoice_amount_precedence.v1
 * - openagents_web.swap_destination.amount_edit_clears_invoice.v1
 */
import { describe, expect, test } from "vite-plus/test";

import { parseBolt11 } from "./bolt11.js";
import {
  effectiveAmountMsat,
  fundingGate,
  initialEntryState,
  reduceEntryEvent,
  type DestinationEntryState,
  type EntryConfig,
} from "./entry.js";
import { destinationFieldView } from "./view.js";
import {
  DEFAULT_TEST_TIMESTAMP,
  encodeTestInvoice,
  encodeTestSegwitAddress,
} from "./testkit.js";

const NOW = DEFAULT_TEST_TIMESTAMP + 60;

const reachableAll: EntryConfig = { isRailReachable: () => true };
const reachableNone: EntryConfig = { isRailReachable: () => false };

const input = (
  state: DestinationEntryState,
  text: string,
  config: EntryConfig = reachableAll,
  source: "typing" | "paste" | "qr" = "paste",
) => reduceEntryEvent(state, { type: "input", source, text, nowSeconds: NOW }, config);

describe("amountless-invoice refusal (contract: amountless_invoice_refused.v1)", () => {
  test("an amountless invoice never binds and carries swp_invoice_invalid", () => {
    const state = initialEntryState("lightning", "regtest");
    const { state: next } = input(state, encodeTestInvoice({ amount: null }));
    expect(next.bound).toBeNull();
    expect(next.failure?.mode).toBe("invoice_amountless");
    expect(next.failure?.swpError).toBe("swp_invoice_invalid");
    expect(fundingGate(next, NOW)).toBe("blocked");

    const view = destinationFieldView(next, "unavailable", NOW);
    expect(view.validity.state).toBe("invalid");
    if (view.validity.state === "invalid") {
      expect(view.validity.swpError).toBe("swp_invoice_invalid");
    }
  });
});

describe("invoice amount precedence (contract: invoice_amount_precedence.v1)", () => {
  test("an amount-locked invoice overrides the typed amount", () => {
    let state = initialEntryState("lightning", "regtest");
    state = reduceEntryEvent(
      state,
      { type: "amount_edited", amountMsat: 999_000n },
      reachableAll,
    ).state;
    const { state: next } = input(state, encodeTestInvoice({ amount: "2500u" }));
    expect(next.bound?.kind).toBe("bolt11_invoice");
    expect(next.amountSource).toBe("invoice");
    expect(effectiveAmountMsat(next)).toBe(250_000_000n);
    expect(
      next.notices.some(
        (n) => n.notice === "typed_amount_overridden_by_invoice",
      ),
    ).toBe(true);
  });

  test("a unified QR with BIP-21 and invoice amounts yields one amount", () => {
    const address = encodeTestSegwitAddress("regtest", 0, 20);
    const invoice = encodeTestInvoice({ amount: "2500u" });
    const state = initialEntryState("lightning", "regtest");
    const { state: next } = input(
      state,
      `bitcoin:${address}?amount=0.005&lightning=${invoice}`,
      reachableAll,
      "qr",
    );
    expect(next.bound?.kind).toBe("bolt11_invoice");
    // One effective amount: the invoice's, never the BIP-21 value.
    expect(effectiveAmountMsat(next)).toBe(250_000_000n);
    expect(
      next.notices.some((n) => n.notice === "bip21_amount_suppressed"),
    ).toBe(true);
  });
});

describe("amount edits versus bound destinations (contract: amount_edit_clears_invoice.v1)", () => {
  test("editing the amount clears a concrete invoice with an explicit notice", () => {
    let state = initialEntryState("lightning", "regtest");
    state = input(state, encodeTestInvoice({ amount: "2500u" })).state;
    expect(state.bound?.kind).toBe("bolt11_invoice");

    const { state: next } = reduceEntryEvent(
      state,
      { type: "amount_edited", amountMsat: 300_000_000n },
      reachableAll,
    );
    expect(next.bound).toBeNull();
    expect(next.text).toBe("");
    expect(
      next.notices.some((n) => n.notice === "invoice_cleared_by_amount_edit"),
    ).toBe(true);
    expect(effectiveAmountMsat(next)).toBe(300_000_000n);
  });

  test("a deferred destination survives an amount edit", () => {
    let state = initialEntryState("lightning", "regtest");
    state = input(state, "alice@pay.example.com").state;
    expect(state.bound?.kind).toBe("deferred_lightning_address");

    const { state: next } = reduceEntryEvent(
      state,
      { type: "amount_edited", amountMsat: 42_000n },
      reachableAll,
    );
    expect(next.bound?.kind).toBe("deferred_lightning_address");
    expect(next.notices).toHaveLength(0);
    expect(effectiveAmountMsat(next)).toBe(42_000n);
  });
});

describe("paste-driven route switching", () => {
  test("pasting a chain address on a lightning field switches the rail", () => {
    const state = initialEntryState("lightning", "regtest");
    const { state: next, intents } = input(
      state,
      encodeTestSegwitAddress("regtest", 0, 20),
    );
    expect(next.rail).toBe("chain");
    expect(next.bound?.kind).toBe("onchain_address");
    expect(next.notices.some((n) => n.notice === "route_switched")).toBe(true);
    expect(intents.some((i) => i.intent === "switch_rail")).toBe(true);
  });

  test("an unreachable direction refuses with route_unreachable", () => {
    const state = initialEntryState("lightning", "regtest");
    const { state: next, intents } = input(
      state,
      encodeTestSegwitAddress("regtest", 0, 20),
      reachableNone,
    );
    expect(next.rail).toBe("lightning");
    expect(next.bound).toBeNull();
    expect(next.failure?.mode).toBe("route_unreachable");
    if (next.failure?.mode === "route_unreachable") {
      expect(next.failure.requiredRail).toBe("chain");
    }
    expect(intents).toHaveLength(0);
  });
});

describe("staleness guards", () => {
  test("a superseded verdict never overwrites a newer field value", () => {
    let state = initialEntryState("lightning", "regtest");
    state = input(state, encodeTestInvoice({ paymentHashByte: 0x11 })).state;
    const staleEpoch = state.epoch;
    state = input(state, encodeTestInvoice({ paymentHashByte: 0x22 })).state;

    const { state: afterStale } = reduceEntryEvent(
      state,
      { type: "verdict", epoch: staleEpoch, verdict: { verdict: "pass", epoch: staleEpoch } },
      reachableAll,
    );
    // The stale pass is dropped: verification still pending, gate blocked.
    expect(afterStale.verification.status).toBe("pending");
    expect(fundingGate(afterStale, NOW)).toBe("blocked");
  });

  test("a superseded resolution outcome is dropped", () => {
    let state = initialEntryState("lightning", "regtest");
    state = input(state, "alice@pay.example.com").state;
    const staleEpoch = state.epoch;
    state = input(state, "bob@pay.example.com").state;

    const { state: next } = reduceEntryEvent(
      state,
      {
        type: "resolution",
        epoch: staleEpoch,
        outcome: { outcome: "timeout", timeoutMillis: 25_000 },
      },
      reachableAll,
    );
    expect(next.resolution.status).toBe("none");
  });
});

describe("verification verdicts gate funding", () => {
  test("a failed re-derivation keeps funding disabled with the typed identifier", () => {
    let state = initialEntryState("chain", "regtest");
    state = input(state, encodeTestSegwitAddress("regtest", 0, 20)).state;
    const { state: next } = reduceEntryEvent(
      state,
      {
        type: "verdict",
        epoch: state.epoch,
        verdict: {
          verdict: "fail",
          epoch: state.epoch,
          error: "swp_script_commitment_mismatch",
          failedCheck: "output_key_rederivation",
        },
      },
      reachableAll,
    );
    expect(fundingGate(next, NOW)).toBe("blocked");
    const view = destinationFieldView(next, "unavailable", NOW);
    expect(view.validity.state).toBe("verification_failed");
    if (view.validity.state === "verification_failed") {
      expect(view.validity.swpError).toBe("swp_script_commitment_mismatch");
    }
    expect(view.fundingGate).toBe("blocked");
  });

  test("a current-epoch pass makes the destination fundable (necessary condition only)", () => {
    let state = initialEntryState("chain", "regtest");
    state = input(state, encodeTestSegwitAddress("regtest", 0, 20)).state;
    const { state: next } = reduceEntryEvent(
      state,
      {
        type: "verdict",
        epoch: state.epoch,
        verdict: { verdict: "pass", epoch: state.epoch },
      },
      reachableAll,
    );
    expect(fundingGate(next, NOW)).toBe("eligible");
  });
});

describe("direction changes from the shell", () => {
  test("a still-valid on-chain address survives a same-rail refresh", () => {
    let state = initialEntryState("chain", "regtest");
    const address = encodeTestSegwitAddress("regtest", 0, 20);
    state = input(state, address).state;
    const { state: next } = reduceEntryEvent(
      state,
      { type: "rail_changed", rail: "chain", network: "regtest", nowSeconds: NOW },
      reachableAll,
    );
    expect(next.bound?.kind).toBe("onchain_address");
    expect(next.text).toBe(address);
  });

  test("switching to the lightning rail clears a bound address", () => {
    let state = initialEntryState("chain", "regtest");
    state = input(state, encodeTestSegwitAddress("regtest", 0, 20)).state;
    const { state: next } = reduceEntryEvent(
      state,
      {
        type: "rail_changed",
        rail: "lightning",
        network: "regtest",
        nowSeconds: NOW,
      },
      reachableAll,
    );
    expect(next.bound).toBeNull();
    expect(next.text).toBe("");
  });
});

describe("deferred destinations verify their resolved invoice (audit gap 1)", () => {
  const resolvedInvoice = () => {
    const parsed = parseBolt11(encodeTestInvoice({ amount: "2500u" }), {
      network: "regtest",
      nowSeconds: NOW,
    });
    if (!parsed.ok) throw new Error("fixture invoice failed to parse");
    return parsed.invoice;
  };

  test("binding a deferred destination requests no verification and stays blocked", () => {
    const state = initialEntryState("lightning", "regtest");
    const { state: next, intents } = input(state, "alice@pay.example.com");
    expect(next.bound?.kind).toBe("deferred_lightning_address");
    // The verifier port has no operation for a deferred destination;
    // nothing is requested and the gate stays blocked until resolution.
    expect(next.verification.status).toBe("idle");
    expect(intents).toHaveLength(0);
    expect(fundingGate(next, NOW)).toBe("blocked");
  });

  test("the resolved invoice must earn its own verdict before the gate opens", () => {
    // The audit's exact reachable order: amount edit, paste a lightning
    // address, a pass verdict at the current epoch, then resolution.
    let state = initialEntryState("lightning", "regtest");
    state = reduceEntryEvent(
      state,
      { type: "amount_edited", amountMsat: 250_000_000n },
      reachableAll,
    ).state;
    state = input(state, "alice@pay.example.com").state;
    state = reduceEntryEvent(
      state,
      {
        type: "verdict",
        epoch: state.epoch,
        verdict: { verdict: "pass", epoch: state.epoch },
      },
      reachableAll,
    ).state;
    // No request was outstanding, so that pass is dropped.
    expect(state.verification.status).toBe("idle");

    state = reduceEntryEvent(
      state,
      { type: "resolution_started", epoch: state.epoch },
      reachableAll,
    ).state;
    const invoice = resolvedInvoice();
    const { state: resolved, intents } = reduceEntryEvent(
      state,
      {
        type: "resolution",
        epoch: state.epoch,
        outcome: { outcome: "resolved", invoice },
      },
      reachableAll,
    );
    // The invoice the payment actually goes to has no verdict yet: the
    // gate is blocked and a verification request for *that invoice* went
    // out at the current epoch.
    expect(fundingGate(resolved, NOW)).toBe("blocked");
    expect(resolved.verification).toEqual({
      status: "pending",
      epoch: resolved.epoch,
    });
    const request = intents.find((i) => i.intent === "request_verification");
    if (request?.intent !== "request_verification") {
      throw new Error("expected a verification request for the invoice");
    }
    expect(request.destination.kind).toBe("bolt11_invoice");
    if (request.destination.kind === "bolt11_invoice") {
      expect(request.destination.invoice).toBe(invoice.invoice);
    }
    // The view says verifying, not valid, while that request is out.
    expect(
      destinationFieldView(resolved, "unavailable", NOW).validity.state,
    ).toBe("verifying");

    // Only the engine's pass on the resolved invoice opens the gate.
    const { state: verified } = reduceEntryEvent(
      resolved,
      {
        type: "verdict",
        epoch: resolved.epoch,
        verdict: { verdict: "pass", epoch: resolved.epoch },
      },
      reachableAll,
    );
    expect(fundingGate(verified, NOW)).toBe("eligible");
    expect(effectiveAmountMsat(verified)).toBe(250_000_000n);
  });

  test("the request carries the session's expected payment hash", () => {
    const expected = "ab".repeat(32);
    const config: EntryConfig = {
      isRailReachable: () => true,
      expectedPaymentHashHex: expected,
    };
    const state = initialEntryState("lightning", "regtest");
    const { intents } = reduceEntryEvent(
      state,
      {
        type: "input",
        source: "paste",
        text: encodeTestInvoice({ amount: "2500u" }),
        nowSeconds: NOW,
      },
      config,
    );
    const request = intents.find((i) => i.intent === "request_verification");
    if (request?.intent !== "request_verification") {
      throw new Error("expected a verification request");
    }
    expect(request.expectedPaymentHashHex).toBe(expected);
  });
});

describe("amount edits keep a path back to eligibility (audit gap 3)", () => {
  test("editing the amount on a bound address re-requests verification", () => {
    let state = initialEntryState("chain", "regtest");
    state = input(state, encodeTestSegwitAddress("regtest", 0, 20)).state;
    state = reduceEntryEvent(
      state,
      {
        type: "verdict",
        epoch: state.epoch,
        verdict: { verdict: "pass", epoch: state.epoch },
      },
      reachableAll,
    ).state;
    expect(fundingGate(state, NOW)).toBe("eligible");

    const { state: next, intents } = reduceEntryEvent(
      state,
      { type: "amount_edited", amountMsat: 100_000n },
      reachableAll,
    );
    expect(fundingGate(next, NOW)).toBe("blocked");
    expect(next.verification).toEqual({ status: "pending", epoch: next.epoch });
    const request = intents.find((i) => i.intent === "request_verification");
    if (request?.intent !== "request_verification") {
      throw new Error("expected a re-verification request");
    }
    expect(request.epoch).toBe(next.epoch);
    // The field never renders `valid` while the gate is blocked with a
    // verdict outstanding.
    expect(destinationFieldView(next, "unavailable", NOW).validity.state).toBe(
      "verifying",
    );

    const { state: verified } = reduceEntryEvent(
      next,
      {
        type: "verdict",
        epoch: next.epoch,
        verdict: { verdict: "pass", epoch: next.epoch },
      },
      reachableAll,
    );
    expect(fundingGate(verified, NOW)).toBe("eligible");
    expect(
      destinationFieldView(verified, "unavailable", NOW).validity.state,
    ).toBe("valid");
  });
});

describe("invoice expiry after entry (audit gap 4)", () => {
  test("an invoice that expires while the field sits open blocks funding", () => {
    let state = initialEntryState("lightning", "regtest");
    state = input(
      state,
      encodeTestInvoice({ amount: "2500u", expirySeconds: 120 }),
    ).state;
    state = reduceEntryEvent(
      state,
      {
        type: "verdict",
        epoch: state.epoch,
        verdict: { verdict: "pass", epoch: state.epoch },
      },
      reachableAll,
    ).state;
    expect(fundingGate(state, NOW)).toBe("eligible");

    const afterExpiry = DEFAULT_TEST_TIMESTAMP + 121;
    expect(fundingGate(state, afterExpiry)).toBe("blocked");
    const view = destinationFieldView(state, "unavailable", afterExpiry);
    expect(view.fundingGate).toBe("blocked");
    expect(view.validity.state).toBe("invalid");
    if (view.validity.state === "invalid") {
      expect(view.validity.mode).toBe("invoice_expired");
    }
  });
});
