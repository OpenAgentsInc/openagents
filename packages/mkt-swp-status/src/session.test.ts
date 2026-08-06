/**
 * Behaviour-contract oracles for status/progress (issue #9321).
 *
 * Enforced contracts (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-status):
 * - openagents_web.swap_status.gap_renders_unknown.v1
 * - openagents_web.swap_status.fork_retained_loud.v1
 * - openagents_web.swap_status.rung_never_inferred_upward.v1
 *
 * The adversarial status-sequence table lives at the bottom: gap, fork,
 * duplicate, out-of-order, expiry-crossing, and unresolved sequences all
 * run through the one production projection.
 */
import { describe, expect, test } from "vite-plus/test";

import type { StatusClaim, SwapEvidence } from "./model.js";
import { projectSession, type SessionInput, type SwapProgressView } from "./session.js";
import {
  claim,
  close,
  evidence,
  happyPathClaims,
  shuffled,
  statusId,
  TEST_ORDER_ID,
  TEST_SESSION_ID,
} from "./testkit.js";

const FULL_EVIDENCE: readonly SwapEvidence[] = [
  evidence({ class: "bitcoin_output", authority: "bitcoin_adapter", rung: "measured" }),
  evidence({
    class: "bitcoin_output",
    authority: "bitcoin_adapter",
    rung: "settled",
    final: true,
  }),
  evidence({ class: "lightning_payment", authority: "lightning_adapter", rung: "paid" }),
  evidence({
    class: "bitcoin_spend",
    authority: "bitcoin_adapter",
    rung: "settled",
    final: true,
  }),
];

const session = (
  statuses: readonly StatusClaim[],
  overrides: Partial<SessionInput> = {},
): SwapProgressView =>
  projectSession({
    flow: "submarine",
    sessionId: TEST_SESSION_ID,
    orderId: TEST_ORDER_ID,
    statuses,
    evidence: FULL_EVIDENCE,
    ...overrides,
  });

describe("control: a well-evidenced happy path advances to completed", () => {
  test("both lanes contiguous, display reaches completed, rung is settled", () => {
    const view = session(happyPathClaims("submarine"));
    expect(view.integrity).toBe("intact");
    expect(view.lastValidSwpState).toBe("completed");
    expect(view.baseState).toBe("completed");
    expect(view.displayKey).toBe("swap.status.display.completed");
    expect(view.rung.proven).toBe("settled");
  });

  test("statuses from another session or order are ignored", () => {
    const foreign = claim("provider", 0, "accepted", { sessionId: "9".repeat(64) });
    const view = session([...happyPathClaims("submarine", "accepted"), foreign]);
    expect(view.lanes.provider.validClaims).toHaveLength(1);
  });
});

describe("gap rendering (contract: openagents_web.swap_status.gap_renders_unknown.v1)", () => {
  const withGap = (): StatusClaim[] => {
    const base = happyPathClaims("submarine", "funding_final");
    // Provider seq 2 (lightning_payment_pending) is deliberately missing;
    // seq 3 and seq 4 arrive anyway.
    return [
      ...base,
      claim("provider", 3, "lightning_paid"),
      claim("provider", 4, "provider_claim_pending"),
    ];
  };

  test("a missing sequence renders as unknown-with-explanation, never optimistic progress", () => {
    const view = session(withGap());
    expect(view.integrity).toBe("unknown_gap");
    expect(view.displayKey).toBe("swap.status.display.unknown_gap");
    expect(view.lanes.provider.gaps).toEqual([{ seq: 2, error: "swp_status_gap" }]);
    // The claims after the gap are retained, not advanced.
    expect(view.lastValidSwpState).toBe("funding_final");
    const laterClaim = view.retained.find((r) => r.claim.swpState === "lightning_paid")!;
    expect(laterClaim.disposition.kind).not.toBe("advanced");
  });

  test("a later Status does not close the gap", () => {
    const view = session([...withGap(), claim("provider", 5, "provider_claimed")]);
    expect(view.integrity).toBe("unknown_gap");
    expect(view.lanes.provider.gaps).toEqual([{ seq: 2, error: "swp_status_gap" }]);
    expect(view.lastValidSwpState).toBe("funding_final");
  });

  test("only the exact missing record closes the gap", () => {
    const view = session([...withGap(), claim("provider", 2, "lightning_payment_pending")]);
    expect(view.integrity).toBe("intact");
    expect(view.lanes.provider.gaps).toEqual([]);
  });
});

describe("fork rendering (contract: openagents_web.swap_status.fork_retained_loud.v1)", () => {
  const forkA = claim("provider", 2, "lightning_payment_pending", { id: "aa".padEnd(64, "a") });
  const forkB = claim("provider", 2, "lightning_paid", { id: "bb".padEnd(64, "b") });
  const base = happyPathClaims("submarine", "funding_final");

  test("both conflicting claims are retained with their author; no arrival-time winner exists", () => {
    const oneOrder = session([...base, forkA, forkB]);
    const otherOrder = session([...base, forkB, forkA]);
    expect(oneOrder).toEqual(otherOrder);
    expect(oneOrder.integrity).toBe("unknown_fork");
    expect(oneOrder.displayKey).toBe("swap.status.display.unknown_fork");
    expect(oneOrder.lanes.provider.forks).toEqual([
      { seq: 2, ids: [forkA.id, forkB.id], error: "swp_status_fork" },
    ]);
    const slot = oneOrder.lanes.provider.slots.find((s) => s.seq === 2)!;
    expect(slot.records.map((record) => record.id)).toEqual([forkA.id, forkB.id]);
    expect(slot.records.every((record) => record.author === forkA.author)).toBe(true);
  });

  test("a later Status cannot erase the fork", () => {
    const view = session([...base, forkA, forkB, claim("provider", 3, "provider_claim_pending")]);
    expect(view.integrity).toBe("unknown_fork");
    expect(view.lanes.provider.forks).toHaveLength(1);
    expect(view.lastValidSwpState).toBe("funding_final");
  });
});

describe("rungs (contract: openagents_web.swap_status.rung_never_inferred_upward.v1)", () => {
  test("a completed Status without verifier evidence renders at the proved rung, not settled", () => {
    const claims = happyPathClaims("submarine");
    const partialEvidence: readonly SwapEvidence[] = [
      evidence({ class: "bitcoin_output", authority: "bitcoin_adapter", rung: "measured" }),
      evidence({
        class: "bitcoin_output",
        authority: "bitcoin_adapter",
        rung: "settled",
        final: true,
      }),
      evidence({ class: "lightning_payment", authority: "lightning_adapter", rung: "paid" }),
      // No settled spend evidence: `completed` is only a claim.
    ];
    const view = session(claims, { evidence: partialEvidence });
    expect(view.lastValidSwpState).toBe("provider_claimed");
    expect(view.displayKey).toBe("swap.status.display.provider_claimed");
    // The settled fact on hand is about the FUNDING output — attributed, it
    // cannot dress up the completion claim, whose own classes prove `paid`.
    const overclaim = view.retained.find((r) => r.claim.swpState === "completed")!;
    expect(overclaim.disposition).toEqual({
      kind: "unproven",
      reason: "swp_settlement_overclaim",
      requiredRung: "settled",
      provenRung: "paid",
    });
  });

  test("provider status alone can never produce a rung above pledged", () => {
    const view = session(happyPathClaims("submarine", "accepted"), {
      evidence: [
        evidence({
          class: "bitcoin_spend",
          authority: "provider_status",
          rung: "settled",
          final: true,
        }),
      ],
    });
    expect(view.rung.proven).toBe("pledged");
    expect(view.rung.facts[0]!.overclaim).toBe(true);
  });
});

describe("signer and transition discipline", () => {
  test("the chain source-funding instruction is invalid until destination preflight is signed", () => {
    const view = session(
      [
        ...happyPathClaims("chain", "requester_source_verified"),
        claim("provider", 2, "source_funding_required"),
      ],
      { flow: "chain" },
    );
    const invalid = view.retained.find(
      (retained) => retained.claim.swpState === "source_funding_required",
    )!;
    expect(invalid.disposition).toEqual({
      kind: "invalid",
      reason: "swp_status_transition_invalid",
    });
    expect(view.lastValidSwpState).toBe("requester_source_verified");
  });

  test("a requester cannot claim the chain source-funding instruction", () => {
    const view = session(
      [
        ...happyPathClaims("chain", "requester_source_verified"),
        claim("requester", 1, "source_funding_required"),
      ],
      { flow: "chain" },
    );
    const invalid = view.retained.find(
      (retained) =>
        retained.claim.swpState === "source_funding_required" &&
        retained.claim.role === "requester",
    )!;
    expect(invalid.disposition).toEqual({
      kind: "invalid",
      reason: "swp_status_signer_invalid",
    });
    expect(view.lastValidSwpState).toBe("requester_source_verified");
  });

  test("a status from a signer the state machine does not admit is retained invalid", () => {
    const view = session([
      ...happyPathClaims("submarine", "funding_final"),
      claim("requester", 5, "provider_claimed"),
    ]);
    const invalid = view.retained.find(
      (r) => r.claim.swpState === "provider_claimed" && r.claim.role === "requester",
    )!;
    expect(invalid.disposition).toEqual({
      kind: "invalid",
      reason: "swp_status_signer_invalid",
    });
    expect(view.lastValidSwpState).toBe("funding_final");
  });

  test("a status that skips a required action is retained and does not advance", () => {
    const view = session([
      claim("provider", 0, "accepted"),
      claim("provider", 1, "provider_claimed"),
    ]);
    const skipped = view.retained.find((r) => r.claim.swpState === "provider_claimed")!;
    expect(skipped.disposition).toEqual({
      kind: "invalid",
      reason: "swp_status_transition_invalid",
    });
    expect(view.lastValidSwpState).toBe("accepted");
  });

  test("a claimed local projection (contract_bound) is never established by a Status", () => {
    const view = session([
      claim("provider", 0, "accepted"),
      claim("provider", 1, "contract_bound"),
    ]);
    const local = view.retained.find((r) => r.claim.swpState === "contract_bound")!;
    expect(local.disposition).toEqual({
      kind: "invalid",
      reason: "swp_status_transition_invalid",
    });
    expect(view.lastValidSwpState).toBe("accepted");
  });

  test("a carried base state that disagrees with the §9 derivation is invalid", () => {
    const view = session([claim("provider", 0, "accepted", { baseState: "completed" })]);
    expect(view.retained[0]!.disposition).toEqual({
      kind: "invalid",
      reason: "swp_status_transition_invalid",
    });
    expect(view.lastValidSwpState).toBeNull();
  });
});

/**
 * The adversarial status-sequence table (issue #9321 verification): every
 * row runs through the one production projection, and every row that takes
 * the same record set in a different arrival order must converge to the
 * identical view.
 */
describe("adversarial status-sequence table", () => {
  interface Row {
    readonly name: string;
    readonly statuses: readonly StatusClaim[];
    readonly overrides?: Partial<SessionInput>;
    readonly assert: (view: SwapProgressView) => void;
  }

  const funded = happyPathClaims("submarine", "funding_final");

  const rows: readonly Row[] = [
    {
      name: "gap: missing provider seq renders unknown, later records advance nothing",
      statuses: [...funded, claim("provider", 3, "lightning_paid")],
      assert: (view) => {
        expect(view.integrity).toBe("unknown_gap");
        expect(view.displayKey).toBe("swap.status.display.unknown_gap");
        expect(view.lastValidSwpState).toBe("funding_final");
      },
    },
    {
      name: "fork: two provider records at one seq, both retained, session frozen",
      statuses: [
        ...funded,
        claim("provider", 2, "lightning_payment_pending", { id: "aa".padEnd(64, "a") }),
        claim("provider", 2, "refund_prepared", { id: "bb".padEnd(64, "b") }),
      ],
      assert: (view) => {
        expect(view.integrity).toBe("unknown_fork");
        expect(view.lanes.provider.forks).toHaveLength(1);
        expect(view.lanes.provider.forks[0]!.ids).toHaveLength(2);
        expect(view.lastValidSwpState).toBe("funding_final");
      },
    },
    {
      name: "duplicate: the same records delivered twice are idempotent",
      statuses: [...funded, ...funded],
      assert: (view) => {
        expect(view).toEqual(session(funded));
      },
    },
    {
      name: "out-of-order: shuffled arrival converges to the in-order view",
      statuses: shuffled(happyPathClaims("submarine"), 20260804),
      assert: (view) => {
        expect(view).toEqual(session(happyPathClaims("submarine")));
        expect(view.lastValidSwpState).toBe("completed");
      },
    },
    {
      name: "expiry-crossing: height past H_refund switches the exit and stops trusting claims",
      statuses: funded,
      overrides: {
        ladder: {
          kind: "submarine",
          hFund: 100,
          hClaim: 130,
          hRefund: 160,
          invoiceExpirationTime: 1_785_900_000,
        },
        currentHeight: 161,
      },
      assert: (view) => {
        expect(view.ladder!.stopTrustingCounterpartyClaims).toBe(true);
        const refundRung = view.ladder!.rungs.find((rung) => rung.id === "refund_valid")!;
        expect(refundRung.status).toBe("passed");
        expect(refundRung.exitNow).toBe("refund");
        expect(refundRung.timeIsEstimate).toBe(true);
      },
    },
    {
      name: "unresolved: displayed as unresolved (not failed, not complete) and never watch-terminal",
      statuses: [...funded, claim("requester", 5, "unresolved")],
      overrides: { closes: [close("requester", "unresolved")] },
      assert: (view) => {
        expect(view.lastValidSwpState).toBe("unresolved");
        expect(view.displayKey).toBe("swap.status.display.unresolved");
        expect(view.baseState).toBe("failed"); // §9 base derivation...
        expect(view.displayKey).not.toContain("failed"); // ...but never the display
        expect(view.displayKey).not.toContain("completed");
        expect(view.watchTerminal).toBe(false);
        expect(view.closes.closes[0]!.descriptor.exit).toBe("keep_watching");
      },
    },
    {
      name: "conflicting closes: provider completed vs requester unresolved, both visible, watch continues",
      statuses: funded,
      overrides: {
        closes: [close("provider", "completed"), close("requester", "unresolved")],
      },
      assert: (view) => {
        expect(view.closes.closes).toHaveLength(2);
        expect(view.closes.conflict).toBe(true);
        expect(view.watchTerminal).toBe(false);
      },
    },
    {
      name: "reconnect replay: EOSE snapshot plus overlapping live events equals the union once",
      statuses: [
        ...happyPathClaims("submarine"),
        ...shuffled(happyPathClaims("submarine"), 7).slice(0, 8),
      ],
      assert: (view) => {
        expect(view).toEqual(session(happyPathClaims("submarine")));
      },
    },
    {
      name: "gap plus fork in different lanes: fork severity wins the headline",
      statuses: [
        ...funded.filter((status) => !(status.role === "requester" && status.seq === 3)),
        claim("requester", 4, "funding_final", { previous: statusId("requester", 3) }),
        claim("provider", 2, "lightning_payment_pending", { id: "aa".padEnd(64, "a") }),
        claim("provider", 2, "lightning_paid", { id: "bb".padEnd(64, "b") }),
      ],
      assert: (view) => {
        expect(view.lanes.requester.integrity.kind).toBe("gap");
        expect(view.lanes.provider.integrity.kind).toBe("fork");
        expect(view.integrity).toBe("unknown_fork");
      },
    },
    {
      name: "broken previous chain: contiguous seqs with a wrong reference freeze the lane",
      statuses: [
        claim("provider", 0, "accepted"),
        claim("provider", 1, "lock_terms_ready", { previous: "0".repeat(64) }),
      ],
      assert: (view) => {
        expect(view.lanes.provider.integrity).toEqual({
          kind: "previous_mismatch",
          atSeq: 1,
        });
        expect(view.integrity).toBe("unknown_chain_break");
        expect(view.lastValidSwpState).toBe("accepted");
      },
    },
    {
      name: "funding_observed claimed with zero evidence is an unproven claim, not progress",
      statuses: happyPathClaims("submarine", "funding_observed"),
      overrides: { evidence: [] },
      assert: (view) => {
        expect(view.lastValidSwpState).toBe("requester_funding_broadcast");
        const unproven = view.retained.find((r) => r.claim.swpState === "funding_observed")!;
        expect(unproven.disposition.kind).toBe("unproven");
        expect(view.rung.proven).toBeNull();
      },
    },
    {
      name: "refund branch: prepared -> pending -> refunded advances with settled refund evidence",
      statuses: [
        ...funded,
        claim("requester", 5, "refund_prepared"),
        claim("requester", 6, "refund_pending"),
        claim("requester", 7, "refunded"),
      ],
      overrides: {
        evidence: [
          ...FULL_EVIDENCE,
          evidence({ class: "refund", authority: "bitcoin_adapter", rung: "settled", final: true }),
        ],
        closes: [close("requester", "refunded"), close("provider", "refunded")],
      },
      assert: (view) => {
        expect(view.lastValidSwpState).toBe("refunded");
        expect(view.displayKey).toBe("swap.status.display.refunded");
        expect(view.watchTerminal).toBe(true);
        expect(view.closes.closes[0]!.descriptor.exit).toBe("none_needed");
      },
    },
  ];

  for (const row of rows) {
    test(row.name, () => {
      const view = session(row.statuses, row.overrides);
      row.assert(view);
      // Every row converges under a different arrival order.
      const reordered = session(shuffled(row.statuses, 99), row.overrides);
      expect(reordered).toEqual(view);
    });
  }

  test("the table covers the six mandated adversarial families", () => {
    const names = rows.map((row) => row.name).join(" ");
    for (const family of [
      "gap",
      "fork",
      "duplicate",
      "out-of-order",
      "expiry-crossing",
      "unresolved",
    ]) {
      expect(names).toContain(family);
    }
    expect(rows.length).toBeGreaterThanOrEqual(12);
  });
});
