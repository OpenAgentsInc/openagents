/**
 * Reservation fork tests (issue #9318 §3, MKT-SWP §5): forks are retained
 * and attributable, never resolved by arrival time. Part of the behaviour
 * contract `openagents_web.swap_compare.reservation_proof_class_distinct.v1`
 * (registry: `@openagentsinc/behavior-contracts`, market-swap-compare).
 */
import { describe, expect, test } from "vite-plus/test";

import {
  capacityBucketMapKey,
  detectReservationForks,
  forkedQuoteEventIds,
  observeReservations,
} from "./reservation.js";
import { testFirmHardQuote, testHexId, testReservation } from "./testkit.js";

const providerPubkey = testHexId(0xf2);

const quoteWithSequence = (idSeed: number, sequence: bigint) =>
  testFirmHardQuote({
    eventId: testHexId(idSeed),
    reservation: testReservation({
      reservationId: testHexId(idSeed + 0x100),
      allocationSequence: sequence,
      proofClass: "covenant_reserve",
    }),
  });

describe("detectReservationForks", () => {
  test("two reservations claiming one allocation sequence are an attributable fork", () => {
    const a = quoteWithSequence(0x21, 42n);
    const b = quoteWithSequence(0x22, 42n);
    const forks = detectReservationForks(observeReservations([a, b]));
    expect(forks).toHaveLength(1);
    expect(forks[0]?.error).toBe("swp_reservation_fork");
    expect(forks[0]?.kind).toBe("duplicate_allocation_sequence");
    expect(forks[0]?.providerPubkey).toBe(providerPubkey);
    expect(forks[0]?.memberQuoteEventIds).toEqual(
      [a.eventId, b.eventId].sort(),
    );
  });

  test("never resolved by arrival time: both ingestion orders produce identical retained forks", () => {
    const a = quoteWithSequence(0x23, 7n);
    const b = quoteWithSequence(0x24, 7n);
    const forward = detectReservationForks(observeReservations([a, b]));
    const backward = detectReservationForks(observeReservations([b, a]));
    expect(forward).toEqual(backward);
    // Neither member is dropped in favour of the earlier/later arrival.
    expect(forward[0]?.memberQuoteEventIds).toContain(a.eventId);
    expect(forward[0]?.memberQuoteEventIds).toContain(b.eventId);
  });

  test("distinct sequences in one bucket are not a fork", () => {
    const a = quoteWithSequence(0x25, 1n);
    const b = quoteWithSequence(0x26, 2n);
    expect(detectReservationForks(observeReservations([a, b]))).toEqual([]);
  });

  test("capacity overallocation forks when committed capacity is known", () => {
    const a = quoteWithSequence(0x27, 1n);
    const b = quoteWithSequence(0x28, 2n);
    const key = capacityBucketMapKey(
      providerPubkey,
      "bucket-btc-out",
      "btc:mainnet",
    );
    // Each quote reserves 250_000 sats; commit only 400_000.
    const forks = detectReservationForks(
      observeReservations([a, b]),
      new Map([[key, 400_000n]]),
    );
    expect(forks).toHaveLength(1);
    expect(forks[0]?.kind).toBe("capacity_exceeded");
    // Unknown capacity performs no check — and passes nothing.
    expect(detectReservationForks(observeReservations([a, b]), null)).toEqual([]);
  });

  test("forkedQuoteEventIds implicates every member for the table", () => {
    const a = quoteWithSequence(0x29, 9n);
    const b = quoteWithSequence(0x2a, 9n);
    const forks = detectReservationForks(observeReservations([a, b]));
    const implicated = forkedQuoteEventIds(forks);
    expect(implicated.has(a.eventId)).toBe(true);
    expect(implicated.has(b.eventId)).toBe(true);
  });
});
