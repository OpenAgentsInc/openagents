import { Context, Effect, Layer, Ref, Schema as S } from "effect";

export const ANALYTICS_SCHEMA_VERSION = "openagents.analytics.events.v1";
export const ANALYTICS_BATCH_MAX_EVENTS = 20;

export const AnalyticsEventName = S.Literals(["page_view", "github_view", "forge_view"]);
export type AnalyticsEventName = typeof AnalyticsEventName.Type;

export const AnalyticsClientKind = S.Literals(["web", "mobile", "desktop"]);
export type AnalyticsClientKind = typeof AnalyticsClientKind.Type;

const AnalyticsEventId = S.String.check(
  S.isMinLength(16),
  S.isMaxLength(80),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

const AnalyticsRouteId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(120),
  S.isPattern(/^\/[A-Za-z0-9_./:-]*$/u),
);

const IsoTimestamp = S.String.check(
  S.isMaxLength(40),
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/u),
);

export const AnalyticsEvent = S.Struct({
  schemaVersion: S.Literal(ANALYTICS_SCHEMA_VERSION),
  eventId: AnalyticsEventId,
  name: AnalyticsEventName,
  client: AnalyticsClientKind,
  routeId: AnalyticsRouteId,
  occurredAt: IsoTimestamp,
});

export interface AnalyticsEvent extends S.Schema.Type<typeof AnalyticsEvent> {}

export const AnalyticsEventBatch = S.Struct({
  schemaVersion: S.Literal(ANALYTICS_SCHEMA_VERSION),
  events: S.Array(AnalyticsEvent).check(
    S.isMinLength(1),
    S.isMaxLength(ANALYTICS_BATCH_MAX_EVENTS),
  ),
});

export interface AnalyticsEventBatch extends S.Schema.Type<
  typeof AnalyticsEventBatch
> {}

export class AnalyticsTransportError extends S.TaggedErrorClass<AnalyticsTransportError>()(
  "AnalyticsTransportError",
  {
    cause: S.Defect(),
  },
) {}

export type AnalyticsTransportShape = Readonly<{
  send: (
    batch: AnalyticsEventBatch,
  ) => Effect.Effect<void, AnalyticsTransportError>;
}>;

export class AnalyticsTransport extends Context.Service<
  AnalyticsTransport,
  AnalyticsTransportShape
>()("@openagentsinc/analytics/Transport") {
  static layer = (transport: AnalyticsTransportShape) =>
    Layer.succeed(AnalyticsTransport, transport);
}

export type AnalyticsEventSourceShape = Readonly<{
  nextId: () => Effect.Effect<string>;
  nowIso: () => Effect.Effect<string>;
}>;

export class AnalyticsEventSource extends Context.Service<
  AnalyticsEventSource,
  AnalyticsEventSourceShape
>()("@openagentsinc/analytics/EventSource") {
  static layer = (source: AnalyticsEventSourceShape) =>
    Layer.succeed(AnalyticsEventSource, source);
}

export type AnalyticsClientConfigShape = Readonly<{
  client: AnalyticsClientKind;
  maxBatchSize: number;
  maxBufferedEvents: number;
}>;

export class AnalyticsClientConfig extends Context.Service<
  AnalyticsClientConfig,
  AnalyticsClientConfigShape
>()("@openagentsinc/analytics/ClientConfig") {
  static layer = (config: AnalyticsClientConfigShape) =>
    Layer.succeed(AnalyticsClientConfig, config);
}

export type AnalyticsClientShape = Readonly<{
  track: (
    name: AnalyticsEventName,
    routeId: string,
  ) => Effect.Effect<void, never>;
  flush: () => Effect.Effect<void, never>;
  bufferedCount: () => Effect.Effect<number>;
}>;

export class AnalyticsClient extends Context.Service<
  AnalyticsClient,
  AnalyticsClientShape
>()("@openagentsinc/analytics/Client") {}

export const AnalyticsClientLive = Layer.effect(
  AnalyticsClient,
  Effect.gen(function* () {
    const transport = yield* AnalyticsTransport;
    const source = yield* AnalyticsEventSource;
    const config = yield* AnalyticsClientConfig;
    const buffer = yield* Ref.make<ReadonlyArray<AnalyticsEvent>>([]);
    const maxBatchSize = Math.max(
      1,
      Math.min(ANALYTICS_BATCH_MAX_EVENTS, Math.floor(config.maxBatchSize)),
    );
    const maxBufferedEvents = Math.max(
      maxBatchSize,
      Math.floor(config.maxBufferedEvents),
    );

    const flush = Effect.fn("AnalyticsClient.flush")(function* () {
      const batchEvents = yield* Ref.modify(buffer, (events) => [
        events.slice(0, maxBatchSize),
        events.slice(maxBatchSize),
      ]);
      if (batchEvents.length === 0) return;

      const batch = AnalyticsEventBatch.make({
        schemaVersion: ANALYTICS_SCHEMA_VERSION,
        events: batchEvents,
      });

      yield* transport.send(batch).pipe(
        Effect.retry({ times: 1 }),
        Effect.catch(() =>
          Ref.update(buffer, (current) =>
            [...batchEvents, ...current].slice(-maxBufferedEvents),
          ),
        ),
      );
    });

    const track = Effect.fn("AnalyticsClient.track")(function* (
      name: AnalyticsEventName,
      routeId: string,
    ) {
      const eventId = yield* source.nextId();
      const occurredAt = yield* source.nowIso();
      const decoded = yield* S.decodeUnknownEffect(AnalyticsEvent)({
        schemaVersion: ANALYTICS_SCHEMA_VERSION,
        eventId,
        name,
        client: config.client,
        routeId,
        occurredAt,
      }).pipe(Effect.option);

      if (decoded._tag === "None") return;
      const shouldFlush = yield* Ref.modify(buffer, (events) => {
        const next = [...events, decoded.value].slice(-maxBufferedEvents);
        return [next.length >= maxBatchSize, next];
      });
      if (shouldFlush) {
        yield* flush().pipe(Effect.forkDetach);
      }
    });

    return AnalyticsClient.of({
      bufferedCount: () =>
        Ref.get(buffer).pipe(Effect.map((events) => events.length)),
      flush,
      track,
    });
  }),
);

export const makeAnalyticsClientLayer = (
  transport: AnalyticsTransportShape,
  source: AnalyticsEventSourceShape,
  config: AnalyticsClientConfigShape,
) =>
  AnalyticsClientLive.pipe(
    Layer.provide(AnalyticsTransport.layer(transport)),
    Layer.provide(AnalyticsEventSource.layer(source)),
    Layer.provide(AnalyticsClientConfig.layer(config)),
  );
