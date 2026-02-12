import { Effect, Layer } from "effect";

import { ConfigError } from "../errors.js";

import { ApertureConfigCompilerLive } from "../compiler/apertureCompiler.js";
import { ConvexControlPlaneLive } from "../controlPlane/convex.js";
import { ConvexTransportLive } from "../controlPlane/convexTransport.js";
import { makeInMemoryControlPlaneHarness } from "../controlPlane/inMemory.js";
import { smokePaywalls } from "../fixtures/smokePaywalls.js";
import { makeHttpGatewayLayer } from "../gateway/http.js";
import { makeInMemoryGatewayHarness } from "../gateway/inMemory.js";
import { reconcileAndDeployOnce } from "./reconcileAndDeploy.js";
import { OpsRuntimeConfigLive } from "../runtime/config.js";

export type StagingSmokeMode = "mock" | "convex";

const STAGING_GATEWAY_DEFAULTS: Record<string, string> = {
  OA_LIGHTNING_OPS_GATEWAY_BASE_URL: "https://l402.openagents.com",
  OA_LIGHTNING_OPS_CHALLENGE_URL: "https://l402.openagents.com/staging",
  OA_LIGHTNING_OPS_PROXY_URL: "https://l402.openagents.com/staging",
};

const env = (key: string): Effect.Effect<string, ConfigError> =>
  Effect.sync(() => process.env[key]?.trim() ?? STAGING_GATEWAY_DEFAULTS[key] ?? "").pipe(
    Effect.flatMap((value) =>
      value
        ? Effect.succeed(value)
        : ConfigError.make({
            field: key,
            message: "missing required environment variable",
          }),
    ),
  );

const loadHttpGatewayConfigFromEnv = () =>
  Effect.gen(function* () {
    const baseUrl = yield* env("OA_LIGHTNING_OPS_GATEWAY_BASE_URL");
    const challengeUrl = yield* env("OA_LIGHTNING_OPS_CHALLENGE_URL");
    const proxyUrl = yield* env("OA_LIGHTNING_OPS_PROXY_URL");

    const opsToken = process.env.OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN?.trim() || undefined;
    const healthPath = process.env.OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH?.trim() || undefined;
    const proxyAuthorizationHeader =
      process.env.OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER?.trim() || undefined;

    return {
      baseUrl,
      challengeUrl,
      proxyUrl,
      ...(healthPath ? { healthPath } : {}),
      ...(opsToken ? { opsToken } : {}),
      ...(proxyAuthorizationHeader ? { proxyAuthorizationHeader } : {}),
    };
  });

const runReconcile = (layers: Layer.Layer<any, any, never>, requestId: string) =>
  reconcileAndDeployOnce({ requestId }).pipe(
    Effect.provide(layers),
    Effect.provide(ApertureConfigCompilerLive),
  );

const runMockSmoke = (requestId: string) => {
  const controlPlaneHarness = makeInMemoryControlPlaneHarness({
    paywalls: smokePaywalls,
  });

  const gatewayHarness = makeInMemoryGatewayHarness({
    initialDeployment: {
      deploymentId: "dep_prev_smoke",
      configHash: "cfg_prev_smoke",
      imageDigest: "sha256:prev_smoke",
    },
  });

  return runReconcile(
    Layer.mergeAll(controlPlaneHarness.layer, gatewayHarness.layer),
    requestId,
  );
};

const runConvexSmoke = (requestId: string) =>
  Effect.gen(function* () {
    const gatewayConfig = yield* loadHttpGatewayConfigFromEnv();

    const controlPlaneLayer = ConvexControlPlaneLive.pipe(
      Layer.provideMerge(ConvexTransportLive),
      Layer.provideMerge(OpsRuntimeConfigLive),
    );
    const gatewayLayer = makeHttpGatewayLayer(gatewayConfig);

    return yield* runReconcile(
      Layer.mergeAll(controlPlaneLayer, gatewayLayer),
      requestId,
    );
  });

export const runStagingSmoke = (input?: {
  readonly mode?: StagingSmokeMode;
  readonly requestId?: string;
}) => {
  const mode = input?.mode ?? "mock";
  const requestId = input?.requestId ?? "smoke:staging";

  return mode === "convex" ? runConvexSmoke(requestId) : runMockSmoke(requestId);
};
