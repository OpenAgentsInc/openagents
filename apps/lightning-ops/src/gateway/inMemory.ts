import { Effect, Layer } from "effect";

import { GatewayRuntimeError } from "../errors.js";

import { GatewayService, type GatewayDeploymentSnapshot } from "./service.js";

export type GatewayFailureStage =
  | "active_lookup"
  | "apply"
  | "health"
  | "challenge"
  | "proxy"
  | "rollback";

export type InMemoryGatewayState = {
  activeDeployment: GatewayDeploymentSnapshot | null;
  appliedConfigHashes: Array<string>;
  rollbackTargets: Array<string>;
  calls: Array<{ method: string; requestId: string; deploymentId?: string }>;
};

const failStage = (stage: GatewayFailureStage, reason?: string) =>
  GatewayRuntimeError.make({
    stage,
    reason: reason ?? `${stage}_failed`,
  });

const digestForConfigHash = (configHash: string): string =>
  `sha256:${configHash.replace(/^cfg_/, "").slice(0, 32)}`;

export const makeInMemoryGatewayHarness = (input?: {
  readonly initialDeployment?: GatewayDeploymentSnapshot;
  readonly failAt?: GatewayFailureStage;
  readonly failStages?: ReadonlyArray<GatewayFailureStage>;
  readonly challengeAuthorizationHeader?: string;
}) => {
  const state: InMemoryGatewayState = {
    activeDeployment: input?.initialDeployment ?? null,
    appliedConfigHashes: [],
    rollbackTargets: [],
    calls: [],
  };

  const challengeAuthorizationHeader = input?.challengeAuthorizationHeader ?? "L402 smoke-proof";
  const failStages = new Set<GatewayFailureStage>([
    ...(input?.failStages ?? []),
    ...(input?.failAt ? [input.failAt] : []),
  ]);

  const assertNotFailing = (stage: GatewayFailureStage) => {
    if (failStages.has(stage)) {
      return Effect.fail(failStage(stage));
    }
    return Effect.void;
  };

  const layer = Layer.succeed(
    GatewayService,
    GatewayService.of({
      getActiveDeployment: () =>
        Effect.gen(function* () {
          yield* assertNotFailing("active_lookup");
          state.calls.push({ method: "getActiveDeployment", requestId: "n/a" });
          return state.activeDeployment ? { ...state.activeDeployment } : null;
        }),

      applyConfig: (args) =>
        Effect.gen(function* () {
          yield* assertNotFailing("apply");
          state.calls.push({ method: "applyConfig", requestId: args.requestId, deploymentId: args.deploymentId });

          const next: GatewayDeploymentSnapshot = {
            deploymentId: args.deploymentId,
            configHash: args.configHash,
            imageDigest: digestForConfigHash(args.configHash),
          };

          state.activeDeployment = next;
          state.appliedConfigHashes.push(args.configHash);
          return { ...next };
        }),

      checkHealth: (args) =>
        Effect.gen(function* () {
          yield* assertNotFailing("health");
          state.calls.push({ method: "checkHealth", requestId: args.requestId, deploymentId: args.deploymentId });
          return { ok: true as const, statusCode: 200 };
        }),

      checkChallenge: (args) =>
        Effect.gen(function* () {
          yield* assertNotFailing("challenge");
          state.calls.push({ method: "checkChallenge", requestId: args.requestId, deploymentId: args.deploymentId });

          return {
            ok: true as const,
            statusCode: 402,
            authorizationHeader: challengeAuthorizationHeader,
          };
        }),

      checkProxy: (args) =>
        Effect.gen(function* () {
          yield* assertNotFailing("proxy");
          state.calls.push({ method: "checkProxy", requestId: args.requestId, deploymentId: args.deploymentId });

          if (args.authorizationHeader !== challengeAuthorizationHeader) {
            return yield* Effect.fail(
              failStage("proxy", "proxy_authorization_header_mismatch"),
            );
          }

          return {
            ok: true as const,
            statusCode: 200,
          };
        }),

      rollbackTo: (args) =>
        Effect.gen(function* () {
          yield* assertNotFailing("rollback");
          state.calls.push({ method: "rollbackTo", requestId: args.requestId, deploymentId: args.deploymentId });

          const restored: GatewayDeploymentSnapshot = {
            deploymentId: args.deploymentId,
            configHash: args.target.configHash,
            imageDigest: args.target.imageDigest ?? digestForConfigHash(args.target.configHash),
          };

          state.activeDeployment = restored;
          state.rollbackTargets.push(args.target.configHash);
          return { ...restored };
        }),
    }),
  );

  return {
    state,
    layer,
  };
};
