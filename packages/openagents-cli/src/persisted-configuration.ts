import { Effect, Layer, Option, Schema } from "effect";
import * as Context from "effect/Context";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

import { ConfigurationError } from "./errors.js";
import { EnvironmentConfiguration } from "./environment.js";

export interface PersistedConfigurationValues {
  readonly profile: Option.Option<string>;
  readonly apiUrl: Option.Option<string>;
}

export class PersistedConfiguration extends Context.Service<
  PersistedConfiguration,
  PersistedConfigurationValues
>()("@openagentsinc/cli/PersistedConfiguration") {}

const ConfigurationFile = Schema.Struct({
  profile: Schema.optionalKey(Schema.String),
  api_url: Schema.optionalKey(Schema.String),
});

class ConfigurationReadFailed extends Schema.TaggedErrorClass<ConfigurationReadFailed>()(
  "OpenAgentsCli.Internal.ConfigurationReadFailed",
  { cause: Schema.Defect() },
) {}

const emptyConfiguration = PersistedConfiguration.of({
  profile: Option.none(),
  apiUrl: Option.none(),
});

export const persistedConfigurationLayer = Layer.effect(
  PersistedConfiguration,
  Effect.gen(function* () {
    const environment = yield* EnvironmentConfiguration;
    const path = Option.isSome(environment.configPath)
      ? environment.configPath.value
      : join(homedir(), ".config", "openagents", "config.json");
    const contents = yield* Effect.tryPromise({
      try: () => readFile(path, { encoding: "utf8" }),
      catch: (cause) => new ConfigurationReadFailed({ cause }),
    }).pipe(
      Effect.catchTag("OpenAgentsCli.Internal.ConfigurationReadFailed", (failure) => {
        const error = failure.cause as NodeJS.ErrnoException;
        return error.code === "ENOENT"
          ? Effect.succeed(null as string | null)
          : Effect.fail(
              new ConfigurationError({ message: `The CLI could not read its configuration file.` }),
            );
      }),
    );
    if (contents === null) return emptyConfiguration;
    if (Buffer.byteLength(contents, "utf8") > 16_384) {
      return yield* new ConfigurationError({
        message: "The CLI configuration file exceeds 16384 bytes.",
      });
    }
    const decoded = yield* Effect.try({
      try: () => JSON.parse(contents) as unknown,
      catch: () => new ConfigurationError({ message: "The CLI configuration file is not JSON." }),
    }).pipe(
      Effect.flatMap(Schema.decodeUnknownEffect(ConfigurationFile)),
      Effect.mapError(
        () => new ConfigurationError({ message: "The CLI configuration file has invalid fields." }),
      ),
    );
    return PersistedConfiguration.of({
      profile: decoded.profile === undefined ? Option.none() : Option.some(decoded.profile),
      apiUrl: decoded.api_url === undefined ? Option.none() : Option.some(decoded.api_url),
    });
  }),
);

export const persistedConfigurationTestLayer = (
  values: Partial<{ readonly profile: string; readonly apiUrl: string }>,
) =>
  Layer.succeed(PersistedConfiguration, {
    profile: values.profile === undefined ? Option.none() : Option.some(values.profile),
    apiUrl: values.apiUrl === undefined ? Option.none() : Option.some(values.apiUrl),
  });
