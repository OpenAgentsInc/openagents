import { Context, Effect, Layer, Ref, Schema, Semaphore } from "effect";

import {
  IdeLanguageCancelRequestSchema,
  IdeLanguageProviderStartSchema,
  IdeLanguageStartRefSchema,
  IdeLanguageRequestSchema,
  IdeLanguageResultSchema,
  IdeLanguageServiceSnapshotSchema,
  IdeLanguageStopRequestSchema,
  type IdeLanguageCancelRequest,
  type IdeLanguageProviderStart,
  type IdeLanguageRequest,
  type IdeLanguageResult,
  type IdeLanguageServiceSnapshot,
  type IdeLanguageStopRequest,
} from "./language-contract.ts";
import {
  IdeLanguageServiceRefSchema,
  IdePlacementRefSchema,
  IdeServiceGenerationSchema,
  IdeTimestampSchema,
} from "./project-contract.ts";

export class IdeLanguageInvalidInput extends Schema.TaggedErrorClass<IdeLanguageInvalidInput>()(
  "IdeLanguage.InvalidInput",
  {
    operation: Schema.String,
    detail: Schema.String,
  },
) {}

export class IdeLanguageStaleGeneration extends Schema.TaggedErrorClass<IdeLanguageStaleGeneration>()(
  "IdeLanguage.StaleGeneration",
  {
    operation: Schema.String,
    generationKind: Schema.Literals(["service", "attachment", "language", "document", "document_version"]),
    expected: Schema.Number,
    actual: Schema.Number,
  },
) {}

export class IdeLanguageProviderUnavailable extends Schema.TaggedErrorClass<IdeLanguageProviderUnavailable>()(
  "IdeLanguage.ProviderUnavailable",
  {
    operation: Schema.String,
    reason: Schema.String,
    retry: Schema.Literals(["none", "manual", "bounded_backoff"]),
  },
) {}

export class IdeLanguageTimedOut extends Schema.TaggedErrorClass<IdeLanguageTimedOut>()(
  "IdeLanguage.TimedOut",
  {
    operation: Schema.String,
    requestRef: Schema.String,
    timeoutMs: Schema.Number,
  },
) {}

export class IdeLanguageMalformedResult extends Schema.TaggedErrorClass<IdeLanguageMalformedResult>()(
  "IdeLanguage.MalformedResult",
  {
    operation: Schema.String,
    requestRef: Schema.String,
    detail: Schema.String,
  },
) {}

export class IdeLanguageStopped extends Schema.TaggedErrorClass<IdeLanguageStopped>()(
  "IdeLanguage.Stopped",
  {
    operation: Schema.String,
    reason: Schema.String,
  },
) {}

export class IdeLanguageQueueFull extends Schema.TaggedErrorClass<IdeLanguageQueueFull>()(
  "IdeLanguage.QueueFull",
  {
    operation: Schema.String,
    limit: Schema.Number,
  },
) {}

export const IdeLanguageServiceErrorSchema = Schema.Union([
  IdeLanguageInvalidInput,
  IdeLanguageStaleGeneration,
  IdeLanguageProviderUnavailable,
  IdeLanguageTimedOut,
  IdeLanguageMalformedResult,
  IdeLanguageStopped,
  IdeLanguageQueueFull,
]);
export type IdeLanguageServiceError = typeof IdeLanguageServiceErrorSchema.Type;

export interface IdeLanguageProvider {
  readonly start: () => Promise<unknown>;
  readonly request: (request: IdeLanguageRequest) => Promise<unknown>;
  readonly cancel: (request: IdeLanguageCancelRequest) => Promise<void>;
  readonly stop: (request: IdeLanguageStopRequest) => Promise<void>;
}

export interface IdeLanguageServiceShape {
  readonly snapshot: () => Effect.Effect<IdeLanguageServiceSnapshot>;
  readonly request: (
    request: IdeLanguageRequest,
  ) => Effect.Effect<IdeLanguageResult, IdeLanguageServiceError>;
  readonly cancel: (
    request: IdeLanguageCancelRequest,
  ) => Effect.Effect<boolean, IdeLanguageInvalidInput>;
  readonly restart: (
    reason: string,
  ) => Effect.Effect<IdeLanguageServiceSnapshot, IdeLanguageServiceError>;
  readonly stop: (
    request: IdeLanguageStopRequest,
  ) => Effect.Effect<IdeLanguageServiceSnapshot, IdeLanguageInvalidInput>;
}

export class IdeLanguageService extends Context.Service<IdeLanguageService, IdeLanguageServiceShape>()(
  "@openagentsinc/openagents-desktop/IdeLanguageService",
) {}

const serviceRef = IdeLanguageServiceRefSchema.make("ide.language-service.typescript");
const placementRef = IdePlacementRefSchema.make("ide.placement.project-local");
const maximumPendingRequests = 64;
const startupTimeoutMs = 5_000;

const timestamp = (): typeof IdeTimestampSchema.Type =>
  IdeTimestampSchema.make(new Date().toISOString());

const decodeInput = <S extends Schema.ConstraintDecoder<unknown, never>>(
  operation: string,
  schema: S,
  value: unknown,
): Effect.Effect<S["Type"], IdeLanguageInvalidInput> =>
  Schema.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(cause => new IdeLanguageInvalidInput({
      operation,
      detail: String(cause).slice(0, 800),
    })),
  );

const startRefFor = (generation: number, restartCount: number) =>
  IdeLanguageStartRefSchema.make(`ide.language-start.${generation}.${restartCount}`);

const stoppedProviderRequest = (
  grantRef: string,
  reason: IdeLanguageStopRequest["reason"],
): IdeLanguageStopRequest => IdeLanguageStopRequestSchema.make({
  schemaVersion: "openagents.desktop.ide-language-stop.v1",
  grantRef,
  reason,
});

const staleResult = (
  result: IdeLanguageResult,
  reason: Extract<IdeLanguageResult["state"], { _tag: "Stale" }>["reason"],
): IdeLanguageResult => IdeLanguageResultSchema.make({
  ...result,
  state: { _tag: "Stale", reason },
  items: [],
  excerpt: null,
});

export const makeIdeLanguageService = (
  provider: IdeLanguageProvider,
): Effect.Effect<IdeLanguageServiceShape> => Effect.gen(function* () {
  const state = yield* Ref.make<IdeLanguageServiceSnapshot>(
    IdeLanguageServiceSnapshotSchema.cases.Unconfigured.make({ serviceRef }),
  );
  const generation = yield* Ref.make(1);
  const restartCount = yield* Ref.make(0);
  const pending = yield* Ref.make(new Map<string, IdeLanguageRequest>());
  const latestByDocumentCapability = yield* Ref.make(new Map<string, string>());
  const startLock = yield* Semaphore.make(1);
  const lastGrantRef = yield* Ref.make("workspace.language");

  const markProviderFailure = Effect.fn("IdeLanguageService.markProviderFailure")(
    function* (reason: string, recoverable: boolean, advanceGeneration = false) {
      const currentGeneration = advanceGeneration
        ? yield* Ref.updateAndGet(generation, value => value + 1)
        : yield* Ref.get(generation);
      const currentRestarts = yield* Ref.updateAndGet(restartCount, value => Math.min(100, value + 1));
      const startRef = startRefFor(currentGeneration, currentRestarts);
      const retryAt = recoverable
        ? new Date(Date.now() + Math.min(2_000, 100 * 2 ** Math.min(4, currentRestarts))).toISOString()
        : null;
      yield* Ref.set(state, IdeLanguageServiceSnapshotSchema.cases.Degraded.make({
        serviceRef,
        serviceGeneration: IdeServiceGenerationSchema.make(currentGeneration),
        startRef,
        placementRef,
        evidenceTier: "project_local",
        reason: reason.slice(0, 800),
        recoverable,
        retryAt,
        restartCount: currentRestarts,
      }));
    },
  );

  const start = Effect.fn("IdeLanguageService.start")(function* () {
    return yield* startLock.withPermit(Effect.gen(function* () {
      const current = yield* Ref.get(state);
      if (current._tag === "Ready") return current;
      if (current._tag === "Stopped") {
        return yield* new IdeLanguageStopped({
          operation: "IdeLanguageService.start",
          reason: current.reason,
        });
      }
      const currentGeneration = yield* Ref.get(generation);
      const currentRestarts = yield* Ref.get(restartCount);
      const startRef = startRefFor(currentGeneration, currentRestarts);
      yield* Ref.set(state, IdeLanguageServiceSnapshotSchema.cases.Starting.make({
        serviceRef,
        serviceGeneration: IdeServiceGenerationSchema.make(currentGeneration),
        startRef,
        since: timestamp(),
        attempt: currentRestarts + 1,
      }));
      const started = yield* Effect.timeoutOrElse(
        Effect.tryPromise({
          try: () => provider.start(),
          catch: cause => new IdeLanguageProviderUnavailable({
            operation: "IdeLanguageService.start",
            reason: String(cause).slice(0, 800),
            retry: "bounded_backoff",
          }),
        }),
        {
          duration: `${startupTimeoutMs} millis`,
          orElse: () => Effect.fail(new IdeLanguageTimedOut({
            operation: "IdeLanguageService.start",
            requestRef: startRef,
            timeoutMs: startupTimeoutMs,
          })),
        },
      ).pipe(
        Effect.flatMap(value => Schema.decodeUnknownEffect(IdeLanguageProviderStartSchema)(value).pipe(
          Effect.mapError(cause => new IdeLanguageProviderUnavailable({
            operation: "IdeLanguageService.start",
            reason: `Malformed provider start response: ${String(cause).slice(0, 700)}`,
            retry: "bounded_backoff",
          })),
        )),
        Effect.tapError(error => markProviderFailure(String(error), true)),
      );
      const ready = IdeLanguageServiceSnapshotSchema.cases.Ready.make({
        serviceRef,
        serviceGeneration: IdeServiceGenerationSchema.make(currentGeneration),
        startRef,
        placementRef,
        evidenceTier: "project_local",
        executable: started.executable,
        providerVersion: started.providerVersion,
        capabilities: started.capabilities,
        startedAt: timestamp(),
        activeRequests: 0,
        queuedRequests: 0,
        restartCount: currentRestarts,
      });
      yield* Ref.set(state, ready);
      return ready;
    }));
  });

  const cancel = Effect.fn("IdeLanguageService.cancel")(function* (value: IdeLanguageCancelRequest) {
    const request = yield* decodeInput("IdeLanguageService.cancel", IdeLanguageCancelRequestSchema, value);
    const all = yield* Ref.get(pending);
    const existed = all.has(String(request.requestRef));
    if (existed) {
      yield* Effect.promise(() => provider.cancel(request).catch(() => undefined));
      yield* Ref.update(pending, current => {
        const next = new Map(current);
        next.delete(String(request.requestRef));
        return next;
      });
    }
    return existed;
  });

  const request = Effect.fn("IdeLanguageService.request")(function* (value: IdeLanguageRequest) {
    const input = yield* decodeInput("IdeLanguageService.request", IdeLanguageRequestSchema, value);
    yield* Ref.set(lastGrantRef, input.grantRef);
    const pendingBefore = yield* Ref.get(pending);
    if (pendingBefore.size >= maximumPendingRequests) {
      return yield* new IdeLanguageQueueFull({
        operation: "IdeLanguageService.request",
        limit: maximumPendingRequests,
      });
    }
    const ready = yield* start();
    if (input.expectedServiceGeneration !== null && input.expectedServiceGeneration !== ready.serviceGeneration) {
      return yield* new IdeLanguageStaleGeneration({
        operation: "IdeLanguageService.request",
        generationKind: "service",
        expected: input.expectedServiceGeneration,
        actual: ready.serviceGeneration,
      });
    }
    const pendingNow = yield* Ref.get(pending);
    const key = `${input.documentRef}:${input.capability}`;
    const previousRequestRef = (yield* Ref.get(latestByDocumentCapability)).get(key);
    if (previousRequestRef !== undefined && previousRequestRef !== input.requestRef) {
      const previous = pendingNow.get(previousRequestRef);
      if (previous !== undefined) {
        yield* Effect.promise(() => provider.cancel(IdeLanguageCancelRequestSchema.make({
          schemaVersion: "openagents.desktop.ide-language-cancel.v1",
          grantRef: previous.grantRef,
          requestRef: previous.requestRef,
          reason: "superseded",
        })).catch(() => undefined));
      }
    }
    yield* Ref.update(latestByDocumentCapability, current => {
      const next = new Map(current);
      next.set(key, String(input.requestRef));
      return next;
    });
    yield* Ref.update(pending, current => {
      const next = new Map(current);
      next.set(String(input.requestRef), input);
      return next;
    });
    yield* Ref.update(state, current => current._tag !== "Ready" ? current : {
      ...current,
      activeRequests: Math.min(1_000, current.activeRequests + 1),
      queuedRequests: Math.min(1_000, Math.max(0, pendingNow.size - 3)),
    });

    const cleanup = Effect.gen(function* () {
      yield* Ref.update(pending, current => {
        const next = new Map(current);
        next.delete(String(input.requestRef));
        return next;
      });
      yield* Ref.update(state, current => current._tag !== "Ready" ? current : {
        ...current,
        activeRequests: Math.max(0, current.activeRequests - 1),
        queuedRequests: Math.max(0, current.queuedRequests - 1),
      });
    });

    const run = Effect.timeoutOrElse(
        Effect.tryPromise({
          try: () => provider.request(input),
          catch: cause => new IdeLanguageProviderUnavailable({
            operation: "IdeLanguageService.request",
            reason: String(cause).slice(0, 800),
            retry: "bounded_backoff",
          }),
        }),
        {
          duration: `${input.timeoutMs} millis`,
          orElse: () => Effect.andThen(
            Effect.promise(() => provider.cancel(IdeLanguageCancelRequestSchema.make({
              schemaVersion: "openagents.desktop.ide-language-cancel.v1",
              grantRef: input.grantRef,
              requestRef: input.requestRef,
              reason: "superseded",
            })).catch(() => undefined)),
            Effect.fail(new IdeLanguageTimedOut({
              operation: "IdeLanguageService.request",
              requestRef: input.requestRef,
              timeoutMs: input.timeoutMs,
            })),
          ),
        },
      ).pipe(
      Effect.ensuring(cleanup),
      Effect.tapError(error => error._tag === "IdeLanguage.ProviderUnavailable"
        ? markProviderFailure(error.reason, true, true)
        : Effect.void),
    );
    const raw = yield* run;
    const decoded = yield* Schema.decodeUnknownEffect(IdeLanguageResultSchema)(raw).pipe(
      Effect.mapError(cause => new IdeLanguageMalformedResult({
        operation: "IdeLanguageService.request",
        requestRef: input.requestRef,
        detail: String(cause).slice(0, 800),
      })),
    );
    if (decoded.requestRef !== input.requestRef || decoded.documentRef !== input.documentRef) {
      return yield* new IdeLanguageMalformedResult({
        operation: "IdeLanguageService.request",
        requestRef: input.requestRef,
        detail: "Provider response identity did not match the request.",
      });
    }
    const latest = (yield* Ref.get(latestByDocumentCapability)).get(key);
    if (latest !== input.requestRef) return staleResult(decoded, "document_version_replaced");
    const currentSnapshot = yield* Ref.get(state);
    if (currentSnapshot._tag !== "Ready") return staleResult(decoded, "service_generation_replaced");
    if (decoded.serviceGeneration !== currentSnapshot.serviceGeneration) {
      return staleResult(decoded, "service_generation_replaced");
    }
    if (decoded.attachmentGeneration !== input.attachmentGeneration) {
      return staleResult(decoded, "attachment_generation_replaced");
    }
    if (decoded.languageGeneration !== input.languageGeneration) {
      return staleResult(decoded, "language_generation_replaced");
    }
    if (decoded.documentGeneration !== input.documentGeneration) {
      return staleResult(decoded, "document_generation_replaced");
    }
    if (decoded.documentVersion !== input.documentVersion) {
      return staleResult(decoded, "document_version_replaced");
    }
    return decoded;
  });

  const restart = Effect.fn("IdeLanguageService.restart")(function* (reason: string) {
    const grantRef = yield* Ref.get(lastGrantRef);
    const boundedReason = reason.slice(0, 800);
    yield* Effect.promise(() => provider.stop(stoppedProviderRequest(grantRef, "manual_restart")).catch(() => undefined));
    const pendingRequests = yield* Ref.get(pending);
    for (const value of pendingRequests.values()) {
      yield* Effect.promise(() => provider.cancel(IdeLanguageCancelRequestSchema.make({
        schemaVersion: "openagents.desktop.ide-language-cancel.v1",
        grantRef: value.grantRef,
        requestRef: value.requestRef,
        reason: "project_stopped",
      })).catch(() => undefined));
    }
    yield* Ref.set(pending, new Map());
    yield* Ref.update(generation, value => value + 1);
    yield* markProviderFailure(boundedReason, true);
    return yield* start();
  });

  const stop = Effect.fn("IdeLanguageService.stop")(function* (value: IdeLanguageStopRequest) {
    const input = yield* decodeInput("IdeLanguageService.stop", IdeLanguageStopRequestSchema, value);
    const pendingRequests = yield* Ref.get(pending);
    for (const requestValue of pendingRequests.values()) {
      yield* Effect.promise(() => provider.cancel(IdeLanguageCancelRequestSchema.make({
        schemaVersion: "openagents.desktop.ide-language-cancel.v1",
        grantRef: requestValue.grantRef,
        requestRef: requestValue.requestRef,
        reason: "project_stopped",
      })).catch(() => undefined));
    }
    yield* Effect.promise(() => provider.stop(input).catch(() => undefined));
    yield* Ref.set(pending, new Map());
    yield* Ref.set(latestByDocumentCapability, new Map());
    const stopped = IdeLanguageServiceSnapshotSchema.cases.Stopped.make({
      serviceRef,
      reason: input.reason,
      stoppedAt: timestamp(),
      activeRequests: 0,
      queuedRequests: 0,
    });
    yield* Ref.set(state, stopped);
    return stopped;
  });

  return {
    snapshot: () => Ref.get(state),
    request,
    cancel,
    restart,
    stop,
  } satisfies IdeLanguageServiceShape;
});

export const IdeLanguageServiceLive = (
  provider: IdeLanguageProvider,
): Layer.Layer<IdeLanguageService> => Layer.effect(
  IdeLanguageService,
  makeIdeLanguageService(provider),
);

export type IdeLanguageProviderStartProjection = IdeLanguageProviderStart;
