import { Effect, Layer } from "effect";

import type {
  CompileDiagnostic,
  ControlPlaneInvoiceRecord,
  ControlPlanePaywall,
  ControlPlaneSettlementRecord,
  DeploymentIntentRecord,
  GatewayEventRecord,
  InvoiceLifecycleStatus,
} from "../contracts.js";
import { formatPaymentProofReference, normalizePreimageHex } from "../settlements/proof.js";

import {
  ControlPlaneService,
  type RecordDeploymentIntentInput,
  type RecordInvoiceLifecycleInput,
  type RecordSettlementInput,
  type RecordSettlementResult,
} from "./service.js";

type ControlPlaneApi = Parameters<typeof ControlPlaneService.of>[0];

export type InMemoryControlPlaneState = {
  paywalls: Array<ControlPlanePaywall>;
  deployments: Array<DeploymentIntentRecord>;
  events: Array<GatewayEventRecord>;
  invoices: Array<ControlPlaneInvoiceRecord>;
  settlements: Array<ControlPlaneSettlementRecord>;
  writeCalls: Array<RecordDeploymentIntentInput>;
  invoiceWriteCalls: Array<RecordInvoiceLifecycleInput>;
  settlementWriteCalls: Array<RecordSettlementInput>;
};

const invoiceRank: Record<InvoiceLifecycleStatus, number> = {
  open: 0,
  canceled: 1,
  expired: 1,
  settled: 2,
};

const chooseInvoiceStatus = (
  current: InvoiceLifecycleStatus,
  incoming: InvoiceLifecycleStatus,
): InvoiceLifecycleStatus => (invoiceRank[incoming] > invoiceRank[current] ? incoming : current);

const cloneDiagnostics = (diagnostics: ReadonlyArray<CompileDiagnostic>) =>
  diagnostics.map((diag) => ({ ...diag }));

const clonePaywall = (paywall: ControlPlanePaywall): ControlPlanePaywall => ({
  ...paywall,
  policy: {
    ...paywall.policy,
    allowedHosts: paywall.policy.allowedHosts ? [...paywall.policy.allowedHosts] : undefined,
    blockedHosts: paywall.policy.blockedHosts ? [...paywall.policy.blockedHosts] : undefined,
  },
  routes: paywall.routes.map((route) => ({ ...route })),
});

const cloneDeployment = (deployment: DeploymentIntentRecord): DeploymentIntentRecord => ({
  ...deployment,
  diagnostics: deployment.diagnostics,
});

const cloneInvoice = (invoice: ControlPlaneInvoiceRecord): ControlPlaneInvoiceRecord => ({
  ...invoice,
});

const cloneSettlement = (settlement: ControlPlaneSettlementRecord): ControlPlaneSettlementRecord => ({
  ...settlement,
  metadata: settlement.metadata,
});

const cloneRecordInvoiceInput = (input: RecordInvoiceLifecycleInput): RecordInvoiceLifecycleInput => ({
  ...input,
});

const cloneRecordSettlementInput = (input: RecordSettlementInput): RecordSettlementInput => ({
  ...input,
  metadata: input.metadata,
});

export const makeInMemoryControlPlaneHarness = (input?: {
  readonly paywalls?: ReadonlyArray<ControlPlanePaywall>;
}) => {
  const state: InMemoryControlPlaneState = {
    paywalls: [...(input?.paywalls ?? [])].map(clonePaywall),
    deployments: [],
    events: [],
    invoices: [],
    settlements: [],
    writeCalls: [],
    invoiceWriteCalls: [],
    settlementWriteCalls: [],
  };
  let nextDeployment = 1;
  let nextEvent = 1;

  const writeInvoiceLifecycle = (
    args: RecordInvoiceLifecycleInput,
    options?: { readonly trackWrite?: boolean },
  ): ControlPlaneInvoiceRecord => {
    if (options?.trackWrite !== false) {
      state.invoiceWriteCalls.push(cloneRecordInvoiceInput(args));
    }

    const now = Date.now();
    const existingIndex = state.invoices.findIndex((invoice) => invoice.invoiceId === args.invoiceId);

    if (existingIndex < 0) {
      const created: ControlPlaneInvoiceRecord = {
        invoiceId: args.invoiceId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        amountMsats: args.amountMsats,
        status: args.status,
        ...(args.paymentHash ? { paymentHash: args.paymentHash } : {}),
        ...(args.paymentRequest ? { paymentRequest: args.paymentRequest } : {}),
        ...(args.paymentProofRef ? { paymentProofRef: args.paymentProofRef } : {}),
        ...(args.requestId ? { requestId: args.requestId } : {}),
        createdAtMs: now,
        updatedAtMs: now,
        ...(args.status === "settled" ? { settledAtMs: args.settledAtMs ?? now } : {}),
      };
      state.invoices.push(created);
      return cloneInvoice(created);
    }

    const existing = state.invoices[existingIndex]!;
    const nextStatus = chooseInvoiceStatus(existing.status, args.status);
    const nextPaymentHash = existing.paymentHash ?? args.paymentHash;
    const nextPaymentRequest = existing.paymentRequest ?? args.paymentRequest;
    const nextPaymentProofRef = existing.paymentProofRef ?? args.paymentProofRef;
    const nextRequestId = existing.requestId ?? args.requestId;
    const nextSettledAtMs =
      nextStatus === "settled" ? (existing.settledAtMs ?? args.settledAtMs ?? now) : existing.settledAtMs;

    const updated: ControlPlaneInvoiceRecord = {
      ...existing,
      paywallId: args.paywallId,
      ownerId: args.ownerId,
      amountMsats: args.amountMsats,
      status: nextStatus,
      ...(nextPaymentHash ? { paymentHash: nextPaymentHash } : {}),
      ...(nextPaymentRequest ? { paymentRequest: nextPaymentRequest } : {}),
      ...(nextPaymentProofRef ? { paymentProofRef: nextPaymentProofRef } : {}),
      ...(nextRequestId ? { requestId: nextRequestId } : {}),
      ...(nextSettledAtMs !== undefined ? { settledAtMs: nextSettledAtMs } : {}),
      updatedAtMs: now,
    };

    state.invoices[existingIndex] = updated;
    return cloneInvoice(updated);
  };

  const listPaywallsForCompile = () => Effect.sync(() => state.paywalls.map(clonePaywall));

  const recordDeploymentIntent = (args: RecordDeploymentIntentInput) =>
    Effect.sync(() => {
      state.writeCalls.push({ ...args, diagnostics: cloneDiagnostics(args.diagnostics) });
      const deploymentId = args.deploymentId ?? `dep_mem_${nextDeployment++}`;
      const now = Date.now();
      const existingIndex = state.deployments.findIndex((deployment) => deployment.deploymentId === deploymentId);
      const next: DeploymentIntentRecord = {
        deploymentId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        configHash: args.configHash,
        imageDigest: args.imageDigest,
        status: args.status,
        diagnostics: {
          diagnostics: cloneDiagnostics(args.diagnostics),
          metadata: args.metadata,
          requestId: args.requestId,
        },
        appliedAtMs: args.appliedAtMs,
        rolledBackFrom: args.rolledBackFrom,
        createdAtMs: existingIndex >= 0 ? state.deployments[existingIndex]!.createdAtMs : now,
        updatedAtMs: now,
      };

      if (existingIndex >= 0) {
        state.deployments[existingIndex] = next;
      } else {
        state.deployments.push(next);
      }

      return cloneDeployment(next);
    });

  const recordGatewayEvent: ControlPlaneApi["recordGatewayEvent"] = (args) =>
    Effect.sync(() => {
      const event: GatewayEventRecord = {
        eventId: `evt_mem_${nextEvent++}`,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        eventType: args.eventType,
        level: args.level,
        ...(args.requestId ? { requestId: args.requestId } : {}),
        ...(args.metadata !== undefined ? { metadata: args.metadata } : {}),
        createdAtMs: Date.now(),
      };
      state.events.push(event);
      return { ...event };
    });

  const recordInvoiceLifecycle: ControlPlaneApi["recordInvoiceLifecycle"] = (args) =>
    Effect.sync(() => writeInvoiceLifecycle(args, { trackWrite: true }));

  const recordSettlement: ControlPlaneApi["recordSettlement"] = (args) =>
    Effect.sync(() => {
      state.settlementWriteCalls.push(cloneRecordSettlementInput(args));
      const existing = state.settlements.find((settlement) => settlement.settlementId === args.settlementId);

      const upsertLinkedInvoice = () =>
        args.invoiceId
          ? writeInvoiceLifecycle(
              {
                invoiceId: args.invoiceId,
                paywallId: args.paywallId,
                ownerId: args.ownerId,
                amountMsats: args.amountMsats,
                status: "settled",
                ...(args.paymentHash ? { paymentHash: args.paymentHash } : {}),
                ...(args.requestId ? { requestId: args.requestId } : {}),
              },
              { trackWrite: true },
            )
          : undefined;

      if (existing) {
        const invoice = upsertLinkedInvoice();
        const result: RecordSettlementResult = {
          existed: true,
          settlement: cloneSettlement(existing),
          ...(invoice ? { invoice } : {}),
        };
        return result;
      }

      if (args.paymentProofType !== "lightning_preimage") {
        throw new Error("invalid_payment_proof_type");
      }

      const preimageHex = normalizePreimageHex(args.paymentProofValue);
      if (!preimageHex) {
        throw new Error("invalid_preimage");
      }

      const settlement: ControlPlaneSettlementRecord = {
        settlementId: args.settlementId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        ...(args.invoiceId ? { invoiceId: args.invoiceId } : {}),
        amountMsats: args.amountMsats,
        paymentProofRef: formatPaymentProofReference(preimageHex),
        ...(args.requestId ? { requestId: args.requestId } : {}),
        ...(args.metadata !== undefined ? { metadata: args.metadata } : {}),
        createdAtMs: Date.now(),
      };
      state.settlements.push(settlement);

      const invoice = upsertLinkedInvoice();
      const result: RecordSettlementResult = {
        existed: false,
        settlement: cloneSettlement(settlement),
        ...(invoice ? { invoice } : {}),
      };
      return result;
    });

  const layer = Layer.succeed(
    ControlPlaneService,
    ControlPlaneService.of({
      listPaywallsForCompile,
      recordDeploymentIntent,
      recordGatewayEvent,
      recordInvoiceLifecycle,
      recordSettlement,
    }),
  );

  return {
    layer,
    state,
  };
};
