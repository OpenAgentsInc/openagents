import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useQuery, useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';
import { usePaneStore } from '@/stores/pane';
import { Effect, Schedule, Duration, Exit, Runtime } from 'effect';

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface Session {
  id: string;
  projectPath: string;
  messages: any[];
  inputMessage: string;
  isLoading: boolean;
  isInitializing?: boolean;
}

interface MobileSession {
  sessionId: string;
  projectPath: string;
  title?: string;
}

export const useMobileSessionSyncConfect = (
  sessions: Session[],
  setSessions: (sessions: Session[] | ((prev: Session[]) => Session[])) => void,
  isAppInitialized: boolean
) => {
  const [isProcessing, setIsProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [processedSessions, setProcessedSessions] = useState<Set<string>>(new Set());
  const [sessionIdMapping, setSessionIdMapping] = useState<Map<string, string>>(new Map());

  const { openChatPane } = usePaneStore();

  // Use standard Convex queries and mutations
  const pendingMobileSessions = useQuery(api.claude.getPendingMobileSessions) || [];
  const updateStatusMutation = useMutation(api.claude.updateSessionStatus);

  // Effect-based session processing
  const processMobileSession = useCallback((session: MobileSession) => {
    return Effect.gen(function* () {
      console.log(`🔄 [CONFECT-SYNC] Processing mobile session: ${session.sessionId}`);

      // Check if session already exists locally
      const existingSession = sessions.find(s => s.id === session.sessionId);
      if (existingSession) {
        console.log(`⚠️ [CONFECT-SYNC] Session already exists: ${session.sessionId}`);
        return Effect.void;
      }

      // Create Tauri session
      const tauriResult = yield* Effect.tryPromise({
        try: () => invoke<CommandResult<string>>("create_session", {
          projectPath: session.projectPath,
        }),
        catch: (error) => new Error(`Tauri session creation failed: ${error}`)
      });

      if (!tauriResult.success || !tauriResult.data) {
        return yield* Effect.fail(new Error(`Tauri session creation failed: ${tauriResult.error}`));
      }

      const localSessionId = tauriResult.data;
      console.log(`✅ [CONFECT-SYNC] Created Tauri session: ${localSessionId}`);

      // Create local session state
      const newSession: Session = {
        id: localSessionId,
        projectPath: session.projectPath,
        messages: [],
        inputMessage: "",
        isLoading: false,
      };

      // Store mapping between Claude Code UUID and mobile session ID for persistence
      setSessionIdMapping(prev => {
        const newMapping = new Map(prev);
        newMapping.set(localSessionId, session.sessionId);
        console.log('🗺️ [CONFECT-SYNC] Stored session mapping:', localSessionId, '→', session.sessionId);
        return newMapping;
      });

      // Update local state
      setSessions(prev => [...prev, newSession]);
      openChatPane(localSessionId, session.projectPath);

      // Update Convex session status to processed
      yield* Effect.tryPromise({
        try: () => updateStatusMutation({
          sessionId: session.sessionId,
          status: "processed" as const
        }),
        catch: (error) => new Error(`Failed to update session status: ${error}`)
      });

      console.log(`✅ [CONFECT-SYNC] Successfully processed session: ${session.sessionId}`);
      
      // Mark as processed locally
      setProcessedSessions(prev => new Set(prev).add(session.sessionId));

      return Effect.void;
    }).pipe(
      Effect.retry(
        Schedule.exponential(Duration.seconds(1)).pipe(
          Schedule.intersect(Schedule.recurs(3))
        )
      ),
      Effect.timeout(Duration.seconds(30)),
      Effect.catchAll((error) => {
        console.error(`❌ [CONFECT-SYNC] Failed to process session ${session.sessionId}:`, error);
        setError(String(error));
        return Effect.void;
      })
    );
  }, [sessions, setSessions, openChatPane, updateStatusMutation]);

  // Process all pending sessions
  const processAllSessions = useCallback(async () => {
    if (!isAppInitialized || !pendingMobileSessions || pendingMobileSessions.length === 0 || isProcessing) {
      return;
    }

    console.log(`🚀 [CONFECT-SYNC] Processing ${pendingMobileSessions.length} pending sessions`);
    setIsProcessing(true);
    setError(null);

    try {
      // Process sessions with Effect concurrency control
      const sessionsToProcess = pendingMobileSessions.filter((session: MobileSession) => !processedSessions.has(session.sessionId));
      
      const processingProgram = Effect.forEach(
        sessionsToProcess,
        (session) => processMobileSession(session),
        { concurrency: 3 } // Process up to 3 sessions concurrently
      );

      const runtime = Runtime.defaultRuntime;
      const exit = await Runtime.runPromiseExit(runtime)(processingProgram);

      if (Exit.isFailure(exit)) {
        console.error(`❌ [CONFECT-SYNC] Batch processing failed:`, exit.cause);
        setError("Failed to process some mobile sessions");
      } else {
        console.log(`✅ [CONFECT-SYNC] Successfully processed all sessions`);
      }
    } catch (error) {
      console.error(`❌ [CONFECT-SYNC] Unexpected error:`, error);
      setError(String(error));
    } finally {
      setIsProcessing(false);
    }
  }, [isAppInitialized, pendingMobileSessions, isProcessing, processedSessions, processMobileSession]);

  // Auto-process sessions when they appear
  useEffect(() => {
    if (pendingMobileSessions && pendingMobileSessions.length > 0 && isAppInitialized) {
      console.log(`🔍 [CONFECT-SYNC] Found ${pendingMobileSessions.length} pending mobile sessions`);
      processAllSessions();
    }
  }, [pendingMobileSessions, isAppInitialized, processAllSessions]);

  return {
    pendingMobileSessions,
    isProcessing,
    error,
    processedCount: processedSessions.size,
    processAllSessions,
    sessionIdMapping, // For compatibility with App.tsx and SessionManager
  };
};