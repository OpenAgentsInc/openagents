import { createHash, randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import {
  decodeWorkCutoverExecuteRequest,
  decodeWorkCutoverReadRequest,
  decodeWorkCutoverState,
  type WorkCutoverExecuteRequest,
  type WorkCutoverExecuteResult,
  WorkCutoverExecuteResultSchema,
  type WorkCutoverReadResult,
  WorkCutoverReadResultSchema,
  type WorkCutoverReceipt,
  WorkCutoverReceiptSchema,
  type WorkCutoverState,
  WorkCutoverStateSchema,
} from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

export const WORK_CUTOVER_AUTHORITY_STATE_SCHEMA =
  "openagents.all_work_cutover_authority_state.v1" as const;
export const WORK_CUTOVER_WRITE_CAPABILITY = "capability:work-cutover:write" as const;

export const WorkCutoverAuthorityStateSchema = S.Struct({
  schema: S.Literal(WORK_CUTOVER_AUTHORITY_STATE_SCHEMA),
  cutover: WorkCutoverStateSchema,
  receipts: S.Array(WorkCutoverReceiptSchema),
});
export interface WorkCutoverAuthorityState extends S.Schema.Type<
  typeof WorkCutoverAuthorityStateSchema
> {}

export class WorkCutoverAuthorityError extends S.TaggedErrorClass<WorkCutoverAuthorityError>()(
  "WorkCutoverAuthority.Error",
  {
    reason: S.Literals([
      "invalid_state",
      "invalid_request",
      "storage_unavailable",
      "revision_conflict",
      "idempotency_conflict",
      "stale_generation",
      "forbidden",
      "wrong_writer",
      "source_changed",
      "native_history_gap",
      "cursor_conflict",
    ]),
    detail: S.String,
  },
) {}

export interface WorkCutoverStateStoreShape {
  readonly load: Effect.Effect<WorkCutoverAuthorityState | null, WorkCutoverAuthorityError>;
  readonly save: (
    expectedRevision: number,
    state: WorkCutoverAuthorityState,
  ) => Effect.Effect<void, WorkCutoverAuthorityError>;
}

export class WorkCutoverStateStore extends Context.Service<
  WorkCutoverStateStore,
  WorkCutoverStateStoreShape
>()("WorkCutoverAuthority.StateStore") {}

const digest = (value: unknown): string =>
  createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex");

const decodeState = (input: unknown) =>
  S.decodeUnknownEffect(WorkCutoverAuthorityStateSchema)(input, {
    onExcessProperty: "error",
  }).pipe(
    Effect.mapError(
      () => new WorkCutoverAuthorityError({ reason: "invalid_state", detail: "decode" }),
    ),
  );

export const emptyWorkCutoverAuthorityState = (input: {
  readonly organizationRef: string;
  readonly authorizedPrincipalRefs: ReadonlyArray<string>;
  readonly sourceDigest: string;
  readonly sourceCursor: string;
}): WorkCutoverAuthorityState => ({
  schema: WORK_CUTOVER_AUTHORITY_STATE_SCHEMA,
  cutover: decodeWorkCutoverState({
    contractVersion: "openagents.all_work_boundary.v1",
    organizationRef: input.organizationRef,
    authorizedPrincipalRefs: [...new Set(input.authorizedPrincipalRefs)].sort((a, b) =>
      a.localeCompare(b),
    ),
    revision: 1,
    generation: 1,
    writer: "legacy_github",
    sourceDigest: input.sourceDigest,
    sourceCursor: input.sourceCursor,
    nativeHighWatermark: null,
    activationReceiptRef: null,
    rollbackReceiptRef: null,
  }),
  receipts: [],
});

const cursorOrdinal = (cursor: string): number | null => {
  const match = /:(\d+)$/u.exec(cursor);
  if (match === null) return null;
  const value = Number(match[1]);
  return Number.isSafeInteger(value) ? value : null;
};

const applyCommand = (
  current: WorkCutoverState,
  request: WorkCutoverExecuteRequest,
): WorkCutoverState => {
  if (request.organizationRef !== current.organizationRef) {
    throw new WorkCutoverAuthorityError({ reason: "forbidden", detail: "organization" });
  }
  if (!current.authorizedPrincipalRefs.includes(request.effectivePrincipalRef)) {
    throw new WorkCutoverAuthorityError({ reason: "forbidden", detail: "principal" });
  }
  if (request.capabilityRef !== WORK_CUTOVER_WRITE_CAPABILITY) {
    throw new WorkCutoverAuthorityError({ reason: "forbidden", detail: "capability" });
  }
  const command = request.command;
  let patch: Partial<WorkCutoverState>;
  switch (command.command) {
    case "bind_shadow":
      if (current.writer !== "legacy_github") {
        throw new WorkCutoverAuthorityError({ reason: "wrong_writer", detail: current.writer });
      }
      patch = { sourceDigest: command.sourceDigest, sourceCursor: command.sourceCursor };
      break;
    case "activate_native":
      if (current.writer !== "legacy_github") {
        throw new WorkCutoverAuthorityError({ reason: "wrong_writer", detail: current.writer });
      }
      if (
        command.sourceDigest !== current.sourceDigest ||
        command.reconciledCursor !== current.sourceCursor
      ) {
        throw new WorkCutoverAuthorityError({
          reason: "source_changed",
          detail: "shadow source digest or cursor changed before activation",
        });
      }
      patch = {
        writer: "native_omega",
        generation: current.generation + 1,
        nativeHighWatermark: command.reconciledCursor,
        activationReceiptRef: command.receiptRef,
        rollbackReceiptRef: null,
      };
      break;
    case "record_native_write": {
      if (current.writer !== "native_omega") {
        throw new WorkCutoverAuthorityError({ reason: "wrong_writer", detail: current.writer });
      }
      const previous =
        current.nativeHighWatermark === null ? null : cursorOrdinal(current.nativeHighWatermark);
      const next = cursorOrdinal(command.eventCursor);
      if (previous === null || next === null || next <= previous) {
        throw new WorkCutoverAuthorityError({
          reason: "cursor_conflict",
          detail: "native event cursor must advance monotonically",
        });
      }
      patch = { nativeHighWatermark: command.eventCursor };
      break;
    }
    case "rollback_legacy":
      if (current.writer !== "native_omega") {
        throw new WorkCutoverAuthorityError({ reason: "wrong_writer", detail: current.writer });
      }
      if (command.reconciledNativeCursor !== current.nativeHighWatermark) {
        throw new WorkCutoverAuthorityError({
          reason: "native_history_gap",
          detail: "rollback cursor does not cover the native high-water mark",
        });
      }
      patch = {
        writer: "legacy_github",
        generation: current.generation + 1,
        sourceCursor: command.reconciledNativeCursor,
        rollbackReceiptRef: command.receiptRef,
      };
      break;
  }
  return decodeWorkCutoverState({ ...current, ...patch, revision: current.revision + 1 });
};

export interface WorkCutoverAuthorityShape {
  readonly read: (
    input: unknown,
  ) => Effect.Effect<WorkCutoverReadResult, WorkCutoverAuthorityError>;
  readonly execute: (
    input: unknown,
  ) => Effect.Effect<WorkCutoverExecuteResult, WorkCutoverAuthorityError>;
}

export class WorkCutoverAuthority extends Context.Service<
  WorkCutoverAuthority,
  WorkCutoverAuthorityShape
>()("WorkCutoverAuthority.Service") {}

export const WorkCutoverAuthorityLive = Layer.effect(
  WorkCutoverAuthority,
  Effect.gen(function* () {
    const store = yield* WorkCutoverStateStore;
    const load = store.load.pipe(
      Effect.flatMap((state) =>
        state === null
          ? Effect.fail(
              new WorkCutoverAuthorityError({ reason: "invalid_state", detail: "store is empty" }),
            )
          : Effect.succeed(state),
      ),
    );
    return WorkCutoverAuthority.of({
      read: Effect.fn("WorkCutoverAuthority.read")(function* (input: unknown) {
        yield* Effect.try({
          try: () => decodeWorkCutoverReadRequest(input),
          catch: () =>
            new WorkCutoverAuthorityError({ reason: "invalid_request", detail: "decode" }),
        });
        const state = yield* load;
        return yield* S.decodeUnknownEffect(WorkCutoverReadResultSchema)({
          state: state.cutover,
        }).pipe(
          Effect.mapError(
            () => new WorkCutoverAuthorityError({ reason: "invalid_state", detail: "read" }),
          ),
        );
      }),
      execute: Effect.fn("WorkCutoverAuthority.execute")(function* (input: unknown) {
        const request = yield* Effect.try({
          try: () => decodeWorkCutoverExecuteRequest(input),
          catch: () =>
            new WorkCutoverAuthorityError({ reason: "invalid_request", detail: "decode" }),
        });
        const state = yield* load;
        const commandDigest = digest(request);
        const prior = state.receipts.find(
          (receipt) => receipt.idempotencyKey === request.idempotencyKey,
        );
        if (prior !== undefined) {
          if (prior.commandDigest !== commandDigest) {
            return yield* new WorkCutoverAuthorityError({
              reason: "idempotency_conflict",
              detail: request.idempotencyKey,
            });
          }
          return yield* S.decodeUnknownEffect(WorkCutoverExecuteResultSchema)({
            state: state.cutover,
            receipt: prior,
          }).pipe(
            Effect.mapError(
              () => new WorkCutoverAuthorityError({ reason: "invalid_state", detail: "replay" }),
            ),
          );
        }
        if (request.expectedRevision !== state.cutover.revision) {
          return yield* new WorkCutoverAuthorityError({
            reason: "revision_conflict",
            detail: `expected ${request.expectedRevision}, found ${state.cutover.revision}`,
          });
        }
        if (request.expectedGeneration !== state.cutover.generation) {
          return yield* new WorkCutoverAuthorityError({
            reason: "stale_generation",
            detail: `expected ${request.expectedGeneration}, found ${state.cutover.generation}`,
          });
        }
        const nextCutover = yield* Effect.try({
          try: () => applyCommand(state.cutover, request),
          catch: (error) =>
            error instanceof WorkCutoverAuthorityError
              ? error
              : new WorkCutoverAuthorityError({
                  reason: "invalid_request",
                  detail: "transition",
                }),
        });
        const receipt = yield* S.decodeUnknownEffect(WorkCutoverReceiptSchema)({
          intentRef: request.intentRef,
          idempotencyKey: request.idempotencyKey,
          commandDigest,
          previousRevision: state.cutover.revision,
          revision: nextCutover.revision,
          generation: nextCutover.generation,
          writer: nextCutover.writer,
          effectivePrincipalRef: request.effectivePrincipalRef,
          acceptedAt: request.occurredAt,
          githubWriteCount: 0,
        }).pipe(
          Effect.mapError(
            () => new WorkCutoverAuthorityError({ reason: "invalid_state", detail: "receipt" }),
          ),
        );
        const next = yield* decodeState({
          schema: WORK_CUTOVER_AUTHORITY_STATE_SCHEMA,
          cutover: nextCutover,
          receipts: [...state.receipts, receipt],
        });
        yield* store.save(state.cutover.revision, next);
        return yield* S.decodeUnknownEffect(WorkCutoverExecuteResultSchema)({
          state: next.cutover,
          receipt,
        }).pipe(
          Effect.mapError(
            () => new WorkCutoverAuthorityError({ reason: "invalid_state", detail: "result" }),
          ),
        );
      }),
    });
  }),
);

export const inMemoryWorkCutoverStateStoreLayer = (
  initial: WorkCutoverAuthorityState,
): Layer.Layer<WorkCutoverStateStore> =>
  Layer.effect(
    WorkCutoverStateStore,
    Effect.gen(function* () {
      const state = yield* Ref.make(initial);
      return WorkCutoverStateStore.of({
        load: Ref.get(state),
        save: (expectedRevision, next) =>
          Ref.modify(state, (current) =>
            current.cutover.revision !== expectedRevision
              ? [
                  Effect.fail(
                    new WorkCutoverAuthorityError({
                      reason: "revision_conflict",
                      detail: `expected ${expectedRevision}, found ${current.cutover.revision}`,
                    }),
                  ),
                  current,
                ]
              : [Effect.void, next],
          ).pipe(Effect.flatten),
      });
    }),
  );

export const workCutoverStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "work-cutover.v1.json");

const storageError = (detail: string) =>
  new WorkCutoverAuthorityError({ reason: "storage_unavailable", detail });
const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";

const readFileState = (
  filePath: string,
): Effect.Effect<WorkCutoverAuthorityState | null, WorkCutoverAuthorityError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (error) => (isNotFound(error) ? storageError("not_found") : storageError("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({ try: () => JSON.parse(contents), catch: () => storageError("json") }),
    ),
    Effect.flatMap(decodeState),
    Effect.catch((error) =>
      error.detail === "not_found" ? Effect.succeed(null) : Effect.fail(error),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: WorkCutoverAuthorityState,
): Effect.Effect<void, WorkCutoverAuthorityError> =>
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
    catch: () => storageError("write"),
  });

export const fileWorkCutoverStateStoreLayer = (
  rootDir: string,
): Layer.Layer<WorkCutoverStateStore> => {
  const filePath = workCutoverStatePath(rootDir);
  const load = readFileState(filePath);
  return Layer.succeed(
    WorkCutoverStateStore,
    WorkCutoverStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.gen(function* () {
          const current = yield* load;
          if (current === null || current.cutover.revision !== expectedRevision) {
            return yield* new WorkCutoverAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${current?.cutover.revision ?? "none"}`,
            });
          }
          yield* atomicWrite(filePath, state);
        }),
    }),
  );
};

export const initializeFileWorkCutoverState = (
  rootDir: string,
  state: WorkCutoverAuthorityState,
): Effect.Effect<void, WorkCutoverAuthorityError> => {
  const filePath = workCutoverStatePath(rootDir);
  return readFileState(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};
