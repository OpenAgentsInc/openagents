import { Effect, Ref } from "effect";
import { describe, expect, it } from "@effect/vitest";

import {
  CONVEX_RECORD_INVOICE_LIFECYCLE_FN,
  CONVEX_RECORD_SETTLEMENT_FN,
  ConvexControlPlaneLive,
} from "../src/controlPlane/convex.js";
import { makeConvexTransportTestLayer } from "../src/controlPlane/convexTransport.js";
import { ControlPlaneTransportError } from "../src/errors.js";
import { ingestSettlementEvents, type SettlementIngestEvent } from "../src/programs/ingestSettlements.js";
import { makeOpsRuntimeConfigTestLayer } from "../src/runtime/config.js";
import { formatPaymentProofReference } from "../src/settlements/proof.js";

describe("lightning-ops settlement convex pipeline", () => {
  it.effect("maps lifecycle events to Convex settlement mutations with deterministic proof refs", () =>
    Effect.gen(function* () {
      const callsRef = yield* Ref.make<Array<{ fn: string; args: Record<string, unknown> }>>([]);
      const seenSettlementIds = yield* Ref.make<Set<string>>(new Set());

      const transportLayer = makeConvexTransportTestLayer({
        query: (functionName) =>
          Effect.fail(
            ControlPlaneTransportError.make({
              operation: functionName,
              reason: "query_not_expected",
            }),
          ),
        mutation: (functionName, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { fn: functionName, args }]);

            if (functionName === CONVEX_RECORD_INVOICE_LIFECYCLE_FN) {
              return {
                ok: true,
                changed: true,
                invoice: {
                  invoiceId: String(args.invoiceId),
                  paywallId: String(args.paywallId),
                  ownerId: String(args.ownerId),
                  amountMsats: Number(args.amountMsats),
                  status: String(args.status),
                  ...(args.paymentHash ? { paymentHash: String(args.paymentHash) } : {}),
                  ...(args.paymentRequest ? { paymentRequest: String(args.paymentRequest) } : {}),
                  ...(args.paymentProofRef ? { paymentProofRef: String(args.paymentProofRef) } : {}),
                  ...(args.requestId ? { requestId: String(args.requestId) } : {}),
                  createdAtMs: 1_733_000_000_000,
                  updatedAtMs: 1_733_000_000_001,
                  ...(args.status === "settled" ? { settledAtMs: 1_733_000_000_001 } : {}),
                },
              };
            }

            if (functionName === CONVEX_RECORD_SETTLEMENT_FN) {
              const settlementId = String(args.settlementId);
              const preimage = String(args.paymentProofValue);
              const existed = yield* Ref.modify(seenSettlementIds, (seen) => {
                const next = new Set(seen);
                const has = next.has(settlementId);
                next.add(settlementId);
                return [has, next] as const;
              });

              return {
                ok: true,
                existed,
                settlement: {
                  settlementId,
                  paywallId: String(args.paywallId),
                  ownerId: String(args.ownerId),
                  ...(args.invoiceId ? { invoiceId: String(args.invoiceId) } : {}),
                  amountMsats: Number(args.amountMsats),
                  paymentProofRef: formatPaymentProofReference(preimage),
                  ...(args.requestId ? { requestId: String(args.requestId) } : {}),
                  ...(args.metadata !== undefined ? { metadata: args.metadata } : {}),
                  createdAtMs: 1_733_000_000_002,
                },
                invoice: {
                  invoiceId: String(args.invoiceId ?? "inv_missing"),
                  paywallId: String(args.paywallId),
                  ownerId: String(args.ownerId),
                  amountMsats: Number(args.amountMsats),
                  status: "settled",
                  ...(args.paymentHash ? { paymentHash: String(args.paymentHash) } : {}),
                  paymentProofRef: formatPaymentProofReference(preimage),
                  ...(args.requestId ? { requestId: String(args.requestId) } : {}),
                  createdAtMs: 1_733_000_000_000,
                  updatedAtMs: 1_733_000_000_002,
                  settledAtMs: 1_733_000_000_002,
                },
              };
            }

            return yield* Effect.fail(
              ControlPlaneTransportError.make({
                operation: functionName,
                reason: "mutation_not_expected",
              }),
            );
          }),
      });

      const events: ReadonlyArray<SettlementIngestEvent> = [
        {
          kind: "invoice_lifecycle",
          occurredAtMs: 1_735_000_000_000,
          invoiceId: "inv_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          amountMsats: 2_500,
          status: "open",
          requestId: "req_1",
        },
        {
          kind: "settlement",
          occurredAtMs: 1_735_000_000_100,
          settlementId: "set_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          invoiceId: "inv_1",
          amountMsats: 2_500,
          paymentHash: "hash_1",
          paymentProofType: "lightning_preimage",
          paymentProofValue: "b".repeat(64),
          requestId: "req_1",
          taskId: "task_1",
          routeId: "route_1",
        },
        {
          kind: "settlement",
          occurredAtMs: 1_735_000_000_101,
          settlementId: "set_1",
          paywallId: "pw_1",
          ownerId: "owner_1",
          invoiceId: "inv_1",
          amountMsats: 2_500,
          paymentHash: "hash_1",
          paymentProofType: "lightning_preimage",
          paymentProofValue: "b".repeat(64),
          requestId: "req_1",
          taskId: "task_1",
          routeId: "route_1",
        },
      ];

      const summary = yield* ingestSettlementEvents(events).pipe(
        Effect.provide(ConvexControlPlaneLive),
        Effect.provide(transportLayer),
        Effect.provide(
          makeOpsRuntimeConfigTestLayer({
            convexUrl: "https://example.convex.cloud",
            opsSecret: "ops-secret",
          }),
        ),
      );

      const calls = yield* Ref.get(callsRef);
      expect(summary.processed).toBe(3);
      expect(summary.settlements).toHaveLength(2);
      expect(summary.settlements.map((row) => row.existed)).toEqual([false, true]);
      expect(summary.correlationRefs[0]?.paymentProofRef).toBe(formatPaymentProofReference("b".repeat(64)));

      expect(calls).toHaveLength(3);
      expect(calls.map((call) => call.fn)).toEqual([
        CONVEX_RECORD_INVOICE_LIFECYCLE_FN,
        CONVEX_RECORD_SETTLEMENT_FN,
        CONVEX_RECORD_SETTLEMENT_FN,
      ]);
    }),
  );
});
