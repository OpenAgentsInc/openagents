import { Effect, Layer } from "effect";
import * as ManagedRuntime from "effect/ManagedRuntime";

import { LndProcessTransportNodeLive } from "./lndProcessTransport";
import {
  defaultDesktopSecureStorageConfig,
  DesktopSecureStorageConfigLive,
  DesktopSecureStorageLive,
  type DesktopSecureStorageConfig,
} from "./desktopSecureStorage";
import {
  defaultLndWalletLocalConfig,
  LndWalletLocalConfigLive,
  LndWalletLocalServiceLive,
  type LndWalletLocalConfig,
} from "./lndWalletLocalService";
import {
  defaultLndRuntimeManagerConfig,
  LndRuntimeManagerConfigLive,
  LndRuntimeManagerLive,
  type LndRuntimeManagerConfig,
} from "./lndRuntimeManager";
import {
  defaultLndWalletManagerConfig,
  LndWalletManagerConfigLive,
  LndWalletManagerLive,
  type LndWalletManagerConfig,
} from "./lndWalletManager";
import {
  defaultSparkWalletManagerConfig,
  SparkWalletManagerConfigLive,
  SparkWalletManagerLive,
  type SparkWalletManagerConfig,
} from "./sparkWalletManager";

export type LndRuntimeManagedRuntime = ManagedRuntime.ManagedRuntime<unknown, never>;

export type LndMainProcessConfig = Readonly<{
  readonly runtimeManager: LndRuntimeManagerConfig;
  readonly secureStorage: DesktopSecureStorageConfig;
  readonly walletLocal: LndWalletLocalConfig;
  readonly walletManager: LndWalletManagerConfig;
  readonly sparkWallet: SparkWalletManagerConfig;
}>;

export const defaultLndMainProcessConfig = (input: {
  readonly appPath: string;
  readonly resourcesPath: string;
  readonly userDataPath: string;
  readonly isPackaged: boolean;
  readonly env: NodeJS.ProcessEnv;
}): LndMainProcessConfig => ({
  runtimeManager: defaultLndRuntimeManagerConfig(input),
  secureStorage: defaultDesktopSecureStorageConfig({
    userDataPath: input.userDataPath,
    env: input.env,
  }),
  walletLocal: defaultLndWalletLocalConfig({
    userDataPath: input.userDataPath,
  }),
  walletManager: defaultLndWalletManagerConfig(),
  sparkWallet: defaultSparkWalletManagerConfig({
    userDataPath: input.userDataPath,
    env: input.env,
  }),
});

export const makeLndRuntimeLayer = (config: LndMainProcessConfig) => {
  const runtimeConfigLayer = LndRuntimeManagerConfigLive(config.runtimeManager);
  const runtimeDepsLayer = Layer.provideMerge(LndProcessTransportNodeLive, runtimeConfigLayer);
  const runtimeManagerLayer = Layer.provideMerge(LndRuntimeManagerLive, runtimeDepsLayer);

  const secureStorageLayer = Layer.provideMerge(
    DesktopSecureStorageLive,
    DesktopSecureStorageConfigLive(config.secureStorage),
  );

  const walletLocalLayer = Layer.provideMerge(
    LndWalletLocalServiceLive,
    LndWalletLocalConfigLive(config.walletLocal),
  );

  const walletManagerConfigLayer = LndWalletManagerConfigLive(config.walletManager);
  const walletManagerDepsLayer = Layer.mergeAll(
    runtimeManagerLayer,
    secureStorageLayer,
    walletLocalLayer,
    walletManagerConfigLayer,
  );
  const walletManagerLayer = Layer.provideMerge(LndWalletManagerLive, walletManagerDepsLayer);

  const sparkWalletLayer = Layer.provideMerge(
    SparkWalletManagerLive,
    Layer.mergeAll(
      secureStorageLayer,
      SparkWalletManagerConfigLive(config.sparkWallet),
    ),
  );

  return Layer.mergeAll(runtimeManagerLayer, walletManagerLayer, sparkWalletLayer);
};

export const makeLndRuntimeManagedRuntime = (input: {
  readonly appPath: string;
  readonly resourcesPath: string;
  readonly userDataPath: string;
  readonly isPackaged: boolean;
  readonly env: NodeJS.ProcessEnv;
}): LndRuntimeManagedRuntime => {
  const config = defaultLndMainProcessConfig(input);
  return makeLndRuntimeManagedRuntimeFromConfig(config);
};

export const makeLndRuntimeManagedRuntimeFromConfig = (config: LndMainProcessConfig): LndRuntimeManagedRuntime => {
  const layer = makeLndRuntimeLayer(config);
  const memoMap = Effect.runSync(Layer.makeMemoMap);
  return ManagedRuntime.make(layer as Layer.Layer<unknown, never, never>, memoMap);
};
