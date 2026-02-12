import { Context, Effect } from "effect";

import type { CompiledApertureRule } from "../contracts.js";
import type { GatewayRuntimeError } from "../errors.js";

export type GatewayDeploymentSnapshot = Readonly<{
  deploymentId: string;
  configHash: string;
  imageDigest?: string;
}>;

export type GatewayApplyInput = Readonly<{
  requestId: string;
  deploymentId: string;
  configHash: string;
  apertureYaml: string;
}>;

export type GatewayHealthCheckResult = Readonly<{
  ok: true;
  statusCode: number;
}>;

export type GatewayChallengeCheckResult = Readonly<{
  ok: true;
  statusCode: number;
  authorizationHeader: string;
}>;

export type GatewayProxyCheckResult = Readonly<{
  ok: true;
  statusCode: number;
}>;

export type GatewayApi = Readonly<{
  getActiveDeployment: () => Effect.Effect<GatewayDeploymentSnapshot | null, GatewayRuntimeError>;
  applyConfig: (input: GatewayApplyInput) => Effect.Effect<GatewayDeploymentSnapshot, GatewayRuntimeError>;
  checkHealth: (input: {
    readonly requestId: string;
    readonly deploymentId: string;
  }) => Effect.Effect<GatewayHealthCheckResult, GatewayRuntimeError>;
  checkChallenge: (input: {
    readonly requestId: string;
    readonly deploymentId: string;
    readonly probeRoute: CompiledApertureRule;
  }) => Effect.Effect<GatewayChallengeCheckResult, GatewayRuntimeError>;
  checkProxy: (input: {
    readonly requestId: string;
    readonly deploymentId: string;
    readonly probeRoute: CompiledApertureRule;
    readonly authorizationHeader: string;
  }) => Effect.Effect<GatewayProxyCheckResult, GatewayRuntimeError>;
  rollbackTo: (input: {
    readonly requestId: string;
    readonly target: GatewayDeploymentSnapshot;
    readonly deploymentId: string;
  }) => Effect.Effect<GatewayDeploymentSnapshot, GatewayRuntimeError>;
}>;

export class GatewayService extends Context.Tag("@openagents/lightning-ops/GatewayService")<
  GatewayService,
  GatewayApi
>() {}
