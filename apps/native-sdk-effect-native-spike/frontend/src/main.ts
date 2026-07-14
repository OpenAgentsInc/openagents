import { IntentRef, resolveIntentRef, StaticPayload, type IntentReporter } from "@effect-native/core";
import { Effect, Exit, Scope, SubscriptionRef } from "@effect-native/core/effect";
import { makeDomRenderer } from "@effect-native/render-dom";
import { khalaTheme } from "@effect-native/tokens";

import { startNativeBridgeSync } from "./native-bridge.ts";
import { makeSpikeRuntime } from "./program.ts";
import "./style.css";

const boot = (): void => {
  const root = document.getElementById("effect-native-root");
  if (!(root instanceof HTMLElement)) return;

  const scope = Effect.runSync(Scope.make());
  let disposeNativeIntent = (): void => undefined;
  window.addEventListener("pagehide", () => {
    disposeNativeIntent();
    void Effect.runPromise(Scope.close(scope, Exit.void));
  }, { once: true });

  const mount = Effect.gen(function* () {
    const runtime = yield* makeSpikeRuntime;
    const report: IntentReporter = (ref, runtimeValue) =>
      Effect.gen(function* () {
        yield* runtime.registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null));
      });

    disposeNativeIntent = startNativeBridgeSync(
      () => Effect.runPromise(SubscriptionRef.get(runtime.state)),
      (envelope) => {
        const action = envelope.intent;
        const ref = action._tag === "NewChatRequested"
          ? IntentRef("SpikeNewChatRequested")
          : action._tag === "WorkspaceSelected"
            ? IntentRef("SpikeWorkspaceSelected", StaticPayload(action.workspace))
            : IntentRef("SpikeSessionSelected", StaticPayload(action.sessionRef));
        void Effect.runPromise(runtime.registry.dispatch(resolveIntentRef(ref, null)));
      },
    );

    const renderer = makeDomRenderer({ theme: khalaTheme });
    yield* renderer.mount(root, runtime.program.viewStream, report);
    document.documentElement.dataset.effectNativeMounted = "true";
  });

  void Effect.runPromise(Scope.provide(scope)(mount)).catch((error) => {
    console.error("[native-sdk-effect-native-spike] Effect Native mount failed", error);
    root.textContent = `Effect Native mount failed: ${String(error)}`;
  });
};

if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boot, { once: true });
else boot();
