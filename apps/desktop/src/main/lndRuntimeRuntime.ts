import { Effect, Layer } from "effect";
import * as ManagedRuntime from "effect/ManagedRuntime";

import { LndProcessTransportNodeLive } from "./lndProcessTransport";
import {
  defaultLndRuntimeManagerConfig,
  LndRuntimeManagerConfigLive,
  LndRuntimeManagerLive,
  type LndRuntimeManagerConfig,
} from "./lndRuntimeManager";

export type LndRuntimeManagedRuntime = ManagedRuntime.ManagedRuntime<unknown, never>;

export const makeLndRuntimeLayer = (config: LndRuntimeManagerConfig) => {
  const cfgLayer = LndRuntimeManagerConfigLive(config);
  const withTransport = Layer.provideMerge(LndProcessTransportNodeLive, cfgLayer);
  return Layer.provideMerge(LndRuntimeManagerLive, withTransport);
};

export const makeLndRuntimeManagedRuntime = (input: {
  readonly appPath: string;
  readonly resourcesPath: string;
  readonly userDataPath: string;
  readonly isPackaged: boolean;
  readonly env: NodeJS.ProcessEnv;
}): LndRuntimeManagedRuntime => {
  const config = defaultLndRuntimeManagerConfig(input);
  return makeLndRuntimeManagedRuntimeFromConfig(config);
};

export const makeLndRuntimeManagedRuntimeFromConfig = (
  config: LndRuntimeManagerConfig,
): LndRuntimeManagedRuntime => {
  const layer = makeLndRuntimeLayer(config);
  const memoMap = Effect.runSync(Layer.makeMemoMap);
  return ManagedRuntime.make(layer as Layer.Layer<unknown, never, never>, memoMap);
};
