import { Context, Effect } from "effect";

import type {
  ControlPlanePaywall,
  CompileDiagnostic,
  GatewayEventLevel,
  GatewayEventRecord,
  DeploymentIntentRecord,
  DeploymentIntentStatus,
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
}>;

export class ControlPlaneService extends Context.Tag("@openagents/lightning-ops/ControlPlaneService")<
  ControlPlaneService,
  ControlPlaneApi
>() {}
