import { Effect, Layer, Option, Redacted, Schema, Stream } from "effect";
import * as Context from "effect/Context";
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

import { CredentialPersistenceUnavailable, CredentialStoreError } from "./errors.js";

export type CredentialStoreFailure = CredentialPersistenceUnavailable | CredentialStoreError;
export type CredentialKind = "api" | "computer";

export interface CredentialStoreInterface {
  readonly get: (
    origin: string,
    kind?: CredentialKind,
  ) => Effect.Effect<Option.Option<Redacted.Redacted<string>>, CredentialStoreFailure>;
  readonly set: (
    origin: string,
    token: Redacted.Redacted<string>,
    kind?: CredentialKind,
  ) => Effect.Effect<void, CredentialStoreFailure>;
  readonly remove: (
    origin: string,
    kind?: CredentialKind,
  ) => Effect.Effect<void, CredentialStoreFailure>;
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

const keychainService = (kind: CredentialKind): string =>
  kind === "computer" ? "openagents-cli-computer" : "openagents-cli";
const encoder = new TextEncoder();

interface ProcessResult {
  readonly exitCode: number;
  readonly stdout: string;
}

interface CredentialCommand {
  readonly command: string;
  readonly args: ReadonlyArray<string>;
  readonly input?: string;
}

export const credentialCommandFor = (
  platform: NodeJS.Platform,
  operation: "get" | "set" | "remove",
  origin: string,
  token?: string,
  kind: CredentialKind = "api",
): CredentialCommand | undefined => {
  if (platform === "darwin") {
    if (operation === "get") {
      return {
        command: "security",
        args: ["find-generic-password", "-a", origin, "-s", keychainService(kind), "-w"],
      };
    }
    if (operation === "set" && token !== undefined) {
      return {
        command: "security",
        args: [
          "add-generic-password",
          "-U",
          "-a",
          origin,
          "-s",
          keychainService(kind),
          "-w",
          token,
        ],
      };
    }
    if (operation === "remove") {
      return {
        command: "security",
        args: ["delete-generic-password", "-a", origin, "-s", keychainService(kind)],
      };
    }
    return undefined;
  }

  if (platform === "linux") {
    if (operation === "get") {
      return {
        command: "secret-tool",
        args: ["lookup", "service", keychainService(kind), "origin", origin],
      };
    }
    if (operation === "set" && token !== undefined) {
      return {
        command: "secret-tool",
        args: [
          "store",
          "--label=OpenAgents CLI",
          "service",
          keychainService(kind),
          "origin",
          origin,
        ],
        input: token,
      };
    }
    if (operation === "remove") {
      return {
        command: "secret-tool",
        args: ["clear", "service", keychainService(kind), "origin", origin],
      };
    }
  }

  return undefined;
};

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

    const unsupported = () => Effect.fail(unavailable("Persistent authentication"));

    const get = Effect.fn("CredentialStore.OS.get")(function* (
      origin: string,
      kind: CredentialKind = "api",
    ) {
      const command = credentialCommandFor(process.platform, "get", origin, undefined, kind);
      if (command === undefined) return yield* unsupported();
      const result = yield* run(command.command, command.args, command.input).pipe(
        Effect.mapError((cause) => storeError("read the OS credential store", cause)),
      );
      if (result.exitCode !== 0) return Option.none();
      const token = result.stdout.trim();
      const validPrefix =
        kind === "computer" ? token.startsWith("smct_") : token.startsWith("oa_pat_");
      if (!validPrefix || token.length >= 160) {
        return yield* storeError("read the OS credential store", new Error("invalid token record"));
      }
      return Option.some(Redacted.make(token));
    });

    const set = Effect.fn("CredentialStore.OS.set")(function* (
      origin: string,
      token: Redacted.Redacted<string>,
      kind: CredentialKind = "api",
    ) {
      const tokenValue = Redacted.value(token);
      const command = credentialCommandFor(process.platform, "set", origin, tokenValue, kind);
      if (command === undefined) return yield* unsupported();
      const result = yield* run(command.command, command.args, command.input).pipe(
        Effect.mapError(() =>
          storeError(
            "write the OS credential store",
            new Error("credential command could not start"),
          ),
        ),
      );
      if (result.exitCode !== 0) {
        return yield* storeError(
          "write the OS credential store",
          new Error(`credential command exited ${result.exitCode}`),
        );
      }
      const stored = yield* get(origin, kind);
      if (Option.isNone(stored) || Redacted.value(stored.value) !== tokenValue) {
        return yield* storeError(
          "verify the OS credential store",
          new Error("credential readback did not match"),
        );
      }
    });

    const remove = Effect.fn("CredentialStore.OS.remove")(function* (
      origin: string,
      kind: CredentialKind = "api",
    ) {
      const command = credentialCommandFor(process.platform, "remove", origin, undefined, kind);
      if (command === undefined) return yield* unsupported();
      yield* run(command.command, command.args, command.input).pipe(
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

const credentialKey = (origin: string, kind: CredentialKind): string =>
  kind === "computer" ? `computer:${origin}` : origin;

const hasErrorCode = (value: unknown): value is { readonly code: string } =>
  typeof value === "object" && value !== null && "code" in value && typeof value.code === "string";

const storeError = (operation: string, cause: unknown) =>
  new CredentialStoreError({
    operation,
    message: `The CLI could not ${operation}.`,
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

    const get = Effect.fn("CredentialStore.TestFile.get")(function* (
      origin: string,
      kind: CredentialKind = "api",
    ) {
      const file = yield* load();
      const token = file.tokens[credentialKey(origin, kind)];
      return token === undefined ? Option.none() : Option.some(Redacted.make(token));
    });

    const set = Effect.fn("CredentialStore.TestFile.set")(function* (
      origin: string,
      token: Redacted.Redacted<string>,
      kind: CredentialKind = "api",
    ) {
      const file = yield* load();
      yield* save({
        ...file,
        tokens: { ...file.tokens, [credentialKey(origin, kind)]: Redacted.value(token) },
      });
    });

    const remove = Effect.fn("CredentialStore.TestFile.remove")(function* (
      origin: string,
      kind: CredentialKind = "api",
    ) {
      const file = yield* load();
      const tokens = { ...file.tokens };
      delete tokens[credentialKey(origin, kind)];
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
