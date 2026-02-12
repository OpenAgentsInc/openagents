import { Cause, Effect, Layer, Option, Stream } from "effect";
import { describe, expect, it, vi } from "vitest";

import { ConvexService } from "../../src/effect/convex";
import {
  LightningApiError,
  LightningApiLive,
  LightningApiService,
} from "../../src/effect/lightning";
import {
  RequestContextService,
  makeServerRequestContext,
  type RequestContext,
} from "../../src/effect/requestContext";
import { TelemetryService, type TelemetryClient } from "../../src/effect/telemetry";

const telemetryStub: TelemetryClient = {
  log: () => Effect.void,
  event: () => Effect.void,
  identify: () => Effect.void,
  withNamespace: () => telemetryStub,
  withFields: () => telemetryStub,
};

const convexStub = ConvexService.of({
  query: () => Effect.fail(new Error("convex.query not used")),
  mutation: () => Effect.fail(new Error("convex.mutation not used")),
  action: () => Effect.fail(new Error("convex.action not used")),
  subscribeQuery: () => Stream.fail(new Error("convex.subscribeQuery not used")),
});

const makeLayer = (ctx: RequestContext) =>
  Layer.provideMerge(
    LightningApiLive,
    Layer.mergeAll(
      Layer.succeed(ConvexService, convexStub),
      Layer.succeed(TelemetryService, telemetryStub),
      Layer.succeed(RequestContextService, ctx),
    ),
  );

const runClient = <A>(
  effect: Effect.Effect<A, LightningApiError, LightningApiService>,
  ctx: RequestContext = { _tag: "Client" },
) => Effect.runPromise(effect.pipe(Effect.provide(makeLayer(ctx))));

describe("apps/web lightning api client", () => {
  it("lists paywalls with typed decode and request correlation", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          ok: true,
          requestId: "req_body_1",
          paywalls: [
            {
              paywallId: "pw_1",
              ownerId: "owner_1",
              name: "Premium Data",
              status: "active",
              createdAtMs: 1,
              updatedAtMs: 2,
              policy: {
                paywallId: "pw_1",
                ownerId: "owner_1",
                pricingMode: "fixed",
                fixedAmountMsats: 2500,
                killSwitch: false,
                createdAtMs: 1,
                updatedAtMs: 2,
              },
              routes: [
                {
                  routeId: "route_1",
                  paywallId: "pw_1",
                  ownerId: "owner_1",
                  hostPattern: "openagents.com",
                  pathPattern: "/premium",
                  upstreamUrl: "https://api.example.com/premium",
                  protocol: "https",
                  timeoutMs: 15000,
                  priority: 1,
                  createdAtMs: 1,
                  updatedAtMs: 2,
                },
              ],
            },
          ],
        }),
        {
          status: 200,
          headers: {
            "content-type": "application/json",
            "x-oa-request-id": "req_header_1",
          },
        },
      ),
    );

    try {
      const result = await runClient(
        Effect.gen(function* () {
          const lightning = yield* LightningApiService;
          return yield* lightning.listPaywalls({ status: "active", limit: 10 });
        }),
      );

      expect(result.requestId).toBe("req_body_1");
      expect(result.paywalls).toHaveLength(1);
      expect(result.paywalls[0]?.paywallId).toBe("pw_1");

      expect(fetchMock).toHaveBeenCalledTimes(1);
      const [url, init] = fetchMock.mock.calls[0] ?? [];
      expect(String(url)).toContain("/api/lightning/paywalls?status=active&limit=10");
      expect(init?.method).toBe("GET");
    } finally {
      fetchMock.mockRestore();
    }
  });

  it("maps authorization failures into deterministic LightningApiError", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: false, error: "forbidden" }), {
        status: 403,
        headers: { "content-type": "application/json" },
      }),
    );

    try {
      const exit = await Effect.runPromiseExit(
        Effect.gen(function* () {
          const lightning = yield* LightningApiService;
          return yield* lightning.listDeployments({ limit: 5 });
        }).pipe(Effect.provide(makeLayer({ _tag: "Client" }))),
      );

      expect(exit._tag).toBe("Failure");
      if (exit._tag === "Failure") {
        const failure = Cause.failureOption(exit.cause);
        expect(Option.isSome(failure)).toBe(true);
        if (Option.isSome(failure)) {
          expect(failure.value._tag).toBe("LightningApiError");
          expect(failure.value.operation).toBe("listDeployments.http");
          expect(failure.value.status).toBe(403);
          expect(String(failure.value.error)).toContain("forbidden");
        }
      }
    } finally {
      fetchMock.mockRestore();
    }
  });

  it("uses server request context for base URL and forwards auth headers", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          ok: true,
          requestId: null,
          events: [
            {
              eventId: "evt_1",
              paywallId: "pw_1",
              ownerId: "owner_1",
              eventType: "gateway_reconcile_ok",
              level: "info",
              createdAtMs: 11,
            },
          ],
          nextCursor: null,
        }),
        {
          status: 200,
          headers: {
            "content-type": "application/json",
            "x-oa-request-id": "req_header_2",
          },
        },
      ),
    );

    try {
      const result = await runClient(
        Effect.gen(function* () {
          const lightning = yield* LightningApiService;
          return yield* lightning.listDeploymentEvents({ level: "info", limit: 10 });
        }),
        makeServerRequestContext(
          new Request("https://openagents.com/home", {
            headers: {
              cookie: "oa_session=abc",
              authorization: "Bearer test-token",
            },
          }),
        ),
      );

      expect(result.requestId).toBe("req_header_2");
      expect(result.events).toHaveLength(1);

      const [url, init] = fetchMock.mock.calls[0] ?? [];
      expect(String(url)).toBe("https://openagents.com/api/lightning/deployments/events?level=info&limit=10");
      expect(init?.method).toBe("GET");
      const headers = init?.headers as Headers;
      expect(headers.get("cookie")).toBe("oa_session=abc");
      expect(headers.get("authorization")).toBe("Bearer test-token");
    } finally {
      fetchMock.mockRestore();
    }
  });
});
