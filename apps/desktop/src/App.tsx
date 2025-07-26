import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PaneManager } from "@/panes/PaneManager";
import { Hotbar } from "@/components/hud/Hotbar";
import { usePaneStore } from "@/stores/pane";
import { TooltipProvider } from "@/components/ui/tooltip";
import { HandTracking, HandPose } from "@/components/hands";
import type { PinchCoordinates, HandLandmarks } from "@/components/hands";
import { SessionStreamManager } from "@/components/SessionStreamManager";
import { ConvexDemo } from "@/components/ConvexDemo";
import { useQuery, useMutation } from "convex/react";
import { api } from "./convex/_generated/api";

interface Message {
  id: string;
  message_type: string;
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface Session {
  id: string;
  projectPath: string;
  messages: Message[];
  inputMessage: string;
  isLoading: boolean;
  isInitializing?: boolean;
}

interface HandDataContext {
  activeHandPose: HandPose;
  pinchMidpoint: PinchCoordinates | null;
  primaryHandLandmarks: HandLandmarks | null;
  trackedHandsCount: number;
}

function App() {
  const [claudeStatus, setClaudeStatus] = useState<string>("Not initialized");
  const [sessions, setSessions] = useState<Session[]>([]);
  const [newProjectPath, setNewProjectPath] = useState("");
  const [isDiscoveryLoading, setIsDiscoveryLoading] = useState(false);
  const [isHandTrackingActive, setIsHandTrackingActive] = useState(false);
  const [handData, setHandData] = useState<HandDataContext | null>(null);
  
  // Startup state management to prevent excessive initial calls
  const [isAppInitialized, setIsAppInitialized] = useState(false);
  const initializationTimeoutRef = useRef<NodeJS.Timeout>();
  
  // Pinch-to-drag state
  const [draggingPaneId, setDraggingPaneId] = useState<string | null>(null);
  const initialPinchPositionRef = useRef<{ x: number; y: number } | null>(null);
  const paneStartPosRef = useRef<{ x: number; y: number } | null>(null);
  
  const { openChatPane, toggleMetadataPane, toggleSettingsPane, toggleStatsPane, organizePanes, panes, bringPaneToFront, updatePanePosition, activePaneId, removePane, updateSessionMessages, getSessionMessages } = usePaneStore();

  // Convex hooks for Claude Code sync
  const pendingMobileSessions = useQuery(api.claude.getPendingMobileSessions) || [];
  const createConvexSession = useMutation(api.claude.createClaudeSession);
  const markMobileSessionProcessed = useMutation(api.claude.markMobileSessionProcessed);
  
  // Debug logging for mobile sessions (only when there are sessions to avoid noise)
  useEffect(() => {
    if (pendingMobileSessions.length > 0 && isAppInitialized) {
      console.log('🔍 [MOBILE-SYNC] Detected pending mobile sessions:', pendingMobileSessions.length);
      console.log('🔍 [MOBILE-SYNC] Mobile sessions data:', JSON.stringify(pendingMobileSessions, null, 2));
    }
  }, [pendingMobileSessions, isAppInitialized]);
  
  // State tracking for sessions currently being processed to prevent duplicates
  const [processingSessions, setProcessingSessions] = useState<Set<string>>(new Set());
  // State tracking for mobile sessions that have been successfully processed
  const [processedMobileSessions, setProcessedMobileSessions] = useState<Set<string>>(new Set());
  
  // Global rate limiting for mobile session processing
  const [isProcessingAnyMobileSession, setIsProcessingAnyMobileSession] = useState(false);
  const [lastGlobalProcessTime, setLastGlobalProcessTime] = useState(0);
  const GLOBAL_PROCESS_DELAY = 3000; // 3 seconds between processing any mobile sessions
  
  // Debounce ref to prevent excessive useEffect firing
  const processingTimeoutRef = useRef<NodeJS.Timeout>();
  const isProcessingRef = useRef(false);
  
  // Circuit breaker to prevent processing same session repeatedly
  const lastProcessedTimeRef = useRef<Record<string, number>>({});
  const PROCESSING_COOLDOWN = 5000; // 5 seconds

  // Clean up desktop-created sessions that were incorrectly synced as mobile
  const cleanupIncorrectlySyncedSessions = useCallback(async (sessions: any[]) => {
    try {
      let cleanedCount = 0;
      
      for (const session of sessions) {
        // Check if this is a desktop-created session ID (UUID format)
        const isDesktopSessionId = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(session.sessionId);
        if (isDesktopSessionId) {
          try {
            console.log(`🗑️ [CLEANUP] Marking incorrectly-synced desktop session as processed: ${session.sessionId}`);
            await markMobileSessionProcessed({
              mobileSessionId: session.sessionId,
            });
            cleanedCount++;
          } catch (error) {
            console.error(`❌ [CLEANUP] Failed to clean up session ${session.sessionId}:`, error);
          }
        }
      }
      
      if (cleanedCount > 0) {
        console.log(`✅ [CLEANUP] Cleaned up ${cleanedCount} incorrectly-synced desktop sessions`);
      } else {
        console.log('✅ [CLEANUP] No incorrectly-synced sessions found');
      }
    } catch (error) {
      console.error('❌ [CLEANUP] Failed to clean up incorrectly-synced sessions:', error);
    }
  }, [markMobileSessionProcessed]);

  // Get project directory (git root or current directory) on mount
  useEffect(() => {
    invoke("get_project_directory").then((result: any) => {
      if (result.success && result.data) {
        setNewProjectPath(result.data);
      }
    }).catch(console.error);
    
    // Initialize Claude Code discovery and then enable mobile session processing
    const initializeApp = async () => {
      try {
        console.log('🔍 [APP] Discovering Claude Code...');
        await discoverClaude();
        console.log('✅ [APP] Claude Code discovered, enabling mobile session processing');
        
        // Wait a bit for initial data to load
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        setIsAppInitialized(true);
      } catch (error) {
        console.error('❌ [APP] Claude Code discovery failed:', error);
        // Still enable the app but mobile sessions will be skipped
        setIsAppInitialized(true);
      }
    };
    
    // Initialize after brief delay to prevent startup spam
    initializationTimeoutRef.current = setTimeout(initializeApp, 1000);
    
    return () => {
      if (initializationTimeoutRef.current) {
        clearTimeout(initializationTimeoutRef.current);
      }
    };
  }, []);

  // Clean up incorrectly-synced sessions when app is initialized
  useEffect(() => {
    if (isAppInitialized && pendingMobileSessions.length > 0) {
      // Check if any sessions need cleanup
      const hasIncorrectSessions = pendingMobileSessions.some((session: any) => 
        /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(session.sessionId)
      );
      
      if (hasIncorrectSessions) {
        console.log('🧹 [CLEANUP] Found incorrectly-synced desktop sessions, cleaning up...');
        cleanupIncorrectlySyncedSessions(pendingMobileSessions);
      }
    }
  }, [isAppInitialized, pendingMobileSessions, cleanupIncorrectlySyncedSessions]);

  // Monitor for mobile-initiated sessions and create local Claude Code sessions
  useEffect(() => {
    // Skip processing until app is fully initialized
    if (!isAppInitialized) {
      console.log('⏸️ [MOBILE-SYNC] App not initialized yet, skipping mobile session processing');
      return;
    }
    
    // Check global rate limiting
    const now = Date.now();
    if (now - lastGlobalProcessTime < GLOBAL_PROCESS_DELAY) {
      console.log(`⏳ [MOBILE-SYNC] Global rate limit active, waiting ${GLOBAL_PROCESS_DELAY - (now - lastGlobalProcessTime)}ms`);
      return;
    }
    
    // Prevent concurrent processing of multiple sessions
    if (isProcessingAnyMobileSession) {
      console.log('🚫 [MOBILE-SYNC] Already processing a mobile session, skipping');
      return;
    }
    
    const timestamp = new Date().toISOString();
    const pendingIds = pendingMobileSessions.map((s: any) => s.sessionId);
    const processedIds = Array.from(processedMobileSessions);
    const processingIds = Array.from(processingSessions);
    const overlaps = pendingIds.filter((id: string) => processedIds.includes(id));
    
    // Only log when there are actually sessions to process to reduce noise
    if (pendingIds.length > 0) {
      console.log(`🔄 [${timestamp}] useEffect TRIGGERED`);
      console.log(`📥 Pending sessions: [${pendingIds.join(', ')}]`);
      console.log(`✅ Processed sessions: [${processedIds.join(', ')}]`);
      console.log(`⏳ Currently processing: [${processingIds.join(', ')}]`);
      console.log(`🔄 Overlap detection: [${overlaps.join(', ')}]${overlaps.length > 0 ? ' ← PROBLEM!' : ''}`);
      console.log(`🔒 Processing lock: ${isProcessingRef.current}`);
    }

    // Clear existing timeout
    if (processingTimeoutRef.current) {
      clearTimeout(processingTimeoutRef.current);
    }

    // Prevent concurrent processing
    if (isProcessingRef.current) {
      console.log('⏸️ [MOBILE-SYNC] Already processing, skipping this execution');
      return;
    }

    // Debounce processing by 200ms to batch rapid changes
    processingTimeoutRef.current = setTimeout(() => {
      if (pendingMobileSessions.length === 0) {
        console.log('🔍 [MOBILE-SYNC] No pending mobile sessions, skipping processing');
        return;
      }

      console.log('🚀 [MOBILE-SYNC] Starting debounced processing...');
      isProcessingRef.current = true;

      const createSessionFromMobile = async (mobileSession: { sessionId: string; projectPath: string; title?: string }) => {
      const sessionId = mobileSession.sessionId;
      const now = Date.now();
      const lastProcessed = lastProcessedTimeRef.current[sessionId] || 0;
      
      // Circuit breaker: don't process same session within cooldown period
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
      
      // Mark session as being processed
      setProcessingSessions(prev => new Set(prev).add(mobileSession.sessionId));
      console.log('📝 [MOBILE-SYNC] Marked session as processing:', mobileSession.sessionId);
      
      try {
        // Check if we already have a local session for this Convex session
        const existingLocalSession = sessions.find(s => s.id === mobileSession.sessionId);
        if (existingLocalSession) {
          console.log('⚠️ [MOBILE-SYNC] Local session already exists for:', mobileSession.sessionId);
          return;
        }

        console.log('🔧 [MOBILE-SYNC] Invoking Tauri create_session command...');
        // Create Claude Code session via Tauri backend
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
          
          // Create local session state  
          const newSession: Session = {
            id: localSessionId,
            projectPath: mobileSession.projectPath,
            messages: [],
            inputMessage: "",
            isLoading: false,
          };

          // Add to local sessions
          setSessions(prev => [...prev, newSession]);
          console.log('📋 [MOBILE-SYNC] Added new session to local state');
          
          // Open chat pane
          openChatPane(localSessionId, mobileSession.projectPath);
          console.log('🖼️ [MOBILE-SYNC] Opened chat pane for session');
          
          console.log('🔄 [MOBILE-SYNC] Syncing session to Convex...');
          // Update the Convex session to link it to the local session
          try {
            const convexResult = await createConvexSession({
              sessionId: localSessionId,
              projectPath: mobileSession.projectPath,
              createdBy: "desktop", // Mark as desktop-created to avoid re-processing
              title: mobileSession.title || `Mobile Session - ${mobileSession.projectPath}`,
              metadata: {
                workingDirectory: mobileSession.projectPath,
                originalMobileSessionId: mobileSession.sessionId,
              },
            });
            console.log('✅ [MOBILE-SYNC] Successfully synced session to Convex:', convexResult);
          } catch (convexError) {
            console.error('❌ [MOBILE-SYNC] Failed to sync session to Convex:', convexError);
            throw convexError; // Re-throw to prevent marking as processed
          }

          // Mark mobile session as processed in the database
          console.log('🏁 [MOBILE-SYNC] Marking mobile session as processed in database...');
          try {
            const markResult = await markMobileSessionProcessed({
              mobileSessionId: mobileSession.sessionId,
            });
            console.log('✅ [MOBILE-SYNC] Successfully marked mobile session as processed:', markResult);
          } catch (markError) {
            console.error('❌ [MOBILE-SYNC] Failed to mark mobile session as processed:', markError);
            throw markError; // Re-throw to prevent local state update
          }

          // Mark mobile session as successfully processed locally
          setProcessedMobileSessions(prev => new Set(prev).add(mobileSession.sessionId));
          console.log('✅ [MOBILE-SYNC] Successfully created and synced local session from mobile request');
        } else {
          console.error('❌ [MOBILE-SYNC] Failed to create local Claude Code session:', result.error);
          
          // If Claude Code is not initialized, mark the mobile session as failed to prevent retries
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
        // Remove session from processing set
        setProcessingSessions(prev => {
          const newSet = new Set(prev);
          newSet.delete(mobileSession.sessionId);
          return newSet;
        });
        console.log('🏁 [MOBILE-SYNC] Finished processing session:', mobileSession.sessionId);
      }
    };

    // Process pending mobile sessions sequentially to avoid race conditions
    const processMobileSessions = async () => {
      try {
        console.log('🔄 [MOBILE-SYNC] Processing', pendingMobileSessions.length, 'mobile sessions');
        console.log('🔄 [MOBILE-SYNC] Currently processing sessions:', Array.from(processingSessions));
        console.log('🔄 [MOBILE-SYNC] Already processed sessions:', Array.from(processedMobileSessions));
        console.log('🔄 [MOBILE-SYNC] Current local sessions:', sessions.map(s => ({ id: s.id, path: s.projectPath })));
        
        // Process only ONE session at a time to prevent rapid creation
        const sessionToProcess = pendingMobileSessions[0];
        if (!sessionToProcess) return;
        
        const mobileSession = sessionToProcess;
        console.log('🔍 [MOBILE-SYNC] Evaluating mobile session:', mobileSession.sessionId);
        
        // Skip if this is a desktop-created session ID (UUID format)
        const isDesktopSessionId = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(mobileSession.sessionId);
        if (isDesktopSessionId) {
          console.log('⏭️ [MOBILE-SYNC] Skipping desktop-created session (UUID format):', mobileSession.sessionId);
          return;
        }
        
        // Skip if already processing this session
        if (processingSessions.has(mobileSession.sessionId)) {
          console.log('⏭️ [MOBILE-SYNC] Skipping - already processing:', mobileSession.sessionId);
          return;
        }
        
        // Check if this mobile session has already been successfully processed
        const alreadyProcessed = processedMobileSessions.has(mobileSession.sessionId);
        
        console.log('🔍 [MOBILE-SYNC] Already processed check for', mobileSession.sessionId, '- alreadyProcessed:', alreadyProcessed);
        
        if (!alreadyProcessed) {
          console.log('🎯 [MOBILE-SYNC] Creating session for:', mobileSession.sessionId);
          
          // Set global processing flag
          setIsProcessingAnyMobileSession(true);
          setLastGlobalProcessTime(Date.now());
          
          await createSessionFromMobile(mobileSession);
          
          // Clear global processing flag after a delay
          setTimeout(() => {
            setIsProcessingAnyMobileSession(false);
          }, 1000); // 1 second cooldown before allowing next session
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
    }, 200); // 200ms debounce

    // Cleanup timeout on unmount
    return () => {
      if (processingTimeoutRef.current) {
        clearTimeout(processingTimeoutRef.current);
      }
    };
  }, [pendingMobileSessions, isAppInitialized, lastGlobalProcessTime, isProcessingAnyMobileSession]); // Include rate limiting state

  // Toggle hand tracking
  const toggleHandTracking = useCallback(() => {
    const newState = !isHandTrackingActive;
    setIsHandTrackingActive(newState);
    if (!newState && draggingPaneId) {
      setDraggingPaneId(null);
      initialPinchPositionRef.current = null;
      paneStartPosRef.current = null;
    }
  }, [isHandTrackingActive, draggingPaneId]);

  // Use a ref to compare previous and current data to avoid unnecessary state updates
  const prevHandDataRef = useRef<HandDataContext | null>(null);

  // Handle hand data updates
  const handleHandDataUpdate = useCallback((data: HandDataContext) => {
    if (
      !prevHandDataRef.current ||
      data.activeHandPose !== prevHandDataRef.current.activeHandPose ||
      data.trackedHandsCount !== prevHandDataRef.current.trackedHandsCount ||
      JSON.stringify(data.pinchMidpoint) !==
      JSON.stringify(prevHandDataRef.current.pinchMidpoint)
    ) {
      prevHandDataRef.current = data;
      setHandData(data);
    }
  }, []);

  // Claude discovery is now handled in the app initialization process above


  const handleMessagesUpdate = useCallback((sessionId: string, messages: Message[]) => {
    setSessions(prev => prev.map(session => {
      if (session.id !== sessionId) return session;
      
      const currentMessages = session.messages;
      
      // Keep optimistic user messages that haven't appeared in backend yet
      const optimisticMessages = currentMessages.filter(msg => 
        msg.id.startsWith('user-') && 
        !messages.some(backend => 
          backend.message_type === 'user' && 
          backend.content === msg.content
        )
      );
      
      // Combine optimistic messages with backend messages
      const allMessages = [...optimisticMessages, ...messages];
      
      // Sort by timestamp to maintain order
      allMessages.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
      
      // Persist messages to store - defer to avoid updating during render
      setTimeout(() => {
        updateSessionMessages(sessionId, allMessages);
      }, 0);
      
      return { ...session, messages: allMessages };
    }));
  }, [updateSessionMessages]);

  const handleStreamError = useCallback((sessionId: string, error: Error) => {
    console.error(`Streaming error for session ${sessionId}:`, error);
  }, []);

  const sendMessage = async (sessionId: string, messageContent?: string) => {
    const session = sessions.find(s => s.id === sessionId);
    if (!session) {
      return;
    }
    
    const messageToSend = messageContent || session.inputMessage;
    if (!messageToSend.trim()) {
      return;
    }
    
    // Immediately add user message to UI
    const userMessage: Message = {
      id: `user-${Date.now()}`,
      message_type: "user",
      content: messageToSend,
      timestamp: new Date().toISOString(),
    };
    
    // Clear input, add message, and set loading state
    setSessions(prev => prev.map(s => 
      s.id === sessionId 
        ? { ...s, messages: [...s.messages, userMessage], inputMessage: "", isLoading: true }
        : s
    ));
    
    // Persist the updated messages including the new user message
    // Defer to avoid updating during render
    setTimeout(() => {
      const updatedSession = sessions.find(s => s.id === sessionId);
      if (updatedSession) {
        updateSessionMessages(sessionId, [...updatedSession.messages, userMessage]);
      }
    }, 0);
    
    try {
      const result = await invoke<CommandResult<void>>("send_message", {
        sessionId,
        message: messageToSend,
      });
      if (!result.success) {
        alert(`Error sending message: ${result.error}`);
        console.error("Send message failed:", result.error);
      }
    } catch (error) {
      alert(`Error: ${error}`);
      console.error("Send message error:", error);
    } finally {
      setSessions(prev => prev.map(s => 
        s.id === sessionId 
          ? { ...s, isLoading: false }
          : s
      ));
    }
  };

  const updateSessionInput = (sessionId: string, value: string) => {
    setSessions(prev => prev.map(s => 
      s.id === sessionId ? { ...s, inputMessage: value } : s
    ));
  };


  const discoverClaude = async () => {
    setIsDiscoveryLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("discover_claude");
      if (result.success && result.data) {
        setClaudeStatus(`Claude found at: ${result.data}`);
      } else {
        setClaudeStatus(`Error: ${result.error || "Unknown error"}`);
        console.error("Discovery failed:", result.error);
      }
    } catch (error) {
      setClaudeStatus(`Error: ${error}`);
      console.error("Discovery error:", error);
    }
    setIsDiscoveryLoading(false);
  };

  const createSession = async () => {
    if (!newProjectPath) {
      alert("Please enter a project path");
      return;
    }

    
    // Create a temporary session ID
    const tempSessionId = `temp-${Date.now()}`;
    
    // Create session with initializing state
    // Load any persisted messages for this session
    const persistedMessages = getSessionMessages(tempSessionId);
    const newSession: Session = {
      id: tempSessionId,
      projectPath: newProjectPath,
      messages: persistedMessages,
      inputMessage: "",
      isLoading: false,
      isInitializing: true,
    };
    
    // Add session and open pane immediately
    setSessions(prev => [...prev, newSession]);
    openChatPane(tempSessionId, newProjectPath);
    
    // Initialize Claude in the background
    try {
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath: newProjectPath,
      });
      if (result.success && result.data) {
        const realSessionId = result.data;
        
        // Update the session with the real ID and remove initializing state
        setSessions(prev => prev.map(s => 
          s.id === tempSessionId 
            ? { ...s, id: realSessionId, isInitializing: false }
            : s
        ));

        // Sync session to Convex
        try {
          await createConvexSession({
            sessionId: realSessionId,
            projectPath: newProjectPath,
            createdBy: "desktop",
            title: `Desktop Session - ${newProjectPath}`,
            metadata: {
              workingDirectory: newProjectPath,
            },
          });
          console.log('Session synced to Convex:', realSessionId);
        } catch (error) {
          console.error('Failed to sync session to Convex:', error);
          // Don't fail the session creation if Convex sync fails
        }
        
        // Update the pane ID to match the real session ID
        // Defer state update to avoid updating during render
        setTimeout(() => {
          usePaneStore.setState(state => {
            const updatedPanes = state.panes.map(p => 
              p.id === `chat-${tempSessionId}`
                ? { ...p, id: `chat-${realSessionId}`, content: { ...p.content, sessionId: realSessionId } }
                : p
            );
            
            // Transfer persisted messages from temp to real session ID
            const tempMessages = state.sessionMessages[tempSessionId];
            const updatedSessionMessages = { ...state.sessionMessages };
            if (tempMessages && tempMessages.length > 0) {
              updatedSessionMessages[realSessionId] = tempMessages;
              delete updatedSessionMessages[tempSessionId];
            }
            
            return {
              panes: updatedPanes,
              activePaneId: state.activePaneId === `chat-${tempSessionId}` 
                ? `chat-${realSessionId}` 
                : state.activePaneId,
              sessionMessages: updatedSessionMessages
            };
          });
        }, 0);
        
        // No need to fetch messages - SessionStreamManager will handle it
      } else {
        // Remove the session and close the pane on error
        setSessions(prev => prev.filter(s => s.id !== tempSessionId));
        usePaneStore.getState().removePane(`chat-${tempSessionId}`);
        alert(`Error creating session: ${result.error}`);
        console.error("Session creation failed:", result.error);
      }
    } catch (error) {
      // Remove the session and close the pane on error
      setSessions(prev => prev.filter(s => s.id !== tempSessionId));
      usePaneStore.getState().removePane(`chat-${tempSessionId}`);
      alert(`Error: ${error}`);
      console.error("Session creation error:", error);
    }
  };

  const stopSession = async (sessionId: string) => {
    setSessions(prev => prev.map(s => 
      s.id === sessionId ? { ...s, isLoading: true } : s
    ));
    
    try {
      const result = await invoke<CommandResult<void>>("stop_session", {
        sessionId,
      });
      if (result.success) {
        setSessions(prev => prev.filter(s => s.id !== sessionId));
      } else {
        alert(`Error stopping session: ${result.error}`);
        setSessions(prev => prev.map(s => 
          s.id === sessionId ? { ...s, isLoading: false } : s
        ));
      }
    } catch (error) {
      alert(`Error: ${error}`);
      setSessions(prev => prev.map(s => 
        s.id === sessionId ? { ...s, isLoading: false } : s
      ));
    }
  };

  // Set up keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check if the user is typing in an input field
      const target = event.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {
        return;
      }

      // Handle Escape key to close active pane
      if (event.key === 'Escape' && activePaneId) {
        event.preventDefault();
        removePane(activePaneId);
        return;
      }

      // Handle modifier + digit combinations
      const modifier = navigator.platform.toUpperCase().indexOf('MAC') >= 0
        ? event.metaKey // Mac uses Cmd key
        : event.ctrlKey; // Windows/Linux use Ctrl key

      if (!modifier) return;

      const digit = parseInt(event.key);
      if (isNaN(digit) || digit < 1 || digit > 9) return;

      event.preventDefault();

      // Call the appropriate function based on the digit
      switch (digit) {
        case 1:
          if (newProjectPath) {
            createSession();
          }
          break;
        case 2:
          organizePanes();
          break;
        case 3:
          toggleMetadataPane();
          break;
        case 4:
          toggleStatsPane();
          break;
        case 7:
          toggleSettingsPane();
          break;
        // case 8:
        //   console.log('Help');
        //   break;
        case 9:
          toggleHandTracking();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [toggleMetadataPane, toggleSettingsPane, toggleStatsPane, organizePanes, newProjectPath, createSession, toggleHandTracking, activePaneId, removePane]);

  // Effect for pinch-to-drag logic
  useEffect(() => {
    const TITLE_BAR_HEIGHT = 32;
    
    if (
      !isHandTrackingActive ||
      !handData ||
      !handData.pinchMidpoint ||
      handData.trackedHandsCount === 0
    ) {
      if (draggingPaneId) {
        setDraggingPaneId(null);
        initialPinchPositionRef.current = null;
        paneStartPosRef.current = null;
      }
      return;
    }

    const { activeHandPose, pinchMidpoint } = handData;

    if (activeHandPose === HandPose.PINCH_CLOSED) {
      if (!draggingPaneId) {
        // Check from topmost pane (end of array) to find pinch target
        for (let i = panes.length - 1; i >= 0; i--) {
          const pane = panes[i];
          // Check if pinch is in the title bar area
          if (
            pinchMidpoint.x >= pane.x &&
            pinchMidpoint.x <= pane.x + pane.width &&
            pinchMidpoint.y >= pane.y &&
            pinchMidpoint.y <= pane.y + TITLE_BAR_HEIGHT
          ) {
            setDraggingPaneId(pane.id);
            paneStartPosRef.current = { x: pane.x, y: pane.y };
            initialPinchPositionRef.current = {
              x: pinchMidpoint.x,
              y: pinchMidpoint.y,
            };
            if (pane.id !== activePaneId) {
              bringPaneToFront(pane.id);
            }
            break;
          }
        }
      } else if (initialPinchPositionRef.current && paneStartPosRef.current) {
        // Continue dragging
        const deltaX = pinchMidpoint.x - initialPinchPositionRef.current.x;
        const deltaY = pinchMidpoint.y - initialPinchPositionRef.current.y;

        // Only update if the move is at least 1px in either direction
        if (Math.abs(deltaX) >= 1 || Math.abs(deltaY) >= 1) {
          const newX = paneStartPosRef.current.x + deltaX;
          const newY = paneStartPosRef.current.y + deltaY;

          // Update the refs to the new values to track relative movement
          initialPinchPositionRef.current = {
            x: pinchMidpoint.x,
            y: pinchMidpoint.y,
          };
          paneStartPosRef.current = { x: newX, y: newY };

          // Update the store
          updatePanePosition(draggingPaneId, newX, newY);
        }
      }
    } else {
      // Pinch released or pose changed
      if (draggingPaneId) {
        setDraggingPaneId(null);
        initialPinchPositionRef.current = null;
        paneStartPosRef.current = null;
      }
    }
  }, [
    isHandTrackingActive,
    handData,
    draggingPaneId,
    panes,
    activePaneId,
    bringPaneToFront,
    updatePanePosition,
  ]);

  // Set dark mode on mount
  useEffect(() => {
    document.documentElement.classList.add("dark");
  }, []);

  // Provide session data to child components through a context or prop drilling
  // For now, we'll use a global object (this should be replaced with proper state management)
  // Update immediately without useEffect to ensure real-time updates
  (window as any).__openagents_data = {
    claudeStatus,
    sessions,
    newProjectPath,
    isDiscoveryLoading,
    setNewProjectPath,
    createSession,
    sendMessage,
    updateSessionInput,
    stopSession,
  };

  return (
    <TooltipProvider>
      <div className="relative h-full w-full font-mono overflow-hidden">
        {/* Pane Manager */}
        <PaneManager />
        
        {/* Session Stream Managers */}
        {sessions.map(session => (
          <SessionStreamManager
            key={session.id}
            sessionId={session.id}
            isInitializing={session.isInitializing || false}
            onMessagesUpdate={handleMessagesUpdate}
            onError={handleStreamError}
          />
        ))}
        
        {/* Hand Tracking */}
        <HandTracking
          showHandTracking={isHandTrackingActive}
          setShowHandTracking={setIsHandTrackingActive}
          onHandDataUpdate={handleHandDataUpdate}
        />
        
        {/* Hotbar */}
        <Hotbar 
          onNewChat={createSession}
          isHandTrackingActive={isHandTrackingActive}
          onToggleHandTracking={toggleHandTracking}
        />
        
        {/* Convex Demo - Floating widget in bottom right */}
        <div className="absolute bottom-4 right-4 z-50">
          <ConvexDemo />
        </div>
      </div>
    </TooltipProvider>
  );
}

export default App;