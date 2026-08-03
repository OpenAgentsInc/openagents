import { randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { Effect, Layer, Schema as S } from "effect";

import {
  PlanningAuthorityError,
  type PlanningAuthorityState,
  PlanningAuthorityStateSchema,
  PlanningStateStore,
} from "./planning-authority.ts";

export const planningAuthorityStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "planning-authority.v1.json");

const storageError = (detail: string) =>
  new PlanningAuthorityError({ reason: "storage_unavailable", detail });

const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";

const readUnknown = (filePath: string): Effect.Effect<unknown | null, PlanningAuthorityError> =>
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
): Effect.Effect<PlanningAuthorityState, PlanningAuthorityError> =>
  S.decodeUnknownEffect(PlanningAuthorityStateSchema)(input, {
    onExcessProperty: "error",
  }).pipe(
    Effect.mapError(
      () => new PlanningAuthorityError({ reason: "invalid_state", detail: "decode" }),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: PlanningAuthorityState,
): Effect.Effect<void, PlanningAuthorityError> =>
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

/**
 * Durable public-safe planning state. A save compares the persisted graph
 * revision before an atomic replace. A multi-process host must serialize this
 * port with its normal single-writer lease; revision mismatch still fails
 * closed after restart or stale replay.
 */
export const filePlanningStateStoreLayer = (rootDir: string): Layer.Layer<PlanningStateStore> => {
  const filePath = planningAuthorityStatePath(rootDir);
  const load = readUnknown(filePath).pipe(
    Effect.flatMap((input) => (input === null ? Effect.succeed(null) : decodeState(input))),
  );
  return Layer.succeed(
    PlanningStateStore,
    PlanningStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.gen(function* () {
          const current = yield* load;
          if (current === null || current.graph.revision !== expectedRevision) {
            return yield* new PlanningAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${current?.graph.revision ?? "none"}`,
            });
          }
          yield* atomicWrite(filePath, state);
        }),
    }),
  );
};

export const initializeFilePlanningState = (
  rootDir: string,
  state: PlanningAuthorityState,
): Effect.Effect<void, PlanningAuthorityError> => {
  const filePath = planningAuthorityStatePath(rootDir);
  return readUnknown(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};
