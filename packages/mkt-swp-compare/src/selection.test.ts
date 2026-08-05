/**
 * Order selection discipline tests (issue #9318 §7) and the local
 * expiry-refusal half of the behaviour contract
 * `openagents_web.swap_compare.quote_expiry_enforced.v1`
 * (registry: `@openagentsinc/behavior-contracts`, market-swap-compare).
 */
import { describe, expect, test } from "vite-plus/test";

import { orderAcceptance, selectOrder } from "./selection.js";
import {
  testFirmHardQuote,
  testHexId,
  testQuote,
  testReservation,
} from "./testkit.js";

const NOW = 1_800_000_100;

describe("selectOrder", () => {
  test("an expired quote cannot be ordered: the client refuses locally with swp_quote_expired", () => {
    const expired = testQuote({ expiresAtSeconds: NOW - 1 });
    const result = selectOrder(expired, {}, NOW);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("swp_quote_expired");
      expect(result.field).toBe("quote");
    }
  });

  test("an expired reservation also refuses locally", () => {
    const quote = testFirmHardQuote({
      reservation: testReservation({ reservationExpiresAtSeconds: NOW - 1 }),
    });
    const result = selectOrder(quote, {}, NOW);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("swp_quote_expired");
  });

  test("a nonconforming quote (firm without reservation) is refused, not reranked", () => {
    const quote = testQuote({ quoteClass: "firm", reservationClass: "none" });
    const result = selectOrder(quote, {}, NOW);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("swp_reservation_missing");
  });

  test("a disallowed proof/class pairing (provider_signed backing hard) is refused", () => {
    const quote = testFirmHardQuote({
      reservation: testReservation({ proofClass: "provider_signed" }),
    });
    const result = selectOrder(quote, {}, NOW);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe("swp_reservation_proof_invalid");
  });

  test("a quote with no selectable object permits no selection at all", () => {
    const quote = testFirmHardQuote({ selectable: null });
    const refused = selectOrder(quote, { feePayer: "requester" }, NOW);
    expect(refused.ok).toBe(false);
    if (!refused.ok) expect(refused.error).toBe("swp_order_selection_invalid");
    const bare = selectOrder(quote, {}, NOW);
    expect(bare.ok).toBe(true);
    if (bare.ok) expect(bare.quoteEventId).toBe(quote.eventId);
  });

  test("every out-of-list choice is swp_order_selection_invalid, field-addressed", () => {
    const quote = testFirmHardQuote({
      selectable: {
        inputAmountRangeSats: { minSats: 100_000n, maxSats: 1_000_000n },
        feePayers: ["requester"],
        confirmationPolicies: ["1conf"],
        publicReceiptConsent: ["none"],
      },
    });
    const amount = selectOrder(quote, { inputAmountSats: 2_000_000n }, NOW);
    expect(!amount.ok && amount.field === "inputAmountSats").toBe(true);
    const fee = selectOrder(quote, { feePayer: "provider" }, NOW);
    expect(!fee.ok && fee.field === "feePayer").toBe(true);
    const conf = selectOrder(quote, { confirmationPolicy: "0conf" }, NOW);
    expect(!conf.ok && conf.field === "confirmationPolicy").toBe(true);
    const consent = selectOrder(quote, { publicReceiptConsent: "full" }, NOW);
    expect(!consent.ok && consent.field === "publicReceiptConsent").toBe(true);
    for (const result of [amount, fee, conf, consent]) {
      if (!result.ok) expect(result.error).toBe("swp_order_selection_invalid");
    }
  });

  test("a conforming in-list selection commits the exact quote event id and only the bounded selection", () => {
    const quote = testFirmHardQuote({
      selectable: {
        inputAmountRangeSats: { minSats: 100_000n, maxSats: 1_000_000n },
        feePayers: ["requester", "provider"],
        confirmationPolicies: ["1conf"],
        publicReceiptConsent: ["none"],
      },
    });
    const result = selectOrder(
      quote,
      { inputAmountSats: 500_000n, feePayer: "provider" },
      NOW,
    );
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.quoteEventId).toBe(quote.eventId);
      expect(result.selection).toEqual({
        inputAmountSats: 500_000n,
        feePayer: "provider",
      });
      // The draft carries no price, amount-out, asset, fee, route, or
      // expiry fields to restate: inheritance is by event-id reference.
      expect(Object.keys(result)).toEqual(["ok", "quoteEventId", "selection"]);
    }
  });
});

describe("orderAcceptance", () => {
  test("a firm quote's acceptance is its own declaration — labeled as a declaration", () => {
    const state = orderAcceptance(testFirmHardQuote(), null);
    expect(state.accepted).toBe(true);
    if (state.accepted) expect(state.via).toBe("firm_quote_declaration");
  });

  test("indicative: silence, relay acceptance, an invoice, or an address is never acceptance", () => {
    const quote = testQuote();
    expect(orderAcceptance(quote, null)).toEqual({
      accepted: false,
      reason: "awaiting_provider_status",
    });
    for (const kind of [
      "silence",
      "relay_ok",
      "invoice_received",
      "address_received",
    ] as const) {
      expect(orderAcceptance(quote, { kind })).toEqual({
        accepted: false,
        reason: "evidence_is_not_acceptance",
      });
    }
  });

  test("indicative: only a provider Status state=accepted from the quoting provider counts", () => {
    const quote = testQuote();
    const wrongSigner = orderAcceptance(quote, {
      kind: "provider_status",
      state: "accepted",
      statusEventId: testHexId(0x51),
      signerPubkey: testHexId(0x99),
    });
    expect(wrongSigner.accepted).toBe(false);
    if (!wrongSigner.accepted) {
      expect(wrongSigner.reason).toBe("status_signer_mismatch");
    }
    const wrongState = orderAcceptance(quote, {
      kind: "provider_status",
      state: "received",
      statusEventId: testHexId(0x52),
      signerPubkey: quote.providerPubkey,
    });
    expect(wrongState.accepted).toBe(false);
    const accepted = orderAcceptance(quote, {
      kind: "provider_status",
      state: "accepted",
      statusEventId: testHexId(0x53),
      signerPubkey: quote.providerPubkey,
    });
    expect(accepted).toEqual({
      accepted: true,
      via: "provider_status_accepted",
      statusEventId: testHexId(0x53),
    });
  });
});
