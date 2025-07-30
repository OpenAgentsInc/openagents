/**
 * Simplified Session Resilience Service - Basic retry and fallback patterns
 */

import { Effect, Schedule } from "effect";
import {
  SessionCreationError,
  CreateSessionParams,
  SessionData,
} from "../types/session-service-types.js";

export interface SessionResilienceServiceInterface {
  readonly withRetry: <A, E>(operation: Effect.Effect<A, E>, maxRetries?: number) => Effect.Effect<A, E>;
  readonly withFallback: <A, E>(
    primary: Effect.Effect<A, E>, 
    fallback: Effect.Effect<A, never>
  ) => Effect.Effect<A, never>;
  readonly createSessionResilient: (params: CreateSessionParams) => Effect.Effect<SessionData, never>;
}

export const createSessionResilienceService = (): SessionResilienceServiceInterface => ({
  withRetry: <A, E>(operation: Effect.Effect<A, E>, maxRetries: number = 3) =>
    operation.pipe(
      Effect.retry(Schedule.recurs(maxRetries))
    ),

  withFallback: <A, E>(
    primary: Effect.Effect<A, E>, 
    fallback: Effect.Effect<A, never>
  ) =>
    primary.pipe(
      Effect.catchAll(() => fallback)
    ),

  createSessionResilient: (params: CreateSessionParams) => {
    const self = createSessionResilienceService();
    
    // Primary operation - would normally call the actual service
    const primaryOperation = Effect.gen(function* () {
      console.log(`Creating session with resilience: ${params.sessionId}`);
      
      // Simulate potential failure
      if (Math.random() > 0.7) {
        return yield* Effect.fail(new SessionCreationError({
          reason: "Simulated database failure",
          sessionId: params.sessionId
        }));
      }
      
      const sessionData: SessionData = {
        sessionId: params.sessionId,
        projectPath: params.projectPath,
        title: params.title || `${params.createdBy} Session`,
        status: "active",
        createdBy: params.createdBy,
        lastActivity: Date.now(),
        createdAt: Date.now(),
        userId: "current-user",
        metadata: params.metadata || {},
        version: 1
      };
      
      return sessionData;
    });
    
    // Fallback operation - creates offline session
    const fallbackOperation = Effect.gen(function* () {
      console.log(`📱 [RESILIENCE] Creating offline session for: ${params.sessionId}`);
      
      const offlineSession: SessionData = {
        sessionId: `offline-${params.sessionId}`,
        projectPath: params.projectPath,
        title: params.title || `Offline Session - ${params.createdBy}`,
        status: "offline",
        createdBy: params.createdBy,
        lastActivity: Date.now(),
        createdAt: Date.now(),
        userId: "offline-user",
        metadata: {
          ...params.metadata,
          // Note: Adding custom properties that extend SessionMetadata
        } as any,
        version: 1
      };
      
      return offlineSession;
    });
    
    return self.withFallback(
      self.withRetry(primaryOperation, 3),
      fallbackOperation
    );
  }
});

// Export a default instance
export const defaultSessionResilienceService = createSessionResilienceService();