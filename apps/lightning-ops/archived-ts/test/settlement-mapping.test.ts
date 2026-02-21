import { describe, expect, it } from "@effect/vitest";

import {
  mapInvoiceLifecycleEvent,
  mapSettlementEvent,
  type InvoiceLifecycleEvent,
  type SettlementEvent,
} from "../src/programs/ingestSettlements.js";
import { formatPaymentProofReference, normalizePreimageHex } from "../src/settlements/proof.js";

describe("lightning-ops settlement mapping", () => {
  it("normalizes preimages and maps settlement events deterministically", () => {
    const event: SettlementEvent = {
      kind: "settlement",
      occurredAtMs: 1_735_000_000_000,
      settlementId: "set_1",
      paywallId: "pw_1",
      ownerId: "owner_1",
      invoiceId: "inv_1",
      amountMsats: 2_500,
      paymentHash: "hash_1",
      paymentProofType: "lightning_preimage",
      paymentProofValue: "AA".repeat(32),
      requestId: "req_1",
      taskId: "task_1",
      routeId: "route_1",
    };

    const mapped = mapSettlementEvent(event);
    expect(mapped.paymentProofValue).toBe("aa".repeat(32));
    expect(formatPaymentProofReference(mapped.paymentProofValue)).toBe(`lightning_preimage:${"aa".repeat(12)}`);
  });

  it("maps invoice lifecycle events without mutation", () => {
    const event: InvoiceLifecycleEvent = {
      kind: "invoice_lifecycle",
      occurredAtMs: 1_735_000_000_000,
      invoiceId: "inv_2",
      paywallId: "pw_2",
      ownerId: "owner_2",
      amountMsats: 1_800,
      status: "open",
      requestId: "req_2",
    };

    const mapped = mapInvoiceLifecycleEvent(event);
    expect(mapped).toMatchObject({
      invoiceId: "inv_2",
      paywallId: "pw_2",
      ownerId: "owner_2",
      amountMsats: 1_800,
      status: "open",
      requestId: "req_2",
    });
  });

  it("rejects invalid preimages deterministically", () => {
    expect(normalizePreimageHex("xyz-not-hex")).toBeNull();

    const invalidEvent: SettlementEvent = {
      kind: "settlement",
      occurredAtMs: 1_735_000_000_000,
      settlementId: "set_bad",
      paywallId: "pw_1",
      ownerId: "owner_1",
      amountMsats: 2_500,
      paymentProofType: "lightning_preimage",
      paymentProofValue: "zz-not-hex",
    };

    expect(() => mapSettlementEvent(invalidEvent)).toThrow(/invalid_preimage/);
  });
});
