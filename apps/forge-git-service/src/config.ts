import { Config, Context, Effect, Layer, Option, Redacted, Schema } from "effect";

const PositiveIntegerFromString = Schema.NumberFromString.pipe(
  Schema.check(Schema.isInt()),
  Schema.check(Schema.isGreaterThan(0)),
);

export interface ForgeGitConfigurationShape {
  readonly databaseUrl: Redacted.Redacted<string>;
  readonly gitBinary: string;
  readonly maxReceivePackBytes: number;
  readonly mirrorEnabled: boolean;
  readonly policyAuthorityToken: Redacted.Redacted<string>;
  readonly policyAuthorityUrl: string;
  readonly repositoryRoot: string;
}

export class ForgeGitConfiguration extends Context.Service<
  ForgeGitConfiguration,
  ForgeGitConfigurationShape
>()("@openagentsinc/forge-git-service/Configuration") {}

export const layerConfiguration = Layer.effect(
  ForgeGitConfiguration,
  Effect.gen(function* () {
    const databaseUrl = yield* Config.redacted("FORGE_GIT_DATABASE_URL");
    const gitBinary = yield* Config.string("FORGE_GIT_BINARY").pipe(Config.withDefault("git"));
    const maxReceivePackBytes = yield* Config.schema(
      PositiveIntegerFromString,
      "FORGE_GIT_MAX_RECEIVE_PACK_BYTES",
    ).pipe(Config.withDefault(512 * 1024 * 1024));
    const mirrorEnabled = yield* Config.boolean("FORGE_GIT_GCS_MIRROR_ENABLED").pipe(
      Config.withDefault(true),
    );
    const policyAuthorityToken = yield* Config.redacted("FORGE_GIT_POLICY_AUTHORITY_TOKEN");
    const policyAuthorityUrl = yield* Config.string("FORGE_GIT_POLICY_AUTHORITY_URL");
    const repositoryRoot = yield* Config.string("FORGE_GIT_REPOSITORY_ROOT").pipe(
      Config.withDefault("/var/lib/forge/repositories"),
    );

    return ForgeGitConfiguration.of({
      databaseUrl,
      gitBinary,
      maxReceivePackBytes,
      mirrorEnabled,
      policyAuthorityToken,
      policyAuthorityUrl,
      repositoryRoot,
    });
  }),
);

export const makeTestConfiguration = (
  input: Omit<
    ForgeGitConfigurationShape,
    "databaseUrl" | "policyAuthorityToken" | "policyAuthorityUrl"
  > & {
    readonly databaseUrl?: string;
    readonly policyAuthorityToken?: Redacted.Redacted<string>;
    readonly policyAuthorityUrl?: string;
  },
): ForgeGitConfigurationShape => ({
  ...input,
  databaseUrl: Redacted.make(input.databaseUrl ?? "postgres://unused"),
  policyAuthorityToken:
    input.policyAuthorityToken ?? Redacted.make("forge-git-service-test-secret"),
  policyAuthorityUrl:
    input.policyAuthorityUrl ?? "https://openagents.test/internal/forge/git-authorize",
});

export const optionalString = (value: Option.Option<string>): string | undefined =>
  Option.isSome(value) ? value.value : undefined;
