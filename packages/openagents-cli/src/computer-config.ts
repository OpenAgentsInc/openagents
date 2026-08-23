import { Effect, Layer, Schema } from "effect";
import * as Context from "effect/Context";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { homedir } from "node:os";

import { ConfigurationError } from "./errors.js";
import { EnvironmentConfiguration } from "./environment.js";
import { resolveRoots, type PolicyConfig } from "./computer-policy.js";

export interface ComputerPaths {
  readonly config: string;
  readonly journal: string;
}

export interface ComputerConfigurationValues extends PolicyConfig {
  readonly paths: ComputerPaths;
}

export class ComputerConfiguration extends Context.Service<
  ComputerConfiguration,
  ComputerConfigurationValues
>()("@openagentsinc/cli/ComputerConfiguration") {}

const StoredConfiguration = Schema.Struct({
  tier: Schema.optionalKey(Schema.Literals(["probe", "curated", "shell"])),
  roots: Schema.optionalKey(Schema.Array(Schema.String)),
  pre_approved: Schema.optionalKey(Schema.Array(Schema.String)),
});

const defaultConfig = (paths: ComputerPaths): ComputerConfigurationValues => ({
  tier: "probe",
  roots: [],
  preApproved: [],
  paths,
});

const errorCode = (cause: unknown): string | undefined =>
  typeof cause === "object" && cause !== null && "code" in cause && typeof cause.code === "string"
    ? cause.code
    : undefined;

export const computerPaths = (configPath?: string): ComputerPaths => {
  const directory =
    configPath === undefined ? join(homedir(), ".config", "openagents") : dirname(configPath);
  return {
    config: join(directory, "computer.json"),
    journal: join(directory, "journal.ndjson"),
  };
};

const readConfiguration = (
  paths: ComputerPaths,
): Effect.Effect<ComputerConfigurationValues, ConfigurationError> =>
  Effect.gen(function* () {
    const contents = yield* Effect.sync(() => {
      try {
        return { _tag: "Read" as const, value: readFileSync(paths.config, "utf8") };
      } catch (cause) {
        return errorCode(cause) === "ENOENT"
          ? { _tag: "Missing" as const }
          : { _tag: "Error" as const };
      }
    });
    if (contents._tag === "Error") {
      return yield* new ConfigurationError({
        message: "The local Computer configuration could not be read.",
      });
    }
    if (contents._tag === "Missing") return defaultConfig(paths);
    if (Buffer.byteLength(contents.value, "utf8") > 16_384) {
      return yield* new ConfigurationError({
        message: "The local Computer configuration is too large.",
      });
    }
    const decoded = yield* Effect.try({
      try: () => JSON.parse(contents.value) as unknown,
      catch: () =>
        new ConfigurationError({ message: "The local Computer configuration is not JSON." }),
    }).pipe(
      Effect.flatMap(Schema.decodeUnknownEffect(StoredConfiguration)),
      Effect.mapError(
        () =>
          new ConfigurationError({
            message: "The local Computer configuration has invalid fields.",
          }),
      ),
    );
    return {
      tier: decoded.tier ?? "probe",
      roots: resolveRoots(decoded.roots ?? []),
      preApproved: [...new Set(decoded.pre_approved ?? [])].slice(0, 64),
      paths,
    };
  });

export const computerConfigurationLayer = Layer.effect(
  ComputerConfiguration,
  Effect.gen(function* () {
    const environment = yield* EnvironmentConfiguration;
    const paths = computerPaths(
      environment.configPath._tag === "Some" ? environment.configPath.value : undefined,
    );
    return yield* readConfiguration(paths);
  }),
);

export const computerConfigurationTestLayer = (
  values: Partial<PolicyConfig> & { readonly paths?: ComputerPaths } = {},
): Layer.Layer<ComputerConfiguration> =>
  Layer.succeed(
    ComputerConfiguration,
    ComputerConfiguration.of({
      tier: values.tier ?? "probe",
      roots: resolveRoots(values.roots ?? []),
      preApproved: values.preApproved ?? [],
      paths: values.paths ?? computerPaths(),
    }),
  );

export const writeComputerConfiguration = (
  config: PolicyConfig,
  paths: ComputerPaths,
): Effect.Effect<void, ConfigurationError> =>
  Effect.try({
    try: () => {
      mkdirSync(dirname(paths.config), { recursive: true, mode: 0o700 });
      writeFileSync(
        paths.config,
        `${JSON.stringify(
          {
            tier: config.tier,
            roots: resolveRoots(config.roots),
            pre_approved: config.preApproved,
          },
          null,
          2,
        )}\n`,
        { encoding: "utf8", mode: 0o600 },
      );
      chmodSync(paths.config, 0o600);
    },
    catch: () =>
      new ConfigurationError({ message: "The local Computer configuration could not be written." }),
  });
