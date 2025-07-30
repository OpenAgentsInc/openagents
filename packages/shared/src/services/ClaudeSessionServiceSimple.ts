/**
 * Simplified Claude Session Service Implementation
 * Compatible with existing codebase patterns
 */

import { Effect } from "effect";
import {
  SessionCreationError,
  SessionNotFoundError,
  SessionPermissionError,
  CreateSessionParams,
  SessionData,
  SessionStatus,
  SessionQueryCriteria,
  SessionQueryResult,
} from "../types/session-service-types.js";

// Simplified service interface
export interface ClaudeSessionServiceInterface {
  readonly createSession: (params: CreateSessionParams) => Effect.Effect<SessionData, SessionCreationError>;
  readonly getSession: (sessionId: string, userId?: string) => Effect.Effect<SessionData, SessionNotFoundError>;
  readonly updateSessionStatus: (sessionId: string, status: SessionStatus) => Effect.Effect<SessionData, never>;
  readonly deleteSession: (sessionId: string, userId: string) => Effect.Effect<void, SessionPermissionError>;
  readonly querySessionsAdvanced: (criteria: SessionQueryCriteria) => Effect.Effect<SessionQueryResult, never>;
}

// Simple implementation without complex Effect.Service pattern
export const createClaudeSessionService = (): ClaudeSessionServiceInterface => ({
  createSession: (params: CreateSessionParams) =>
    Effect.gen(function* () {
      console.log(`🔄 [SESSION_SERVICE] Creating session: ${params.sessionId}`);
      
      // Basic validation
      if (!params.sessionId || params.sessionId.trim().length === 0) {
        return yield* Effect.fail(new SessionCreationError({
          reason: "Session ID is required",
          sessionId: params.sessionId || "unknown"
        }));
      }
      
      if (!params.projectPath || params.projectPath.trim().length === 0) {
        return yield* Effect.fail(new SessionCreationError({
          reason: "Project path is required", 
          sessionId: params.sessionId
        }));
      }
      
      // Build session data
      const sessionData: SessionData = {
        sessionId: params.sessionId,
        projectPath: params.projectPath.trim(),
        title: params.title || `${params.createdBy} Session - ${new Date().toLocaleString()}`,
        status: "active" as const,
        createdBy: params.createdBy,
        lastActivity: Date.now(),
        createdAt: Date.now(),
        userId: "current-user", // TODO: Get from auth
        metadata: params.metadata || {},
        version: 1
      };
      
      console.log(`✅ [SESSION_SERVICE] Session created: ${sessionData.sessionId}`);
      return sessionData;
    }),

  getSession: (sessionId: string, userId?: string) =>
    Effect.gen(function* () {
      // Simulate database lookup
      const sessionData: SessionData = {
        sessionId,
        projectPath: "/example/project",
        title: "Example Session",
        status: "active",
        createdBy: "desktop",
        lastActivity: Date.now(),
        createdAt: Date.now(),
        userId: userId || "current-user",
        metadata: {},
        version: 1
      };
      
      return sessionData;
    }),

  updateSessionStatus: (sessionId: string, status: SessionStatus) =>
    Effect.gen(function* () {
      console.log(`🔄 [SESSION_SERVICE] Updating session status: ${sessionId} -> ${status}`);
      
      const updatedSession: SessionData = {
        sessionId,
        projectPath: "/example/project",
        title: "Example Session",
        status,
        createdBy: "desktop",
        lastActivity: Date.now(),
        createdAt: Date.now() - 60000,
        userId: "current-user",
        metadata: {},
        version: 1
      };
      
      return updatedSession;
    }),

  deleteSession: (sessionId: string, userId: string) =>
    Effect.gen(function* () {
      console.log(`🗑️ [SESSION_SERVICE] Deleting session: ${sessionId} (user: ${userId})`);
      return Effect.void;
    }),

  querySessionsAdvanced: (criteria: SessionQueryCriteria) =>
    Effect.gen(function* () {
      console.log(`🔍 [SESSION_SERVICE] Querying sessions:`, criteria);
      
      // Simulate query results
      const sessions: SessionData[] = [
        {
          sessionId: "example-1",
          projectPath: "/example/project1",
          title: "Example Session 1",
          status: "active",
          createdBy: "mobile",
          lastActivity: Date.now(),
          createdAt: Date.now() - 120000,
          userId: criteria.userId || "current-user",
          metadata: {},
          version: 1
        }
      ];
      
      return {
        sessions,
        totalCount: sessions.length,
        hasMore: false
      } as SessionQueryResult;
    })
});

// Export a default instance
export const defaultClaudeSessionService = createClaudeSessionService();