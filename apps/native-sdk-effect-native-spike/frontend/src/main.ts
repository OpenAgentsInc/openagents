import { IntentRef, resolveIntentRef, StaticPayload, type IntentReporter } from "@effect-native/core";
import { Effect, Exit, Scope, SubscriptionRef } from "@effect-native/core/effect";
import { makeStubCodeEditorDriver } from "@effect-native/render-dom";
import { makeReactDomRenderer } from "@effect-native/render-dom/react";
import { khalaTheme } from "@effect-native/tokens";
import "@openagentsinc/openagents-desktop/renderer.css";

import { resolveNativeDispatch, startNativeBridgeSync } from "./native-bridge.ts";
import { initialSpikeState, makeSpikeRuntime } from "./program.ts";
import { persistSpikeState, restoreSpikeState, spikeStorageNamespace } from "./state-storage.ts";
import "./style.css";

const boot = (): void => {
  const root = document.getElementById("openagents-desktop-root");
  if (!(root instanceof HTMLElement)) return;

  const scope = Effect.runSync(Scope.make());
  let rendererScope = Effect.runSync(Scope.make());
  let disposeNativeIntent = (): void => undefined;
  let scopeClosed = false;
  window.addEventListener("pagehide", (event) => {
    // WebKit may place the child pane in its back-forward cache while Native
    // SDK reconciles a reload token. Keep the mounted program alive for that
    // reversible transition; a non-persisted document exit owns teardown.
    if (event.persisted || scopeClosed) return;
    scopeClosed = true;
    disposeNativeIntent();
    void Effect.runPromise(Scope.close(rendererScope, Exit.void));
    void Effect.runPromise(Scope.close(scope, Exit.void));
  });

  const mount = Effect.gen(function* () {
    const storageNamespace = spikeStorageNamespace(window.location.href);
    const restored = restoreSpikeState(window.localStorage, storageNamespace);
    const runtime = yield* makeSpikeRuntime(initialSpikeState(restored));
    let acknowledgedNativeSequence = restored.acknowledgedNativeSequence;
    window.addEventListener("pageshow", (event) => {
      if (!event.persisted) return;
      // The pane generation changed even though product state did not. A new
      // immutable state identity advances the bounded native projection.
      void Effect.runPromise(SubscriptionRef.update(runtime.state, (current) => ({ ...current })));
    });
    const report: IntentReporter = (ref, runtimeValue) =>
      Effect.gen(function* () {
        yield* runtime.registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null));
      });

    const renderer = makeReactDomRenderer({
      theme: khalaTheme,
      hostDrivers: [makeStubCodeEditorDriver()],
    });
    yield* Scope.provide(rendererScope)(renderer.mount(root, runtime.program.viewStream, report));
    document.documentElement.dataset.effectNativeMounted = "true";
    // Native receives no authoritative projection until the real React-owned
    // Desktop pane has completed its first Effect Native mount.
    disposeNativeIntent = startNativeBridgeSync(
      () => Effect.runPromise(SubscriptionRef.get(runtime.state)),
      async (envelope) => {
        if (envelope.intent._tag === "RendererReloadRequested") {
          await Effect.runPromise(Scope.close(rendererScope, Exit.void));
          rendererScope = Effect.runSync(Scope.make());
          await Effect.runPromise(Scope.provide(rendererScope)(
            renderer.mount(root, runtime.program.viewStream, report),
          ));
          await Effect.runPromise(SubscriptionRef.update(runtime.state, (current) => ({ ...current })));
          return null;
        }
        const dispatch = resolveNativeDispatch(envelope);
        const ref = dispatch.payload === null
          ? IntentRef(dispatch.intentName)
          : IntentRef(dispatch.intentName, StaticPayload(dispatch.payload));
        await Effect.runPromise(runtime.registry.dispatch(resolveIntentRef(ref, null)));
        return dispatch.appliedCommand;
      },
      {
        // One host-generation fence in addition to storage decode's document
        // bump prevents a just-terminated WebKit process from racing its last
        // asynchronous localStorage flush during the headed restart proof.
        initialRevision: restored.revision + 1,
        initialAcknowledgedSequence: acknowledgedNativeSequence,
        onProjection: (state, revision) => {
          persistSpikeState(window.localStorage, storageNamespace, state, revision, acknowledgedNativeSequence);
        },
        onAcknowledged: (state, revision, sequence) => {
          acknowledgedNativeSequence = sequence;
          persistSpikeState(window.localStorage, storageNamespace, state, revision, acknowledgedNativeSequence);
        },
      },
    );
  });

  void Effect.runPromise(Scope.provide(scope)(mount)).catch((error) => {
    console.error("[native-sdk-effect-native-spike] Effect Native mount failed", error);
    root.textContent = `Effect Native mount failed: ${String(error)}`;
  });
};

const isNamedEffectNativeSurface = (): boolean => {
  try {
    return new URLSearchParams(window.location.hash.slice(1)).get("surface") === "effect-native";
  } catch {
    return false;
  }
};

// Native SDK's production asset source also creates a primary WebView. Only
// the explicitly marked child pane may own Desktop state, storage, or bridge
// polling; otherwise two app instances race the same persisted projection.
if (isNamedEffectNativeSurface()) {
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boot, { once: true });
  else boot();
} else {
  document.documentElement.dataset.openagentsSurface = "native-primary-host";
}
