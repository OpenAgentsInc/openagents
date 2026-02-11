import { Layer } from 'effect';
import { AppConfigService } from './config';
import { AutopilotStoreLive } from "./autopilotStore";
import { ContractsApiLive } from "./contracts";
import { AuthServiceLive } from './auth';
import { ChatServiceLive } from './chat';
import { ConvexServiceLive } from './convex';
import { PaneSystemLive, PaneSystemService } from './paneSystem';
import { RequestContextService, makeDefaultRequestContext } from './requestContext';
import { TelemetryLive } from './telemetry';

import type * as Context from 'effect/Context';
import type { AppConfig } from './config';
import type { AutopilotStoreService } from "./autopilotStore";
import type { ContractsApiService } from "./contracts";
import type { AuthService } from './auth';
import type { ChatService } from './chat';
import type { ConvexService } from './convex';
import type { TelemetryService } from './telemetry';

export type AppServices =
  | Context.Tag.Identifier<typeof AppConfigService>
  | Context.Tag.Identifier<typeof AutopilotStoreService>
  | Context.Tag.Identifier<typeof ContractsApiService>
  | Context.Tag.Identifier<typeof AuthService>
  | Context.Tag.Identifier<typeof ChatService>
  | Context.Tag.Identifier<typeof ConvexService>
  | Context.Tag.Identifier<typeof PaneSystemService>
  | Context.Tag.Identifier<typeof RequestContextService>
  | Context.Tag.Identifier<typeof TelemetryService>;

export const makeAppLayer = (config: AppConfig) => {
  // Build a base layer first, then feed its outputs into dependents.
  // `provideMerge` flows left-to-right: base -> Auth -> Convex -> Autopilot store + contracts + chat.
  const base = Layer.mergeAll(
    TelemetryLive,
    PaneSystemLive,
    Layer.succeed(AppConfigService, config),
    Layer.succeed(RequestContextService, makeDefaultRequestContext()),
  );

  const withAuth = Layer.provideMerge(AuthServiceLive, base);
  const withConvex = Layer.provideMerge(ConvexServiceLive, withAuth);
  const withStore = Layer.provideMerge(AutopilotStoreLive, withConvex);
  const withContracts = Layer.provideMerge(ContractsApiLive, withStore);
  return Layer.provideMerge(ChatServiceLive, withContracts);
};
