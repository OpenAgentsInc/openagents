import { Context, Effect, Layer, SubscriptionRef } from "effect";

import { initialDesktopRuntimeState, type DesktopRuntimeState } from "./model";

export type DesktopStateApi = Readonly<{
  readonly get: () => Effect.Effect<DesktopRuntimeState>;
  readonly set: (next: DesktopRuntimeState) => Effect.Effect<void>;
  readonly update: (f: (current: DesktopRuntimeState) => DesktopRuntimeState) => Effect.Effect<void>;
}>;

export class DesktopStateService extends Context.Tag("@openagents/desktop/DesktopStateService")<
  DesktopStateService,
  DesktopStateApi
>() {}

export const DesktopStateLive = Layer.effect(
  DesktopStateService,
  Effect.gen(function* () {
    const ref = yield* SubscriptionRef.make(initialDesktopRuntimeState());
    return DesktopStateService.of({
      get: () => SubscriptionRef.get(ref),
      set: (next) => SubscriptionRef.set(ref, next),
      update: (f) => SubscriptionRef.update(ref, f),
    });
  }),
);
