import { Effect, Fiber, Layer, Redacted } from "effect";
import { TestClock } from "effect/testing";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { TransportError } from "../src/errors.js";
import { FleetClient, fleetClientLayer } from "../src/fleet-client.js";

const token = Redacted.make("test-token");

const targetFixture = (status: string, overrides: Record<string, unknown> = {}) => ({
  id: "0d4e8a70-0000-4000-8000-000000000001",
  repo: "openagents.com",
  sha: "a".repeat(40),
  status,
  terminal: ["live", "failed", "reverted"].includes(status),
  promoted_by: "operator:1",
  environment: "production",
  source: "api",
  artifact_digest: null,
  deployment_lane: null,
  error_code: null,
  promoted_at: "2026-08-24T00:00:00Z",
  updated_at: "2026-08-24T00:00:00Z",
  status_url:
    "http://localhost:4000/api/v1/admin/forge/targets/0d4e8a70-0000-4000-8000-000000000001",
  ...overrides,
});

const promoteInput = {
  origin: "http://localhost:4000",
  token,
  repo: "openagents.com",
  sha: "a".repeat(40),
  environment: "production",
  idempotencyKey: "release-2026-08-24-a",
} as const;

const layerFromHandler = (
  handler: (input: ApiRequest) => Effect.Effect<ApiResponse, TransportError>,
): Layer.Layer<FleetClient> => fleetClientLayer.pipe(Layer.provide(apiTransportTestLayer(handler)));

describe("fleet client", () => {
  it("promotes with the exact repository, SHA, environment, and idempotency key", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return { status: 202, body: { ...targetFixture("queued"), replayed: false } };
      }),
    );

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fleet = yield* FleetClient;
        return yield* fleet.promote({
          ...promoteInput,
          expectedCurrentTargetId: "0d4e8a70-0000-4000-8000-000000000000",
        });
      }).pipe(Effect.provide(layer)),
    );

    expect(requests).toHaveLength(1);
    expect(requests[0]?.method).toBe("POST");
    expect(requests[0]?.path).toBe("/api/v1/admin/forge/targets");
    expect(requests[0]?.body).toEqual({
      repo: "openagents.com",
      sha: "a".repeat(40),
      environment: "production",
      idempotency_key: "release-2026-08-24-a",
      expected_current_target_id: "0d4e8a70-0000-4000-8000-000000000000",
    });
    expect(result.accepted).toBe(true);
    expect(result.replayed).toBe(false);
  });

  it("reports a 200 replay as the original promotion, not a new acceptance", async () => {
    const layer = layerFromHandler(() =>
      Effect.succeed({ status: 200, body: { ...targetFixture("live"), replayed: true } }),
    );

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fleet = yield* FleetClient;
        return yield* fleet.promote(promoteInput);
      }).pipe(Effect.provide(layer)),
    );

    expect(result.accepted).toBe(false);
    expect(result.replayed).toBe(true);
  });

  it("re-sends after a failed transport with the same idempotency key", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.suspend(() => {
        requests.push(input);
        if (requests.length === 1) {
          return Effect.fail(
            new TransportError({
              operation: "sending the request",
              message: "connection closed",
              cause: new Error("closed"),
            }),
          );
        }
        return Effect.succeed({ status: 202, body: targetFixture("queued") });
      }),
    );

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.gen(function* () {
          const fleet = yield* FleetClient;
          return yield* fleet.promote(promoteInput);
        }).pipe(Effect.provide(layer), Effect.forkChild);
        yield* TestClock.adjust("5 seconds");
        return yield* Fiber.join(fiber);
      }).pipe(Effect.provide(TestClock.layer())),
    );

    expect(result.accepted).toBe(true);
    expect(requests).toHaveLength(2);
    const keys = requests.map(
      (request) => (request.body as Record<string, unknown>)["idempotency_key"],
    );
    expect(keys).toEqual(["release-2026-08-24-a", "release-2026-08-24-a"]);
  });

  it("surfaces an exhausted transport as a transport failure, never a deployment failure", async () => {
    let attempts = 0;
    const layer = layerFromHandler(() =>
      Effect.suspend(() => {
        attempts += 1;
        return Effect.fail(
          new TransportError({
            operation: "sending the request",
            message: "connection closed",
            cause: new Error("closed"),
          }),
        );
      }),
    );

    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.gen(function* () {
          const fleet = yield* FleetClient;
          return yield* fleet.promote(promoteInput);
        }).pipe(Effect.provide(layer), Effect.flip, Effect.forkChild);
        yield* TestClock.adjust("10 seconds");
        return yield* Fiber.join(fiber);
      }).pipe(Effect.provide(TestClock.layer())),
    );

    expect(attempts).toBe(3);
    expect(outcome._tag).toBe("OpenAgentsCli.TransportError");
  });

  it("never re-sends a promotion the server refused", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 409,
          body: {
            message: "The fleet target changed before this request",
            code: "precondition_failed",
            status: 409,
            request_id: "req-1",
            errors: {},
          },
        };
      }),
    );

    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const fleet = yield* FleetClient;
        return yield* fleet.promote(promoteInput);
      }).pipe(Effect.provide(layer), Effect.flip),
    );

    expect(requests).toHaveLength(1);
    expect(failure._tag).toBe("OpenAgentsCli.ApiError");
    expect(failure._tag === "OpenAgentsCli.ApiError" && failure.code).toBe("precondition_failed");
  });

  it("polls with bounded backoff until the target is live", async () => {
    const statuses = ["queued", "building", "deploying", "live"];
    let call = 0;
    const layer = layerFromHandler(() =>
      Effect.sync(() => {
        const status = statuses[Math.min(call, statuses.length - 1)] ?? "live";
        call += 1;
        return { status: 200, body: targetFixture(status) };
      }),
    );

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.gen(function* () {
          const fleet = yield* FleetClient;
          return yield* fleet.wait({
            origin: "http://localhost:4000",
            token,
            id: "0d4e8a70-0000-4000-8000-000000000001",
            timeoutMs: 600_000,
          });
        }).pipe(Effect.provide(layer), Effect.forkChild);
        // Backoff is 2s, 4s, 8s; 20 seconds covers all three sleeps.
        yield* TestClock.adjust("20 seconds");
        return yield* Fiber.join(fiber);
      }).pipe(Effect.provide(TestClock.layer())),
    );

    expect(call).toBe(4);
    expect(result["status"]).toBe("live");
  });

  it("times out while nonterminal without claiming the deployment failed", async () => {
    let call = 0;
    const layer = layerFromHandler(() =>
      Effect.sync(() => {
        call += 1;
        return { status: 200, body: targetFixture("building") };
      }),
    );

    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.gen(function* () {
          const fleet = yield* FleetClient;
          return yield* fleet.wait({
            origin: "http://localhost:4000",
            token,
            id: "0d4e8a70-0000-4000-8000-000000000001",
            timeoutMs: 5_000,
          });
        }).pipe(Effect.provide(layer), Effect.flip, Effect.forkChild);
        yield* TestClock.adjust("30 seconds");
        return yield* Fiber.join(fiber);
      }).pipe(Effect.provide(TestClock.layer())),
    );

    expect(call).toBeGreaterThan(1);
    expect(failure._tag).toBe("OpenAgentsCli.DeploymentWaitTimeout");
    if (failure._tag === "OpenAgentsCli.DeploymentWaitTimeout") {
      expect(failure.lastStatus).toBe("building");
      expect(failure.message).toContain("has not failed");
      expect(failure.message).toContain("deploy view");
    }
  });

  it("stops polling on needs_rolling_replace instead of waiting forever", async () => {
    const layer = layerFromHandler(() =>
      Effect.succeed({ status: 200, body: targetFixture("needs_rolling_replace") }),
    );

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fleet = yield* FleetClient;
        return yield* fleet.wait({
          origin: "http://localhost:4000",
          token,
          id: "0d4e8a70-0000-4000-8000-000000000001",
          timeoutMs: 600_000,
        });
      }).pipe(Effect.provide(layer)),
    );

    expect(result["status"]).toBe("needs_rolling_replace");
  });

  it("lists targets through the bounded history route", async () => {
    const requests: Array<ApiRequest> = [];
    const layer = layerFromHandler((input) =>
      Effect.sync(() => {
        requests.push(input);
        return {
          status: 200,
          body: { repo: "openagents.com", targets: [targetFixture("live")] },
        };
      }),
    );

    await Effect.runPromise(
      Effect.gen(function* () {
        const fleet = yield* FleetClient;
        return yield* fleet.list({
          origin: "http://localhost:4000",
          token,
          repo: "openagents.com",
          limit: 5,
        });
      }).pipe(Effect.provide(layer)),
    );

    expect(requests[0]?.method).toBe("GET");
    expect(requests[0]?.path).toBe("/api/v1/admin/forge/targets?repo=openagents.com&limit=5");
  });
});
