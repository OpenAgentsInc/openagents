import { useState, useEffect, useCallback, useRef } from "react";
import { Effect, Runtime, Exit } from "effect";
import { useMutation, useQuery } from "@rjdellecese/confect/react";
import { 
  MobileSyncService, 
  MobileSyncServiceLive, 
  type MobileSession,
  MobileSyncError,
  SessionValidationError,
  ProcessingTimeoutError
} from "../services/MobileSyncService";

interface UseConfectMobileSyncOptions {
  enabled?: boolean;
  pollingInterval?: number;
}

interface UseConfectMobileSyncReturn {
  processMobileSession: (session: MobileSession) => Promise<void>;
  pendingSessions: MobileSession[];
  isProcessing: boolean;
  error: string | null;
  retryCount: number;
}

export function useConfectMobileSync(
  confectApi: any,
  options: UseConfectMobileSyncOptions = {}
): UseConfectMobileSyncReturn {
  const { enabled = true, pollingInterval = 5000 } = options;
  
  const [isProcessing, setIsProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryCount, setRetryCount] = useState(0);
  
  const runtimeRef = useRef<Runtime.Runtime<MobileSyncService>>();
  const processedSessionsRef = useRef<Set<string>>(new Set());

  // Initialize Effect runtime
  useEffect(() => {
    runtimeRef.current = Runtime.defaultRuntime.pipe(
      Runtime.provide(MobileSyncServiceLive.Default)
    );
  }, []);

  // Set up Confect mutations and queries
  const createSessionMutation = useMutation({
    mutation: confectApi.createClaudeSession,
    args: confectApi.CreateClaudeSessionArgs,
    returns: confectApi.CreateClaudeSessionResult,
  });

  const updateStatusMutation = useMutation({
    mutation: confectApi.updateSessionStatus,
    args: confectApi.UpdateSessionStatusArgs,
    returns: confectApi.UpdateSessionStatusResult,
  });

  const pendingSessionsQuery = useQuery({
    query: confectApi.getPendingMobileSessions,
    args: confectApi.GetPendingMobileSessionsArgs,
    returns: confectApi.GetPendingMobileSessionsResult,
  });

  const pendingSessions = pendingSessionsQuery({});

  // Process mobile session with Effect patterns
  const processMobileSession = useCallback(async (session: MobileSession) => {
    if (!runtimeRef.current || !enabled) return;
    
    // Skip if already processed
    if (processedSessionsRef.current.has(session.sessionId)) {
      console.log(`🚫 [CONFECT-SYNC] Session ${session.sessionId} already processed`);
      return;
    }

    setIsProcessing(true);
    setError(null);

    const program = Effect.gen(function* () {
      const syncService = yield* MobileSyncService;
      
      console.log(`🔄 [CONFECT-SYNC] Processing mobile session: ${session.sessionId}`);
      
      // Process the session using Effect patterns
      const result = yield* syncService.processMobileSession(session);
      
      // Mark as processed
      processedSessionsRef.current.add(session.sessionId);
      
      // Update status to processed
      yield* syncService.updateSessionStatus(session.sessionId, "processed");
      
      console.log(`✅ [CONFECT-SYNC] Successfully processed session: ${session.sessionId}`);
      
      return result;
    });

    try {
      const exit = await Runtime.runPromiseExit(runtimeRef.current)(program);
      
      if (Exit.isFailure(exit)) {
        const cause = exit.cause;
        let errorMessage = "Unknown error";
        
        // Handle specific error types
        if (cause._tag === "Fail") {
          const error = cause.error;
          if (error instanceof MobileSyncError) {
            errorMessage = `Mobile sync failed: ${error.message}`;
          } else if (error instanceof SessionValidationError) {
            errorMessage = `Session validation failed: ${error.reason}`;
          } else if (error instanceof ProcessingTimeoutError) {
            errorMessage = `Processing timeout after ${error.timeoutMs}ms`;
          } else {
            errorMessage = String(error);
          }
        }
        
        console.error(`❌ [CONFECT-SYNC] Failed to process session ${session.sessionId}:`, errorMessage);
        setError(errorMessage);
        setRetryCount(prev => prev + 1);
        
        // Remove from processed set to allow retry
        processedSessionsRef.current.delete(session.sessionId);
      } else {
        setRetryCount(0);
      }
    } catch (error) {
      console.error(`❌ [CONFECT-SYNC] Unexpected error processing session:`, error);
      setError(String(error));
      setRetryCount(prev => prev + 1);
      processedSessionsRef.current.delete(session.sessionId);
    } finally {
      setIsProcessing(false);
    }
  }, [enabled, createSessionMutation, updateStatusMutation]);

  // Auto-process pending sessions
  useEffect(() => {
    if (!enabled || !pendingSessions || pendingSessions.length === 0) return;

    const processAllSessions = async () => {
      for (const session of pendingSessions) {
        if (!processedSessionsRef.current.has(session.sessionId)) {
          await processMobileSession(session);
          // Add small delay between processing sessions
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      }
    };

    processAllSessions();
  }, [pendingSessions, enabled, processMobileSession]);

  return {
    processMobileSession,
    pendingSessions: pendingSessions || [],
    isProcessing,
    error,
    retryCount,
  };
}