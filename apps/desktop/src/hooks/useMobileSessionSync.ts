import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useQuery, useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';
import { usePaneStore } from '@/stores/pane';

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

export const useMobileSessionSync = (
  sessions: Session[],
  setSessions: (sessions: Session[] | ((prev: Session[]) => Session[])) => void,
  isAppInitialized: boolean
) => {
  console.log('🚀 [MOBILE-SYNC] useMobileSessionSync hook mounted');
  
  const [processingSessions, setProcessingSessions] = useState<Set<string>>(new Set());
  const [processedMobileSessions, setProcessedMobileSessions] = useState<Set<string>>(new Set());
  const [mobileSessionsToInitialize, setMobileSessionsToInitialize] = useState<{mobileSessionId: string; localSessionId: string}[]>([]);
  
  const [isProcessingAnyMobileSession, setIsProcessingAnyMobileSession] = useState(false);
  const [lastGlobalProcessTime, setLastGlobalProcessTime] = useState(0);
  const GLOBAL_PROCESS_DELAY = 3000;
  
  const processingTimeoutRef = useRef<NodeJS.Timeout>();
  const isProcessingRef = useRef(false);
  const lastProcessedTimeRef = useRef<Record<string, number>>({});
  const PROCESSING_COOLDOWN = 5000;

  const pendingMobileSessions = useQuery(api.claude.getPendingMobileSessions) || [];
  const createConvexSession = useMutation(api.claude.createClaudeSession);
  const markMobileSessionProcessed = useMutation(api.claude.markMobileSessionProcessed);
  const { openChatPane } = usePaneStore();

  // Debug logging to check if hook is receiving Convex updates
  useEffect(() => {
    console.log(
      '🛰️ [MOBILE-SYNC] pendingMobileSessions update:',
      pendingMobileSessions ? pendingMobileSessions.length : 'undefined'
    );
  }, [pendingMobileSessions]);

  useEffect(() => {
    console.log('📊 [MOBILE-SYNC] State check:', {
      pendingSessionsLength: pendingMobileSessions.length,
      isAppInitialized,
      shouldProcess: pendingMobileSessions.length > 0 && isAppInitialized
    });
    
    if (pendingMobileSessions.length > 0 && isAppInitialized) {
      console.log('🔍 [MOBILE-SYNC] Detected pending mobile sessions:', pendingMobileSessions.length);
      console.log('🔍 [MOBILE-SYNC] Mobile sessions data:', JSON.stringify(pendingMobileSessions, null, 2));
    }
  }, [pendingMobileSessions, isAppInitialized]);

  const createSessionFromMobile = useCallback(async (mobileSession: MobileSession) => {
    const sessionId = mobileSession.sessionId;
    const now = Date.now();
    const lastProcessed = lastProcessedTimeRef.current[sessionId] || 0;
    
    if (now - lastProcessed < PROCESSING_COOLDOWN) {
      const timeSinceLastProcessed = (now - lastProcessed) / 1000;
      console.log(`🚫 [CIRCUIT-BREAKER] Skipping ${sessionId} - processed ${timeSinceLastProcessed.toFixed(1)}s ago (cooldown: ${PROCESSING_COOLDOWN/1000}s)`);
      return;
    }
    
    lastProcessedTimeRef.current[sessionId] = now;
    
    console.log('🚀 [MOBILE-SYNC] Starting session creation from mobile request:', {
      sessionId: mobileSession.sessionId,
      projectPath: mobileSession.projectPath,
      title: mobileSession.title,
      lastProcessedAgo: lastProcessed ? `${((now - lastProcessed) / 1000).toFixed(1)}s ago` : 'never'
    });
    
    setProcessingSessions(prev => new Set(prev).add(mobileSession.sessionId));
    console.log('📝 [MOBILE-SYNC] Marked session as processing:', mobileSession.sessionId);
    
    try {
      const existingLocalSession = sessions.find(s => s.id === mobileSession.sessionId);
      if (existingLocalSession) {
        console.log('⚠️ [MOBILE-SYNC] Local session already exists for:', mobileSession.sessionId);
        return;
      }

      console.log('🔧 [MOBILE-SYNC] Invoking Tauri create_session command...');
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath: mobileSession.projectPath,
      });

      console.log('📋 [MOBILE-SYNC] Tauri create_session result:', {
        success: result.success,
        hasData: !!result.data,
        error: result.error
      });

      if (result.success && result.data) {
        const localSessionId = result.data;
        console.log('✅ [MOBILE-SYNC] Claude Code session created with ID:', localSessionId);
        
        const newSession: Session = {
          id: localSessionId,
          projectPath: mobileSession.projectPath,
          messages: [],
          inputMessage: "",
          isLoading: false,
        };

        setSessions(prev => {
          const updated = [...prev, newSession];
          console.log('📋 [MOBILE-SYNC] Added new session to local state. Total sessions:', updated.length);
          return updated;
        });
        
        console.log('🖼️ [MOBILE-SYNC] Opening chat pane for session:', localSessionId);
        openChatPane(localSessionId, mobileSession.projectPath);
        console.log('✅ [MOBILE-SYNC] Chat pane opened successfully');
        
        console.log('🔄 [MOBILE-SYNC] Syncing session to Convex...');
        try {
          const convexResult = await createConvexSession({
            sessionId: localSessionId,
            projectPath: mobileSession.projectPath,
            createdBy: "desktop",
            title: mobileSession.title || `Mobile Session - ${mobileSession.projectPath}`,
            metadata: {
              workingDirectory: mobileSession.projectPath,
              originalMobileSessionId: mobileSession.sessionId,
            },
          });
          console.log('✅ [MOBILE-SYNC] Successfully synced session to Convex:', convexResult);
        } catch (convexError) {
          console.error('❌ [MOBILE-SYNC] Failed to sync session to Convex:', convexError);
          throw convexError;
        }
        
        console.log('📥 [MOBILE-SYNC] Queueing mobile session for initial message retrieval');
        setMobileSessionsToInitialize(prev => {
          const newQueue = [...prev, {
            mobileSessionId: mobileSession.sessionId,
            localSessionId: localSessionId
          }];
          console.log('📋 [MOBILE-SYNC] Updated initialization queue:', newQueue);
          return newQueue;
        });

        console.log('🏁 [MOBILE-SYNC] Marking mobile session as processed in database...');
        try {
          const markResult = await markMobileSessionProcessed({
            mobileSessionId: mobileSession.sessionId,
          });
          console.log('✅ [MOBILE-SYNC] Successfully marked mobile session as processed:', markResult);
        } catch (markError) {
          console.error('❌ [MOBILE-SYNC] Failed to mark mobile session as processed:', markError);
          throw markError;
        }

        setProcessedMobileSessions(prev => new Set(prev).add(mobileSession.sessionId));
        console.log('✅ [MOBILE-SYNC] Successfully created and synced local session from mobile request');
      } else {
        console.error('❌ [MOBILE-SYNC] Failed to create local Claude Code session:', result.error);
        
        if (result.error?.includes('Claude Code not initialized') || result.error?.includes('Manager not initialized')) {
          console.log('🚫 [MOBILE-SYNC] Claude Code not ready, marking mobile session as failed to prevent retries');
          try {
            await markMobileSessionProcessed({
              mobileSessionId: mobileSession.sessionId,
            });
            console.log('✅ [MOBILE-SYNC] Marked failed mobile session as processed to prevent retries');
          } catch (markError) {
            console.error('❌ [MOBILE-SYNC] Failed to mark mobile session as processed:', markError);
          }
        }
      }
    } catch (error) {
      console.error('💥 [MOBILE-SYNC] Error creating session from mobile:', error);
    } finally {
      setProcessingSessions(prev => {
        const newSet = new Set(prev);
        newSet.delete(mobileSession.sessionId);
        return newSet;
      });
      console.log('🏁 [MOBILE-SYNC] Finished processing session:', mobileSession.sessionId);
    }
  }, [sessions, setSessions, openChatPane, createConvexSession, markMobileSessionProcessed]);

  useEffect(() => {
    console.log('🎯 [MOBILE-SYNC] Main processing effect triggered:', {
      isAppInitialized,
      pendingSessionsLength: pendingMobileSessions.length,
      willProcess: isAppInitialized && pendingMobileSessions.length > 0
    });
    
    if (!isAppInitialized || pendingMobileSessions.length === 0) {
      return;
    }

    const timeSinceLastProcess = Date.now() - lastGlobalProcessTime;
    if (timeSinceLastProcess < GLOBAL_PROCESS_DELAY) {
      console.log(`⏳ [MOBILE-SYNC] Waiting for global cooldown. ${(GLOBAL_PROCESS_DELAY - timeSinceLastProcess) / 1000}s remaining`);
      return;
    }

    if (isProcessingAnyMobileSession) {
      console.log('🚫 [MOBILE-SYNC] Already processing a mobile session, skipping');
      return;
    }
    
    const timestamp = new Date().toISOString();
    const pendingIds = pendingMobileSessions.map((s: any) => s.sessionId);
    const processedIds = Array.from(processedMobileSessions);
    const processingIds = Array.from(processingSessions);
    const overlaps = pendingIds.filter((id: string) => processedIds.includes(id));
    
    if (pendingIds.length > 0) {
      console.log(`🔄 [${timestamp}] useEffect TRIGGERED`);
      console.log(`📥 Pending sessions: [${pendingIds.join(', ')}]`);
      console.log(`✅ Processed sessions: [${processedIds.join(', ')}]`);
      console.log(`⏳ Currently processing: [${processingIds.join(', ')}]`);
      console.log(`🔄 Overlap detection: [${overlaps.join(', ')}]${overlaps.length > 0 ? ' ← PROBLEM!' : ''}`);
      console.log(`🔒 Processing lock: ${isProcessingRef.current}`);
    }

    if (processingTimeoutRef.current) {
      clearTimeout(processingTimeoutRef.current);
    }

    if (isProcessingRef.current) {
      console.log('⏸️ [MOBILE-SYNC] Already processing, skipping this execution');
      return;
    }

    processingTimeoutRef.current = setTimeout(() => {
      if (pendingMobileSessions.length === 0) {
        console.log('🔍 [MOBILE-SYNC] No pending mobile sessions, skipping processing');
        return;
      }

      console.log('🚀 [MOBILE-SYNC] Starting debounced processing...');
      isProcessingRef.current = true;

      const processMobileSessions = async () => {
        try {
          console.log('🔄 [MOBILE-SYNC] Processing', pendingMobileSessions.length, 'mobile sessions');
          console.log('🔄 [MOBILE-SYNC] Currently processing sessions:', Array.from(processingSessions));
          console.log('🔄 [MOBILE-SYNC] Already processed sessions:', Array.from(processedMobileSessions));
          console.log('🔄 [MOBILE-SYNC] Current local sessions:', sessions.map(s => ({ id: s.id, path: s.projectPath })));
          
          const sessionToProcess = pendingMobileSessions[0];
          if (!sessionToProcess) return;
          
          const mobileSession = sessionToProcess;
          console.log('🔍 [MOBILE-SYNC] Evaluating mobile session:', mobileSession.sessionId);
          
          const isDesktopSessionId = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(mobileSession.sessionId);
          if (isDesktopSessionId) {
            console.log('⏭️ [MOBILE-SYNC] Skipping desktop-created session (UUID format):', mobileSession.sessionId);
            return;
          }
          
          if (processingSessions.has(mobileSession.sessionId)) {
            console.log('⏭️ [MOBILE-SYNC] Skipping - already processing:', mobileSession.sessionId);
            return;
          }
          
          const alreadyProcessed = processedMobileSessions.has(mobileSession.sessionId);
          
          console.log('🔍 [MOBILE-SYNC] Already processed check for', mobileSession.sessionId, '- alreadyProcessed:', alreadyProcessed);
          
          if (!alreadyProcessed) {
            console.log('🎯 [MOBILE-SYNC] Creating session for:', mobileSession.sessionId);
            
            setIsProcessingAnyMobileSession(true);
            setLastGlobalProcessTime(Date.now());
            
            await createSessionFromMobile(mobileSession);
            
            setTimeout(() => {
              setIsProcessingAnyMobileSession(false);
            }, 1000);
          } else {
            console.log('⏭️ [MOBILE-SYNC] Skipping - already successfully processed:', mobileSession.sessionId);
          }
          console.log('✅ [MOBILE-SYNC] Finished processing all mobile sessions');
        } catch (error) {
          console.error('💥 [MOBILE-SYNC] Error processing mobile sessions:', error);
        } finally {
          isProcessingRef.current = false;
        }
      };

      processMobileSessions();
    }, 200);

    return () => {
      if (processingTimeoutRef.current) {
        clearTimeout(processingTimeoutRef.current);
      }
    };
  }, [pendingMobileSessions, isAppInitialized, lastGlobalProcessTime, isProcessingAnyMobileSession, 
      processingSessions, processedMobileSessions, sessions, createSessionFromMobile]);

  const handleInitialMessageSent = useCallback((mobileSessionId: string) => {
    console.log('✉️ [MOBILE-SYNC] Initial message sent for mobile session:', mobileSessionId);
    setMobileSessionsToInitialize(prev => 
      prev.filter(s => s.mobileSessionId !== mobileSessionId)
    );
  }, []);

  return {
    processingSessions,
    processedMobileSessions,
    mobileSessionsToInitialize,
    isProcessingAnyMobileSession,
    handleInitialMessageSent,
  };
};