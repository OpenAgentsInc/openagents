import { Layer } from "effect";
import type * as Context from "effect/Context";

import { AuthGatewayLive } from "./authGateway";
import { DesktopConfigLive, type DesktopConfig } from "./config";
import { ConnectivityProbeLive } from "./connectivity";
import { DesktopAppLive, DesktopAppService } from "./app";
import { DesktopStateLive } from "./state";
import { ExecutorLoopLive } from "./executorLoop";
import { TaskProviderLive } from "./taskProvider";
import { LndRuntimeGatewayLive } from "./lndRuntimeGateway";
import { LndWalletGatewayLive } from "./lndWalletGateway";
import { DesktopSessionLive } from "./session";
import { L402ExecutorLive } from "./l402Executor";

export type DesktopLayerOverrides = Readonly<{
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly authGateway?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly connectivity?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly taskProvider?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly lndRuntimeGateway?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly lndWalletGateway?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly l402Executor?: Layer.Layer<any, any, any>;
}>;

export const makeDesktopLayer = (
  config?: Partial<DesktopConfig>,
  overrides?: DesktopLayerOverrides,
): Layer.Layer<Context.Tag.Identifier<typeof DesktopAppService>, never, never> => {
  const configLayer = DesktopConfigLive(config);
  const taskProviderLayer = Layer.provideMerge(overrides?.taskProvider ?? TaskProviderLive, configLayer);
  const authGatewayLayer = Layer.provideMerge(overrides?.authGateway ?? AuthGatewayLive, configLayer);
  const connectivityLayer = Layer.provideMerge(overrides?.connectivity ?? ConnectivityProbeLive, configLayer);

  const base = Layer.mergeAll(
    DesktopStateLive,
    DesktopSessionLive,
    configLayer,
    taskProviderLayer,
    overrides?.lndRuntimeGateway ?? LndRuntimeGatewayLive,
    overrides?.lndWalletGateway ?? LndWalletGatewayLive,
    overrides?.l402Executor ?? L402ExecutorLive,
  );

  const withAuth = Layer.provideMerge(authGatewayLayer, base);
  const withConnectivity = Layer.provideMerge(connectivityLayer, withAuth);
  const withExecutor = Layer.provideMerge(ExecutorLoopLive, withConnectivity);
  return Layer.provideMerge(DesktopAppLive, withExecutor) as Layer.Layer<
    Context.Tag.Identifier<typeof DesktopAppService>,
    never,
    never
  >;
};
