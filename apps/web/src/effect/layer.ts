import { Layer } from 'effect';
import { AppConfigService } from './config';
import { AgentApiLive } from './agentApi';
import { AgentRpcClientLive } from './api/agentRpcClient';
import { TelemetryLive } from './telemetry';

import type * as Context from 'effect/Context';
import type { AppConfig } from './config';
import type { AgentApiService } from './agentApi';
import type { AgentRpcClientService } from './api/agentRpcClient';
import type { TelemetryService } from './telemetry';

export type AppServices =
  | Context.Tag.Identifier<typeof AppConfigService>
  | Context.Tag.Identifier<typeof AgentApiService>
  | Context.Tag.Identifier<typeof AgentRpcClientService>
  | Context.Tag.Identifier<typeof TelemetryService>;

export const makeAppLayer = (config: AppConfig) =>
  AgentApiLive.pipe(
    Layer.provideMerge(TelemetryLive),
    Layer.provideMerge(Layer.succeed(AppConfigService, config)),
    Layer.provideMerge(AgentRpcClientLive),
  );
