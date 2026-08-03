import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { Effect, Layer, Schema as S } from "effect";

import {
  SignedWorkroomError,
  type SignedWorkroomState,
  SignedWorkroomStateSchema,
  SignedWorkroomStateStore,
  validateSignedWorkroomState,
} from "./signed-workroom-authority.ts";

export const signedWorkroomStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "signed-workroom.v1.json");

const error = (detail: string) =>
  new SignedWorkroomError({ reason: "storage_unavailable", detail });
const notFound = (cause: unknown): boolean =>
  typeof cause === "object" && cause !== null && Reflect.get(cause, "code") === "ENOENT";

const migratePreDeliveryOutbox = (input: unknown): unknown => {
  if (typeof input !== "object" || input === null) return input;
  const ledger = Reflect.get(input, "ledger");
  if (typeof ledger !== "object" || ledger === null) return input;
  const outbox = Reflect.get(ledger, "outbox");
  if (!Array.isArray(outbox)) return input;
  return {
    ...input,
    ledger: {
      ...ledger,
      outbox: outbox.map((record) =>
        typeof record === "object" && record !== null && !Object.hasOwn(record, "deliveryAttempts")
          ? { ...record, deliveryAttempts: [] }
          : record,
      ),
    },
  };
};

const readState = (
  filePath: string,
): Effect.Effect<SignedWorkroomState | null, SignedWorkroomError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (cause) => (notFound(cause) ? error("not_found") : error("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({ try: () => JSON.parse(contents), catch: () => error("json") }),
    ),
    Effect.flatMap((input) =>
      S.decodeUnknownEffect(SignedWorkroomStateSchema)(migratePreDeliveryOutbox(input), {
        onExcessProperty: "error",
      }).pipe(Effect.mapError(() => error("decode"))),
    ),
    Effect.flatMap((state) => validateSignedWorkroomState(state).pipe(Effect.map(() => state))),
    Effect.catch((cause) =>
      cause.detail === "not_found" ? Effect.succeed(null) : Effect.fail(cause),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: SignedWorkroomState,
): Effect.Effect<void, SignedWorkroomError> =>
  Effect.tryPromise({
    try: async () => {
      const directory = path.dirname(filePath);
      await mkdir(directory, { recursive: true });
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
    catch: () => error("write"),
  });

export const fileSignedWorkroomStateStoreLayer = (
  rootDir: string,
): Layer.Layer<SignedWorkroomStateStore> => {
  const filePath = signedWorkroomStatePath(rootDir);
  const load = readState(filePath);
  return Layer.succeed(
    SignedWorkroomStateStore,
    SignedWorkroomStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.gen(function* () {
          const current = yield* load;
          if (current === null || current.ledger.revision !== expectedRevision) {
            return yield* new SignedWorkroomError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${current?.ledger.revision ?? "none"}`,
            });
          }
          yield* atomicWrite(filePath, state);
        }),
    }),
  );
};

export const initializeFileSignedWorkroomState = (
  rootDir: string,
  state: SignedWorkroomState,
): Effect.Effect<void, SignedWorkroomError> => {
  const filePath = signedWorkroomStatePath(rootDir);
  return readState(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};
