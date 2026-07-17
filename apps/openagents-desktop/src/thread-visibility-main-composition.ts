import { Effect, Schema } from "effect";

import {
  registerDesktopThreadVisibilityMainHandler,
  type DesktopThreadVisibilityMainHandlerDependencies,
} from "./thread-visibility-main-handler.ts";
import { openDesktopThreadVisibilityPolicyStore } from "./thread-visibility-policy-store.ts";

export class DesktopThreadVisibilityMainCompositionUnavailable extends Schema.TaggedErrorClass<DesktopThreadVisibilityMainCompositionUnavailable>()(
  "DesktopThreadVisibilityMainCompositionUnavailable",
  { stage: Schema.Literal("register") },
) {}

export type DesktopThreadVisibilityMainCompositionDependencies = Readonly<{
  policyFile: string;
  register: DesktopThreadVisibilityMainHandlerDependencies["register"];
  isTrustedSender: DesktopThreadVisibilityMainHandlerDependencies["isTrustedSender"];
  makeReceiptRef: DesktopThreadVisibilityMainHandlerDependencies["makeReceiptRef"];
  observedAt: DesktopThreadVisibilityMainHandlerDependencies["observedAt"];
}>;

export type DesktopThreadVisibilityMainComposition = Readonly<{ close: () => void }>;

const closeWithoutProjection = (registration: DesktopThreadVisibilityMainComposition): void => {
  try {
    registration.close();
  } catch {
    // Native cleanup details stay inside the host boundary.
  }
};

/**
 * Owns the private visibility store and its one fixed-channel handler. The
 * store, its path, and Effect runtime remain main-process implementation
 * details; the caller receives only the handler lifetime resource.
 */
export const openDesktopThreadVisibilityMainComposition = Effect.fn(
  "DesktopThreadVisibilityMainComposition.open",
)(function* (dependencies: DesktopThreadVisibilityMainCompositionDependencies) {
  const store = openDesktopThreadVisibilityPolicyStore(dependencies.policyFile);
  const registration = yield* Effect.try({
    try: () =>
      registerDesktopThreadVisibilityMainHandler({
        register: dependencies.register,
        isTrustedSender: dependencies.isTrustedSender,
        makeReceiptRef: dependencies.makeReceiptRef,
        observedAt: dependencies.observedAt,
        apply: (input) => Effect.runPromise(store.apply(input)),
      }),
    catch: () => new DesktopThreadVisibilityMainCompositionUnavailable({ stage: "register" }),
  });

  let closed = false;
  return {
    close: (): void => {
      if (closed) return;
      closed = true;
      closeWithoutProjection(registration);
    },
  } satisfies DesktopThreadVisibilityMainComposition;
});
