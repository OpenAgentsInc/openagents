import { Context, Effect } from "effect";

import type {
  CompileDiagnostic,
  ControlPlaneCredentialRoleState,
  ControlPlaneInvoiceRecord,
  ControlPlaneOwnerSecurityControl,
  ControlPlaneSettlementRecord,
  ControlPlaneSecurityGlobal,
  ControlPlanePaywall,
  CredentialRole,
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

export type ControlPlaneSecurityState = Readonly<{
  global: ControlPlaneSecurityGlobal;
  ownerControls: ReadonlyArray<ControlPlaneOwnerSecurityControl>;
  credentialRoles: ReadonlyArray<ControlPlaneCredentialRoleState>;
}>;

export type SetGlobalPauseInput = Readonly<{
  active: boolean;
  reason?: string;
  updatedBy?: string;
}>;

export type SetOwnerKillSwitchInput = Readonly<{
  ownerId: string;
  active: boolean;
  reason?: string;
  updatedBy?: string;
}>;

export type CredentialRoleOperationInput = Readonly<{
  role: CredentialRole;
  fingerprint?: string;
  note?: string;
}>;

export type ControlPlaneApi = Readonly<{
  listPaywallsForCompile: () => Effect.Effect<ReadonlyArray<ControlPlanePaywall>, unknown>;
  getSecurityState: () => Effect.Effect<ControlPlaneSecurityState, unknown>;
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
  setGlobalPause: (input: SetGlobalPauseInput) => Effect.Effect<ControlPlaneSecurityGlobal, unknown>;
  setOwnerKillSwitch: (
    input: SetOwnerKillSwitchInput,
  ) => Effect.Effect<ControlPlaneOwnerSecurityControl, unknown>;
  rotateCredentialRole: (
    input: CredentialRoleOperationInput,
  ) => Effect.Effect<ControlPlaneCredentialRoleState, unknown>;
  activateCredentialRole: (
    input: CredentialRoleOperationInput,
  ) => Effect.Effect<ControlPlaneCredentialRoleState, unknown>;
  revokeCredentialRole: (
    input: Omit<CredentialRoleOperationInput, "fingerprint">,
  ) => Effect.Effect<ControlPlaneCredentialRoleState, unknown>;
}>;

export class ControlPlaneService extends Context.Tag("@openagents/lightning-ops/ControlPlaneService")<
  ControlPlaneService,
  ControlPlaneApi
>() {}
