import { Context, Effect, Layer } from "effect";

import { ConfigError } from "../errors.js";

export type OpsRuntimeConfig = Readonly<{
  opsSecret: string;
}>;

export class OpsRuntimeConfigService extends Context.Tag("@openagents/lightning-ops/OpsRuntimeConfigService")<
  OpsRuntimeConfigService,
  OpsRuntimeConfig
>() {}

const loadConfigFromEnv = (): Effect.Effect<OpsRuntimeConfig, ConfigError> =>
  Effect.gen(function* () {
    const opsSecret = process.env.OA_LIGHTNING_OPS_SECRET?.trim() ?? "";

    if (!opsSecret) {
      return yield* ConfigError.make({
        field: "OA_LIGHTNING_OPS_SECRET",
        message: "missing required environment variable",
      });
    }

    return {
      opsSecret,
    };
  });

export const OpsRuntimeConfigLive = Layer.effect(OpsRuntimeConfigService, loadConfigFromEnv());

export const makeOpsRuntimeConfigTestLayer = (config: OpsRuntimeConfig) =>
  Layer.succeed(OpsRuntimeConfigService, config);
