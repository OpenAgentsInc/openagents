import { Effect, Option, Data } from "effect";
import { ConvexReactClient } from "convex/react";
import type { 
  AuthUser, 
  AuthState, 
  APMSessionData 
} from "@openagentsinc/shared";

// Integration error types
export class ConvexIntegrationError extends Data.TaggedError("ConvexIntegrationError")<{
  operation: string;
  message: string;
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

// Sync auth user to Confect backend
export const syncUserToConfect = (
  client: ConvexReactClient, 
  user: AuthUser, 
  token: string
) =>
  Effect.tryPromise({
    try: async () => {
      // Import Confect user functions dynamically to avoid circular imports
      const { getOrCreateUser } = await import("./users");
      
      // This would typically be done via authenticated Convex client
      // For now, we'll simulate the operation
      const result = await client.mutation("getOrCreateUser", {
        email: user.email,
        name: user.name,
        avatar: user.avatar,
        githubId: user.githubId,
        githubUsername: user.githubUsername,
      });
      
      return result;
    },
    catch: (error) => new ConvexIntegrationError({
      operation: "syncUserToConfect",
      message: String(error),
      cause: error
    })
  });

// Sync APM session data to Confect backend
export const syncAPMDataToConfect = (
  client: ConvexReactClient,
  sessionData: APMSessionData,
  userId?: string
) =>
  Effect.tryPromise({
    try: async () => {
      // Sync to userDeviceSessions table via Confect
      const result = await client.mutation("trackDeviceSession", {
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
          version: "1.0.0", // Could be extracted from app config
        },
      });
      
      return result;
    },
    catch: (error) => new ConvexIntegrationError({
      operation: "syncAPMDataToConfect",
      message: String(error),
      cause: error
    })
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
      yield* Effect.tryPromise({
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
      });
      
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
  Effect.tryPromise({
    try: async () => {
      const sessionId = await client.mutation("requestDesktopSession", {
        projectPath,
        initialMessage,
        title,
      });
      
      return sessionId;
    },
    catch: (error) => new ConvexIntegrationError({
      operation: "requestDesktopSession",
      message: String(error),
      cause: error
    })
  });

// Get user APM stats from Confect backend
export const getUserAPMStatsFromConfect = (
  client: ConvexReactClient,
  includeDeviceBreakdown: boolean = false
) =>
  Effect.tryPromise({
    try: async () => {
      const stats = await client.query("getUserAPMStats", {
        includeDeviceBreakdown,
      });
      
      return stats;
    },
    catch: (error) => new ConvexIntegrationError({
      operation: "getUserAPMStats",
      message: String(error),
      cause: error
    })
  });

// Subscribe to real-time session updates
export const subscribeToSessionUpdates = (
  client: ConvexReactClient,
  onSessionUpdate: (sessions: any[]) => void
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