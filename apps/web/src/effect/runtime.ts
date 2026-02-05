import { Effect } from 'effect';
import * as ManagedRuntime from 'effect/ManagedRuntime';
import { AppConfigService } from './config';
import { makeAppLayer } from './layer';
import { TelemetryService } from './telemetry';

import type { AppConfig } from './config';
import type { AppServices } from './layer';

export type AppRuntime = ManagedRuntime.ManagedRuntime<AppServices, never>;

let didInit = false;

export const makeAppRuntime = (config: AppConfig): AppRuntime => {
  const runtime = ManagedRuntime.make(makeAppLayer(config));

  if (!didInit) {
    didInit = true;

    runtime.runSync(
      Effect.gen(function* () {
        const telemetry = yield* TelemetryService;
        const appConfig = yield* AppConfigService;

        yield* telemetry.withNamespace('app.init').log('info', 'Effect services initialized', {
          runtime: typeof window === 'undefined' ? 'server' : 'client',
          services: ['AppConfigService', 'TelemetryService'],
          convexUrl: appConfig.convexUrl,
        });
      }),
    );
  }

  return runtime;
};
