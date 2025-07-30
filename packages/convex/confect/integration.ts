import { Effect, Data, Schedule, Duration } from "effect";
import { ConvexReactClient } from "convex/react";
import type { 
  AuthUser, 
  AuthState
} from "../../shared/src/services/SimpleAuthService";
import type { 
  APMSessionData 
} from "../../shared/src/services/SimpleAPMService";

// Integration error types
export class ConvexIntegrationError extends Data.TaggedError("ConvexIntegrationError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class ConvexMutationError extends Data.TaggedError("ConvexMutationError")<{
  mutation: string;
  args: unknown;
  message: string;
  retryable: boolean;
  cause?: unknown;
}> {}

export class ConvexQueryError extends Data.TaggedError("ConvexQueryError")<{
  query: string;
  args: unknown;
  message: string;
  retryable: boolean;
  cause?: unknown;
}> {}

// Integration configuration
export interface ConfectIntegrationConfig {
  convexUrl: string;
  enableRealTimeSync?: boolean;
  debugMode?: boolean;
}

// Create Convex client for Effect services integration
export const createConvexClient = (config: ConfectIntegrationConfig) =>
  Effect.sync(() => {
    return new ConvexReactClient(config.convexUrl);
  });

// Safe Convex mutation wrapper with Effect error boundaries
export const safeConvexMutation = <TArgs, TResult>(
  client: ConvexReactClient,
  mutationName: string,
  args: TArgs
): Effect.Effect<TResult, ConvexMutationError, never> =>
  Effect.tryPromise({
    try: async () => {
      const result = await client.mutation(mutationName as any, args);
      return result as TResult;
    },
    catch: (error) => new ConvexMutationError({
      mutation: mutationName,
      args,
      message: error instanceof Error ? error.message : String(error),
      retryable: isRetryableError(error),
      cause: error
    })
  });

// Safe Convex query wrapper with Effect error boundaries  
export const safeConvexQuery = <TArgs, TResult>(
  client: ConvexReactClient,
  queryName: string,
  args: TArgs
): Effect.Effect<TResult, ConvexQueryError, never> =>
  Effect.tryPromise({
    try: async () => {
      const result = await client.query(queryName as any, args);
      return result as TResult;
    },
    catch: (error) => new ConvexQueryError({
      query: queryName,
      args,
      message: error instanceof Error ? error.message : String(error),
      retryable: isRetryableError(error),
      cause: error
    })
  });

// Helper to determine if an error is retryable
const isRetryableError = (error: unknown): boolean => {
  if (error instanceof Error) {
    // Network errors, timeouts, and 5xx errors are retryable
    return error.message.includes('network') ||
           error.message.includes('timeout') ||
           error.message.includes('503') ||
           error.message.includes('502') ||
           error.message.includes('500');
  }
  return false;
};

// Get app version from package.json dynamically
const getAppVersion = (): Effect.Effect<string, never, never> =>
  Effect.sync(() => {
    try {
      // Try to get version from environment variable first
      if (typeof process !== 'undefined' && process.env?.REACT_APP_VERSION) {
        return process.env.REACT_APP_VERSION;
      }
      
      // Fallback to a default version
      return "1.0.0";
    } catch {
      return "1.0.0";
    }
  });

// Sync auth user to Confect backend
export const syncUserToConfect = (
  client: ConvexReactClient, 
  user: AuthUser, 
  _token: string
) =>
  Effect.gen(function* () {
    yield* Effect.logInfo(`🔐 [CONFECT] Syncing user to backend: ${user.githubUsername}`);
    
    const result = yield* safeConvexMutation(client, "getOrCreateUser", {
      email: user.email,
      name: user.name,
      avatar: user.avatar,
      githubId: user.githubId,
      githubUsername: user.githubUsername,
    }).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(1)).pipe(
          Schedule.intersect(Schedule.recurs(2))
        )
      ),
      Effect.mapError(error => new ConvexIntegrationError({
        operation: "syncUserToConfect",
        message: `Failed to sync user ${user.githubUsername}: ${error.message}`,
        cause: error
      }))
    );
    
    yield* Effect.logInfo(`✅ [CONFECT] User sync completed: ${user.githubUsername}`);
    return result;
  });

// Sync APM session data to Confect backend
export const syncAPMDataToConfect = (
  client: ConvexReactClient,
  sessionData: APMSessionData,
  _userId?: string
) =>
  Effect.gen(function* () {
    yield* Effect.logInfo(`📊 [CONFECT] Syncing APM data for device: ${sessionData.deviceId}`);
    
    // Get app version from package.json
    const appVersion = yield* getAppVersion();
    
    const result = yield* safeConvexMutation(client, "trackDeviceSession", {
      deviceId: sessionData.deviceId,
      deviceType: sessionData.platform === "ios" || sessionData.platform === "android" 
        ? "mobile" as const 
        : "desktop" as const,
      sessionStart: sessionData.sessionStart,
      sessionEnd: sessionData.sessionEnd,
      actions: {
        messages: sessionData.messagesSent,
        toolUses: 0, // Not tracked in simple APM
        githubEvents: 0,
      },
      metadata: {
        platform: sessionData.platform,
        version: appVersion,
      },
    }).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(2)).pipe(
          Schedule.intersect(Schedule.recurs(2))
        )
      ),
      Effect.mapError(error => new ConvexIntegrationError({
        operation: "syncAPMDataToConfect",
        message: `Failed to sync APM data for device ${sessionData.deviceId}: ${error.message}`,
        cause: error
      }))
    );
    
    yield* Effect.logInfo(`✅ [CONFECT] APM data sync completed for device: ${sessionData.deviceId}`);
    return result;
  });

// Enhanced APM service that uses Confect backend
export const enhancedSendSessionDataToBackend = (
  client: ConvexReactClient,
  sessionData: APMSessionData,
  authState?: AuthState
) =>
  Effect.gen(function* () {
    yield* Effect.log(`📊 [CONFECT] Enhanced session sync for device: ${sessionData.deviceId}`);
    
    // If user is authenticated, sync through Confect
    if (authState?.isAuthenticated && authState.user) {
      // First ensure user exists in backend
      yield* syncUserToConfect(client, authState.user, authState.token!);
      
      // Then sync APM data
      yield* syncAPMDataToConfect(client, sessionData, authState.user.id);
      
      // Trigger aggregated APM calculation
      yield* safeConvexMutation(client, "calculateUserAPM", {
        timeWindow: "1h" as const
      }).pipe(
        Effect.catchAll(() => 
          Effect.logWarning("Failed to calculate user APM, continuing anyway")
        )
      );
      
      // Remove old tryPromise block - replaced with safeConvexMutation
      /*yield* Effect.tryPromise({
        try: async () => {
          await client.mutation("calculateUserAPM", {
            timeWindow: "1h" as const
          });
        },
        catch: (error) => new ConvexIntegrationError({
          operation: "calculateUserAPM",
          message: String(error),
          cause: error
        })
      });*/
      
      yield* Effect.log(`✅ [CONFECT] Session data synced to backend with user association`);
    } else {
      // Fallback to local-only tracking for unauthenticated users
      yield* Effect.log(`⚠️ [CONFECT] User not authenticated, skipping backend sync`);
    }
  });

// Create mobile session request via Confect
export const requestDesktopSessionViaConfect = (
  client: ConvexReactClient,
  projectPath: string,
  initialMessage?: string,
  title?: string
) =>
  Effect.gen(function* () {
    yield* Effect.logInfo(`📱 [CONFECT] Requesting desktop session for: ${projectPath}`);
    
    const sessionId = yield* safeConvexMutation(client, "requestDesktopSession", {
      projectPath,
      initialMessage,
      title,
    }).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(1)).pipe(
          Schedule.intersect(Schedule.recurs(2))
        )
      ),
      Effect.mapError(error => new ConvexIntegrationError({
        operation: "requestDesktopSession",
        message: `Failed to request desktop session for ${projectPath}: ${error.message}`,
        cause: error
      }))
    );
    
    yield* Effect.logInfo(`✅ [CONFECT] Desktop session requested: ${sessionId}`);
    return sessionId;
  });

// Get user APM stats from Confect backend
export const getUserAPMStatsFromConfect = (
  client: ConvexReactClient,
  includeDeviceBreakdown: boolean = false
) =>
  Effect.gen(function* () {
    yield* Effect.logInfo(`📊 [CONFECT] Fetching user APM stats (deviceBreakdown: ${includeDeviceBreakdown})`);
    
    const stats = yield* safeConvexQuery(client, "getUserAPMStats", {
      includeDeviceBreakdown,
    }).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(1)).pipe(
          Schedule.intersect(Schedule.recurs(2))
        )
      ),
      Effect.mapError(error => new ConvexIntegrationError({
        operation: "getUserAPMStats",
        message: `Failed to fetch APM stats: ${error.message}`,
        cause: error
      }))
    );
    
    yield* Effect.logInfo(`✅ [CONFECT] APM stats fetched successfully`);
    return stats;
  });

// Subscribe to real-time session updates
export const subscribeToSessionUpdates = (
  _client: ConvexReactClient,
  _onSessionUpdate: (sessions: any[]) => void
) =>
  Effect.sync(() => {
    // This would be implemented with Convex's real-time subscriptions
    // For now, we'll return a cleanup function
    return () => {
      // Cleanup subscription
    };
  });

// Integration utilities
export const createIntegratedEffectServices = (config: ConfectIntegrationConfig) =>
  Effect.gen(function* () {
    const client = yield* createConvexClient(config);
    
    return {
      client,
      syncUser: (user: AuthUser, token: string) => 
        syncUserToConfect(client, user, token),
      syncAPMData: (sessionData: APMSessionData, authState?: AuthState) =>
        enhancedSendSessionDataToBackend(client, sessionData, authState),
      requestDesktopSession: (projectPath: string, initialMessage?: string, title?: string) =>
        requestDesktopSessionViaConfect(client, projectPath, initialMessage, title),
      getUserAPMStats: (includeDeviceBreakdown?: boolean) =>
        getUserAPMStatsFromConfect(client, includeDeviceBreakdown),
      subscribeToSessions: (onSessionUpdate: (sessions: any[]) => void) =>
        subscribeToSessionUpdates(client, onSessionUpdate),
    };
  });