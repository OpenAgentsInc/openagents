import { Effect, TestClock } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { LndRuntimeManagerService } from "../src/main/lndRuntimeManager";
import { makeLndRuntimeHarness } from "./support/lndRuntimeHarness";

const settleAsync = () => Effect.promise(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));

describe("lnd runtime manager", () => {
  it.effect("starts and stops local runtime deterministically", () => {
    const harness = makeLndRuntimeHarness();

    return Effect.gen(function* () {
      const manager = yield* LndRuntimeManagerService;

      yield* manager.start();
      const running = yield* manager.snapshot();
      expect(running.lifecycle).toBe("running");
      expect(running.pid).not.toBeNull();
      expect(running.health === "healthy" || running.health === "starting").toBe(true);
      expect(harness.spawnCalls.length).toBe(1);

      yield* manager.stop();
      const stopped = yield* manager.snapshot();
      expect(stopped.lifecycle).toBe("stopped");
      expect(stopped.pid).toBeNull();
      expect(harness.controllers[0]?.wasKilled()).toBe(true);
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });

  it.effect("applies backoff and restarts after unexpected crash", () => {
    const harness = makeLndRuntimeHarness({
      restartBackoffBaseMs: 1_000,
      restartBackoffMaxMs: 1_000,
      maxCrashRestarts: 3,
    });

    return Effect.gen(function* () {
      const manager = yield* LndRuntimeManagerService;

      yield* manager.start();
      expect(harness.controllers.length).toBe(1);

      harness.controllers[0]?.exitUnexpected({ code: 2, signal: null });
      yield* settleAsync();
      yield* settleAsync();

      const backoff = yield* manager.snapshot();
      expect(backoff.lifecycle).toBe("backoff");
      expect(backoff.crashCount).toBe(1);
      expect(backoff.nextRestartAtMs).not.toBeNull();

      yield* TestClock.adjust("1000 millis");
      yield* settleAsync();
      yield* settleAsync();

      const restarted = yield* manager.snapshot();
      expect(harness.controllers.length).toBe(2);
      expect(restarted.lifecycle).toBe("running");
      expect(restarted.crashCount).toBe(1);

      yield* manager.stop();
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });

  it.effect("bounds crash loops and transitions to failed", () => {
    const harness = makeLndRuntimeHarness({
      restartBackoffBaseMs: 500,
      restartBackoffMaxMs: 500,
      maxCrashRestarts: 1,
    });

    return Effect.gen(function* () {
      const manager = yield* LndRuntimeManagerService;

      yield* manager.start();
      harness.controllers[0]?.exitUnexpected({ code: 3, signal: null });
      yield* settleAsync();
      yield* settleAsync();
      yield* TestClock.adjust("500 millis");
      yield* settleAsync();
      yield* settleAsync();

      expect(harness.controllers.length).toBe(2);

      harness.controllers[1]?.exitUnexpected({ code: 4, signal: null });
      yield* settleAsync();
      yield* settleAsync();

      const failed = yield* manager.snapshot();
      expect(failed.lifecycle).toBe("failed");
      expect(failed.health).toBe("unhealthy");
      expect(failed.lastError).toBe("crash_loop_exhausted");
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });
});
