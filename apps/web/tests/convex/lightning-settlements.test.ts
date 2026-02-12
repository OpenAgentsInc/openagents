import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  formatPaymentProofReference,
  ingestInvoiceLifecycleImpl,
  ingestSettlementImpl,
  listOwnerSettlementsImpl,
  listPaywallSettlementsImpl,
} from "../../convex/lightning/settlements";
import { makeInMemoryDb } from "./inMemoryDb";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const authedCtx = (db: any, subject = "owner-1") => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.some({ subject })),
  },
});

const mutationCtx = (db: any) => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.none()),
  },
});

describe("convex/lightning settlements", () => {
  it("maps payment proof references deterministically", () => {
    expect(formatPaymentProofReference("ABCD1234EF")).toBe("lightning_preimage:abcd1234ef");
    expect(formatPaymentProofReference("f".repeat(64))).toBe(`lightning_preimage:${"f".repeat(24)}`);
  });

  it("upserts invoice lifecycle with deterministic transition handling", async () => {
    const db = makeInMemoryDb();
    const ctx = mutationCtx(db);
    const previousSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      const opened = await run(
        ingestInvoiceLifecycleImpl(ctx, {
          secret: "ops-secret",
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          amountMsats: 2_100,
          status: "open",
          requestId: "req_open",
        }),
      );
      expect(opened.changed).toBe(true);
      expect(opened.invoice.status).toBe("open");

      const canceled = await run(
        ingestInvoiceLifecycleImpl(ctx, {
          secret: "ops-secret",
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          amountMsats: 2_100,
          status: "canceled",
          requestId: "req_cancel",
        }),
      );
      expect(canceled.invoice.status).toBe("canceled");

      const settled = await run(
        ingestInvoiceLifecycleImpl(ctx, {
          secret: "ops-secret",
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          amountMsats: 2_100,
          status: "settled",
          paymentProofRef: "lightning_preimage:abc",
          requestId: "req_settle",
        }),
      );
      expect(settled.invoice.status).toBe("settled");
      expect(settled.invoice.paymentProofRef).toBe("lightning_preimage:abc");

      const reopened = await run(
        ingestInvoiceLifecycleImpl(ctx, {
          secret: "ops-secret",
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          amountMsats: 2_100,
          status: "open",
          requestId: "req_reopen",
        }),
      );
      expect(reopened.changed).toBe(false);
      expect(reopened.invoice.status).toBe("settled");

      const expiredInvoice = await run(
        ingestInvoiceLifecycleImpl(ctx, {
          secret: "ops-secret",
          invoiceId: "inv_2",
          paywallId: "pw_1",
          ownerId: "owner-1",
          amountMsats: 1_500,
          status: "expired",
        }),
      );
      expect(expiredInvoice.invoice.status).toBe("expired");
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = previousSecret;
    }
  });

  it("ingests settlements idempotently and writes ADR-0013 payment proof metadata", async () => {
    const db = makeInMemoryDb();
    const ctx = mutationCtx(db);
    const previousSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      const first = await run(
        ingestSettlementImpl(ctx, {
          secret: "ops-secret",
          settlementId: "set_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          invoiceId: "inv_set_1",
          amountMsats: 3_300,
          paymentHash: "hash_1",
          paymentProofType: "lightning_preimage",
          paymentProofValue: "a".repeat(64),
          requestId: "req_set_1",
          taskId: "task_1",
          routeId: "route_1",
          metadata: { source: "test" },
        }),
      );

      expect(first.existed).toBe(false);
      expect(first.settlement.paymentProofRef).toBe(`lightning_preimage:${"a".repeat(24)}`);
      expect(first.invoice?.status).toBe("settled");
      const firstReceipt = (first.settlement.metadata as any)?.receipt;
      expect(firstReceipt?.payment_proof?.type).toBe("lightning_preimage");
      expect(firstReceipt?.correlation?.task_id).toBe("task_1");
      expect(firstReceipt?.correlation?.request_id).toBe("req_set_1");

      const duplicate = await run(
        ingestSettlementImpl(ctx, {
          secret: "ops-secret",
          settlementId: "set_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          invoiceId: "inv_set_1",
          amountMsats: 3_300,
          paymentHash: "hash_1",
          paymentProofType: "lightning_preimage",
          paymentProofValue: "a".repeat(64),
          requestId: "req_set_1",
          taskId: "task_1",
          routeId: "route_1",
        }),
      );

      expect(duplicate.existed).toBe(true);
      expect(db.__tables.l402Settlements).toHaveLength(1);
      expect(db.__tables.l402Invoices).toHaveLength(1);
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = previousSecret;
    }
  });

  it("lists settlements with owner-scoped pagination and paywall filtering", async () => {
    const db = makeInMemoryDb();
    const mutCtx = mutationCtx(db);
    const queryCtxOwner = authedCtx(db, "owner-1");
    const queryCtxOther = authedCtx(db, "owner-2");
    const previousSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      await db.insert("l402Paywalls", {
        paywallId: "pw_1",
        ownerId: "owner-1",
        name: "pw1",
        status: "active",
        createdAtMs: 1,
        updatedAtMs: 1,
      });
      await db.insert("l402Paywalls", {
        paywallId: "pw_2",
        ownerId: "owner-1",
        name: "pw2",
        status: "active",
        createdAtMs: 1,
        updatedAtMs: 1,
      });
      await db.insert("l402Paywalls", {
        paywallId: "pw_other",
        ownerId: "owner-2",
        name: "pw3",
        status: "active",
        createdAtMs: 1,
        updatedAtMs: 1,
      });

      await run(
        ingestSettlementImpl(mutCtx, {
          secret: "ops-secret",
          settlementId: "set_own_1",
          paywallId: "pw_1",
          ownerId: "owner-1",
          amountMsats: 1_000,
          paymentProofType: "lightning_preimage",
          paymentProofValue: "b".repeat(64),
        }),
      );
      await run(
        ingestSettlementImpl(mutCtx, {
          secret: "ops-secret",
          settlementId: "set_own_2",
          paywallId: "pw_2",
          ownerId: "owner-1",
          amountMsats: 2_000,
          paymentProofType: "lightning_preimage",
          paymentProofValue: "c".repeat(64),
        }),
      );
      await run(
        ingestSettlementImpl(mutCtx, {
          secret: "ops-secret",
          settlementId: "set_other_1",
          paywallId: "pw_other",
          ownerId: "owner-2",
          amountMsats: 3_000,
          paymentProofType: "lightning_preimage",
          paymentProofValue: "d".repeat(64),
        }),
      );

      const ownerListPageOne = await run(
        listOwnerSettlementsImpl(queryCtxOwner, {
          limit: 1,
        }),
      );
      expect(ownerListPageOne.settlements).toHaveLength(1);
      expect(ownerListPageOne.settlements[0]?.ownerId).toBe("owner-1");
      expect(ownerListPageOne.nextCursor).toEqual(expect.any(Number));

      const ownerListPageTwo = await run(
        listOwnerSettlementsImpl(queryCtxOwner, {
          limit: 10,
          beforeCreatedAtMs: ownerListPageOne.nextCursor ?? undefined,
        }),
      );
      expect(ownerListPageTwo.settlements.every((row) => row.ownerId === "owner-1")).toBe(true);

      const paywallOnly = await run(
        listPaywallSettlementsImpl(queryCtxOwner, {
          paywallId: "pw_1",
          limit: 10,
        }),
      );
      expect(paywallOnly.settlements).toHaveLength(1);
      expect(paywallOnly.settlements[0]?.paywallId).toBe("pw_1");

      await expect(
        run(
          listPaywallSettlementsImpl(queryCtxOther, {
            paywallId: "pw_1",
          }),
        ),
      ).rejects.toThrow(/forbidden/);
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = previousSecret;
    }
  });
});
