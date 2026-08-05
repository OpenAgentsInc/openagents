/**
 * Behaviour-contract oracle for the comparison ranking rule (issue #9318).
 *
 * Enforced contract (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-compare):
 * - openagents_web.swap_compare.firm_indicative_distinct.v1 (ranking half:
 *   a cheaper indicative quote is never presented as if it were as binding
 *   as a firm one).
 */
import { describe, expect, test } from "vite-plus/test";

import { commitmentTier, compareQuotes, rankQuotes } from "./ranking.js";
import {
  testFirmHardQuote,
  testHexId,
  testQuote,
  testReservation,
} from "./testkit.js";

const NOW = 1_800_000_100;

describe("commitment tier", () => {
  test("firm+hard < firm+soft < indicative < nonconforming < expired", () => {
    const firmHard = testFirmHardQuote();
    const firmSoft = testQuote({
      eventId: testHexId(3),
      quoteClass: "firm",
      reservationClass: "soft",
      reservation: testReservation(),
    });
    const indicative = testQuote({ eventId: testHexId(4) });
    const nonconforming = testQuote({
      eventId: testHexId(5),
      quoteClass: "firm",
      reservationClass: "none",
    });
    const expired = testFirmHardQuote({
      eventId: testHexId(6),
      expiresAtSeconds: NOW - 1,
      reservation: testReservation({ reservationExpiresAtSeconds: NOW - 1 }),
    });
    expect(commitmentTier(firmHard, NOW)).toBe(0);
    expect(commitmentTier(firmSoft, NOW)).toBe(1);
    expect(commitmentTier(indicative, NOW)).toBe(2);
    expect(commitmentTier(nonconforming, NOW)).toBe(3);
    expect(commitmentTier(expired, NOW)).toBe(4);
  });
});

describe("ranking rule", () => {
  test("a cheaper indicative quote never outranks any firm quote", () => {
    // Indicative pays out substantially more…
    const richIndicative = testQuote({
      eventId: testHexId(7),
      outputAmountSats: 1_500_000n,
    });
    // …but both firm quotes still rank above it: class-major ordering.
    const firmHard = testFirmHardQuote({ outputAmountSats: 990_000n });
    const firmSoft = testQuote({
      eventId: testHexId(8),
      quoteClass: "firm",
      reservationClass: "soft",
      reservation: testReservation(),
      outputAmountSats: 980_000n,
    });
    const ranked = rankQuotes([richIndicative, firmSoft, firmHard], NOW);
    expect(ranked.map(quote => quote.eventId)).toEqual([
      firmHard.eventId,
      firmSoft.eventId,
      richIndicative.eventId,
    ]);
  });

  test("within a tier, best execution (larger output) ranks first", () => {
    const smaller = testFirmHardQuote({
      eventId: testHexId(9),
      outputAmountSats: 980_000n,
    });
    const larger = testFirmHardQuote({
      eventId: testHexId(10),
      outputAmountSats: 995_000n,
    });
    const ranked = rankQuotes([smaller, larger], NOW);
    expect(ranked.map(quote => quote.eventId)).toEqual([
      larger.eventId,
      smaller.eventId,
    ]);
  });

  test("deterministic and stable: input order never changes the result, and equal terms tiebreak by event id", () => {
    const a = testFirmHardQuote({ eventId: testHexId(0x0b) });
    const b = testFirmHardQuote({ eventId: testHexId(0x0a) });
    const c = testQuote({ eventId: testHexId(0x0c) });
    const forward = rankQuotes([a, b, c], NOW);
    const backward = rankQuotes([c, b, a], NOW);
    const shuffled = rankQuotes([b, c, a], NOW);
    expect(forward.map(quote => quote.eventId)).toEqual(
      backward.map(quote => quote.eventId),
    );
    expect(forward.map(quote => quote.eventId)).toEqual(
      shuffled.map(quote => quote.eventId),
    );
    // a and b have identical terms; the tiebreak is ascending event id.
    expect(forward.map(quote => quote.eventId).slice(0, 2)).toEqual(
      [a.eventId, b.eventId].sort(),
    );
    // Comparator is a total order: 0 only for the identical event.
    expect(compareQuotes(NOW)(a, a)).toBe(0);
    expect(compareQuotes(NOW)(a, b)).not.toBe(0);
  });

  test("expired quotes sink below every active quote regardless of price", () => {
    const expiredRich = testFirmHardQuote({
      eventId: testHexId(0x0d),
      outputAmountSats: 2_000_000n,
      expiresAtSeconds: NOW - 10,
    });
    const activeIndicative = testQuote({
      eventId: testHexId(0x0e),
      outputAmountSats: 900_000n,
    });
    const ranked = rankQuotes([expiredRich, activeIndicative], NOW);
    expect(ranked.map(quote => quote.eventId)).toEqual([
      activeIndicative.eventId,
      expiredRich.eventId,
    ]);
  });
});
