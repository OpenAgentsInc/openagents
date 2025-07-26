import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { usePaneStore } from '@/stores/pane';
import { useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';

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

export const useSessionManager = () => {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [newProjectPath, setNewProjectPath] = useState("/Users/christopherdavid/code/openagents");
  const { openChatPane, updateSessionMessages } = usePaneStore();
  const createClaudeSession = useMutation(api.claude.createClaudeSession);

  const createSession = useCallback(async () => {
    if (!newProjectPath) {
      alert("Please enter a project path");
      return;
    }

    try {
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath: newProjectPath,
      });

      if (result.success && result.data) {
        const sessionId = result.data;
        
        // Create the Convex session document so mobile can see it
        try {
          await createClaudeSession({
            sessionId,
            projectPath: newProjectPath,
            createdBy: "desktop",
            title: `Desktop Session - ${new Date().toLocaleString()}`,
          });
          console.log('✅ Created Convex session document for:', sessionId);
        } catch (convexError) {
          console.error('❌ Failed to create Convex session:', convexError);
          // Continue anyway - local session still works
        }
        
        const newSession: Session = {
          id: sessionId,
          projectPath: newProjectPath,
          messages: [],
          inputMessage: "",
          isLoading: false,
        };
        
        setSessions(prev => [...prev, newSession]);
        openChatPane(sessionId, newProjectPath);
        setNewProjectPath("");
      } else {
        alert(`Error creating session: ${result.error}`);
      }
    } catch (error) {
      alert(`Error: ${error}`);
    }
  }, [newProjectPath, openChatPane, createClaudeSession]);

  const stopSession = useCallback(async (sessionId: string) => {
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
  }, []);

  const sendMessage = useCallback(async (sessionId: string, messageContent?: string) => {
    console.log('📮 [SEND-MESSAGE] Called with:', { sessionId, messageContent, sessionsCount: sessions.length });
    const session = sessions.find(s => s.id === sessionId);
    if (!session) {
      console.error('❌ [SEND-MESSAGE] Session not found:', sessionId);
      console.log('📋 [SEND-MESSAGE] Available sessions:', sessions.map(s => s.id));
      return;
    }
    
    const messageToSend = messageContent || session.inputMessage;
    if (!messageToSend.trim()) {
      return;
    }
    
    const userMessage: Message = {
      id: `user-${Date.now()}`,
      message_type: "user",
      content: messageToSend,
      timestamp: new Date().toISOString(),
    };
    
    setSessions(prev => prev.map(s => 
      s.id === sessionId 
        ? { ...s, messages: [...s.messages, userMessage], inputMessage: "", isLoading: true }
        : s
    ));
    
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
  }, [sessions, updateSessionMessages]);

  // Separate function for replaying existing messages (doesn't add to local state)
  const replayMessage = useCallback(async (sessionId: string, messageContent: string) => {
    console.log('🔁 [REPLAY-MESSAGE] Replaying existing message:', { sessionId, messageContent });
    
    if (!messageContent.trim()) {
      return;
    }
    
    try {
      const result = await invoke<CommandResult<void>>("send_message", {
        sessionId,
        message: messageContent,
      });
      if (!result.success) {
        console.error("Replay message failed:", result.error);
      } else {
        console.log('✅ [REPLAY-MESSAGE] Successfully replayed message to Claude Code');
      }
    } catch (error) {
      console.error("Replay message error:", error);
    }
  }, []);

  const updateSessionInput = useCallback((sessionId: string, value: string) => {
    setSessions(prev => prev.map(s => 
      s.id === sessionId ? { ...s, inputMessage: value } : s
    ));
  }, []);

  return {
    sessions,
    setSessions,
    newProjectPath,
    setNewProjectPath,
    createSession,
    stopSession,
    sendMessage,
    replayMessage, // New function for replaying existing messages
    updateSessionInput,
  };
};