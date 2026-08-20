import { Effect, Fiber, Layer, Redacted, Ref } from "effect";
import { TestClock } from "effect/testing";
import { describe, expect, it } from "vitest";

import {
  ApiTransport,
  apiTransportTestLayer,
  type ApiResponse,
  type ApiTransportInterface,
} from "../src/api-transport.js";
import { DeviceClient, deviceClientLayer } from "../src/device-client.js";

const authorization = {
  device_code: "secret-device-code",
  user_code: "ABCD-EFGH",
  verification_uri: "http://localhost:4000/device",
  verification_uri_complete: "http://localhost:4000/device?user_code=ABCD-EFGH",
  expires_in: 600,
  interval: 5,
};

describe("device authorization client", () => {
  it("starts without a bearer token and polls with the secret only in the request body", async () => {
    const program = Effect.gen(function* () {
      const polls = yield* Ref.make(0);
      const requests = yield* Ref.make<Array<{ readonly path: string; readonly body?: unknown }>>(
        [],
      );
      const request: ApiTransportInterface["request"] = (input) =>
        Effect.gen(function* () {
          yield* Ref.update(requests, (seen) => [...seen, { path: input.path, body: input.body }]);
          if (!input.path.endsWith("/token")) {
            return { status: 201, body: authorization } satisfies ApiResponse;
          }
          const count = yield* Ref.getAndUpdate(polls, (current) => current + 1);
          return count === 0
            ? ({
                status: 428,
                body: { code: "authorization_pending" },
              } satisfies ApiResponse)
            : ({
                status: 200,
                body: {
                  access_token: "oa_pat_fixture",
                  token_type: "Bearer",
                  scope: "forge:write",
                  expires_in: 2_592_000,
                },
              } satisfies ApiResponse);
        });
      const transport = ApiTransport.of({ request });
      const layer = deviceClientLayer.pipe(Layer.provide(Layer.succeed(ApiTransport, transport)));
      const fiber = yield* Effect.gen(function* () {
        const client = yield* DeviceClient;
        const started = yield* client.start("http://localhost:4000");
        return yield* client.wait("http://localhost:4000", started);
      }).pipe(Effect.provide(layer), Effect.forkChild);

      yield* TestClock.adjust("5 seconds");
      return { token: yield* Fiber.join(fiber), requests: yield* Ref.get(requests) };
    }).pipe(Effect.provide(TestClock.layer()));

    const result = await Effect.runPromise(program);
    expect(Redacted.value(result.token)).toBe("oa_pat_fixture");
    expect(result.requests[0]).toEqual({
      path: "/api/v3/device/authorizations",
      body: {},
    });
    expect(result.requests[1]).toEqual({
      path: "/api/v3/device/authorizations/token",
      body: { device_code: "secret-device-code" },
    });
  });

  it("preserves the loopback-only network policy in tests", async () => {
    const layer = deviceClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer(() => Effect.succeed({ status: 201, body: authorization })),
      ),
    );
    const started = await Effect.runPromise(
      Effect.gen(function* () {
        const client = yield* DeviceClient;
        return yield* client.start("http://localhost:4000");
      }).pipe(Effect.provide(layer)),
    );
    expect(started.user_code).toBe("ABCD-EFGH");
  });
});
