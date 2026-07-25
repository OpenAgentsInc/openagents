import { Effect, ManagedRuntime, Schema as S } from "effect";
import { describe, expect, test } from "vitest";

import {
  ANALYTICS_SCHEMA_VERSION,
  AnalyticsClient,
  AnalyticsEventBatch,
  AnalyticsTransportError,
  makeAnalyticsClientLayer,
} from "./index.js";

describe("portable analytics", () => {
  test("accepts only the closed event contract", () => {
    const decode = S.decodeUnknownSync(AnalyticsEventBatch);
    expect(
      decode({
        schemaVersion: ANALYTICS_SCHEMA_VERSION,
        events: [
          {
            schemaVersion: ANALYTICS_SCHEMA_VERSION,
            eventId: "event.test.00000001",
            name: "github_view",
            client: "mobile",
            routeId: "/",
            occurredAt: "2026-07-25T18:00:00.000Z",
          },
        ],
      }).events,
    ).toHaveLength(1);

    expect(() =>
      decode({
        schemaVersion: ANALYTICS_SCHEMA_VERSION,
        events: [
          {
            schemaVersion: ANALYTICS_SCHEMA_VERSION,
            eventId: "event.test.00000002",
            name: "arbitrary_event",
            client: "mobile",
            routeId: "/",
            occurredAt: "2026-07-25T18:00:00.000Z",
          },
        ],
      }),
    ).toThrow();
  });

  test("buffers, batches, retries once, and never fails the caller", async () => {
    const batches: Array<unknown> = [];
    let attempts = 0;
    let nextId = 0;
    const runtime = ManagedRuntime.make(
      makeAnalyticsClientLayer(
        {
          send: (batch) =>
            Effect.gen(function* () {
              attempts += 1;
              if (attempts === 1) {
                return yield* new AnalyticsTransportError({
                  cause: "temporary",
                });
              }
              batches.push(batch);
            }),
        },
        {
          nextId: () =>
            Effect.succeed(`event.test.${String(++nextId).padStart(8, "0")}`),
          nowIso: () => Effect.succeed("2026-07-25T18:00:00.000Z"),
        },
        { client: "web", maxBatchSize: 10, maxBufferedEvents: 20 },
      ),
    );

    await runtime.runPromise(
      Effect.gen(function* () {
        const client = yield* AnalyticsClient;
        yield* client.track("page_view", "/");
        yield* client.track("github_view", "/");
        yield* client.flush();
      }),
    );

    expect(attempts).toBe(2);
    expect(batches).toHaveLength(1);
    expect(
      await runtime.runPromise(
        Effect.flatMap(AnalyticsClient, (client) => client.bufferedCount()),
      ),
    ).toBe(0);
    await runtime.dispose();
  });
});
