import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { Effect, Layer, Schema as S } from "effect";

import { IsoTimestampSchema, SafeIntegerSchema } from "./generated.ts";
import {
  SignedWorkroomActorGrantResolver,
  SignedWorkroomActorGrantSchema,
  SignedWorkroomError,
  type SignedWorkroomActorGrant,
} from "./signed-workroom-authority.ts";

export const SIGNED_WORKROOM_ACTOR_GRANT_STATE_SCHEMA =
  "openagents.signed_workroom_actor_grants.v1" as const;

export const SignedWorkroomActorGrantStateSchema = S.Struct({
  schema: S.Literal(SIGNED_WORKROOM_ACTOR_GRANT_STATE_SCHEMA),
  revision: SafeIntegerSchema,
  updatedAt: IsoTimestampSchema,
  grants: S.Array(SignedWorkroomActorGrantSchema).check(S.isMaxLength(50_000)),
}).annotate({ identifier: "SignedWorkroomActorGrantState" });
export interface SignedWorkroomActorGrantState extends S.Schema.Type<
  typeof SignedWorkroomActorGrantStateSchema
> {}

export const signedWorkroomActorGrantStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "signed-workroom-actor-grants.v1.json");

const storageError = (detail: string) =>
  new SignedWorkroomError({ reason: "storage_unavailable", detail });
const notFound = (cause: unknown): boolean =>
  typeof cause === "object" && cause !== null && Reflect.get(cause, "code") === "ENOENT";

const validateUniqueGrants = (
  state: SignedWorkroomActorGrantState,
): Effect.Effect<SignedWorkroomActorGrantState, SignedWorkroomError> => {
  const grantRefs = new Set(state.grants.map((grant) => grant.grantRef));
  return grantRefs.size === state.grants.length
    ? Effect.succeed(state)
    : Effect.fail(
        new SignedWorkroomError({
          reason: "invalid_actor_grant",
          detail: "actor grant authority contains duplicate grant refs",
        }),
      );
};

export const makeSignedWorkroomActorGrantState = (input: {
  readonly revision: number;
  readonly updatedAt: string;
  readonly grants: ReadonlyArray<SignedWorkroomActorGrant>;
}): SignedWorkroomActorGrantState =>
  S.decodeUnknownSync(SignedWorkroomActorGrantStateSchema)(
    {
      schema: SIGNED_WORKROOM_ACTOR_GRANT_STATE_SCHEMA,
      revision: input.revision,
      updatedAt: input.updatedAt,
      grants: [...input.grants].sort((left, right) => left.grantRef.localeCompare(right.grantRef)),
    },
    { onExcessProperty: "error" },
  );

export const emptySignedWorkroomActorGrantState = (
  updatedAt: string,
): SignedWorkroomActorGrantState =>
  makeSignedWorkroomActorGrantState({ revision: 0, updatedAt, grants: [] });

const readState = (
  filePath: string,
): Effect.Effect<SignedWorkroomActorGrantState | null, SignedWorkroomError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (cause) =>
      notFound(cause) ? storageError("actor_grants_not_found") : storageError("actor_grants_read"),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({
        try: () => JSON.parse(contents),
        catch: () => storageError("actor_grants_json"),
      }),
    ),
    Effect.flatMap((input) =>
      S.decodeUnknownEffect(SignedWorkroomActorGrantStateSchema)(input, {
        onExcessProperty: "error",
      }).pipe(Effect.mapError(() => storageError("actor_grants_decode"))),
    ),
    Effect.flatMap(validateUniqueGrants),
    Effect.catch((cause) =>
      cause.detail === "actor_grants_not_found" ? Effect.succeed(null) : Effect.fail(cause),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: SignedWorkroomActorGrantState,
): Effect.Effect<void, SignedWorkroomError> =>
  Effect.tryPromise({
    try: async () => {
      const directory = path.dirname(filePath);
      await mkdir(directory, { recursive: true, mode: 0o700 });
      const temporary = path.join(
        directory,
        `.${path.basename(filePath)}.${randomBytes(8).toString("hex")}.tmp`,
      );
      await writeFile(temporary, `${JSON.stringify(state, null, 2)}\n`, {
        encoding: "utf8",
        mode: 0o600,
      });
      await rename(temporary, filePath);
    },
    catch: () => storageError("actor_grants_write"),
  });

export const fileSignedWorkroomActorGrantResolverLayer = (
  rootDir: string,
): Layer.Layer<SignedWorkroomActorGrantResolver> => {
  const filePath = signedWorkroomActorGrantStatePath(rootDir);
  return Layer.succeed(
    SignedWorkroomActorGrantResolver,
    SignedWorkroomActorGrantResolver.of({
      resolve: (grantRef) =>
        readState(filePath).pipe(
          Effect.flatMap((state) =>
            state === null
              ? Effect.fail(storageError("actor_grants_not_initialized"))
              : Effect.succeed(
                  state.grants.find((candidate) => candidate.grantRef === grantRef) ?? null,
                ),
          ),
        ),
    }),
  );
};

export const initializeFileSignedWorkroomActorGrantState = (
  rootDir: string,
  state: SignedWorkroomActorGrantState,
): Effect.Effect<void, SignedWorkroomError> => {
  const filePath = signedWorkroomActorGrantStatePath(rootDir);
  return readState(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};

export const provisionFileSignedWorkroomActorGrantState = (
  rootDir: string,
  expectedRevision: number,
  next: SignedWorkroomActorGrantState,
): Effect.Effect<void, SignedWorkroomError> => {
  const filePath = signedWorkroomActorGrantStatePath(rootDir);
  return Effect.gen(function* () {
    yield* validateUniqueGrants(next);
    const current = yield* readState(filePath);
    return yield* current === null ||
    current.revision !== expectedRevision ||
    next.revision !== expectedRevision + 1
      ? Effect.fail(
          new SignedWorkroomError({
            reason: "revision_conflict",
            detail: "actor grant authority revision conflict",
          }),
        )
      : atomicWrite(filePath, next);
  });
};
