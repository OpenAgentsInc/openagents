import { Effect, Layer } from "effect";
import * as ManagedRuntime from "effect/ManagedRuntime";

import { makeDesktopConfig, type DesktopConfig } from "./config";
import { makeDesktopLayer } from "./layer";

export type DesktopRuntime = ManagedRuntime.ManagedRuntime<unknown, unknown>;

let singletonRuntime: DesktopRuntime | null = null;
let singletonMemoMap: Layer.MemoMap | null = null;

export const makeDesktopRuntime = (
  config?: Partial<DesktopConfig>,
  layer = makeDesktopLayer(makeDesktopConfig(config)),
): DesktopRuntime => {
  if (!singletonRuntime) {
    singletonMemoMap ??= Effect.runSync(Layer.makeMemoMap);
    singletonRuntime = ManagedRuntime.make(
      layer as Layer.Layer<unknown, unknown, never>,
      singletonMemoMap,
    );
  }
  return singletonRuntime;
};
