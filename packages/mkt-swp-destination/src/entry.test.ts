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
    expect(fundingGate(next)).toBe("blocked");

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
    expect(fundingGate(afterStale)).toBe("blocked");
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
    expect(fundingGate(next)).toBe("blocked");
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
    expect(fundingGate(next)).toBe("eligible");
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
