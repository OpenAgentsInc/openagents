import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PaneManager } from "@/panes/PaneManager";
import { Hotbar } from "@/components/hud/Hotbar";
import { usePaneStore } from "@/stores/pane";
import { TooltipProvider } from "@/components/ui/tooltip";
import { HandTracking, HandPose } from "@/components/hands";
import type { PinchCoordinates, HandLandmarks } from "@/components/hands";

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
  
  // Pinch-to-drag state
  const [draggingPaneId, setDraggingPaneId] = useState<string | null>(null);
  const initialPinchPositionRef = useRef<{ x: number; y: number } | null>(null);
  const paneStartPosRef = useRef<{ x: number; y: number } | null>(null);
  
  const { openChatPane, toggleMetadataPane, panes, bringPaneToFront, updatePanePosition, activePaneId, removePane, updateSessionMessages, getSessionMessages } = usePaneStore();

  // Get project directory (git root or current directory) on mount
  useEffect(() => {
    invoke("get_project_directory").then((result: any) => {
      if (result.success && result.data) {
        setNewProjectPath(result.data);
      }
    }).catch(console.error);
  }, []);

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

  // Initialize Claude on mount
  useEffect(() => {
    console.log("App mounted, starting Claude discovery...");
    discoverClaude();
  }, []);

  // Track which sessions we've already attempted to restore
  const restoredSessionsRef = useRef<Set<string>>(new Set());

  // Restore sessions from persisted panes after Claude is discovered
  useEffect(() => {
    if (claudeStatus.includes("Claude found at:")) {
      // Find all chat panes and restore their sessions
      const chatPanes = panes.filter(p => p.type === "chat" && p.content?.sessionId);
      
      chatPanes.forEach(async pane => {
        if (!pane.content?.sessionId) return;
        
        const oldSessionId = pane.content.sessionId;
        const projectPath = pane.content.projectPath || newProjectPath;
        
        // Skip if we've already attempted to restore this session
        if (restoredSessionsRef.current.has(oldSessionId)) {
          return;
        }
        
        // Check if session already exists
        const sessionExists = sessions.some(s => s.id === oldSessionId);
        if (!sessionExists) {
          // Mark as attempted
          restoredSessionsRef.current.add(oldSessionId);
          // Get persisted messages
          const persistedMessages = getSessionMessages(oldSessionId);
          
          // Create a temporary frontend session while we create the backend session
          const tempSession: Session = {
            id: oldSessionId,
            projectPath: projectPath,
            messages: persistedMessages,
            inputMessage: "",
            isLoading: false,
            isInitializing: true,
          };
          
          setSessions(prev => [...prev, tempSession]);
          
          try {
            // Create a new backend session
            const result = await invoke<CommandResult<string>>("create_session", {
              projectPath: projectPath,
            });
            
            if (result.success && result.data) {
              const newSessionId = result.data;
              console.log("Restored session with new ID:", newSessionId, "old ID:", oldSessionId);
              
              // Update the frontend session with the new ID
              setSessions(prev => prev.map(s => 
                s.id === oldSessionId 
                  ? { ...s, id: newSessionId, isInitializing: false }
                  : s
              ));
              
              // Update the pane and transfer messages
              setTimeout(() => {
                usePaneStore.setState(state => {
                  const updatedPanes = state.panes.map(p => 
                    p.id === `chat-${oldSessionId}`
                      ? { ...p, id: `chat-${newSessionId}`, content: { ...p.content, sessionId: newSessionId } }
                      : p
                  );
                  
                  // Transfer persisted messages to new session ID
                  const updatedSessionMessages = { ...state.sessionMessages };
                  if (persistedMessages.length > 0) {
                    updatedSessionMessages[newSessionId] = persistedMessages;
                    delete updatedSessionMessages[oldSessionId];
                  }
                  
                  return {
                    panes: updatedPanes,
                    activePaneId: state.activePaneId === `chat-${oldSessionId}` 
                      ? `chat-${newSessionId}` 
                      : state.activePaneId,
                    sessionMessages: updatedSessionMessages
                  };
                });
              }, 0);
            } else {
              // Remove the session if we couldn't create it in the backend
              setSessions(prev => prev.filter(s => s.id !== oldSessionId));
              console.error("Failed to restore session:", result.error);
            }
          } catch (error) {
            console.error("Error restoring session:", error);
            setSessions(prev => prev.filter(s => s.id !== oldSessionId));
          }
        }
      });
    }
  }, [claudeStatus, panes, sessions.length, newProjectPath, getSessionMessages]);

  const fetchMessages = useCallback(async (sessionId: string) => {
    try {
      const result = await invoke<CommandResult<Message[]>>("get_messages", {
        sessionId,
      });
      if (result.success && result.data) {
        setSessions(prev => prev.map(session => {
          if (session.id !== sessionId) return session;
          
          const backendMessages = result.data || [];
          const currentMessages = session.messages;
          
          // Keep optimistic user messages that haven't appeared in backend yet
          const optimisticMessages = currentMessages.filter(msg => 
            msg.id.startsWith('user-') && 
            !backendMessages.some(backend => 
              backend.message_type === 'user' && 
              backend.content === msg.content
            )
          );
          
          // Combine optimistic messages with backend messages
          const allMessages = [...optimisticMessages, ...backendMessages];
          
          // Sort by timestamp to maintain order
          allMessages.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
          
          // Persist messages to store
          updateSessionMessages(sessionId, allMessages);
          
          return { ...session, messages: allMessages };
        }));
      }
    } catch (error) {
      console.error("Error fetching messages:", error);
    }
  }, [updateSessionMessages]);

  const sendMessage = async (sessionId: string) => {
    const session = sessions.find(s => s.id === sessionId);
    if (!session || !session.inputMessage.trim()) {
      console.log("Cannot send message - session:", session, "message:", session?.inputMessage);
      return;
    }
    
    // Don't send messages while initializing
    if (session.isInitializing) {
      console.log("Cannot send message - session is still initializing");
      return;
    }

    console.log("Sending message:", session.inputMessage, "to session:", sessionId);
    const messageToSend = session.inputMessage;
    
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
    const updatedSession = sessions.find(s => s.id === sessionId);
    if (updatedSession) {
      updateSessionMessages(sessionId, [...updatedSession.messages, userMessage]);
    }
    
    try {
      const result = await invoke<CommandResult<void>>("send_message", {
        sessionId,
        message: messageToSend,
      });
      console.log("Send message result:", result);
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

  // Poll for messages for all active sessions
  useEffect(() => {
    if (sessions.length === 0) return;

    const fetchAllMessages = async () => {
      // console.log('Polling sessions:', sessions.map(s => ({ id: s.id, isInitializing: s.isInitializing })));
      await Promise.all(sessions.map(session => {
        // Only fetch messages for non-initializing sessions
        if (!session.isInitializing) {
          return fetchMessages(session.id);
        }
        return Promise.resolve();
      }));
    };

    // Initial fetch
    fetchAllMessages();

    const interval = setInterval(fetchAllMessages, 50); // Poll every 50ms for real-time updates

    return () => clearInterval(interval);
  }, [sessions, fetchMessages]); // Depend on sessions array to recreate interval when IDs change

  const discoverClaude = async () => {
    console.log("Starting Claude discovery...");
    setIsDiscoveryLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("discover_claude");
      console.log("Discovery result:", result);
      if (result.success && result.data) {
        setClaudeStatus(`Claude found at: ${result.data}`);
        console.log("Claude binary found at:", result.data);
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

    console.log("Creating session for project:", newProjectPath);
    
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
      console.log("Create session result:", result);
      
      if (result.success && result.data) {
        const realSessionId = result.data;
        console.log("Session created with ID:", realSessionId);
        
        // Update the session with the real ID and remove initializing state
        setSessions(prev => prev.map(s => 
          s.id === tempSessionId 
            ? { ...s, id: realSessionId, isInitializing: false }
            : s
        ));
        
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
        
        // Manually fetch messages for the new session ID
        setTimeout(() => {
          fetchMessages(realSessionId);
        }, 100);
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
          toggleMetadataPane();
          break;
        case 2:
          if (newProjectPath) {
            createSession();
          }
          break;
        // case 7:
        //   console.log('Settings');
        //   break;
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
  }, [toggleMetadataPane, newProjectPath, createSession, toggleHandTracking, activePaneId, removePane]);

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
      <div className="fixed inset-0 font-mono overflow-hidden">
        {/* Background with grid pattern */}
        <div 
          className="absolute inset-0 opacity-5"
          style={{
            backgroundImage: `
              linear-gradient(to right, #10b981 1px, transparent 1px),
              linear-gradient(to bottom, #10b981 1px, transparent 1px)
            `,
            backgroundSize: '50px 50px'
          }}
        />
        
        {/* Pane Manager */}
        <PaneManager />
        
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
      </div>
    </TooltipProvider>
  );
}

export default App;