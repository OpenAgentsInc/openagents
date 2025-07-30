/**
 * React hook for ClaudeSessionService integration - Mobile
 * Provides Effect-TS service integration with React components
 */

import React, { useContext, useCallback, useState } from "react";
import { Effect, Runtime, Exit } from "effect";
import {
  createClaudeSessionService,
  createSessionResilienceService, 
  CreateSessionParams,
  SessionData,
  SessionStatus,
  SessionQueryCriteria,
  SessionQueryResult,
} from "../../../../packages/shared/src";

// Effect Runtime Context (would be provided at app level)
interface EffectRuntimeContext {
  runtime: Runtime.Runtime<never>;
}

const EffectRuntimeContext = React.createContext<EffectRuntimeContext | null>(null);

export interface UseClaudeSessionServiceOptions {
  onError?: (error: string) => void;
  onSuccess?: (message: string) => void;
}

export interface UseClaudeSessionServiceReturn {
  // Core operations
  createSession: (params: CreateSessionParams) => Promise<SessionData>;
  getSession: (sessionId: string) => Promise<SessionData>;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => Promise<SessionData>;
  deleteSession: (sessionId: string) => Promise<void>;
  querySessionsAdvanced: (criteria: SessionQueryCriteria) => Promise<SessionQueryResult>;
  
  // Resilient operations
  createSessionResilient: (params: CreateSessionParams) => Promise<SessionData>;
  
  // State
  isLoading: boolean;
  error: string | null;
  
  // Utilities
  clearError: () => void;
  getResilienceHealth: () => Promise<any>;
}

export function useClaudeSessionService(
  options: UseClaudeSessionServiceOptions = {}
): UseClaudeSessionServiceReturn {
  const context = useContext(EffectRuntimeContext);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  if (!context) {
    throw new Error("useClaudeSessionService must be used within EffectRuntimeProvider");
  }
  
  const { runtime } = context;
  
  // Helper to run Effect operations with error handling
  const runEffect = useCallback(async <A, E>(
    effect: Effect.Effect<A, E>,
    operationName: string
  ): Promise<A> => {
    setIsLoading(true);
    setError(null);
    
    try {
      const exit = await Runtime.runPromiseExit(runtime)(effect);
      
      if (Exit.isFailure(exit)) {
        const cause = exit.cause;
        let errorMessage = `${operationName} failed: Unknown error`;
        
        // Handle specific error types
        if (cause._tag === "Fail") {
          const error = cause.error as any;
          if (error._tag === "SessionCreationError") {
            errorMessage = `Session creation failed: ${error.reason}`;
          } else if (error._tag === "SessionNotFoundError") {
            errorMessage = `Session not found: ${error.sessionId}`;
          } else if (error._tag === "SessionPermissionError") {
            errorMessage = `Permission denied for session: ${error.sessionId}`;
          } else if (error._tag === "SessionValidationError") {
            errorMessage = `Validation error: ${error.reason}`;
          } else {
            errorMessage = `${operationName} failed: ${error}`;
          }
        }
        
        console.error(`❌ [MOBILE_SESSION_HOOK] ${errorMessage}`, { cause });
        setError(errorMessage);
        options.onError?.(errorMessage);
        throw new Error(errorMessage);
      }
      
      const result = exit.value;
      console.log(`✅ [MOBILE_SESSION_HOOK] ${operationName} completed successfully`);
      options.onSuccess?.(`${operationName} completed successfully`);
      return result;
    } catch (error) {
      const errorMessage = `${operationName} failed: ${error}`;
      console.error(`❌ [MOBILE_SESSION_HOOK] ${errorMessage}`, error);
      setError(errorMessage);
      options.onError?.(errorMessage);
      throw error;
    } finally {
      setIsLoading(false);
    }
  }, [runtime, options]);
  
  // Core session operations
  const createSession = useCallback(async (params: CreateSessionParams): Promise<SessionData> => {
    const sessionService = createClaudeSessionService();
    const program = sessionService.createSession(params);
    
    return runEffect(program, "createSession");
  }, [runEffect]);
  
  const getSession = useCallback(async (sessionId: string): Promise<SessionData> => {
    const sessionService = createClaudeSessionService();
    const program = sessionService.getSession(sessionId);
    
    return runEffect(program, "getSession");
  }, [runEffect]);
  
  const updateSessionStatus = useCallback(async (
    sessionId: string, 
    status: SessionStatus
  ): Promise<SessionData> => {
    const sessionService = createClaudeSessionService();
    const program = sessionService.updateSessionStatus(sessionId, status);
    
    return runEffect(program, "updateSessionStatus");
  }, [runEffect]);
  
  const deleteSession = useCallback(async (sessionId: string): Promise<void> => {
    const sessionService = createClaudeSessionService();
    const program = sessionService.deleteSession(sessionId, "current-user-id"); // TODO: Get from auth
    
    return runEffect(program, "deleteSession");
  }, [runEffect]);
  
  const querySessionsAdvanced = useCallback(async (
    criteria: SessionQueryCriteria
  ): Promise<SessionQueryResult> => {
    const sessionService = createClaudeSessionService();
    const program = sessionService.querySessionsAdvanced(criteria);
    
    return runEffect(program, "querySessionsAdvanced");
  }, [runEffect]);
  
  // Resilient operations
  const createSessionResilient = useCallback(async (params: CreateSessionParams): Promise<SessionData> => {
    const resilienceService = createSessionResilienceService();
    const program = resilienceService.createSessionResilient(params);
    return runEffect(program, "createSessionResilient");
  }, [runEffect]);
  
  // Utility functions
  const clearError = useCallback(() => {
    setError(null);
  }, []);
  
  const getResilienceHealth = useCallback(async () => {
    // Simplified health check
    const healthData = {
      status: "healthy",
      timestamp: Date.now(),
      services: {
        sessionService: "operational",
        resilienceService: "operational"
      }
    };
    
    return Promise.resolve(healthData);
  }, []);
  
  return {
    // Core operations
    createSession,
    getSession,
    updateSessionStatus,
    deleteSession,
    querySessionsAdvanced,
    
    // Resilient operations
    createSessionResilient,
    
    // State
    isLoading,
    error,
    
    // Utilities
    clearError,
    getResilienceHealth,
  };
}

// Effect Runtime Provider Component
export interface EffectRuntimeProviderProps {
  children: React.ReactNode;
  runtime: Runtime.Runtime<never>;
}

export function EffectRuntimeProvider({ children, runtime }: EffectRuntimeProviderProps) {
  return React.createElement(
    EffectRuntimeContext.Provider,
    { value: { runtime } },
    children
  );
}

// Hook for accessing the Effect runtime directly
export function useEffectRuntime() {
  const context = useContext(EffectRuntimeContext);
  
  if (!context) {
    throw new Error("useEffectRuntime must be used within EffectRuntimeProvider");
  }
  
  return context.runtime;
}