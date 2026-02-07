import { Layer } from 'effect';
import { AppConfigService } from './config';
import { AgentApiLive } from './agentApi';
import { AgentRpcClientLive } from './api/agentRpcClient';
import { AuthServiceLive } from './auth';
import { ChatServiceLive } from './chat';
import { ConvexServiceLive } from './convex';
import { RequestContextService, makeDefaultRequestContext } from './requestContext';
import { TelemetryLive } from './telemetry';

import type * as Context from 'effect/Context';
import type { AppConfig } from './config';
import type { AgentApiService } from './agentApi';
import type { AgentRpcClientService } from './api/agentRpcClient';
import type { AuthService } from './auth';
import type { ChatService } from './chat';
import type { ConvexService } from './convex';
import type { TelemetryService } from './telemetry';

export type AppServices =
  | Context.Tag.Identifier<typeof AppConfigService>
  | Context.Tag.Identifier<typeof AgentApiService>
  | Context.Tag.Identifier<typeof AgentRpcClientService>
  | Context.Tag.Identifier<typeof AuthService>
  | Context.Tag.Identifier<typeof ChatService>
  | Context.Tag.Identifier<typeof ConvexService>
  | Context.Tag.Identifier<typeof RequestContextService>
  | Context.Tag.Identifier<typeof TelemetryService>;

export const makeAppLayer = (config: AppConfig) => {
  // Build a base layer first, then feed its outputs into dependents.
  // `provideMerge` flows left-to-right: base -> Auth -> Convex -> AgentApi -> ChatService.
  const base = Layer.mergeAll(
    TelemetryLive,
    Layer.succeed(AppConfigService, config),
    Layer.succeed(RequestContextService, makeDefaultRequestContext()),
    AgentRpcClientLive,
  );

  const withAuth = Layer.provideMerge(AuthServiceLive, base);
  const withConvex = Layer.provideMerge(ConvexServiceLive, withAuth);
  const withApi = Layer.provideMerge(AgentApiLive, withConvex);
  return Layer.provideMerge(ChatServiceLive, withApi);
};
