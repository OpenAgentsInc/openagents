import { mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, resolve, sep } from "node:path";
import { Effect, Schema as S } from "effect";
import { ProbeProvider, validateProbePublicProjection, type JsonValue, type ProviderSecretRef } from "../contracts/provider-account";
import { type OmegaResolvedAuthGrant } from "../omega/grant-client";

export const ProbeBrokeredAuthSecret = S.Struct({
  providerSecretRef: S.String,
  authContent: S.String,
  contentType: S.optional(S.String),
});
export type ProbeBrokeredAuthSecret = typeof ProbeBrokeredAuthSecret.Type;

export const ProbeAuthMaterializedReceipt = S.Struct({
  kind: S.Literal("probe_auth_materialized"),
  provider: ProbeProvider,
  providerSecretRef: S.String,
  targetKind: S.Literals(["env", "file"]),
  envName: S.optional(S.String),
  relativePath: S.optional(S.String),
  materializedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type ProbeAuthMaterializedReceipt = typeof ProbeAuthMaterializedReceipt.Type;

export const ProbeAuthScrubbedReceipt = S.Struct({
  kind: S.Literal("probe_auth_scrubbed"),
  provider: ProbeProvider,
  providerSecretRef: S.String,
  targetKind: S.Literals(["env", "file"]),
  envName: S.optional(S.String),
  relativePath: S.optional(S.String),
  scrubbedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type ProbeAuthScrubbedReceipt = typeof ProbeAuthScrubbedReceipt.Type;

export interface ProbeMaterializedAuth {
  readonly grantRef: string;
  readonly providerSecretRef: ProviderSecretRef;
  readonly runHome: string;
  readonly env: Readonly<Record<string, string>>;
  readonly materializedPath?: string;
  readonly relativePath?: string;
  readonly receipt: ProbeAuthMaterializedReceipt;
}

export class ProbeAuthMaterializationError extends S.TaggedErrorClass<ProbeAuthMaterializationError>()(
  "ProbeAuthMaterializationError",
  {
    reason: S.String,
  },
) {}

export interface ProbeAuthMaterializationInput {
  readonly grant: OmegaResolvedAuthGrant;
  readonly secret: ProbeBrokeredAuthSecret;
  readonly runHome: string;
  readonly now?: Date;
}

export function materializeProbeAuthGrant(
  input: ProbeAuthMaterializationInput,
): Effect.Effect<ProbeMaterializedAuth, ProbeAuthMaterializationError> {
  return Effect.gen(function* () {
    if (input.secret.providerSecretRef !== input.grant.providerSecretRef) {
      return yield* Effect.fail(
        new ProbeAuthMaterializationError({
          reason: "brokered secret ref does not match resolved grant providerSecretRef",
        }),
      );
    }

    const materializedAt = (input.now ?? new Date()).toISOString();
    const target = input.grant.materialization.target;

    if (target.kind === "env") {
      const materialized: ProbeMaterializedAuth = {
        grantRef: input.grant.grantRef,
        providerSecretRef: input.grant.providerSecretRef,
        runHome: input.runHome,
        env: {
          [target.name]: input.secret.authContent,
        },
        receipt: {
          kind: "probe_auth_materialized",
          provider: input.grant.provider,
          providerSecretRef: input.grant.providerSecretRef,
          targetKind: "env",
          envName: target.name,
          materializedAt,
          contentRedacted: true,
        },
      };

      yield* assertReceiptIsPublic(materialized.receipt);
      return materialized;
    }

    const materializedPath = yield* resolveRunRelativePath(input.runHome, target.relativePath);

    yield* Effect.tryPromise({
      try: () => mkdir(dirname(materializedPath), { recursive: true }),
      catch: (error) => new ProbeAuthMaterializationError({ reason: `failed to create auth directory: ${String(error)}` }),
    });

    yield* Effect.tryPromise({
      try: () => writeFile(materializedPath, input.secret.authContent, { mode: 0o600 }),
      catch: (error) => new ProbeAuthMaterializationError({ reason: `failed to write auth material: ${String(error)}` }),
    });

    const materialized: ProbeMaterializedAuth = {
      grantRef: input.grant.grantRef,
      providerSecretRef: input.grant.providerSecretRef,
      runHome: input.runHome,
      env: {},
      materializedPath,
      relativePath: target.relativePath,
      receipt: {
        kind: "probe_auth_materialized",
        provider: input.grant.provider,
        providerSecretRef: input.grant.providerSecretRef,
        targetKind: "file",
        relativePath: target.relativePath,
        materializedAt,
        contentRedacted: true,
      },
    };

    yield* assertReceiptIsPublic(materialized.receipt);
    return materialized;
  });
}

export function scrubProbeMaterializedAuth(
  materialized: ProbeMaterializedAuth,
  now: Date = new Date(),
): Effect.Effect<ProbeAuthScrubbedReceipt, ProbeAuthMaterializationError> {
  return Effect.gen(function* () {
    if (materialized.materializedPath !== undefined) {
      yield* Effect.tryPromise({
        try: () => rm(materialized.materializedPath as string, { force: true }),
        catch: (error) => new ProbeAuthMaterializationError({ reason: `failed to scrub auth material: ${String(error)}` }),
      });
    }

    const receipt: ProbeAuthScrubbedReceipt = {
      kind: "probe_auth_scrubbed",
      provider: materialized.receipt.provider,
      providerSecretRef: materialized.providerSecretRef,
      targetKind: materialized.materializedPath === undefined ? "env" : "file",
      envName: Object.keys(materialized.env)[0],
      relativePath: materialized.relativePath,
      scrubbedAt: now.toISOString(),
      contentRedacted: true,
    };

    yield* assertReceiptIsPublic(receipt);
    return receipt;
  });
}

export function withProbeAuthMaterialization<A, E, R>(
  input: ProbeAuthMaterializationInput,
  use: (materialized: ProbeMaterializedAuth) => Effect.Effect<A, E, R>,
): Effect.Effect<A, E | ProbeAuthMaterializationError, R> {
  return Effect.acquireUseRelease(
    materializeProbeAuthGrant(input),
    use,
    (materialized) => scrubProbeMaterializedAuth(materialized),
  );
}

export function runNoProviderAuthSmoke(
  materialized: ProbeMaterializedAuth,
): Effect.Effect<void, ProbeAuthMaterializationError> {
  if (materialized.materializedPath === undefined) {
    const envName = Object.keys(materialized.env)[0];
    const envValue = envName === undefined ? undefined : materialized.env[envName];

    return envValue === undefined || envValue.length === 0
      ? Effect.fail(new ProbeAuthMaterializationError({ reason: "expected auth env materialization to exist" }))
      : Effect.void;
  }

  return Effect.gen(function* () {
    const fileStat = yield* Effect.tryPromise({
      try: () => stat(materialized.materializedPath as string),
      catch: (error) => new ProbeAuthMaterializationError({ reason: `expected auth file to exist: ${String(error)}` }),
    });

    if (!fileStat.isFile()) {
      return yield* Effect.fail(new ProbeAuthMaterializationError({ reason: "expected auth materialization to be a file" }));
    }

    const content = yield* Effect.tryPromise({
      try: () => readFile(materialized.materializedPath as string, "utf8"),
      catch: (error) => new ProbeAuthMaterializationError({ reason: `failed to read auth material: ${String(error)}` }),
    });

    if (content.length === 0) {
      return yield* Effect.fail(new ProbeAuthMaterializationError({ reason: "expected auth materialization content" }));
    }
  });
}

function resolveRunRelativePath(
  runHome: string,
  relativePath: string,
): Effect.Effect<string, ProbeAuthMaterializationError> {
  if (isAbsolute(relativePath)) {
    return Effect.fail(new ProbeAuthMaterializationError({ reason: "auth materialization path must be relative" }));
  }

  const root = resolve(runHome);
  const target = resolve(root, relativePath);

  if (target !== root && !target.startsWith(`${root}${sep}`)) {
    return Effect.fail(new ProbeAuthMaterializationError({ reason: "auth materialization path escapes run home" }));
  }

  return Effect.succeed(target);
}

function assertReceiptIsPublic(
  receipt: ProbeAuthMaterializedReceipt | ProbeAuthScrubbedReceipt,
): Effect.Effect<void, ProbeAuthMaterializationError> {
  return validateProbePublicProjection(receipt as unknown as JsonValue, "receipt").pipe(
    Effect.mapError((error) => new ProbeAuthMaterializationError({ reason: error.reason })),
  );
}
