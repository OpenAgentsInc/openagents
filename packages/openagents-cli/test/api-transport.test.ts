import { Effect } from "effect";
import { describe, expect, it } from "vitest";

import { NetworkPolicy, networkPolicyTestLoopbackLayer } from "../src/api-transport.js";

const assertOrigin = (origin: string) =>
  Effect.gen(function* () {
    const policy = yield* NetworkPolicy;
    yield* policy.assertAllowed(origin);
  }).pipe(Effect.provide(networkPolicyTestLoopbackLayer));

describe("test network policy", () => {
  it("allows loopback origins", async () => {
    await expect(Effect.runPromise(assertOrigin("http://localhost:4000"))).resolves.toBeUndefined();
  });

  it.each(["https://openagents.com", "https://staging.openagents.com"])(
    "refuses remote test traffic to %s",
    async (origin) => {
      const exit = await Effect.runPromiseExit(assertOrigin(origin));
      expect(exit._tag).toBe("Failure");
      if (exit._tag === "Failure") {
        expect(String(exit.cause)).toContain("NetworkRefused");
      }
    },
  );
});
