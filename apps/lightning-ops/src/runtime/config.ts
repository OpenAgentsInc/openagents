import { Context, Effect, Layer } from "effect";

import { ConfigError } from "../errors.js";

export type OpsRuntimeConfig = Readonly<{
  convexUrl: string;
  opsSecret: string;
}>;

export class OpsRuntimeConfigService extends Context.Tag("@openagents/lightning-ops/OpsRuntimeConfigService")<
  OpsRuntimeConfigService,
  OpsRuntimeConfig
>() {}

const loadConfigFromEnv = (): Effect.Effect<OpsRuntimeConfig, ConfigError> =>
  Effect.gen(function* () {
    const convexUrl = process.env.OA_LIGHTNING_OPS_CONVEX_URL?.trim() ?? "";
    const opsSecret = process.env.OA_LIGHTNING_OPS_SECRET?.trim() ?? "";

    if (!convexUrl) {
      return yield* ConfigError.make({
        field: "OA_LIGHTNING_OPS_CONVEX_URL",
        message: "missing required environment variable",
      });
    }

    if (!opsSecret) {
      return yield* ConfigError.make({
        field: "OA_LIGHTNING_OPS_SECRET",
        message: "missing required environment variable",
      });
    }

    return {
      convexUrl,
      opsSecret,
    };
  });

export const OpsRuntimeConfigLive = Layer.effect(OpsRuntimeConfigService, loadConfigFromEnv());

export const makeOpsRuntimeConfigTestLayer = (config: OpsRuntimeConfig) =>
  Layer.succeed(OpsRuntimeConfigService, config);
