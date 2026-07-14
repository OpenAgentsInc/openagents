import { Effect } from "@effect-native/core/effect";
import { it as vitePlusIt } from "vite-plus/test";

type EffectTest = typeof vitePlusIt & {
  readonly effect: <A, E>(name: string, body: () => Effect.Effect<A, E>, timeout?: number) => void;
};

/** Small Effect-aware adapter until Vite Plus owns this integration itself. */
export const it = Object.assign(vitePlusIt, {
  effect: <A, E>(name: string, body: () => Effect.Effect<A, E>, timeout?: number): void => {
    vitePlusIt(name, () => Effect.runPromise(body()), timeout);
  },
}) as EffectTest;
