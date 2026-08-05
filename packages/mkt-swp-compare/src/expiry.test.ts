/**
 * Behaviour-contract oracle for quote expiry enforcement (issue #9318 §4).
 *
 * Enforced contract (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-compare):
 * - openagents_web.swap_compare.quote_expiry_enforced.v1 (countdown state
 *   half; the local order-refusal half lives in `selection.test.ts`).
 */
import { describe, expect, test } from "vite-plus/test";

import { effectiveExpiresAtSeconds, quoteExpiryState } from "./expiry.js";
import { testFirmHardQuote, testQuote, testReservation } from "./testkit.js";

describe("effective expiry", () => {
  test("without a reservation, the quote expiration is the bound", () => {
    const quote = testQuote({ expiresAtSeconds: 1_800_000_900 });
    expect(effectiveExpiresAtSeconds(quote)).toBe(1_800_000_900);
  });

  test("a reservation expiring earlier than the quote binds first", () => {
    const quote = testFirmHardQuote({
      expiresAtSeconds: 1_800_000_900,
      reservation: testReservation({
        reservationExpiresAtSeconds: 1_800_000_300,
      }),
    });
    expect(effectiveExpiresAtSeconds(quote)).toBe(1_800_000_300);
    const state = quoteExpiryState(quote, 1_800_000_100);
    expect(state.state).toBe("active");
    if (state.state === "active") {
      expect(state.boundByReservation).toBe(true);
      expect(state.secondsRemaining).toBe(200);
    }
  });
});

describe("expiry enforcement", () => {
  test("an active quote counts down with the injected clock", () => {
    const quote = testQuote({ expiresAtSeconds: 1_800_000_900 });
    const state = quoteExpiryState(quote, 1_800_000_890);
    expect(state).toEqual({
      state: "active",
      secondsRemaining: 10,
      expiresAtSeconds: 1_800_000_900,
      boundByReservation: false,
    });
  });

  test("at the bound the quote is expired with the typed identifier — not merely stale-styled", () => {
    const quote = testQuote({ expiresAtSeconds: 1_800_000_900 });
    const state = quoteExpiryState(quote, 1_800_000_900);
    expect(state.state).toBe("expired");
    if (state.state === "expired") {
      expect(state.error).toBe("swp_quote_expired");
      expect(state.via).toBe("quote");
      expect(state.expiredAtSeconds).toBe(1_800_000_900);
    }
  });

  test("a reservation expiring first expires the quote via the reservation", () => {
    const quote = testFirmHardQuote({
      expiresAtSeconds: 1_800_000_900,
      reservation: testReservation({
        reservationExpiresAtSeconds: 1_800_000_300,
      }),
    });
    const state = quoteExpiryState(quote, 1_800_000_400);
    expect(state.state).toBe("expired");
    if (state.state === "expired") {
      expect(state.error).toBe("swp_quote_expired");
      expect(state.via).toBe("reservation");
    }
  });
});
