import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { makeInMemoryControlPlaneHarness } from "../src/controlPlane/inMemory.js";
import { ingestSettlementEvents, type SettlementIngestEvent } from "../src/programs/ingestSettlements.js";

describe("lightning-ops settlement idempotency", () => {
  it.effect("handles duplicate and reordered lifecycle streams without duplicate rows", () =>
    Effect.gen(function* () {
      const harness = makeInMemoryControlPlaneHarness();
      const events: ReadonlyArray<SettlementIngestEvent> = [
        {
          kind: "settlement",
          occurredAtMs: 3_000,
          settlementId: "set_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          invoiceId: "inv_1",
          amountMsats: 2_100,
          paymentHash: "hash_1",
          paymentProofType: "lightning_preimage",
          paymentProofValue: "a".repeat(64),
          requestId: "req_1",
          taskId: "task_1",
          routeId: "route_1",
        },
        {
          kind: "settlement",
          occurredAtMs: 3_100,
          settlementId: "set_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          invoiceId: "inv_1",
          amountMsats: 2_100,
          paymentHash: "hash_1",
          paymentProofType: "lightning_preimage",
          paymentProofValue: "a".repeat(64),
          requestId: "req_1",
          taskId: "task_1",
          routeId: "route_1",
        },
        {
          kind: "invoice_lifecycle",
          occurredAtMs: 4_000,
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          amountMsats: 2_100,
          status: "open",
          requestId: "req_reordered_open",
        },
        {
          kind: "invoice_lifecycle",
          occurredAtMs: 5_000,
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          amountMsats: 2_100,
          status: "canceled",
          requestId: "req_reordered_canceled",
        },
      ];

      const summary = yield* ingestSettlementEvents(events).pipe(Effect.provide(harness.layer));

      expect(summary.processed).toBe(4);
      expect(summary.settlements).toHaveLength(2);
      expect(summary.settlements.map((row) => row.existed)).toEqual([false, true]);

      expect(harness.state.settlements).toHaveLength(1);
      expect(harness.state.invoices).toHaveLength(1);
      expect(harness.state.invoices[0]?.status).toBe("settled");
      expect(harness.state.invoices[0]?.paymentHash).toBe("hash_1");
    }),
  );
});
