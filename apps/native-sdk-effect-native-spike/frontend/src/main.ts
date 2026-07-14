import { IntentRef, resolveIntentRef, StaticPayload, type IntentReporter } from "@effect-native/core";
import { Effect, Exit, Scope, Stream, SubscriptionRef } from "@effect-native/core/effect";
import { makeDomRenderer } from "@effect-native/render-dom";
import { khalaTheme } from "@effect-native/tokens";

import { startNativeBridgeSync } from "./native-bridge.ts";
import { makeSpikeRuntime } from "./program.ts";
import { persistSpikeState, restoreSpikeState, spikeStorageNamespace } from "./state-storage.ts";
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
    const storageNamespace = spikeStorageNamespace(window.location.href);
    const runtime = yield* makeSpikeRuntime(restoreSpikeState(window.localStorage, storageNamespace));
    yield* SubscriptionRef.changes(runtime.state).pipe(
      Stream.runForEach((state) => Effect.sync(() => {
        persistSpikeState(window.localStorage, storageNamespace, state);
      })),
      Effect.forkScoped,
    );
    const report: IntentReporter = (ref, runtimeValue) =>
      Effect.gen(function* () {
        yield* runtime.registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null));
      });

    disposeNativeIntent = startNativeBridgeSync(
      () => Effect.runPromise(SubscriptionRef.get(runtime.state)),
      (envelope) => {
        const action = envelope.intent;
        const ref = action._tag === "NewChatRequested"
          ? IntentRef("DesktopNewChat")
          : action._tag === "WorkspaceSelected"
            ? action.workspace === "settings"
              ? IntentRef("DesktopSettingsToggled")
              : IntentRef("DesktopWorkspaceSelected", StaticPayload(action.workspace))
            : IntentRef("DesktopChatSelected", StaticPayload(action.sessionRef));
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
