import { Effect, Layer, Option, Redacted, Schema } from "effect";
import * as Context from "effect/Context";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

import { CredentialPersistenceUnavailable, CredentialStoreError } from "./errors.js";

export type CredentialStoreFailure = CredentialPersistenceUnavailable | CredentialStoreError;

export interface CredentialStoreInterface {
  readonly get: (
    origin: string,
  ) => Effect.Effect<Option.Option<Redacted.Redacted<string>>, CredentialStoreFailure>;
  readonly set: (
    origin: string,
    token: Redacted.Redacted<string>,
  ) => Effect.Effect<void, CredentialStoreFailure>;
  readonly remove: (origin: string) => Effect.Effect<void, CredentialStoreFailure>;
}

export class CredentialStore extends Context.Service<CredentialStore, CredentialStoreInterface>()(
  "@openagentsinc/cli/CredentialStore",
) {}

const unavailable = (operation: string) =>
  new CredentialPersistenceUnavailable({
    message: `${operation} requires an approved OS-backed credential adapter. Set OPENAGENTS_TOKEN for this invocation.`,
  });

export const credentialStoreUnavailableLayer = Layer.succeed(
  CredentialStore,
  CredentialStore.of({
    get: Effect.fn("CredentialStore.Unavailable.get")((_) =>
      Effect.fail(unavailable("Reading a stored token")),
    ),
    set: Effect.fn("CredentialStore.Unavailable.set")((_, __) =>
      Effect.fail(unavailable("Storing a token")),
    ),
    remove: Effect.fn("CredentialStore.Unavailable.remove")((_) =>
      Effect.fail(unavailable("Removing a stored token")),
    ),
  }),
);

const CredentialFile = Schema.Struct({
  version: Schema.Literal(1),
  tokens: Schema.Record(Schema.String, Schema.String),
});
type CredentialFile = typeof CredentialFile.Type;

const emptyCredentialFile = (): CredentialFile => ({ version: 1, tokens: {} });

const hasErrorCode = (value: unknown): value is { readonly code: string } =>
  typeof value === "object" && value !== null && "code" in value && typeof value.code === "string";

const storeError = (operation: string, cause: unknown) =>
  new CredentialStoreError({
    operation,
    message: `The test credential store could not ${operation}.`,
    cause,
  });

class CredentialFileMissing extends Schema.TaggedErrorClass<CredentialFileMissing>()(
  "OpenAgentsCli.Internal.CredentialFileMissing",
  {},
) {}

/**
 * Creates a plaintext file adapter for isolated tests. Production composition
 * must use `credentialStoreUnavailableLayer` until an OS-backed adapter exists.
 */
export const credentialStoreTestFileLayer = (path: string) =>
  Layer.sync(CredentialStore, () => {
    const load = Effect.fn("CredentialStore.TestFile.load")(function* () {
      const text = yield* Effect.tryPromise({
        try: () => readFile(path, "utf8"),
        catch: (cause) =>
          hasErrorCode(cause) && cause.code === "ENOENT"
            ? new CredentialFileMissing()
            : storeError("read credentials", cause),
      }).pipe(Effect.catchTag("OpenAgentsCli.Internal.CredentialFileMissing", () => Effect.void));
      if (text === undefined) return emptyCredentialFile();
      return yield* Schema.decodeUnknownEffect(Schema.fromJsonString(CredentialFile))(text).pipe(
        Effect.mapError((cause) => storeError("decode credentials", cause)),
      );
    });

    const save = Effect.fn("CredentialStore.TestFile.save")(function* (file: CredentialFile) {
      const encoded = yield* Schema.encodeEffect(Schema.fromJsonString(CredentialFile))(file).pipe(
        Effect.mapError((cause) => storeError("encode credentials", cause)),
      );
      const temporaryPath = `${path}.tmp`;
      yield* Effect.tryPromise({
        try: async () => {
          await mkdir(dirname(path), { recursive: true, mode: 0o700 });
          await writeFile(temporaryPath, encoded, { encoding: "utf8", mode: 0o600 });
          await rename(temporaryPath, path);
        },
        catch: (cause) => storeError("write credentials", cause),
      });
    });

    const get = Effect.fn("CredentialStore.TestFile.get")(function* (origin: string) {
      const file = yield* load();
      const token = file.tokens[origin];
      return token === undefined ? Option.none() : Option.some(Redacted.make(token));
    });

    const set = Effect.fn("CredentialStore.TestFile.set")(function* (
      origin: string,
      token: Redacted.Redacted<string>,
    ) {
      const file = yield* load();
      yield* save({
        ...file,
        tokens: { ...file.tokens, [origin]: Redacted.value(token) },
      });
    });

    const remove = Effect.fn("CredentialStore.TestFile.remove")(function* (origin: string) {
      const file = yield* load();
      const tokens = { ...file.tokens };
      delete tokens[origin];
      if (Object.keys(tokens).length === 0) {
        yield* Effect.tryPromise({
          try: () => rm(path, { force: true }),
          catch: (cause) => storeError("remove credentials", cause),
        });
        return;
      }
      yield* save({ ...file, tokens });
    });

    return CredentialStore.of({ get, set, remove });
  });
