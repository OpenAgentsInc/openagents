import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { Effect, Layer, Schema as S } from "effect";

import {
  ForensicPriorWorkAuthorityError,
  type ForensicPriorWorkState,
  ForensicPriorWorkStateSchema,
  ForensicPriorWorkStateStore,
} from "./forensic-prior-work-authority.ts";

export const forensicPriorWorkStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "forensic-prior-work.v1.json");

const storageError = (detail: string) =>
  new ForensicPriorWorkAuthorityError({ reason: "storage_unavailable", detail });
const notFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";

const fileWriteQueues = new Map<string, Promise<void>>();

const withFileWriteLock = <A>(filePath: string, operation: () => Promise<A>): Promise<A> => {
  const previous = fileWriteQueues.get(filePath) ?? Promise.resolve();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const queued = previous.then(() => gate);
  fileWriteQueues.set(filePath, queued);
  return previous.then(operation).finally(() => {
    release();
    if (fileWriteQueues.get(filePath) === queued) fileWriteQueues.delete(filePath);
  });
};

const readState = (
  filePath: string,
): Effect.Effect<ForensicPriorWorkState | null, ForensicPriorWorkAuthorityError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (error) => (notFound(error) ? storageError("not_found") : storageError("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({ try: () => JSON.parse(contents), catch: () => storageError("json") }),
    ),
    Effect.flatMap((input) =>
      S.decodeUnknownEffect(ForensicPriorWorkStateSchema)(input, {
        onExcessProperty: "error",
      }).pipe(
        Effect.mapError(
          () => new ForensicPriorWorkAuthorityError({ reason: "invalid_state", detail: "decode" }),
        ),
      ),
    ),
    Effect.catch((error) =>
      error.detail === "not_found" ? Effect.succeed(null) : Effect.fail(error),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: ForensicPriorWorkState,
): Effect.Effect<void, ForensicPriorWorkAuthorityError> =>
  Effect.tryPromise({
    try: async () => {
      const directory = path.dirname(filePath);
      await mkdir(directory, { recursive: true });
      const temporaryPath = path.join(
        directory,
        `.${path.basename(filePath)}.${randomBytes(8).toString("hex")}.tmp`,
      );
      await writeFile(temporaryPath, `${JSON.stringify(state, null, 2)}\n`, {
        encoding: "utf8",
        mode: 0o600,
      });
      await rename(temporaryPath, filePath);
    },
    catch: () => storageError("write"),
  });

export const fileForensicPriorWorkStateStoreLayer = (
  rootDir: string,
): Layer.Layer<ForensicPriorWorkStateStore> => {
  const filePath = forensicPriorWorkStatePath(rootDir);
  const load = readState(filePath);
  return Layer.succeed(
    ForensicPriorWorkStateStore,
    ForensicPriorWorkStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.tryPromise({
          try: () =>
            withFileWriteLock(filePath, async () => {
              const current = await Effect.runPromise(load);
              if (current === null || current.revision !== expectedRevision) {
                throw new ForensicPriorWorkAuthorityError({
                  reason: "revision_conflict",
                  detail: `expected ${expectedRevision}, found ${current?.revision ?? "none"}`,
                });
              }
              await Effect.runPromise(atomicWrite(filePath, state));
            }),
          catch: (error) =>
            error instanceof ForensicPriorWorkAuthorityError ? error : storageError("write_lock"),
        }),
    }),
  );
};

export const initializeFileForensicPriorWorkState = (
  rootDir: string,
  state: ForensicPriorWorkState,
): Effect.Effect<void, ForensicPriorWorkAuthorityError> => {
  const filePath = forensicPriorWorkStatePath(rootDir);
  return readState(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};
