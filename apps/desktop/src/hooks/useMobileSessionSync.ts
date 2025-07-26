import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useQuery, useMutation, useConvex } from 'convex/react';
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
  
  // Store mapping between Claude Code UUIDs and mobile session IDs
  const [sessionIdMapping, setSessionIdMapping] = useState<Map<string, string>>(new Map());
  
  const [isProcessingAnyMobileSession, setIsProcessingAnyMobileSession] = useState(false);
  const [lastGlobalProcessTime, setLastGlobalProcessTime] = useState(0);
  const GLOBAL_PROCESS_DELAY = 3000;
  
  const processingTimeoutRef = useRef<NodeJS.Timeout>();
  const isProcessingRef = useRef(false);
  const lastProcessedTimeRef = useRef<Record<string, number>>({});
  const PROCESSING_COOLDOWN = 5000;

  const pendingMobileSessions = useQuery(api.claude.getPendingMobileSessions) || [];
  const updateSessionStatus = useMutation(api.claude.updateSessionStatus);
  const convex = useConvex();
  const { openChatPane } = usePaneStore();

  // Debug logging to check if hook is receiving Convex updates
  useEffect(() => {
    console.log(
      '🛰️ [MOBILE-SYNC] pendingMobileSessions update:',
      pendingMobileSessions ? pendingMobileSessions.length : 'undefined',
      'isAppInitialized:', isAppInitialized
    );
  }, [pendingMobileSessions, isAppInitialized]);
  
  // Debug logging for isAppInitialized changes
  useEffect(() => {
    console.log('🔧 [MOBILE-SYNC] isAppInitialized changed to:', isAppInitialized);
  }, [isAppInitialized]);

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
    console.log('🏁 [MOBILE-SYNC] createSessionFromMobile called with:', {
      sessionId: mobileSession.sessionId,
      projectPath: mobileSession.projectPath,
      title: mobileSession.title
    });
    
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
        
        // Use Claude Code UUID for local state (needed for streaming)
        const newSession: Session = {
          id: localSessionId, // Use Claude Code UUID for streaming
          projectPath: mobileSession.projectPath,
          messages: [],
          inputMessage: "",
          isLoading: false,
        };
        
        // Store mapping between Claude Code UUID and mobile session ID for persistence
        setSessionIdMapping(prev => {
          const newMapping = new Map(prev);
          newMapping.set(localSessionId, mobileSession.sessionId);
          console.log('🗺️ [MOBILE-SYNC] Stored session mapping:', localSessionId, '→', mobileSession.sessionId);
          return newMapping;
        });

        setSessions(prev => {
          const updated = [...prev, newSession];
          console.log('📋 [MOBILE-SYNC] Added new session to local state. Total sessions:', updated.length);
          return updated;
        });
        
        console.log('🖼️ [MOBILE-SYNC] Opening chat pane for session:', localSessionId);
        openChatPane(localSessionId, mobileSession.projectPath);
        console.log('✅ [MOBILE-SYNC] Chat pane opened successfully');
        
        console.log('🔄 [MOBILE-SYNC] Updating mobile session status to active...');
        try {
          await updateSessionStatus({
            sessionId: mobileSession.sessionId,
            status: "active"
          });
          console.log('✅ [MOBILE-SYNC] Successfully updated mobile session to active status');
        } catch (statusError) {
          console.error('❌ [MOBILE-SYNC] Failed to update session status:', statusError);
          throw statusError;
        }
        
        console.log('💬 [MOBILE-SYNC] Triggering Claude Code to respond to existing message...');
        
        // Get existing messages from mobile session to find the initial message
        try {
          // Use proper Convex client to query messages
          const mobileMessages = await convex.query(api.claude.getSessionMessages, {
            sessionId: mobileSession.sessionId,
            limit: 10
          });
          
          console.log('📋 [MOBILE-SYNC] Found messages in mobile session:', mobileMessages.length);
          
          // Find the last user message to trigger Claude Code response
          const lastUserMessage = mobileMessages?.reverse().find((msg: any) => msg.messageType === 'user');
          
          if (lastUserMessage) {
            console.log('🚀 [MOBILE-SYNC] Sending existing message to Claude Code to trigger response');
            
            // Trigger Claude Code response WITHOUT creating a new user message
            const result = await invoke<CommandResult<void>>("trigger_claude_response", {
              sessionId: localSessionId, // Send to Claude Code UUID
              message: lastUserMessage.content,
            });
            
            if (result.success) {
              console.log('✅ [MOBILE-SYNC] Successfully triggered Claude Code response');
            } else {
              console.error('❌ [MOBILE-SYNC] Failed to trigger Claude Code:', result.error);
            }
          } else {
            console.log('⚠️ [MOBILE-SYNC] No user message found to trigger Claude Code response');
          }
        } catch (messageError) {
          console.error('❌ [MOBILE-SYNC] Failed to get mobile session messages:', messageError);
        }

        // Session is now active, no need to mark as processed

        setProcessedMobileSessions(prev => new Set(prev).add(mobileSession.sessionId));
        console.log('✅ [MOBILE-SYNC] Successfully created and synced local session from mobile request');
      } else {
        console.error('❌ [MOBILE-SYNC] Failed to create local Claude Code session:', result.error);
        
        if (result.error?.includes('Claude Code not initialized') || result.error?.includes('Manager not initialized')) {
          console.log('🚫 [MOBILE-SYNC] Claude Code not ready, updating session to error status');
          try {
            await updateSessionStatus({
              sessionId: mobileSession.sessionId,
              status: "error"
            });
            console.log('✅ [MOBILE-SYNC] Updated failed session to error status');
          } catch (statusError) {
            console.error('❌ [MOBILE-SYNC] Failed to update session status:', statusError);
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
  }, [sessions, setSessions, openChatPane, updateSessionStatus, convex]);

  useEffect(() => {
    console.log('🎯 [MOBILE-SYNC] Main processing effect triggered:', {
      isAppInitialized,
      pendingSessionsLength: pendingMobileSessions.length,
      willProcess: isAppInitialized && pendingMobileSessions.length > 0,
      processingSessions: Array.from(processingSessions),
      processedSessions: Array.from(processedMobileSessions),
      isProcessingAnyMobileSession,
      lastGlobalProcessTime: new Date(lastGlobalProcessTime).toISOString()
    });
    
    if (!isAppInitialized) {
      console.log('❌ [MOBILE-SYNC] App not initialized yet, skipping processing');
      return;
    }
    
    if (pendingMobileSessions.length === 0) {
      console.log('❌ [MOBILE-SYNC] No pending sessions to process');
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
      console.log('⏰ [MOBILE-SYNC] Debounce timer fired, checking if we should process...');
      
      if (pendingMobileSessions.length === 0) {
        console.log('🔍 [MOBILE-SYNC] No pending mobile sessions, skipping processing');
        return;
      }

      console.log('🚀 [MOBILE-SYNC] Starting debounced processing...');
      console.log('🔒 [MOBILE-SYNC] Setting isProcessingRef to true');
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
          console.log('🔍 [MOBILE-SYNC] Evaluating mobile session:', {
            sessionId: mobileSession.sessionId,
            status: mobileSession.status,
            createdBy: mobileSession.createdBy,
            title: mobileSession.title,
            projectPath: mobileSession.projectPath
          });
          
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
            
            console.log('📞 [MOBILE-SYNC] Calling createSessionFromMobile...');
            await createSessionFromMobile(mobileSession);
            console.log('✅ [MOBILE-SYNC] createSessionFromMobile completed');
            
            setTimeout(() => {
              console.log('🔓 [MOBILE-SYNC] Setting isProcessingAnyMobileSession to false after cooldown');
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
  }, [pendingMobileSessions, isAppInitialized, processingSessions, processedMobileSessions, 
      sessions, createSessionFromMobile, setIsProcessingAnyMobileSession, setLastGlobalProcessTime]);

  return {
    processingSessions,
    processedMobileSessions,
    isProcessingAnyMobileSession,
    sessionIdMapping, // Expose mapping for message persistence
  };
};