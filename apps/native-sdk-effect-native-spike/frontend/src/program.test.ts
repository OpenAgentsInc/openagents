import { IntentRef, resolveIntentRef } from "@effect-native/core";
import { Effect, Exit, SubscriptionRef } from "@effect-native/core/effect";
import { describe, expect, test } from "vite-plus/test";

import { it } from "./effect-test.ts";
import { adoptionCounts, nativeSdkComponentAdoption } from "./native-sdk-component-adoption.ts";
import { makeSpikeRuntime, spikeView } from "./program.ts";

describe("Native SDK Effect Native spike", () => {
  it.effect("runs a real typed Effect Native intent loop", () =>
    Effect.gen(function* () {
      const runtime = yield* makeSpikeRuntime;
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("IncrementEffectCount"), null));
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("IncrementEffectCount"), null));
      const state = yield* SubscriptionRef.get(runtime.state);
      const events = yield* runtime.registry.events;

      expect(state.effectCount).toBe(2);
      expect(state.lastAction).toContain("decoded and handled");
      expect(events).toHaveLength(2);
      expect(events.every((event) => Exit.isSuccess(event.result))).toBe(true);
    }),
  );

  test("emits the closed Effect Native catalog rather than Native SDK props", () => {
    const view = spikeView({ effectCount: 4, lastAction: "test" });
    const encoded = JSON.stringify(view);

    expect(encoded.startsWith('{"_tag":"Stack"')).toBe(true);
    expect(encoded).toContain('"_tag":"Button"');
    expect(encoded).toContain('"_tag":"Badge"');
    expect(encoded).not.toContain("gpu_backend");
    expect(encoded).not.toContain("style_tokens");
  });

  test("keeps the adoption matrix explicit and unique", () => {
    const effectTags = nativeSdkComponentAdoption.map((entry) => entry.effectNative);

    expect(new Set(effectTags).size).toBe(effectTags.length);
    expect(adoptionCounts.direct).toBeGreaterThanOrEqual(8);
    expect(
      nativeSdkComponentAdoption.find((entry) => entry.effectNative === "CodeEditor")?.lane,
    ).toBe("unsupported");
    expect(
      nativeSdkComponentAdoption.find((entry) => entry.effectNative === "Host(webview)")?.lane,
    ).toBe("host-only");
  });
});
