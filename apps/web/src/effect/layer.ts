import { Layer } from 'effect';
import { AppConfigService } from './config';
import { TelemetryLive } from './telemetry';

import type * as Context from 'effect/Context';
import type { AppConfig } from './config';
import type { TelemetryService } from './telemetry';

export type AppServices =
  | Context.Tag.Identifier<typeof AppConfigService>
  | Context.Tag.Identifier<typeof TelemetryService>;

export const makeAppLayer = (config: AppConfig) =>
  Layer.mergeAll(Layer.succeed(AppConfigService, config), TelemetryLive);
