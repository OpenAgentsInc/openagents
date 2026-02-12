import { Context, Effect } from "effect";

import type {
  ControlPlanePaywall,
  CompileDiagnostic,
  ControlPlaneInvoiceRecord,
  ControlPlaneSettlementRecord,
  GatewayEventLevel,
  GatewayEventRecord,
  DeploymentIntentRecord,
  DeploymentIntentStatus,
  InvoiceLifecycleStatus,
  PaymentProofType,
} from "../contracts.js";

export type RecordDeploymentIntentInput = Readonly<{
  deploymentId?: string;
  paywallId?: string;
  ownerId?: string;
  configHash: string;
  status: DeploymentIntentStatus;
  diagnostics: ReadonlyArray<CompileDiagnostic>;
  metadata?: unknown;
  requestId?: string;
  imageDigest?: string;
  rolledBackFrom?: string;
  appliedAtMs?: number;
}>;

export type RecordInvoiceLifecycleInput = Readonly<{
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

export type RecordSettlementInput = Readonly<{
  settlementId: string;
  paywallId: string;
  ownerId: string;
  invoiceId?: string;
  amountMsats: number;
  paymentHash?: string;
  paymentProofType: PaymentProofType;
  paymentProofValue: string;
  requestId?: string;
  taskId?: string;
  routeId?: string;
  metadata?: unknown;
}>;

export type RecordSettlementResult = Readonly<{
  existed: boolean;
  settlement: ControlPlaneSettlementRecord;
  invoice?: ControlPlaneInvoiceRecord;
}>;

export type ControlPlaneApi = Readonly<{
  listPaywallsForCompile: () => Effect.Effect<ReadonlyArray<ControlPlanePaywall>, unknown>;
  recordDeploymentIntent: (
    input: RecordDeploymentIntentInput,
  ) => Effect.Effect<DeploymentIntentRecord, unknown>;
  recordGatewayEvent: (input: {
    readonly paywallId: string;
    readonly ownerId: string;
    readonly eventType: string;
    readonly level: GatewayEventLevel;
    readonly requestId?: string;
    readonly deploymentId?: string;
    readonly configHash?: string;
    readonly metadata?: unknown;
  }) => Effect.Effect<GatewayEventRecord, unknown>;
  recordInvoiceLifecycle: (
    input: RecordInvoiceLifecycleInput,
  ) => Effect.Effect<ControlPlaneInvoiceRecord, unknown>;
  recordSettlement: (input: RecordSettlementInput) => Effect.Effect<RecordSettlementResult, unknown>;
}>;

export class ControlPlaneService extends Context.Tag("@openagents/lightning-ops/ControlPlaneService")<
  ControlPlaneService,
  ControlPlaneApi
>() {}
