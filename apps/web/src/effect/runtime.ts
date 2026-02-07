import { Effect, Layer } from 'effect';
import * as ManagedRuntime from 'effect/ManagedRuntime';
import { AppConfigService } from './config';
import { makeAppLayer } from './layer';
import { TelemetryService } from './telemetry';

import type { AppConfig } from './config';
import type { AppServices } from './layer';

export type AppRuntime = ManagedRuntime.ManagedRuntime<AppServices, never>;

let singletonRuntime: AppRuntime | null = null;
let singletonMemoMap: Layer.MemoMap | null = null;
let singletonLayer: ReturnType<typeof makeAppLayer> | null = null;

export const makeAppRuntime = (config: AppConfig): AppRuntime => {
  if (singletonRuntime) return singletonRuntime;

  singletonMemoMap ??= Effect.runSync(Layer.makeMemoMap);

  singletonLayer ??= makeAppLayer(config);

  const runtime = ManagedRuntime.make(singletonLayer, singletonMemoMap);
  singletonRuntime = runtime;

  runtime.runSync(
    Effect.gen(function* () {
      const telemetry = yield* TelemetryService;
      const appConfig = yield* AppConfigService;

      yield* telemetry.withNamespace('app.init').log('info', 'Effect services initialized', {
        runtime: typeof window === 'undefined' ? 'server' : 'client',
        services: [
          'AppConfigService',
          'TelemetryService',
          'AgentApiService',
          'AgentRpcClientService',
          'ChatService',
        ],
        convexUrl: appConfig.convexUrl,
      });
    }),
  );

  return runtime;
};

export const getAppMemoMap = (config: AppConfig): Layer.MemoMap => makeAppRuntime(config).memoMap;

/** Access the singleton app layer instance used by `makeAppRuntime` (for MemoMap sharing). */
export const getAppLayer = (config: AppConfig) => {
  makeAppRuntime(config);
  return singletonLayer!;
};
