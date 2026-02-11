import { Layer } from "effect";

import { DesktopSecureStorageInMemoryTestLayer } from "../../src/main/desktopSecureStorage";
import {
  defaultLndWalletManagerConfig,
  LndWalletManagerConfigLive,
  LndWalletManagerLive,
} from "../../src/main/lndWalletManager";
import {
  LndWalletLocalConfigLive,
  LndWalletLocalServiceLive,
} from "../../src/main/lndWalletLocalService";
import { makeLndRuntimeHarness } from "./lndRuntimeHarness";

export const makeLndWalletHarness = () => {
  const runtimeHarness = makeLndRuntimeHarness();

  const walletLocalLayer = Layer.provideMerge(
    LndWalletLocalServiceLive,
    LndWalletLocalConfigLive({
      userDataPath: runtimeHarness.rootDir,
      defaultWalletState: "uninitialized",
    }),
  );

  const walletManagerLayer = Layer.provideMerge(
    LndWalletManagerLive,
    Layer.mergeAll(
      runtimeHarness.layer,
      DesktopSecureStorageInMemoryTestLayer,
      walletLocalLayer,
      LndWalletManagerConfigLive(defaultLndWalletManagerConfig()),
    ),
  );

  return {
    runtimeHarness,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    layer: Layer.mergeAll(runtimeHarness.layer, walletManagerLayer) as Layer.Layer<any, never, never>,
    cleanup: runtimeHarness.cleanup,
  };
};
