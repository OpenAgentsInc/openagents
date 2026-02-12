import * as Fs from "node:fs/promises";
import * as Os from "node:os";
import * as Path from "node:path";

import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { runEp212FullFlowSmoke } from "../src/programs/smokeEp212FullFlow.js";

const mkdtemp = () =>
  Effect.tryPromise({
    try: () => Fs.mkdtemp(Path.join(Os.tmpdir(), "openagents-ep212-full-flow-")),
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

describe("lightning-ops smoke:ep212-full-flow", () => {
  it.effect("mock mode exercises paid, cache-hit, over-cap block, and OpenAgents route success", () =>
    Effect.gen(function* () {
      const tempRoot = yield* mkdtemp();
      const artifactDir = Path.join(tempRoot, "artifacts");

      const summary = yield* runEp212FullFlowSmoke({
        mode: "mock",
        requestId: "smoke:ep212-full-flow:test",
        artifactDir,
      });

      expect(summary.ok).toBe(true);
      expect(summary.mode).toBe("mock");

      expect(summary.sats4ai.challengeStatusCode).toBe(402);
      expect(summary.sats4ai.firstStatusCode).toBe(200);
      expect(summary.sats4ai.firstPaid).toBe(true);
      expect(summary.sats4ai.secondStatusCode).toBe(200);
      expect(summary.sats4ai.secondPaid).toBe(false);
      expect(summary.sats4ai.cacheHit).toBe(true);
      expect(summary.sats4ai.payerCallsAfterSecond).toBe(
        summary.sats4ai.payerCallsAfterFirst,
      );

      expect(summary.openAgentsRoute.challengeStatusCode).toBe(402);
      expect(summary.openAgentsRoute.paidStatusCode).toBe(200);
      expect(summary.openAgentsRoute.paidAmountMsats).toBeGreaterThan(0);

      expect(summary.overCap.challengeStatusCode).toBe(402);
      expect(summary.overCap.blocked).toBe(true);
      expect(summary.overCap.denyReasonCode).toBe("amount_over_cap");
      expect(summary.overCap.payerCallsAfter).toBe(summary.overCap.payerCallsBefore);

      yield* Effect.tryPromise({
        try: async () => {
          await Fs.access(summary.artifacts.summaryPath);
          await Fs.access(summary.artifacts.eventsPath);
        },
        catch: (error) => (error instanceof Error ? error : new Error(String(error))),
      });
    }),
  );

  it.effect("live mode fails fast when wallet executor base URL is missing", () =>
    Effect.gen(function* () {
      const prevBase = process.env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL;
      delete process.env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL;

      try {
        const attempted = yield* Effect.either(
          runEp212FullFlowSmoke({
            mode: "live",
            requestId: "smoke:ep212-full-flow:missing-wallet",
          }),
        );

        expect(attempted._tag).toBe("Left");
        if (attempted._tag === "Left") {
          expect(String(attempted.left)).toContain("ConfigError");
        }
      } finally {
        process.env.OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL = prevBase;
      }
    }),
  );
});
