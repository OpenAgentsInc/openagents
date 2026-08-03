import { createHash, randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { Effect, Layer, Schema as S } from "effect";

import {
  WorkCommandAuthorityError,
  type WorkCommandAuthorityState,
  WorkCommandAuthorityStateSchema,
  WorkCommandStateStore,
} from "./work-command-authority.ts";

const workDigest = (workRef: string): string => createHash("sha256").update(workRef).digest("hex");

export const workCommandAuthorityStatePath = (rootDir: string, workRef: string): string =>
  path.join(rootDir, "all-work", "commands", `${workDigest(workRef)}.v1.json`);

const storageError = (detail: string) =>
  new WorkCommandAuthorityError({ reason: "storage_unavailable", detail });

const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";

const readUnknown = (filePath: string): Effect.Effect<unknown | null, WorkCommandAuthorityError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (error) => (isNotFound(error) ? storageError("not_found") : storageError("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      contents === ""
        ? Effect.fail(storageError("empty"))
        : Effect.try({ try: () => JSON.parse(contents), catch: () => storageError("json") }),
    ),
    Effect.catch((error) =>
      error.detail === "not_found" ? Effect.succeed(null) : Effect.fail(error),
    ),
  );

const decodeState = (
  input: unknown,
): Effect.Effect<WorkCommandAuthorityState, WorkCommandAuthorityError> =>
  S.decodeUnknownEffect(WorkCommandAuthorityStateSchema)(input, {
    onExcessProperty: "error",
  }).pipe(
    Effect.mapError(
      () => new WorkCommandAuthorityError({ reason: "invalid_state", detail: "decode" }),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: WorkCommandAuthorityState,
): Effect.Effect<void, WorkCommandAuthorityError> =>
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

export const fileWorkCommandStateStoreLayer = (
  rootDir: string,
  workRef: string,
): Layer.Layer<WorkCommandStateStore> => {
  const filePath = workCommandAuthorityStatePath(rootDir, workRef);
  const load = readUnknown(filePath).pipe(
    Effect.flatMap((input) => (input === null ? Effect.succeed(null) : decodeState(input))),
  );
  return Layer.succeed(
    WorkCommandStateStore,
    WorkCommandStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.gen(function* () {
          const current = yield* load;
          if (current === null || current.snapshot.summary.revision !== expectedRevision) {
            return yield* new WorkCommandAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${current?.snapshot.summary.revision ?? "none"}`,
            });
          }
          yield* atomicWrite(filePath, state);
        }),
    }),
  );
};

export const initializeFileWorkCommandState = (
  rootDir: string,
  workRef: string,
  state: WorkCommandAuthorityState,
): Effect.Effect<void, WorkCommandAuthorityError> => {
  const filePath = workCommandAuthorityStatePath(rootDir, workRef);
  return readUnknown(filePath).pipe(
    Effect.flatMap((current) => {
      if (current !== null && current.snapshot.summary.workRef !== workRef) {
        return Effect.fail(
          new WorkCommandAuthorityError({
            reason: "invalid_state",
            detail: "hashed state path contains the wrong Work identity",
          }),
        );
      }
      return current === null ? atomicWrite(filePath, state) : Effect.void;
    }),
  );
};
