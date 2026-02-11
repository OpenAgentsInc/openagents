import { Effect, TestClock } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { LndRuntimeManagerService } from "../src/main/lndRuntimeManager";
import { makeLndRuntimeHarness } from "./support/lndRuntimeHarness";

const settleAsync = () => Effect.promise(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));

describe("lnd runtime smoke", () => {
  it.effect("runs start -> crash -> backoff restart -> stop flow", () => {
    const harness = makeLndRuntimeHarness({
      restartBackoffBaseMs: 250,
      restartBackoffMaxMs: 250,
      maxCrashRestarts: 2,
    });

    return Effect.gen(function* () {
      const manager = yield* LndRuntimeManagerService;

      yield* manager.start();
      expect(harness.controllers.length).toBe(1);

      harness.controllers[0]?.exitUnexpected({ code: 10, signal: null });
      yield* settleAsync();
      yield* settleAsync();

      const backoff = yield* manager.snapshot();
      expect(backoff.lifecycle).toBe("backoff");

      yield* TestClock.adjust("250 millis");
      yield* settleAsync();
      yield* settleAsync();

      const restarted = yield* manager.snapshot();
      expect(restarted.lifecycle).toBe("running");
      expect(harness.controllers.length).toBe(2);

      yield* manager.stop();
      const stopped = yield* manager.snapshot();
      expect(stopped.lifecycle).toBe("stopped");
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });
});
