import { describe, expect, test } from "vite-plus/test";

import { foldLane } from "./lane.js";
import { claim, shuffled, statusId } from "./testkit.js";

describe("per-signer lane fold", () => {
  test("a contiguous chained stream is intact and fully valid", () => {
    const lane = foldLane("provider", [
      claim("provider", 0, "accepted"),
      claim("provider", 1, "lock_terms_ready"),
      claim("provider", 2, "lightning_payment_pending"),
    ]);
    expect(lane.integrity).toEqual({ kind: "intact" });
    expect(lane.gaps).toEqual([]);
    expect(lane.forks).toEqual([]);
    expect(lane.validClaims.map((c) => c.seq)).toEqual([0, 1, 2]);
  });

  test("a missing sequence is a gap that later records do not close", () => {
    const lane = foldLane("provider", [
      claim("provider", 0, "accepted"),
      claim("provider", 1, "lock_terms_ready"),
      claim("provider", 3, "lightning_paid"),
      claim("provider", 4, "provider_claimed"),
    ]);
    expect(lane.integrity).toEqual({ kind: "gap", atSeq: 2 });
    expect(lane.gaps).toEqual([{ seq: 2, error: "swp_status_gap" }]);
    // Later records are retained but cannot join the valid prefix.
    expect(lane.validClaims.map((c) => c.seq)).toEqual([0, 1]);
    expect(lane.slots.map((slot) => slot.seq)).toEqual([0, 1, 3, 4]);
  });

  test("the exact missing record arriving late closes the gap (set semantics)", () => {
    const records = [
      claim("provider", 0, "accepted"),
      claim("provider", 1, "lock_terms_ready"),
      claim("provider", 3, "lightning_payment_pending", {
        previous: statusId("provider", 2),
      }),
      claim("provider", 2, "requester_verification_passed"),
    ];
    const lane = foldLane("provider", records);
    expect(lane.integrity).toEqual({ kind: "intact" });
    expect(lane.validClaims.map((c) => c.seq)).toEqual([0, 1, 2, 3]);
  });

  test("two records at one sequence are a fork: both retained, never arrival-resolved", () => {
    const forkA = claim("provider", 2, "lightning_paid", { id: "aa".padEnd(64, "a") });
    const forkB = claim("provider", 2, "refund_prepared", { id: "bb".padEnd(64, "b") });
    const base = [claim("provider", 0, "accepted"), claim("provider", 1, "lock_terms_ready")];
    const oneOrder = foldLane("provider", [...base, forkA, forkB]);
    const otherOrder = foldLane("provider", [...base, forkB, forkA]);
    expect(oneOrder).toEqual(otherOrder);
    expect(oneOrder.integrity).toEqual({ kind: "fork", atSeq: 2 });
    expect(oneOrder.forks).toEqual([
      { seq: 2, ids: [forkA.id, forkB.id], error: "swp_status_fork" },
    ]);
    const slot = oneOrder.slots.find((candidate) => candidate.seq === 2)!;
    expect(slot.records).toHaveLength(2);
    expect(oneOrder.validClaims.map((c) => c.seq)).toEqual([0, 1]);
  });

  test("duplicates are idempotent and arrival order converges deterministically", () => {
    const records = [
      claim("provider", 0, "accepted"),
      claim("provider", 1, "lock_terms_ready"),
      claim("provider", 2, "lightning_payment_pending"),
      claim("provider", 3, "lightning_paid"),
    ];
    const reference = foldLane("provider", records);
    for (const seed of [1, 7, 42, 1337]) {
      const replayed = foldLane("provider", [
        ...shuffled(records, seed),
        ...shuffled(records, seed + 1),
      ]);
      expect(replayed).toEqual(reference);
    }
  });

  test("a broken previous reference stops the valid prefix", () => {
    const lane = foldLane("provider", [
      claim("provider", 0, "accepted"),
      claim("provider", 1, "lock_terms_ready", { previous: "0".repeat(64) }),
    ]);
    expect(lane.integrity).toEqual({ kind: "previous_mismatch", atSeq: 1 });
    expect(lane.validClaims.map((c) => c.seq)).toEqual([0]);
  });

  test("seq 0 with a previous reference is a chain break", () => {
    const lane = foldLane("provider", [
      claim("provider", 0, "accepted", { previous: "0".repeat(64) }),
    ]);
    expect(lane.integrity).toEqual({ kind: "previous_mismatch", atSeq: 0 });
    expect(lane.validClaims).toEqual([]);
  });
});
