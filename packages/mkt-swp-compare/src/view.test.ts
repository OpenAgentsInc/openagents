/**
 * Behaviour-contract oracles for the comparison table view (issue #9318).
 *
 * Enforced contracts (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-compare):
 * - openagents_web.swap_compare.firm_indicative_distinct.v1 (rendering
 *   half: the firm/hard row cannot be styled identically to the
 *   indicative/none row).
 * - openagents_web.swap_compare.reservation_proof_class_distinct.v1
 *   (rendering half: a signed claim and a covenant-enforced reserve never
 *   render the same; forks stay visible).
 */
import { describe, expect, test } from "vite-plus/test";

import {
  COMMITMENT_MESSAGES,
  PROOF_CLASS_MESSAGES,
} from "./messages.js";
import { RESERVATION_PROOF_CLASSES } from "./model.js";
import {
  detectReservationForks,
  observeReservations,
} from "./reservation.js";
import {
  testFirmHardQuote,
  testHexId,
  testQuote,
  testReservation,
} from "./testkit.js";
import { commitmentBadge, compareTableView } from "./view.js";

const NOW = 1_800_000_100;

describe("commitment rendering", () => {
  test("two providers answering one RFQ with different classes render as different commitments", () => {
    const indicativeNone = testQuote();
    const firmHard = testFirmHardQuote();
    const table = compareTableView([indicativeNone, firmHard], {
      nowSeconds: NOW,
    });
    const firmRow = table.rows.find(row => row.eventId === firmHard.eventId);
    const indicativeRow = table.rows.find(
      row => row.eventId === indicativeNone.eventId,
    );
    // The badge token is the render key that selects the style: distinct
    // tokens mean the rows CANNOT be styled identically.
    expect(firmRow?.commitment.token).toBe("firm:hard");
    expect(indicativeRow?.commitment.token).toBe("indicative:none");
    expect(firmRow?.commitment.token).not.toBe(indicativeRow?.commitment.token);
    expect(firmRow?.commitment.label.key).not.toBe(
      indicativeRow?.commitment.label.key,
    );
    expect(firmRow?.commitment.label.message).not.toBe(
      indicativeRow?.commitment.label.message,
    );
    // Firm copy states a declaration, not a proof.
    expect(firmRow?.commitment.label.message).toContain("declares");
    expect(firmRow?.commitment.label.message).toContain("not proven");
  });

  test("every commitment (class × reservation) has a distinct token and label", () => {
    const entries = Object.entries(COMMITMENT_MESSAGES);
    expect(new Set(entries.map(([token]) => token)).size).toBe(entries.length);
    expect(new Set(entries.map(([, m]) => m.key)).size).toBe(entries.length);
    expect(new Set(entries.map(([, m]) => m.message)).size).toBe(entries.length);
  });

  test("commitmentBadge carries the proof class exactly when the quote reserves", () => {
    expect(commitmentBadge(testQuote()).proof).toBeNull();
    const badge = commitmentBadge(testFirmHardQuote());
    expect(badge.proof?.proofClass).toBe("covenant_reserve");
    expect(badge.proof?.label.key).toBe("swap.compare.proof.covenant_reserve");
  });
});

describe("reservation proof-class rendering", () => {
  test("all seven proof classes render pairwise-distinct labels — a signed claim never renders like a covenant reserve", () => {
    const keys = RESERVATION_PROOF_CLASSES.map(
      proofClass => PROOF_CLASS_MESSAGES[proofClass].key,
    );
    const messages = RESERVATION_PROOF_CLASSES.map(
      proofClass => PROOF_CLASS_MESSAGES[proofClass].message,
    );
    expect(new Set(keys).size).toBe(RESERVATION_PROOF_CLASSES.length);
    expect(new Set(messages).size).toBe(RESERVATION_PROOF_CLASSES.length);
    expect(PROOF_CLASS_MESSAGES.provider_signed.message).not.toBe(
      PROOF_CLASS_MESSAGES.covenant_reserve.message,
    );
  });
});

describe("compareTableView", () => {
  test("rows come out ranked: firm quotes above the indicative regardless of price", () => {
    const richIndicative = testQuote({
      eventId: testHexId(0x31),
      outputAmountSats: 2_000_000n,
    });
    const firmHard = testFirmHardQuote({ outputAmountSats: 990_000n });
    const table = compareTableView([richIndicative, firmHard], {
      nowSeconds: NOW,
    });
    expect(table.rows.map(row => row.eventId)).toEqual([
      firmHard.eventId,
      richIndicative.eventId,
    ]);
  });

  test("an expired quote is retained but unusable — not merely styled stale", () => {
    const expired = testQuote({
      eventId: testHexId(0x32),
      expiresAtSeconds: NOW - 5,
    });
    const active = testFirmHardQuote();
    const table = compareTableView([expired, active], { nowSeconds: NOW });
    const expiredRow = table.rows.find(row => row.eventId === expired.eventId);
    expect(expiredRow?.expiry.state).toBe("expired");
    expect(expiredRow?.usability).toEqual({
      usable: false,
      reason: "expired",
      notice: expect.objectContaining({ key: "swap.compare.quote_expired" }),
    });
  });

  test("a reservation fork is retained and displayed on every implicated row rather than resolved", () => {
    const a = testFirmHardQuote({
      eventId: testHexId(0x33),
      reservation: testReservation({
        reservationId: testHexId(0x41),
        allocationSequence: 5n,
        proofClass: "covenant_reserve",
      }),
    });
    const b = testFirmHardQuote({
      eventId: testHexId(0x34),
      reservation: testReservation({
        reservationId: testHexId(0x42),
        allocationSequence: 5n,
        proofClass: "covenant_reserve",
      }),
    });
    const forks = detectReservationForks(observeReservations([a, b]));
    const table = compareTableView([a, b], { nowSeconds: NOW, forks });
    for (const eventId of [a.eventId, b.eventId]) {
      const row = table.rows.find(candidate => candidate.eventId === eventId);
      expect(row?.forks).toHaveLength(1);
      expect(row?.usability).toEqual({
        usable: false,
        reason: "reservation_fork",
        notice: expect.objectContaining({
          key: "swap.compare.reservation_fork",
        }),
      });
    }
  });

  test("provider freshness is derived from the injected clock", () => {
    const quote = testQuote({ createdAtSeconds: NOW - 30 });
    const table = compareTableView([quote], { nowSeconds: NOW });
    expect(table.rows[0]?.provider.ageSeconds).toBe(30);
  });
});
