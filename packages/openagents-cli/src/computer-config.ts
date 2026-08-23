import { Effect, Layer, Schema } from "effect";
import * as Context from "effect/Context";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { homedir } from "node:os";

import { ConfigurationError } from "./errors.js";
import { EnvironmentConfiguration } from "./environment.js";
import { defaultCuratedExecute, resolveRoots, type PolicyConfig } from "./computer-policy.js";

export interface ComputerPaths {
  readonly config: string;
  readonly journal: string;
}

export interface ComputerConfigurationValues extends PolicyConfig {
  readonly paths: ComputerPaths;
}

export interface AgentConfigEntry {
  readonly id: string;
  readonly argv: ReadonlyArray<string>;
  readonly env: ReadonlyArray<string>;
}

export class ComputerConfiguration extends Context.Service<
  ComputerConfiguration,
  ComputerConfigurationValues
>()("@openagentsinc/cli/ComputerConfiguration") {}

const StoredConfiguration = Schema.Struct({
  tier: Schema.optionalKey(Schema.Literals(["probe", "curated", "shell"])),
  roots: Schema.optionalKey(Schema.Array(Schema.String)),
  pre_approved: Schema.optionalKey(Schema.Array(Schema.String)),
  agents: Schema.optionalKey(
    Schema.Record(
      Schema.String,
      Schema.Struct({
        argv: Schema.Array(Schema.String),
        env: Schema.optionalKey(Schema.Array(Schema.String)),
      }),
    ),
  ),
  registry_agents: Schema.optionalKey(Schema.Boolean),
  curated_execute: Schema.optionalKey(Schema.Array(Schema.String)),
});

const defaultConfig = (paths: ComputerPaths): ComputerConfigurationValues => ({
  tier: "probe",
  roots: [],
  preApproved: [],
  paths,
  agents: [],
  registryAgents: false,
  curatedExecute: [...defaultCuratedExecute],
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
      agents: Object.entries(decoded.agents ?? {}).flatMap(([id, entry]) => {
        if (!/^[a-z0-9][a-z0-9._-]{0,63}$/u.test(id) || entry.argv.length === 0) return [];
        return [
          {
            id,
            argv: entry.argv.slice(0, 16).map((value) => value.slice(0, 256)),
            env: [
              ...new Set(
                (entry.env ?? []).filter((value) => /^[A-Z_][A-Z0-9_]{0,63}$/u.test(value)),
              ),
            ].slice(0, 32),
          },
        ];
      }),
      registryAgents: decoded.registry_agents ?? false,
      curatedExecute: [...new Set(decoded.curated_execute ?? defaultCuratedExecute)]
        .filter((value) => value.length > 0)
        .slice(0, 64),
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
            agents: Object.fromEntries(
              (config.agents ?? []).map((entry) => [
                entry.id,
                {
                  argv: entry.argv,
                  env: entry.env,
                },
              ]),
            ),
            registry_agents: config.registryAgents ?? false,
            curated_execute: config.curatedExecute ?? defaultCuratedExecute,
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
