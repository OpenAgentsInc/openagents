import { describe, expect, it } from "vitest";

import {
  settleLease,
  type LaborCloseoutReceipt,
  type ProviderLease,
} from "../src/provider-settlement.js";

const PROVIDER = "npub-provider-0000000000000000000000000000";
const REQUESTER = "npub-requester-1111111111111111111111111111";
const DIGEST = "a".repeat(64);

const lease = (overrides: Partial<ProviderLease> = {}): ProviderLease => ({
  job_id: "job-9f2c",
  lane: "validator_replay",
  provider: PROVIDER,
  price_msats: 1_000,
  expires_at: "2026-08-25T18:00:00.000Z",
  ...overrides,
});

/** A receipt that clears every gate, so each test can break exactly one. */
const closeout = (overrides: Partial<LaborCloseoutReceipt> = {}): LaborCloseoutReceipt => ({
  receiptRef: `lbr-closeout:job-9f2c:${DIGEST}`,
  requestId: "job-9f2c",
  requesterPubkey: REQUESTER,
  providerPubkey: PROVIDER,
  quotedAmountMsats: 1_000,
  verificationCommandRef: "verify:pnpm-run-check-fast",
  testRef: "test:run-4471-passed",
  platformCloseoutRef: "platform-closeout:2026-08-25/job-9f2c",
  digest: DIGEST,
  settled_at: "2026-08-25T17:30:00.000Z",
  ...overrides,
});

describe("provider settlement: payment follows a proof", () => {
  it("pays a job whose closeout receipt carries verification, evidence, and platform closeout", () => {
    const decision = settleLease(lease(), closeout());

    expect(decision.state).toBe("settled");
    expect(decision.earned_msats).toBe(1_000);
    expect(decision.refusal).toBeUndefined();
    expect(decision.receipt_ref).toBe(`lbr-closeout:job-9f2c:${DIGEST}`);
  });

  // The headline gate. Everything else in this file is a way of getting here
  // by a different road: without a receipt that proves the work, there is no
  // amount to be owed.
  it("earns nothing when no closeout receipt covers the job", () => {
    const decision = settleLease(lease());

    expect(decision.state).toBe("unsettled");
    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("no_closeout");
  });

  it("earns nothing when the receipt names no verification command", () => {
    const decision = settleLease(lease(), closeout({ verificationCommandRef: "" }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("work_not_verified");
  });

  it("earns nothing when the verification produced no evidence", () => {
    const decision = settleLease(lease(), closeout({ testRef: "   " }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("work_not_verified");
  });

  it("earns nothing when no platform closeout backs the receipt", () => {
    const decision = settleLease(lease(), closeout({ platformCloseoutRef: "" }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("no_settlement_authority");
  });

  it("earns nothing when the provider is also the requester", () => {
    const decision = settleLease(lease(), closeout({ requesterPubkey: PROVIDER }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("self_dealt");
  });

  it("earns nothing when the receipt closes out a different job", () => {
    const decision = settleLease(lease(), closeout({ requestId: "job-other" }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("closeout_job_mismatch");
  });

  it("earns nothing when the receipt credits a different provider", () => {
    const decision = settleLease(lease(), closeout({ providerPubkey: "npub-somebody-else" }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("closeout_provider_mismatch");
  });

  it("earns nothing when the receipt is not content-addressable", () => {
    const decision = settleLease(lease(), closeout({ digest: "not-a-sha256" }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("receipt_not_addressable");
  });

  it("earns nothing when the work closed out after the lease expired", () => {
    const decision = settleLease(lease(), closeout({ settled_at: "2026-08-25T18:00:01.000Z" }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("lease_expired");
  });

  it("cannot be inflated by a receipt that quotes more than the lease priced", () => {
    const decision = settleLease(lease(), closeout({ quotedAmountMsats: 5_000_000 }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("price_mismatch");
  });

  it("earns nothing on an unpriced lease even with a clean receipt", () => {
    const decision = settleLease(lease({ price_msats: 0 }), closeout({ quotedAmountMsats: 0 }));

    expect(decision.earned_msats).toBe(0);
    expect(decision.refusal).toBe("price_not_payable");
  });
});

describe("provider settlement: presence is never paid", () => {
  // The lesson VP-1's deleted loop and the do-not-build register both carry:
  // being online is not work. There is no argument to `settleLease` that
  // uptime, advertised capacity, or a live lease could reach, and this pins
  // that shape so a later refactor cannot quietly add one.
  it("takes only a lease and a receipt, so uptime has nowhere to enter", () => {
    expect(settleLease.length).toBe(2);
  });

  it("pays a provider that held a live lease all day but never closed out nothing", () => {
    const allDay = lease({ expires_at: "2026-12-31T23:59:59.000Z" });

    expect(settleLease(allDay).earned_msats).toBe(0);
    expect(settleLease(allDay).refusal).toBe("no_closeout");
  });
});

describe("provider settlement: accrual, not custody", () => {
  it("reports no connected payout rail and no custody on a settled job", () => {
    const decision = settleLease(lease(), closeout());

    expect(decision.payout_rail).toBe("not_connected");
    expect(decision.custody).toBe("none");
  });

  it("reports no connected payout rail and no custody on a refusal too", () => {
    const decision = settleLease(lease());

    expect(decision.payout_rail).toBe("not_connected");
    expect(decision.custody).toBe("none");
  });
});
