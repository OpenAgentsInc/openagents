import { Context, Effect } from "effect";

import type {
  ControlPlanePaywall,
  CompileDiagnostic,
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
}>;

export class ControlPlaneService extends Context.Tag("@openagents/lightning-ops/ControlPlaneService")<
  ControlPlaneService,
  ControlPlaneApi
>() {}
