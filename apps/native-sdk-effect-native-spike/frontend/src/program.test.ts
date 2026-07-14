import { IntentRef, resolveIntentRef, StaticPayload } from "@effect-native/core";
import { Effect, Exit, SubscriptionRef } from "@effect-native/core/effect";
import { describe, expect, test } from "vite-plus/test";

import { it } from "./effect-test.ts";
import { bridgePayloadLimit, decodeNativeIntent, projectNativeState } from "./native-bridge.ts";
import { adoptionCounts, nativeSdkComponentAdoption } from "./native-sdk-component-adoption.ts";
import { fixtureSessions, initialSpikeState, makeSpikeRuntime, spikeView } from "./program.ts";
import { assertNativeProductionCommandBindings, nativeProductionCommandBindings } from "./production-command-parity.ts";
import { persistSpikeState, restoreSpikeState, spikeStorageKey, spikeStorageNamespace, type SpikeStorage } from "./state-storage.ts";

const memoryStorage = (): SpikeStorage & { readonly values: Map<string, string> } => {
  const values = new Map<string, string>();
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value); },
  };
};

describe("Native SDK Effect Native parity spike", () => {
  it.effect("runs the MVP composer through a real typed Effect intent loop", () =>
    Effect.gen(function* () {
      const runtime = yield* makeSpikeRuntime();
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopInputChanged", StaticPayload("Ship the parity slice")), null));
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopNoteSubmitted"), null));
      const state = yield* SubscriptionRef.get(runtime.state);
      const events = yield* runtime.registry.events;

      expect(state.messages.at(-1)?.text).toBe("Ship the parity slice");
      expect(state.pending).toBe(true);
      expect(state.input).toBe("");
      expect(events).toHaveLength(2);
      expect(events.every((event) => Exit.isSuccess(event.result))).toBe(true);
    }),
  );

  it.effect("keeps blank submit a no-op and new chat deterministic", () =>
    Effect.gen(function* () {
      const runtime = yield* makeSpikeRuntime();
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopNoteSubmitted", StaticPayload("   ")), null));
      let state = yield* SubscriptionRef.get(runtime.state);
      expect(state.messages).toHaveLength(2);
      yield* runtime.registry.dispatch(resolveIntentRef(IntentRef("DesktopNewChat"), null));
      state = yield* SubscriptionRef.get(runtime.state);
      expect(state.selectedSessionRef).toBeNull();
      expect(state.messages).toEqual([]);
      expect(state.workspace).toBe("chat");
    }),
  );

  test("projects the real MVP transcript and composer catalog", () => {
    const view = spikeView(initialSpikeState());
    const encoded = JSON.stringify(view);

    expect(encoded.startsWith('{"_tag":"Stack"')).toBe(true);
    expect(encoded).toContain('"_tag":"Transcript"');
    expect(encoded).toContain('"_tag":"TextField"');
    expect(encoded).toContain('"_tag":"IconButton"');
    expect(encoded).toContain("Codex");
    expect(encoded).not.toContain("Candidate Native SDK lowerings");
    expect(encoded).not.toContain("gpu_backend");
  });

  test("decodes only the bounded versioned native intent protocol", () => {
    expect(decodeNativeIntent({ protocol: 1, sequence: 4, intent: { _tag: "SessionSelected", sessionRef: fixtureSessions[1].ref, commandId: null } }).sequence).toBe(4);
    expect(() => decodeNativeIntent({ protocol: 2, sequence: 4, intent: { _tag: "NewChatRequested", commandId: "chat.new" } })).toThrow();
    expect(() => decodeNativeIntent({ protocol: 1, sequence: 4, intent: { _tag: "NewChatRequested", commandId: "settings.open" } })).toThrow();
  });

  test("consumes the real Desktop canonical command identities", () => {
    expect(assertNativeProductionCommandBindings()).toBeUndefined();
    expect(nativeProductionCommandBindings.map((binding) => binding.commandId)).toEqual([
      "chat.new",
      "chat.open",
      "workspace.home",
      "settings.open",
    ]);
  });

  test("keeps the native mirror projection small and non-authoritative", () => {
    const state = initialSpikeState();
    const projection = projectNativeState(state);
    expect(projection.selectedSessionRef).toBe(state.selectedSessionRef);
    expect(projection.messageCount).toBe(2);
    expect(new TextEncoder().encode(JSON.stringify(projection)).length).toBeLessThan(bridgePayloadLimit);
    expect(fixtureSessions.length).toBeLessThanOrEqual(3);
  });

  test("keeps the component-adoption audit explicit and unique", () => {
    const effectTags = nativeSdkComponentAdoption.map((entry) => entry.effectNative);
    expect(new Set(effectTags).size).toBe(effectTags.length);
    expect(adoptionCounts.direct).toBeGreaterThanOrEqual(8);
    expect(nativeSdkComponentAdoption.find((entry) => entry.effectNative === "CodeEditor")?.lane).toBe("unsupported");
    expect(nativeSdkComponentAdoption.find((entry) => entry.effectNative === "Host(webview)")?.lane).toBe("host-only");
  });

  test("restores bounded Effect state across reload and restart generations", () => {
    const storage = memoryStorage();
    const state = {
      ...initialSpikeState(),
      selectedSessionRef: fixtureSessions[1].ref,
      messages: [],
      pending: true,
      revision: 7,
    };
    expect(persistSpikeState(storage, "run-1", state)).toBe(true);
    expect(restoreSpikeState(storage, "run-1")).toEqual({
      ...state,
      pending: false,
      revision: 8,
    });
    storage.values.set(spikeStorageKey("run-2"), "not-json");
    expect(restoreSpikeState(storage, "run-2")).toEqual(initialSpikeState());
    expect(spikeStorageNamespace("zero://app/index.html#assurance-run=proof.42")).toBe("proof.42");
    expect(spikeStorageNamespace("zero://app/index.html?assurance-run=../../unsafe")).toBe("default");
  });
});
