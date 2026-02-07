import { Layer } from 'effect';
import { AppConfigService } from './config';
import { AgentApiLive } from './agentApi';
import { AgentRpcClientLive } from './api/agentRpcClient';
import { ChatServiceLive } from './chat';
import { TelemetryLive } from './telemetry';

import type * as Context from 'effect/Context';
import type { AppConfig } from './config';
import type { AgentApiService } from './agentApi';
import type { AgentRpcClientService } from './api/agentRpcClient';
import type { ChatService } from './chat';
import type { TelemetryService } from './telemetry';

export type AppServices =
  | Context.Tag.Identifier<typeof AppConfigService>
  | Context.Tag.Identifier<typeof AgentApiService>
  | Context.Tag.Identifier<typeof AgentRpcClientService>
  | Context.Tag.Identifier<typeof ChatService>
  | Context.Tag.Identifier<typeof TelemetryService>;

export const makeAppLayer = (config: AppConfig) => {
  // Build a base layer first, then feed its outputs into dependents.
  // `provideMerge` flows left-to-right: base -> AgentApi -> ChatService.
  const base = Layer.mergeAll(
    TelemetryLive,
    Layer.succeed(AppConfigService, config),
    AgentRpcClientLive,
  );

  const withApi = Layer.provideMerge(AgentApiLive, base);
  return Layer.provideMerge(ChatServiceLive, withApi);
};
