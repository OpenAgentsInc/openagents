import { Effect } from "effect";

import type { InvoiceLifecycleStatus } from "../contracts.js";

import { ControlPlaneService } from "../controlPlane/service.js";
import type { RecordInvoiceLifecycleInput, RecordSettlementInput } from "../controlPlane/service.js";
import { normalizePreimageHex } from "../settlements/proof.js";

export type InvoiceLifecycleEvent = Readonly<{
  kind: "invoice_lifecycle";
  occurredAtMs: number;
  invoiceId: string;
  paywallId: string;
  ownerId: string;
  amountMsats: number;
  status: InvoiceLifecycleStatus;
  paymentHash?: string;
  paymentRequest?: string;
  paymentProofRef?: string;
  requestId?: string;
  settledAtMs?: number;
}>;

export type SettlementEvent = Readonly<{
  kind: "settlement";
  occurredAtMs: number;
  settlementId: string;
  paywallId: string;
  ownerId: string;
  invoiceId?: string;
  amountMsats: number;
  paymentHash?: string;
  paymentProofType: "lightning_preimage";
  paymentProofValue: string;
  requestId?: string;
  taskId?: string;
  routeId?: string;
  metadata?: unknown;
}>;

export type SettlementIngestEvent = InvoiceLifecycleEvent | SettlementEvent;

export type SettlementIngestSummary = Readonly<{
  processed: number;
  invoiceTransitions: ReadonlyArray<{
    invoiceId: string;
    paywallId: string;
    ownerId: string;
    status: InvoiceLifecycleStatus;
    requestId?: string;
    updatedAtMs: number;
  }>;
  settlements: ReadonlyArray<{
    settlementId: string;
    paywallId: string;
    ownerId: string;
    invoiceId?: string;
    amountMsats: number;
    requestId?: string;
    paymentProofRef: string;
    existed: boolean;
  }>;
  correlationRefs: ReadonlyArray<{
    settlementId: string;
    paymentProofRef: string;
    requestId?: string;
    taskId?: string;
    routeId?: string;
  }>;
}>;

const sortEventsByOccurrence = (events: ReadonlyArray<SettlementIngestEvent>): Array<SettlementIngestEvent> =>
  [...events].sort((a, b) => {
    if (a.occurredAtMs !== b.occurredAtMs) return a.occurredAtMs - b.occurredAtMs;
    if (a.kind === b.kind) return 0;
    return a.kind === "invoice_lifecycle" ? -1 : 1;
  });

export const mapInvoiceLifecycleEvent = (event: InvoiceLifecycleEvent): RecordInvoiceLifecycleInput => ({
  invoiceId: event.invoiceId,
  paywallId: event.paywallId,
  ownerId: event.ownerId,
  amountMsats: event.amountMsats,
  status: event.status,
  ...(event.paymentHash ? { paymentHash: event.paymentHash } : {}),
  ...(event.paymentRequest ? { paymentRequest: event.paymentRequest } : {}),
  ...(event.paymentProofRef ? { paymentProofRef: event.paymentProofRef } : {}),
  ...(event.requestId ? { requestId: event.requestId } : {}),
  ...(event.settledAtMs !== undefined ? { settledAtMs: event.settledAtMs } : {}),
});

export const mapSettlementEvent = (event: SettlementEvent): RecordSettlementInput => {
  const normalizedPreimage = normalizePreimageHex(event.paymentProofValue);
  if (!normalizedPreimage) {
    throw new Error("invalid_preimage");
  }

  return {
    settlementId: event.settlementId,
    paywallId: event.paywallId,
    ownerId: event.ownerId,
    ...(event.invoiceId ? { invoiceId: event.invoiceId } : {}),
    amountMsats: event.amountMsats,
    ...(event.paymentHash ? { paymentHash: event.paymentHash } : {}),
    paymentProofType: event.paymentProofType,
    paymentProofValue: normalizedPreimage,
    ...(event.requestId ? { requestId: event.requestId } : {}),
    ...(event.taskId ? { taskId: event.taskId } : {}),
    ...(event.routeId ? { routeId: event.routeId } : {}),
    ...(event.metadata !== undefined ? { metadata: event.metadata } : {}),
  };
};

type IngestedResult =
  | {
      kind: "invoice_lifecycle";
      invoice: {
        invoiceId: string;
        paywallId: string;
        ownerId: string;
        status: InvoiceLifecycleStatus;
        requestId?: string;
        updatedAtMs: number;
      };
    }
  | {
      kind: "settlement";
      settlement: {
        settlementId: string;
        paywallId: string;
        ownerId: string;
        invoiceId?: string;
        amountMsats: number;
        requestId?: string;
        paymentProofRef: string;
      };
      existed: boolean;
      taskId?: string;
      routeId?: string;
    };

export const ingestSettlementEvents = (events: ReadonlyArray<SettlementIngestEvent>) =>
  Effect.gen(function* () {
    const controlPlane = yield* ControlPlaneService;
    const ordered = sortEventsByOccurrence(events);

    const ingested = yield* Effect.forEach(ordered, (event) => {
      if (event.kind === "invoice_lifecycle") {
        return controlPlane.recordInvoiceLifecycle(mapInvoiceLifecycleEvent(event)).pipe(
          Effect.map(
            (invoice): IngestedResult => ({
              kind: "invoice_lifecycle",
              invoice: {
                invoiceId: invoice.invoiceId,
                paywallId: invoice.paywallId,
                ownerId: invoice.ownerId,
                status: invoice.status,
                ...(invoice.requestId ? { requestId: invoice.requestId } : {}),
                updatedAtMs: invoice.updatedAtMs,
              },
            }),
          ),
        );
      }

      return controlPlane.recordSettlement(mapSettlementEvent(event)).pipe(
        Effect.map(
          (settlement): IngestedResult => ({
            kind: "settlement",
            settlement: {
              settlementId: settlement.settlement.settlementId,
              paywallId: settlement.settlement.paywallId,
              ownerId: settlement.settlement.ownerId,
              ...(settlement.settlement.invoiceId ? { invoiceId: settlement.settlement.invoiceId } : {}),
              amountMsats: settlement.settlement.amountMsats,
              ...(settlement.settlement.requestId ? { requestId: settlement.settlement.requestId } : {}),
              paymentProofRef: settlement.settlement.paymentProofRef,
            },
            existed: settlement.existed,
            ...(event.taskId ? { taskId: event.taskId } : {}),
            ...(event.routeId ? { routeId: event.routeId } : {}),
          }),
        ),
      );
    });

    const invoiceTransitions: Array<SettlementIngestSummary["invoiceTransitions"][number]> = [];
    const settlements: Array<SettlementIngestSummary["settlements"][number]> = [];
    const correlationRefs: Array<SettlementIngestSummary["correlationRefs"][number]> = [];

    for (const result of ingested) {
      if (result.kind === "invoice_lifecycle") {
        invoiceTransitions.push(result.invoice);
        continue;
      }

      settlements.push({
        settlementId: result.settlement.settlementId,
        paywallId: result.settlement.paywallId,
        ownerId: result.settlement.ownerId,
        ...(result.settlement.invoiceId ? { invoiceId: result.settlement.invoiceId } : {}),
        amountMsats: result.settlement.amountMsats,
        ...(result.settlement.requestId ? { requestId: result.settlement.requestId } : {}),
        paymentProofRef: result.settlement.paymentProofRef,
        existed: result.existed,
      });
      correlationRefs.push({
        settlementId: result.settlement.settlementId,
        paymentProofRef: result.settlement.paymentProofRef,
        ...(result.settlement.requestId ? { requestId: result.settlement.requestId } : {}),
        ...(result.taskId ? { taskId: result.taskId } : {}),
        ...(result.routeId ? { routeId: result.routeId } : {}),
      });
    }

    const summary: SettlementIngestSummary = {
      processed: ordered.length,
      invoiceTransitions,
      settlements,
      correlationRefs,
    };

    return summary;
  });
