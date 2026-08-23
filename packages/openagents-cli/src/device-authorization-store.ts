import { Effect, Layer, Option, Schema } from "effect";
import * as Context from "effect/Context";
import { chmod, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

import { EnvironmentConfiguration } from "./environment.js";
import { ConfigurationError } from "./errors.js";

export const PendingDeviceAuthorization = Schema.Struct({
  origin: Schema.String,
  device_code: Schema.String,
  user_code: Schema.String,
  verification_uri: Schema.String,
  verification_uri_complete: Schema.String,
  expires_at_ms: Schema.Number,
  interval: Schema.Number,
  kind: Schema.optionalKey(Schema.Literals(["device", "computer"])),
  state: Schema.optionalKey(Schema.Literals(["pending", "paired"])),
  machine_id: Schema.optionalKey(Schema.String),
});
export interface PendingDeviceAuthorization extends Schema.Schema.Type<
  typeof PendingDeviceAuthorization
> {}

export interface PendingDeviceAuthorizationStoreInterface {
  readonly get: (
    origin: string,
  ) => Effect.Effect<Option.Option<PendingDeviceAuthorization>, ConfigurationError>;
  readonly set: (
    authorization: PendingDeviceAuthorization,
  ) => Effect.Effect<void, ConfigurationError>;
  readonly remove: (origin: string) => Effect.Effect<void, ConfigurationError>;
}

export class PendingDeviceAuthorizationStore extends Context.Service<
  PendingDeviceAuthorizationStore,
  PendingDeviceAuthorizationStoreInterface
>()("@openagentsinc/cli/PendingDeviceAuthorizationStore") {}

const PendingDeviceAuthorizationFile = Schema.Struct({
  version: Schema.Literal(1),
  authorizations: Schema.Record(Schema.String, PendingDeviceAuthorization),
});
type PendingDeviceAuthorizationFile = typeof PendingDeviceAuthorizationFile.Type;
const decodePendingDeviceAuthorizationFile = Schema.decodeUnknownEffect(
  Schema.fromJsonString(PendingDeviceAuthorizationFile),
);
const encodePendingDeviceAuthorizationFile = Schema.encodeEffect(
  Schema.fromJsonString(PendingDeviceAuthorizationFile),
);

const emptyFile = (): PendingDeviceAuthorizationFile => ({ version: 1, authorizations: {} });

const hasErrorCode = (value: unknown): value is { readonly code: string } =>
  typeof value === "object" && value !== null && "code" in value && typeof value.code === "string";

class PendingDeviceAuthorizationFileMissing extends Schema.TaggedErrorClass<PendingDeviceAuthorizationFileMissing>()(
  "OpenAgentsCli.Internal.PendingDeviceAuthorizationFileMissing",
  {},
) {}

const stateError = (operation: string) =>
  new ConfigurationError({
    message: `The CLI could not ${operation} its pending device authorization state.`,
  });

export const pendingDeviceAuthorizationStoreFileLayer = (path: string) =>
  Layer.sync(PendingDeviceAuthorizationStore, () => {
    const load = Effect.fn("PendingDeviceAuthorizationStore.File.load")(function* () {
      const text = yield* Effect.tryPromise({
        try: () => readFile(path, "utf8"),
        catch: (cause) =>
          hasErrorCode(cause) && cause.code === "ENOENT"
            ? new PendingDeviceAuthorizationFileMissing()
            : stateError("read"),
      }).pipe(
        Effect.catchTag(
          "OpenAgentsCli.Internal.PendingDeviceAuthorizationFileMissing",
          () => Effect.void,
        ),
      );
      if (text === undefined) return emptyFile();
      if (Buffer.byteLength(text, "utf8") > 65_536) {
        return yield* stateError("read");
      }
      return yield* decodePendingDeviceAuthorizationFile(text).pipe(
        Effect.mapError(() => stateError("decode")),
      );
    });

    const save = Effect.fn("PendingDeviceAuthorizationStore.File.save")(function* (
      file: PendingDeviceAuthorizationFile,
    ) {
      if (Object.keys(file.authorizations).length === 0) {
        yield* Effect.tryPromise({
          try: () => rm(path, { force: true }),
          catch: () => stateError("remove"),
        });
        return;
      }

      const encoded = yield* encodePendingDeviceAuthorizationFile(file).pipe(
        Effect.mapError(() => stateError("encode")),
      );
      const temporaryPath = `${path}.tmp`;
      yield* Effect.tryPromise({
        try: async () => {
          await mkdir(dirname(path), { recursive: true, mode: 0o700 });
          await writeFile(temporaryPath, encoded, { encoding: "utf8", mode: 0o600 });
          await chmod(temporaryPath, 0o600);
          await rename(temporaryPath, path);
        },
        catch: () => stateError("write"),
      });
    });

    const get = Effect.fn("PendingDeviceAuthorizationStore.File.get")(function* (origin: string) {
      const file = yield* load();
      return Option.fromNullishOr(file.authorizations[origin]);
    });

    const set = Effect.fn("PendingDeviceAuthorizationStore.File.set")(function* (
      authorization: PendingDeviceAuthorization,
    ) {
      const file = yield* load();
      yield* save({
        ...file,
        authorizations: {
          ...file.authorizations,
          [authorization.origin]: authorization,
        },
      });
    });

    const remove = Effect.fn("PendingDeviceAuthorizationStore.File.remove")(function* (
      origin: string,
    ) {
      const file = yield* load();
      const authorizations = { ...file.authorizations };
      delete authorizations[origin];
      yield* save({ ...file, authorizations });
    });

    return PendingDeviceAuthorizationStore.of({ get, set, remove });
  });

export const pendingDeviceAuthorizationStoreLayer = Layer.unwrap(
  Effect.gen(function* () {
    const environment = yield* EnvironmentConfiguration;
    const path = Option.isSome(environment.configPath)
      ? join(dirname(environment.configPath.value), "device-authorizations.json")
      : join(homedir(), ".config", "openagents", "device-authorizations.json");
    return pendingDeviceAuthorizationStoreFileLayer(path);
  }),
);

export const pendingDeviceAuthorizationStoreTestLayer = () =>
  Layer.sync(PendingDeviceAuthorizationStore, () => {
    const authorizations = new Map<string, PendingDeviceAuthorization>();
    return PendingDeviceAuthorizationStore.of({
      get: Effect.fn("PendingDeviceAuthorizationStore.Test.get")((origin: string) =>
        Effect.succeed(Option.fromNullishOr(authorizations.get(origin))),
      ),
      set: Effect.fn("PendingDeviceAuthorizationStore.Test.set")(
        (authorization: PendingDeviceAuthorization) =>
          Effect.sync(() => {
            authorizations.set(authorization.origin, authorization);
          }),
      ),
      remove: Effect.fn("PendingDeviceAuthorizationStore.Test.remove")((origin: string) =>
        Effect.sync(() => {
          authorizations.delete(origin);
        }),
      ),
    });
  });
