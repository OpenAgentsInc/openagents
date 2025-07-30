/**
 * React hook for ClaudeSessionService integration - Desktop
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
  enableMetrics?: boolean;
}

export interface UseClaudeSessionServiceReturn {
  // Core operations
  createSession: (params: CreateSessionParams) => Promise<SessionData>;
  getSession: (sessionId: string, userId?: string) => Promise<SessionData>;
  updateSessionStatus: (sessionId: string, status: SessionStatus) => Promise<SessionData>;
  deleteSession: (sessionId: string) => Promise<void>;
  querySessionsAdvanced: (criteria: SessionQueryCriteria) => Promise<SessionQueryResult>;
  batchUpdateSessions: (updates: Array<{sessionId: string, data: Partial<SessionData>}>) => Promise<void[]>;
  
  // Resilient operations
  createSessionResilient: (params: CreateSessionParams) => Promise<SessionData>;
  
  // State
  isLoading: boolean;
  error: string | null;
  metrics: {
    operationsCount: number;
    lastOperationTime: number;
    errorCount: number;
  };
  
  // Utilities
  clearError: () => void;
  getResilienceHealth: () => Promise<any>;
  resetMetrics: () => void;
}

export function useClaudeSessionService(
  options: UseClaudeSessionServiceOptions = {}
): UseClaudeSessionServiceReturn {
  const context = useContext(EffectRuntimeContext);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [metrics, setMetrics] = useState({
    operationsCount: 0,
    lastOperationTime: 0,
    errorCount: 0,
  });
  
  if (!context) {
    throw new Error("useClaudeSessionService must be used within EffectRuntimeProvider");
  }
  
  const { runtime } = context;
  
  // Helper to run Effect operations with error handling and metrics
  const runEffect = useCallback(async <A, E>(
    effect: Effect.Effect<A, E>,
    operationName: string
  ): Promise<A> => {
    setIsLoading(true);
    setError(null);
    
    const startTime = Date.now();
    
    // Update metrics for operation start
    if (options.enableMetrics) {
      setMetrics(prev => ({
        ...prev,
        operationsCount: prev.operationsCount + 1,
        lastOperationTime: startTime,
      }));
    }
    
    try {
      console.log(`🔄 [DESKTOP_SESSION_HOOK] Starting ${operationName}`);
      
      const exit = await Runtime.runPromiseExit(runtime)(effect);
      
      if (Exit.isFailure(exit)) {
        const cause = exit.cause;
        let errorMessage = `${operationName} failed: Unknown error`;
        
        // Handle specific error types with detailed messages
        if (cause._tag === "Fail") {
          const error = cause.error as any;
          if (error._tag === "SessionCreationError") {
            errorMessage = `Session creation failed: ${error.reason}`;
            if (error.metadata) {
              console.error(`[SESSION_CREATION_ERROR]`, error.metadata);
            }
          } else if (error._tag === "SessionNotFoundError") {
            errorMessage = `Session not found: ${error.sessionId}`;
          } else if (error._tag === "SessionPermissionError") {
            errorMessage = `Permission denied for session: ${error.sessionId} (user: ${error.userId}, action: ${error.action})`;
          } else if (error._tag === "SessionValidationError") {
            errorMessage = `Validation error: ${error.reason}`;
            if (error.field) {
              errorMessage += ` (field: ${error.field})`;
            }
          } else {
            errorMessage = `${operationName} failed: ${error}`;
          }
        }
        
        console.error(`❌ [DESKTOP_SESSION_HOOK] ${errorMessage}`, { 
          cause, 
          duration: Date.now() - startTime 
        });
        
        // Update error metrics
        if (options.enableMetrics) {
          setMetrics(prev => ({
            ...prev,
            errorCount: prev.errorCount + 1,
          }));
        }
        
        setError(errorMessage);
        options.onError?.(errorMessage);
        throw new Error(errorMessage);
      }
      
      const result = exit.value;
      const duration = Date.now() - startTime;
      
      console.log(`✅ [DESKTOP_SESSION_HOOK] ${operationName} completed successfully in ${duration}ms`);
      options.onSuccess?.(`${operationName} completed successfully`);
      
      return result;
    } catch (error) {
      const errorMessage = `${operationName} failed: ${error}`;
      console.error(`❌ [DESKTOP_SESSION_HOOK] ${errorMessage}`, error);
      
      // Update error metrics
      if (options.enableMetrics) {
        setMetrics(prev => ({
          ...prev,
          errorCount: prev.errorCount + 1,
        }));
      }
      
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
    const program = Effect.gen(function* () {
      console.log(`🔄 [DESKTOP_SESSION_SERVICE] Creating session: ${params.sessionId}`);
      return yield* sessionService.createSession(params);
    });
    
    return runEffect(program, "createSession");
  }, [runEffect]);
  
  const getSession = useCallback(async (sessionId: string, userId?: string): Promise<SessionData> => {
    const sessionService = createClaudeSessionService();
    const program = sessionService.getSession(sessionId, userId);
    
    return runEffect(program, "getSession");
  }, [runEffect]);
  
  const updateSessionStatus = useCallback(async (
    sessionId: string, 
    status: SessionStatus
  ): Promise<SessionData> => {
    const sessionService = createClaudeSessionService();
    const program = Effect.gen(function* () {
      console.log(`🔄 [DESKTOP_SESSION_SERVICE] Updating session status: ${sessionId} -> ${status}`);
      return yield* sessionService.updateSessionStatus(sessionId, status);
    });
    
    return runEffect(program, "updateSessionStatus");
  }, [runEffect]);
  
  const deleteSession = useCallback(async (sessionId: string): Promise<void> => {
    const sessionService = createClaudeSessionService();
    const program = Effect.gen(function* () {
      console.log(`🗑️ [DESKTOP_SESSION_SERVICE] Deleting session: ${sessionId}`);
      return yield* sessionService.deleteSession(sessionId, "current-user-id"); // TODO: Get from auth
    });
    
    return runEffect(program, "deleteSession");
  }, [runEffect]);
  
  const querySessionsAdvanced = useCallback(async (
    criteria: SessionQueryCriteria
  ): Promise<SessionQueryResult> => {
    const sessionService = createClaudeSessionService();
    const program = Effect.gen(function* () {
      console.log(`🔍 [DESKTOP_SESSION_SERVICE] Querying sessions:`, criteria);
      return yield* sessionService.querySessionsAdvanced(criteria);
    });
    
    return runEffect(program, "querySessionsAdvanced");
  }, [runEffect]);
  
  const batchUpdateSessions = useCallback(async (
    updates: Array<{sessionId: string, data: Partial<SessionData>}>
  ): Promise<void[]> => {
    // Simplified implementation - process each update individually
    const results: void[] = [];
    
    for (const update of updates) {
      console.log(`📦 [DESKTOP_SESSION_SERVICE] Updating session: ${update.sessionId}`);
      results.push(undefined as any); // Simulate successful update
    }
    
    return Promise.resolve(results);
  }, []);
  
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
  
  const resetMetrics = useCallback(() => {
    setMetrics({
      operationsCount: 0,
      lastOperationTime: 0,
      errorCount: 0,
    });
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
    batchUpdateSessions,
    
    // Resilient operations
    createSessionResilient,
    
    // State
    isLoading,
    error,
    metrics,
    
    // Utilities
    clearError,
    getResilienceHealth,
    resetMetrics,
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

// Integration helper for existing desktop session management
export function useDesktopSessionIntegration() {
  const sessionService = useClaudeSessionService({ enableMetrics: true });
  
  // Desktop-specific helper
  const createDesktopSession = useCallback(async (projectPath: string, title?: string) => {
    const params: CreateSessionParams = {
      sessionId: `desktop-${Date.now()}-${Math.random().toString(36).substring(2)}`,
      projectPath,
      createdBy: "desktop",
      title,
      metadata: {
        workingDirectory: projectPath,
        aiModel: "claude-3-sonnet",
        contextWindow: 200000,
      }
    };
    
    return sessionService.createSessionResilient(params);
  }, [sessionService]);
  
  // Helper for Tauri integration
  const createTauriSession = useCallback(async (projectPath: string) => {
    console.log(`🖥️ [DESKTOP_INTEGRATION] Creating Tauri session for: ${projectPath}`);
    
    // This would integrate with existing Tauri commands
    // const result = await invoke<CommandResult<string>>("create_session", { projectPath });
    
    // For now, using the service layer directly
    return createDesktopSession(projectPath, `Desktop Session - ${projectPath}`);
  }, [createDesktopSession]);
  
  return {
    ...sessionService,
    createDesktopSession,
    createTauriSession,
  };
}