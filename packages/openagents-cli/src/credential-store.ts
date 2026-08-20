import { Effect, Layer, Option, Redacted, Schema, Stream } from "effect";
import * as Context from "effect/Context";
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process";
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

const keychainService = "openagents-cli";
const encoder = new TextEncoder();

interface ProcessResult {
  readonly exitCode: number;
  readonly stdout: string;
}

export const credentialStoreOsLayer = Layer.effect(
  CredentialStore,
  Effect.gen(function* () {
    const spawner = yield* ChildProcessSpawner.ChildProcessSpawner;

    const run = Effect.fn("CredentialStore.OS.run")(function* (
      command: string,
      args: ReadonlyArray<string>,
      input?: string,
    ) {
      const process = ChildProcess.make(command, args, {
        stdin:
          input === undefined
            ? "ignore"
            : { stream: Stream.make(encoder.encode(`${input}\n`)), endOnDone: true },
        stdout: "pipe",
        stderr: "ignore",
      });
      return yield* Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* spawner.spawn(process);
          const stdout = yield* Stream.mkString(Stream.decodeText(handle.stdout));
          const exitCode = yield* handle.exitCode;
          return { exitCode: Number(exitCode), stdout } satisfies ProcessResult;
        }),
      );
    });

    const commandFor = (
      operation: "get" | "set" | "remove",
      origin: string,
    ): readonly [string, ReadonlyArray<string>] | undefined => {
      if (process.platform === "darwin") {
        if (operation === "get") {
          return ["security", ["find-generic-password", "-a", origin, "-s", keychainService, "-w"]];
        }
        if (operation === "set") {
          return [
            "security",
            ["add-generic-password", "-U", "-a", origin, "-s", keychainService, "-w"],
          ];
        }
        return ["security", ["delete-generic-password", "-a", origin, "-s", keychainService]];
      }

      if (process.platform === "linux") {
        if (operation === "get") {
          return ["secret-tool", ["lookup", "service", keychainService, "origin", origin]];
        }
        if (operation === "set") {
          return [
            "secret-tool",
            ["store", "--label=OpenAgents CLI", "service", keychainService, "origin", origin],
          ];
        }
        return ["secret-tool", ["clear", "service", keychainService, "origin", origin]];
      }

      return undefined;
    };

    const unsupported = () => Effect.fail(unavailable("Persistent authentication"));

    const get = Effect.fn("CredentialStore.OS.get")(function* (origin: string) {
      const command = commandFor("get", origin);
      if (command === undefined) return yield* unsupported();
      const result = yield* run(command[0], command[1]).pipe(
        Effect.mapError((cause) => storeError("read the OS credential store", cause)),
      );
      if (result.exitCode !== 0) return Option.none();
      const token = result.stdout.trim();
      if (!token.startsWith("oa_pat_") || token.length >= 160) {
        return yield* storeError("read the OS credential store", new Error("invalid token record"));
      }
      return Option.some(Redacted.make(token));
    });

    const set = Effect.fn("CredentialStore.OS.set")(function* (
      origin: string,
      token: Redacted.Redacted<string>,
    ) {
      const command = commandFor("set", origin);
      if (command === undefined) return yield* unsupported();
      const result = yield* run(command[0], command[1], Redacted.value(token)).pipe(
        Effect.mapError((cause) => storeError("write the OS credential store", cause)),
      );
      if (result.exitCode !== 0) {
        return yield* storeError(
          "write the OS credential store",
          new Error(`credential command exited ${result.exitCode}`),
        );
      }
    });

    const remove = Effect.fn("CredentialStore.OS.remove")(function* (origin: string) {
      const command = commandFor("remove", origin);
      if (command === undefined) return yield* unsupported();
      yield* run(command[0], command[1]).pipe(
        Effect.mapError((cause) => storeError("remove the OS credential", cause)),
      );
    });

    return CredentialStore.of({ get, set, remove });
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

/** Creates a plaintext file adapter for isolated tests only. */
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
