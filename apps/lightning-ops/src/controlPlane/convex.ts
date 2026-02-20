import { Effect, Layer } from "effect";

import {
  type CompileDiagnostic,
  type ControlPlaneCredentialRoleState,
  type ControlPlaneInvoiceRecord,
  type ControlPlaneOwnerSecurityControl,
  type ControlPlanePaywall,
  type ControlPlaneSecurityGlobal,
  type ControlPlaneSettlementRecord,
  type DeploymentIntentRecord,
  type GatewayEventRecord,
} from "../contracts.js";
import { ControlPlaneDecodeError } from "../errors.js";
import { OpsRuntimeConfigService } from "../runtime/config.js";

import { ConvexTransportService } from "./convexTransport.js";
import {
  decodeControlPlaneSecurityStateResponseFromAny,
  decodeControlPlaneSnapshotResponseFromAny,
  decodeDeploymentIntentWriteResponseFromAny,
  decodeGatewayEventWriteResponseFromAny,
  decodeInvoiceLifecycleWriteResponseFromAny,
  decodeSecurityCredentialRoleWriteResponseFromAny,
  decodeSecurityGlobalWriteResponseFromAny,
  decodeSecurityOwnerControlWriteResponseFromAny,
  decodeSettlementWriteResponseFromAny,
} from "./protoAdapters.js";
import { ControlPlaneService, type RecordDeploymentIntentInput } from "./service.js";

type ControlPlaneApi = Parameters<typeof ControlPlaneService.of>[0];

export const CONVEX_LIST_PAYWALLS_FN = "lightning/ops:listPaywallControlPlaneState";
export const CONVEX_RECORD_DEPLOYMENT_FN = "lightning/ops:recordGatewayCompileIntent";
export const CONVEX_RECORD_GATEWAY_EVENT_FN = "lightning/ops:recordGatewayDeploymentEvent";
export const CONVEX_RECORD_INVOICE_LIFECYCLE_FN = "lightning/settlements:ingestInvoiceLifecycle";
export const CONVEX_RECORD_SETTLEMENT_FN = "lightning/settlements:ingestSettlement";
export const CONVEX_GET_SECURITY_STATE_FN = "lightning/security:getControlPlaneSecurityState";
export const CONVEX_SET_GLOBAL_PAUSE_FN = "lightning/security:setGlobalPause";
export const CONVEX_SET_OWNER_KILL_SWITCH_FN = "lightning/security:setOwnerKillSwitch";
export const CONVEX_ROTATE_CREDENTIAL_ROLE_FN = "lightning/security:rotateCredentialRole";
export const CONVEX_ACTIVATE_CREDENTIAL_ROLE_FN = "lightning/security:activateCredentialRole";
export const CONVEX_REVOKE_CREDENTIAL_ROLE_FN = "lightning/security:revokeCredentialRole";

const decodeSnapshot = (raw: unknown): Effect.Effect<ReadonlyArray<ControlPlanePaywall>, ControlPlaneDecodeError> =>
  decodeControlPlaneSnapshotResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.paywalls),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeControlPlaneSnapshotResponse",
        reason: String(error),
      }),
    ),
  );

const decodeDeploymentWrite = (raw: unknown): Effect.Effect<DeploymentIntentRecord, ControlPlaneDecodeError> =>
  decodeDeploymentIntentWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.deployment),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeDeploymentIntentWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeGatewayEventWrite = (raw: unknown): Effect.Effect<GatewayEventRecord, ControlPlaneDecodeError> =>
  decodeGatewayEventWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.event),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeGatewayEventWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeInvoiceLifecycleWrite = (
  raw: unknown,
): Effect.Effect<ControlPlaneInvoiceRecord, ControlPlaneDecodeError> =>
  decodeInvoiceLifecycleWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.invoice),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeInvoiceLifecycleWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeSettlementWrite = (
  raw: unknown,
): Effect.Effect<{
  existed: boolean;
  settlement: ControlPlaneSettlementRecord;
  invoice?: ControlPlaneInvoiceRecord;
}, ControlPlaneDecodeError> =>
  decodeSettlementWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => {
      const result = {
        existed: decoded.existed,
        settlement: decoded.settlement,
        ...(decoded.invoice ? { invoice: decoded.invoice } : {}),
      };
      return result;
    }),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeSettlementWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeControlPlaneSecurityState = (
  raw: unknown,
): Effect.Effect<{
  global: ControlPlaneSecurityGlobal;
  ownerControls: ReadonlyArray<ControlPlaneOwnerSecurityControl>;
  credentialRoles: ReadonlyArray<ControlPlaneCredentialRoleState>;
}, ControlPlaneDecodeError> =>
  decodeControlPlaneSecurityStateResponseFromAny(raw).pipe(
    Effect.map((decoded) => ({
      global: decoded.global,
      ownerControls: decoded.ownerControls,
      credentialRoles: decoded.credentialRoles,
    })),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeControlPlaneSecurityStateResponse",
        reason: String(error),
      }),
    ),
  );

const decodeSecurityGlobalWrite = (
  raw: unknown,
): Effect.Effect<ControlPlaneSecurityGlobal, ControlPlaneDecodeError> =>
  decodeSecurityGlobalWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.global),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeSecurityGlobalWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeOwnerControlWrite = (
  raw: unknown,
): Effect.Effect<ControlPlaneOwnerSecurityControl, ControlPlaneDecodeError> =>
  decodeSecurityOwnerControlWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.ownerControl),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeSecurityOwnerControlWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeCredentialRoleWrite = (
  raw: unknown,
): Effect.Effect<ControlPlaneCredentialRoleState, ControlPlaneDecodeError> =>
  decodeSecurityCredentialRoleWriteResponseFromAny(raw).pipe(
    Effect.map((decoded) => decoded.role),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeSecurityCredentialRoleWriteResponse",
        reason: String(error),
      }),
    ),
  );

const toDiagnosticJson = (diagnostics: ReadonlyArray<CompileDiagnostic>) =>
  diagnostics.map((diag) => ({
    code: diag.code,
    severity: diag.severity,
    message: diag.message,
    paywallId: diag.paywallId,
    routeId: diag.routeId,
    relatedRouteId: diag.relatedRouteId,
    details: diag.details,
  }));

const makeRecordArgs = (secret: string, input: RecordDeploymentIntentInput): Record<string, unknown> => ({
  secret,
  deploymentId: input.deploymentId,
  paywallId: input.paywallId,
  ownerId: input.ownerId,
  configHash: input.configHash,
  status: input.status,
  diagnostics: toDiagnosticJson(input.diagnostics),
  metadata: input.metadata,
  requestId: input.requestId,
  imageDigest: input.imageDigest,
  rolledBackFrom: input.rolledBackFrom,
  appliedAtMs: input.appliedAtMs,
});

const makeInvoiceLifecycleArgs = (
  secret: string,
  input: {
    invoiceId: string;
    paywallId: string;
    ownerId: string;
    amountMsats: number;
    status: "open" | "settled" | "canceled" | "expired";
    paymentHash?: string;
    paymentRequest?: string;
    paymentProofRef?: string;
    requestId?: string;
    settledAtMs?: number;
  },
): Record<string, unknown> => ({
  secret,
  invoiceId: input.invoiceId,
  paywallId: input.paywallId,
  ownerId: input.ownerId,
  amountMsats: input.amountMsats,
  status: input.status,
  paymentHash: input.paymentHash,
  paymentRequest: input.paymentRequest,
  paymentProofRef: input.paymentProofRef,
  requestId: input.requestId,
  settledAtMs: input.settledAtMs,
});

const makeSettlementArgs = (
  secret: string,
  input: {
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
  },
): Record<string, unknown> => ({
  secret,
  settlementId: input.settlementId,
  paywallId: input.paywallId,
  ownerId: input.ownerId,
  invoiceId: input.invoiceId,
  amountMsats: input.amountMsats,
  paymentHash: input.paymentHash,
  paymentProofType: input.paymentProofType,
  paymentProofValue: input.paymentProofValue,
  requestId: input.requestId,
  taskId: input.taskId,
  routeId: input.routeId,
  metadata: input.metadata,
});

const makeGlobalPauseArgs = (
  secret: string,
  input: {
    active: boolean;
    reason?: string;
    updatedBy?: string;
  },
): Record<string, unknown> => ({
  secret,
  active: input.active,
  reason: input.reason,
  updatedBy: input.updatedBy,
});

const makeOwnerKillSwitchArgs = (
  secret: string,
  input: {
    ownerId: string;
    active: boolean;
    reason?: string;
    updatedBy?: string;
  },
): Record<string, unknown> => ({
  secret,
  ownerId: input.ownerId,
  active: input.active,
  reason: input.reason,
  updatedBy: input.updatedBy,
});

const makeCredentialRoleArgs = (
  secret: string,
  input: {
    role: "gateway_invoice" | "settlement_read" | "operator_admin";
    fingerprint?: string;
    note?: string;
  },
): Record<string, unknown> => ({
  secret,
  role: input.role,
  fingerprint: input.fingerprint,
  note: input.note,
});

const makeRevokeCredentialRoleArgs = (
  secret: string,
  input: {
    role: "gateway_invoice" | "settlement_read" | "operator_admin";
    note?: string;
  },
): Record<string, unknown> => ({
  secret,
  role: input.role,
  note: input.note,
});

export const ConvexControlPlaneLive = Layer.effect(
  ControlPlaneService,
  Effect.gen(function* () {
    const config = yield* OpsRuntimeConfigService;
    const transport = yield* ConvexTransportService;

    const listPaywallsForCompile = () =>
      transport
        .query(CONVEX_LIST_PAYWALLS_FN, {
          secret: config.opsSecret,
          statuses: ["active", "paused"],
        })
        .pipe(Effect.flatMap(decodeSnapshot));

    const getSecurityState: ControlPlaneApi["getSecurityState"] = () =>
      transport
        .query(CONVEX_GET_SECURITY_STATE_FN, {
          secret: config.opsSecret,
        })
        .pipe(Effect.flatMap(decodeControlPlaneSecurityState));

    const recordDeploymentIntent = (input: RecordDeploymentIntentInput) =>
      transport
        .mutation(CONVEX_RECORD_DEPLOYMENT_FN, makeRecordArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeDeploymentWrite));

    const recordGatewayEvent: ControlPlaneApi["recordGatewayEvent"] = (input) =>
      transport
        .mutation(CONVEX_RECORD_GATEWAY_EVENT_FN, {
          secret: config.opsSecret,
          paywallId: input.paywallId,
          ownerId: input.ownerId,
          eventType: input.eventType,
          level: input.level,
          requestId: input.requestId,
          deploymentId: input.deploymentId,
          configHash: input.configHash,
          executionPath: "hosted-node",
          metadata: input.metadata,
        })
        .pipe(Effect.flatMap(decodeGatewayEventWrite));

    const recordInvoiceLifecycle: ControlPlaneApi["recordInvoiceLifecycle"] = (input) =>
      transport
        .mutation(CONVEX_RECORD_INVOICE_LIFECYCLE_FN, makeInvoiceLifecycleArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeInvoiceLifecycleWrite));

    const recordSettlement: ControlPlaneApi["recordSettlement"] = (input) =>
      transport
        .mutation(CONVEX_RECORD_SETTLEMENT_FN, makeSettlementArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeSettlementWrite));

    const setGlobalPause: ControlPlaneApi["setGlobalPause"] = (input) =>
      transport
        .mutation(CONVEX_SET_GLOBAL_PAUSE_FN, makeGlobalPauseArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeSecurityGlobalWrite));

    const setOwnerKillSwitch: ControlPlaneApi["setOwnerKillSwitch"] = (input) =>
      transport
        .mutation(CONVEX_SET_OWNER_KILL_SWITCH_FN, makeOwnerKillSwitchArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeOwnerControlWrite));

    const rotateCredentialRole: ControlPlaneApi["rotateCredentialRole"] = (input) =>
      transport
        .mutation(CONVEX_ROTATE_CREDENTIAL_ROLE_FN, makeCredentialRoleArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeCredentialRoleWrite));

    const activateCredentialRole: ControlPlaneApi["activateCredentialRole"] = (input) =>
      transport
        .mutation(CONVEX_ACTIVATE_CREDENTIAL_ROLE_FN, makeCredentialRoleArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeCredentialRoleWrite));

    const revokeCredentialRole: ControlPlaneApi["revokeCredentialRole"] = (input) =>
      transport
        .mutation(CONVEX_REVOKE_CREDENTIAL_ROLE_FN, makeRevokeCredentialRoleArgs(config.opsSecret, input))
        .pipe(Effect.flatMap(decodeCredentialRoleWrite));

    return ControlPlaneService.of({
      listPaywallsForCompile,
      getSecurityState,
      recordDeploymentIntent,
      recordGatewayEvent,
      recordInvoiceLifecycle,
      recordSettlement,
      setGlobalPause,
      setOwnerKillSwitch,
      rotateCredentialRole,
      activateCredentialRole,
      revokeCredentialRole,
    });
  }),
);
