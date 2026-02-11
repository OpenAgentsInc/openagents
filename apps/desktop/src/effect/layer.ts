import { Layer } from "effect";
import type * as Context from "effect/Context";

import { AuthGatewayLive } from "./authGateway";
import { DesktopConfigLive, type DesktopConfig } from "./config";
import { ConnectivityProbeLive } from "./connectivity";
import { DesktopAppLive, DesktopAppService } from "./app";
import { DesktopStateLive } from "./state";
import { ExecutorLoopLive } from "./executorLoop";
import { TaskProviderLive } from "./taskProvider";

export type DesktopLayerOverrides = Readonly<{
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly authGateway?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly connectivity?: Layer.Layer<any, any, any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly taskProvider?: Layer.Layer<any, any, any>;
}>;

export const makeDesktopLayer = (
  config?: Partial<DesktopConfig>,
  overrides?: DesktopLayerOverrides,
): Layer.Layer<Context.Tag.Identifier<typeof DesktopAppService>, never, never> => {
  const base = Layer.mergeAll(
    DesktopConfigLive(config),
    DesktopStateLive,
    overrides?.taskProvider ?? TaskProviderLive,
  );

  const withAuth = Layer.provideMerge(overrides?.authGateway ?? AuthGatewayLive, base);
  const withConnectivity = Layer.provideMerge(overrides?.connectivity ?? ConnectivityProbeLive, withAuth);
  const withExecutor = Layer.provideMerge(ExecutorLoopLive, withConnectivity);
  return Layer.provideMerge(DesktopAppLive, withExecutor) as Layer.Layer<
    Context.Tag.Identifier<typeof DesktopAppService>,
    never,
    never
  >;
};
