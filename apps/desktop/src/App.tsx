import { useEffect, useCallback } from "react";
import { PaneManager } from "@/panes/PaneManager";
import { Hotbar } from "@/components/hud/Hotbar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SessionManager } from "@/components/session/SessionManager";
import { HandTrackingManager } from "@/components/session/HandTrackingManager";
import { useAppStore } from "@/stores/appStore";
import { useClaudeDiscovery } from "@/hooks/useClaudeDiscovery";
import { useSessionManager } from "@/hooks/useSessionManager";
import { useMobileSessionSync } from "@/hooks/useMobileSessionSync";
import { useKeyboardShortcuts } from "@/hooks/useKeyboardShortcuts";
import { useHandTracking } from "@/hooks/useHandTracking";
import { useAppInitialization } from "@/hooks/useAppInitialization";
import { usePaneStore } from "@/stores/pane";
import { useMutation } from 'convex/react';
import { api } from './convex/_generated/api';

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

function App() {
  const { isAppInitialized } = useAppStore();
  const { initializeApp } = useAppInitialization();
  const { claudeStatus } = useClaudeDiscovery();
  const { updateSessionMessages } = usePaneStore();
  const updateSessionStatus = useMutation(api.claude.updateSessionStatus);
  
  const {
    sessions,
    setSessions,
    newProjectPath,
    setNewProjectPath,
    createSession,
    stopSession,
    sendMessage,
    updateSessionInput,
  } = useSessionManager();

  const {
    sessionIdMapping,
  } = useMobileSessionSync(sessions, setSessions, isAppInitialized);

  const {
    isHandTrackingActive,
    toggleHandTracking,
    handleHandDataUpdate,
  } = useHandTracking();

  useKeyboardShortcuts({
    newProjectPath,
    createSession,
    toggleHandTracking,
  });

  useEffect(() => {
    initializeApp();
  }, [initializeApp]);

  useEffect(() => {
    document.documentElement.classList.add("dark");
  }, []);

  // Mark all active desktop sessions as processed when app closes
  useEffect(() => {
    const markSessionsProcessed = async () => {
      console.log('🏁 Marking all desktop sessions as processed...');
      for (const session of sessions) {
        try {
          await updateSessionStatus({
            sessionId: session.id,
            status: "processed"
          });
          console.log('✅ Marked desktop session as processed:', session.id);
        } catch (error) {
          console.error('❌ Failed to mark session as processed:', error);
        }
      }
    };

    // Handle window close/reload
    const handleBeforeUnload = () => {
      // Mark sessions as processed
      markSessionsProcessed();
    };

    window.addEventListener('beforeunload', handleBeforeUnload);

    // Cleanup
    return () => {
      window.removeEventListener('beforeunload', handleBeforeUnload);
      markSessionsProcessed();
    };
  }, [sessions, updateSessionStatus]);

  const handleMessagesUpdate = useCallback((sessionId: string, messages: Message[]) => {
    setSessions(prev => prev.map(session => {
      if (session.id !== sessionId) return session;
      
      const currentMessages = session.messages;
      
      const optimisticMessages = currentMessages.filter(msg => 
        msg.id.startsWith('user-') && 
        !messages.some(backend => 
          backend.message_type === 'user' && 
          backend.content === msg.content
        )
      );
      
      const allMessages = [...optimisticMessages, ...messages];
      allMessages.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
      
      setTimeout(() => {
        updateSessionMessages(sessionId, allMessages);
      }, 0);
      
      return { ...session, messages: allMessages };
    }));
  }, [setSessions, updateSessionMessages]);

  const handleStreamError = useCallback((sessionId: string, error: Error) => {
    console.error(`Streaming error for session ${sessionId}:`, error);
  }, []);

  // Provide session data globally (should be replaced with context)
  (window as any).__openagents_data = {
    claudeStatus,
    sessions,
    newProjectPath,
    isDiscoveryLoading: false,
    setNewProjectPath,
    createSession,
    sendMessage,
    updateSessionInput,
    stopSession,
  };

  return (
    <TooltipProvider>
      <div className="relative h-full w-full font-mono overflow-hidden">
        <PaneManager />
        
        <SessionManager
          sessions={sessions}
          handleMessagesUpdate={handleMessagesUpdate}
          handleStreamError={handleStreamError}
          sessionIdMapping={sessionIdMapping}
        />
        
        
        <HandTrackingManager
          isActive={isHandTrackingActive}
          onHandDataUpdate={handleHandDataUpdate}
        />
        
        <Hotbar 
          onNewChat={() => {
            if (newProjectPath) {
              createSession();
            }
          }}
          isHandTrackingActive={isHandTrackingActive}
          onToggleHandTracking={toggleHandTracking}
        />
      </div>
    </TooltipProvider>
  );
}

export default App;