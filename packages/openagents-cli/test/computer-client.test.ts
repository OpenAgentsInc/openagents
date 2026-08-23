import { Effect, Fiber, Layer, Option, Redacted, Ref } from "effect";
import { TestClock } from "effect/testing";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiResponse } from "../src/api-transport.js";
import { ComputerClient, computerClientLayer } from "../src/computer-client.js";
import { NetworkRefused } from "../src/errors.js";

const pairing = {
  pairing_id: "pairing-id",
  code: "ABCD-EFGH",
  poll_secret: "poll-secret",
  verify_url: "https://openagents.com/computers",
  expires_at: "2099-01-01T00:00:00.000Z",
  interval_seconds: 3,
};

describe("Computer pairing client", () => {
  it("registers with the local policy and claims only after approval", async () => {
    const requests: Array<{ path: string; body?: unknown; headers?: unknown }> = [];
    const polls = await Effect.runPromise(Ref.make(0));
    const transport = apiTransportTestLayer((input) =>
      Effect.gen(function* () {
        requests.push({ path: input.path, body: input.body, headers: input.headers });
        if (input.method === "POST") return { status: 200, body: pairing } satisfies ApiResponse;
        const poll = yield* Ref.getAndUpdate(polls, (count) => count + 1);
        return poll === 0
          ? ({ status: 200, body: { status: "pending" } } satisfies ApiResponse)
          : ({
              status: 200,
              body: {
                status: "approved",
                machine_id: "machine-id",
                name: "devin-box",
                token: "smct_secret",
              },
            } satisfies ApiResponse);
      }),
    );
    const layer = computerClientLayer.pipe(Layer.provide(transport));

    const claim = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.gen(function* () {
          const client = yield* ComputerClient;
          const started = yield* client.start("https://openagents.com", {
            name: "devin-box",
            tier: "probe",
            platform: "linux-x64",
            agentVersion: "0.1.7",
            roots: ["/workspace/project"],
          });
          return yield* client.wait("https://openagents.com", started);
        }).pipe(Effect.provide(layer), Effect.forkChild);
        yield* TestClock.adjust("3 seconds");
        return yield* Fiber.join(fiber);
      }).pipe(Effect.provide(TestClock.layer())),
    );

    expect(claim.machine_id).toBe("machine-id");
    expect(requests).toEqual([
      {
        path: "/controller/pairings",
        body: {
          name: "devin-box",
          tier: "probe",
          platform: "linux-x64",
          agent_version: "0.1.7",
          roots: ["/workspace/project"],
        },
        headers: undefined,
      },
      {
        path: "/controller/pairings/pairing-id",
        body: undefined,
        headers: { "x-pairing-secret": "poll-secret" },
      },
      {
        path: "/controller/pairings/pairing-id",
        body: undefined,
        headers: { "x-pairing-secret": "poll-secret" },
      },
    ]);
  });

  it("distinguishes disabled, expired, refused, and network failures", async () => {
    const disabled = computerClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer(() =>
          Effect.succeed({ status: 404, body: { error: "computer_controller_disabled" } }),
        ),
      ),
    );
    const disabledExit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        yield* (yield* ComputerClient).start("https://openagents.com", {
          name: "box",
          tier: "probe",
          platform: "linux-x64",
          agentVersion: "0.1.7",
          roots: [],
        });
      }).pipe(Effect.provide(disabled)),
    );
    expect(String(disabledExit)).toContain("ComputerDisabled");

    const expired = computerClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer(() => Effect.succeed({ status: 410, body: { status: "expired" } })),
      ),
    );
    const expiredExit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        yield* (yield* ComputerClient).wait("https://openagents.com", pairing);
      }).pipe(Effect.provide(expired)),
    );
    expect(String(expiredExit)).toContain("ComputerPairingExpired");

    const refused = computerClientLayer.pipe(
      Layer.provide(apiTransportTestLayer(() => Effect.succeed({ status: 404, body: {} }))),
    );
    const refusedExit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        yield* (yield* ComputerClient).wait("https://openagents.com", pairing);
      }).pipe(Effect.provide(refused)),
    );
    expect(String(refusedExit)).toContain("ComputerPairingRefused");

    const network = computerClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer(() =>
          Effect.fail(new NetworkRefused({ origin: "https://openagents.com", message: "offline" })),
        ),
      ),
    );
    const networkExit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        yield* (yield* ComputerClient).start("https://openagents.com", {
          name: "box",
          tier: "probe",
          platform: "linux-x64",
          agentVersion: "0.1.7",
          roots: [],
        });
      }).pipe(Effect.provide(network)),
    );
    expect(String(networkExit)).toContain("ComputerPairingNetworkFailure");
  });

  it("reads the caller's remote status without exposing its token", async () => {
    const requests: Array<{ path: string; token?: string }> = [];
    const layer = computerClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer((input) => {
          requests.push(
            input.token === undefined
              ? { path: input.path }
              : { path: input.path, token: Redacted.value(input.token) },
          );
          return Effect.succeed({
            status: 200,
            body: {
              machine_id: "machine-id",
              name: "devin-box",
              status: "active",
              token_expires_at: "2099-01-01T00:00:00.000Z",
            },
          });
        }),
      ),
    );

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        return yield* (yield* ComputerClient).status(
          "https://openagents.com",
          Redacted.make("smct_secret"),
        );
      }).pipe(Effect.provide(layer)),
    );

    expect(Option.isSome(result)).toBe(true);
    if (Option.isSome(result)) {
      expect(result.value).toEqual({
        machine_id: "machine-id",
        name: "devin-box",
        status: "active",
        token_expires_at: "2099-01-01T00:00:00.000Z",
      });
    }
    expect(requests).toEqual([{ path: "/controller/status", token: "smct_secret" }]);
  });

  it("treats remote revocation as unpaired and network failure separately", async () => {
    const revoked = computerClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer(() =>
          Effect.succeed({ status: 401, body: { error: "machine_revoked" } }),
        ),
      ),
    );
    const revokedResult = await Effect.runPromise(
      Effect.gen(function* () {
        return yield* (yield* ComputerClient).status(
          "https://openagents.com",
          Redacted.make("smct_secret"),
        );
      }).pipe(Effect.provide(revoked)),
    );
    expect(revokedResult._tag).toBe("None");

    const network = computerClientLayer.pipe(
      Layer.provide(
        apiTransportTestLayer(() =>
          Effect.fail(new NetworkRefused({ origin: "https://openagents.com", message: "offline" })),
        ),
      ),
    );
    const networkExit = await Effect.runPromiseExit(
      Effect.gen(function* () {
        yield* (yield* ComputerClient).status(
          "https://openagents.com",
          Redacted.make("smct_secret"),
        );
      }).pipe(Effect.provide(network)),
    );
    expect(String(networkExit)).toContain("ComputerStatusNetworkFailure");
  });
});
