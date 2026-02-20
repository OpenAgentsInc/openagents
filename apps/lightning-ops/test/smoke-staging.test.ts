import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { runStagingSmoke } from "../src/programs/smokeStaging.js";

describe("lightning-ops smoke:staging", () => {
  it.effect("mock mode validates challenge and proxy checks with JSON-required fields", () =>
    Effect.gen(function* () {
      const summary = yield* runStagingSmoke({
        mode: "mock",
        requestId: "smoke:staging:test",
      });

      expect(summary.challengeOk).toBe(true);
      expect(summary.proxyOk).toBe(true);
      expect(summary.configHash.startsWith("cfg_")).toBe(true);
      expect(summary.deploymentStatus).toBe("applied");
      expect(summary.executionPath).toBe("hosted-node");
    }),
  );

  it.effect("api mode requires staging gateway environment vars", () =>
    Effect.gen(function* () {
      const prevBase = process.env.OA_LIGHTNING_OPS_GATEWAY_BASE_URL;
      const prevChallenge = process.env.OA_LIGHTNING_OPS_CHALLENGE_URL;
      const prevProxy = process.env.OA_LIGHTNING_OPS_PROXY_URL;

      delete process.env.OA_LIGHTNING_OPS_GATEWAY_BASE_URL;
      delete process.env.OA_LIGHTNING_OPS_CHALLENGE_URL;
      delete process.env.OA_LIGHTNING_OPS_PROXY_URL;

      try {
        const attempted = yield* Effect.either(runStagingSmoke({ mode: "api" }));
        expect(attempted._tag).toBe("Left");
        if (attempted._tag === "Left") {
          expect(String(attempted.left)).toContain("ConfigError");
        }
      } finally {
        process.env.OA_LIGHTNING_OPS_GATEWAY_BASE_URL = prevBase;
        process.env.OA_LIGHTNING_OPS_CHALLENGE_URL = prevChallenge;
        process.env.OA_LIGHTNING_OPS_PROXY_URL = prevProxy;
      }
    }),
  );
});
