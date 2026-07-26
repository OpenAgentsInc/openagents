import { Config, Context, Effect, Layer, Option, Redacted, Schema } from "effect";

const PositiveIntegerFromString = Schema.NumberFromString.pipe(
  Schema.check(Schema.isInt()),
  Schema.check(Schema.isGreaterThan(0)),
);

export interface ForgeGitConfigurationShape {
  readonly databaseUrl: Redacted.Redacted<string>;
  readonly gitBinary: string;
  readonly maxReceivePackBytes: number;
  /** Bound repository creation by one admitted owner; admission never becomes an open registry. */
  readonly maxRepositoriesPerOwner: number;
  readonly mirrorEnabled: boolean;
  readonly policyAuthorityToken: Redacted.Redacted<string>;
  readonly policyAuthorityUrl: string;
  readonly repositoryRoot: string;
  /** Optional owned relay. Without it, outbox rows remain pending and no visibility claim is made. */
  readonly relayUrl: string | undefined;
  readonly webReadPolicyUrl: string;
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
    const maxRepositoriesPerOwner = yield* Config.schema(
      PositiveIntegerFromString,
      "FORGE_GIT_MAX_REPOSITORIES_PER_OWNER",
    ).pipe(Config.withDefault(20));
    const mirrorEnabled = yield* Config.boolean("FORGE_GIT_GCS_MIRROR_ENABLED").pipe(
      Config.withDefault(true),
    );
    const policyAuthorityToken = yield* Config.redacted("FORGE_GIT_POLICY_AUTHORITY_TOKEN");
    const policyAuthorityUrl = yield* Config.string("FORGE_GIT_POLICY_AUTHORITY_URL");
    const repositoryRoot = yield* Config.string("FORGE_GIT_REPOSITORY_ROOT").pipe(
      Config.withDefault("/var/lib/forge/repositories"),
    );
    const relayUrl = yield* Config.option(Config.string("FORGE_GIT_RELAY_URL")).pipe(
      Effect.map((value) => (Option.isSome(value) ? value.value : undefined)),
    );
    const webReadPolicyUrl = yield* Config.string("FORGE_GIT_WEB_READ_POLICY_URL").pipe(
      Config.withDefault(
        new URL("/internal/forge/web-read-authorize", policyAuthorityUrl).toString(),
      ),
    );

    return ForgeGitConfiguration.of({
      databaseUrl,
      gitBinary,
      maxReceivePackBytes,
      maxRepositoriesPerOwner,
      mirrorEnabled,
      policyAuthorityToken,
      policyAuthorityUrl,
      repositoryRoot,
      relayUrl,
      webReadPolicyUrl,
    });
  }),
);

export const makeTestConfiguration = (
  input: Omit<
    ForgeGitConfigurationShape,
    | "databaseUrl"
    | "policyAuthorityToken"
    | "policyAuthorityUrl"
    | "relayUrl"
    | "maxRepositoriesPerOwner"
    | "webReadPolicyUrl"
  > & {
    readonly databaseUrl?: string;
    readonly policyAuthorityToken?: Redacted.Redacted<string>;
    readonly policyAuthorityUrl?: string;
    readonly relayUrl?: string | undefined;
    readonly maxRepositoriesPerOwner?: number;
    readonly webReadPolicyUrl?: string;
  },
): ForgeGitConfigurationShape => ({
  ...input,
  databaseUrl: Redacted.make(input.databaseUrl ?? "postgres://unused"),
  policyAuthorityToken:
    input.policyAuthorityToken ?? Redacted.make("forge-git-service-test-secret"),
  policyAuthorityUrl:
    input.policyAuthorityUrl ?? "https://openagents.test/internal/forge/git-authorize",
  relayUrl: input.relayUrl,
  maxRepositoriesPerOwner: input.maxRepositoriesPerOwner ?? 20,
  webReadPolicyUrl:
    input.webReadPolicyUrl ?? "https://openagents.test/internal/forge/web-read-authorize",
});

export const optionalString = (value: Option.Option<string>): string | undefined =>
  Option.isSome(value) ? value.value : undefined;
