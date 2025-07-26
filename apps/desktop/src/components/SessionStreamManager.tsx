import { useEffect, useRef } from 'react';
import { useClaudeStreaming } from '@/hooks/useClaudeStreaming';

interface SessionStreamManagerProps {
  sessionId: string; // Claude Code UUID for streaming
  persistToSessionId?: string; // Mobile session ID for persistence
  isInitializing: boolean;
  onMessagesUpdate: (sessionId: string, messages: any[]) => void;
  onError: (sessionId: string, error: Error) => void;
}

export function SessionStreamManager({ 
  sessionId, 
  persistToSessionId,
  isInitializing,
  onMessagesUpdate, 
  onError 
}: SessionStreamManagerProps) {
  const isRealSessionId = !sessionId.startsWith('temp-');
  const hasStartedRef = useRef(false);
  
  const { messages, startStreaming, stopStreaming, error } = useClaudeStreaming({
    sessionId, // Claude Code UUID for streaming
    persistToSessionId, // Mobile session ID for persistence
    onMessage: () => {
      // Messages are already accumulated in the hook
    },
    onError: (err) => onError(sessionId, err)
  });

  // Start streaming when we have a real session ID and it's not initializing
  useEffect(() => {
    if (isRealSessionId && !isInitializing && !hasStartedRef.current) {
      hasStartedRef.current = true;
      startStreaming().catch(err => {
        console.error(`Failed to start streaming for session ${sessionId}:`, err);
      });
    }
  }, [isRealSessionId, isInitializing, sessionId, startStreaming]);

  // Update messages when they change
  useEffect(() => {
    if (messages.length > 0) {
      onMessagesUpdate(sessionId, messages);
    }
  }, [messages, sessionId, onMessagesUpdate]);

  // Handle errors
  useEffect(() => {
    if (error) {
      onError(sessionId, error);
    }
  }, [error, sessionId, onError]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (hasStartedRef.current) {
        stopStreaming();
      }
    };
  }, [stopStreaming]);

  return null; // This component doesn't render anything
}