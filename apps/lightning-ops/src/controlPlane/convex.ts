import { Effect, Layer } from "effect";

import {
  decodeControlPlaneSnapshotResponse,
  decodeDeploymentIntentWriteResponse,
  decodeGatewayEventWriteResponse,
  type CompileDiagnostic,
  type ControlPlanePaywall,
  type DeploymentIntentRecord,
  type GatewayEventRecord,
} from "../contracts.js";
import { ControlPlaneDecodeError } from "../errors.js";
import { OpsRuntimeConfigService } from "../runtime/config.js";

import { ConvexTransportService } from "./convexTransport.js";
import { ControlPlaneService, type RecordDeploymentIntentInput } from "./service.js";

type ControlPlaneApi = Parameters<typeof ControlPlaneService.of>[0];

export const CONVEX_LIST_PAYWALLS_FN = "lightning/ops:listPaywallControlPlaneState";
export const CONVEX_RECORD_DEPLOYMENT_FN = "lightning/ops:recordGatewayCompileIntent";
export const CONVEX_RECORD_GATEWAY_EVENT_FN = "lightning/ops:recordGatewayDeploymentEvent";

const decodeSnapshot = (raw: unknown): Effect.Effect<ReadonlyArray<ControlPlanePaywall>, ControlPlaneDecodeError> =>
  decodeControlPlaneSnapshotResponse(raw).pipe(
    Effect.map((decoded) => decoded.paywalls),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeControlPlaneSnapshotResponse",
        reason: String(error),
      }),
    ),
  );

const decodeDeploymentWrite = (raw: unknown): Effect.Effect<DeploymentIntentRecord, ControlPlaneDecodeError> =>
  decodeDeploymentIntentWriteResponse(raw).pipe(
    Effect.map((decoded) => decoded.deployment),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeDeploymentIntentWriteResponse",
        reason: String(error),
      }),
    ),
  );

const decodeGatewayEventWrite = (raw: unknown): Effect.Effect<GatewayEventRecord, ControlPlaneDecodeError> =>
  decodeGatewayEventWriteResponse(raw).pipe(
    Effect.map((decoded) => decoded.event),
    Effect.mapError((error) =>
      ControlPlaneDecodeError.make({
        operation: "decodeGatewayEventWriteResponse",
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

    return ControlPlaneService.of({
      listPaywallsForCompile,
      recordDeploymentIntent,
      recordGatewayEvent,
    });
  }),
);
