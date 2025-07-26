import { useEffect, useState, useCallback, useRef } from 'react';
import { Effect, Stream, pipe, Layer } from 'effect';
import { ClaudeStreamingService, ClaudeStreamingServiceLive, StreamingSession, Message } from '../services/ClaudeStreamingService';
import { TauriEventService, TauriEventServiceLive } from '../services/TauriEventService';
import { invoke } from '@tauri-apps/api/core';
import { useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';

interface UseClaudeStreamingOptions {
  sessionId: string; // Claude Code session ID for streaming
  persistToSessionId?: string; // Optional session ID for message persistence (defaults to sessionId)
  onMessage?: (message: Message) => void;
  onError?: (error: Error) => void;
}

interface UseClaudeStreamingResult {
  messages: Message[];
  isStreaming: boolean;
  error: Error | null;
  startStreaming: () => Promise<void>;
  sendMessage: (message: string) => Promise<void>;
  stopStreaming: () => Promise<void>;
}

// Create the service layer once
const TauriEventLayer = Layer.succeed(TauriEventService, TauriEventServiceLive);
const ServiceLayer = Layer.provideMerge(ClaudeStreamingServiceLive, TauriEventLayer);

export function useClaudeStreaming({
  sessionId,
  persistToSessionId,
  onMessage,
  onError
}: UseClaudeStreamingOptions): UseClaudeStreamingResult {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  
  const sessionRef = useRef<StreamingSession | null>(null);
  const addClaudeMessage = useMutation(api.claude.addClaudeMessage);

  // Start streaming
  const startStreaming = useCallback(async () => {
    if (isStreaming || sessionRef.current) return;
    
    setIsStreaming(true);
    setError(null);
    
    try {
      // Create the streaming program
      const program = Effect.gen(function* () {
        const service = yield* ClaudeStreamingService;
        const session = yield* service.startStreaming(sessionId);
        sessionRef.current = session;
        
        // Run the stream processing in the background
        yield* pipe(
          service.getMessageStream(session),
          Stream.tap((message) =>
            Effect.sync(() => {
              setMessages(prev => {
                // Check if message already exists (for updates)
                const existingIndex = prev.findIndex(m => m.id === message.id);
                if (existingIndex >= 0) {
                  // Update existing message
                  const updated = [...prev];
                  updated[existingIndex] = message;
                  return updated;
                } else {
                  // Add new message
                  return [...prev, message];
                }
              });
              
              // Persist message to Convex
              persistMessageToConvex(message);
              
              onMessage?.(message);
            })
          ),
          Stream.runDrain,
          Effect.catchAll((error) => 
            Effect.sync(() => {
              const err = new Error(String(error));
              setError(err);
              onError?.(err);
            })
          ),
          Effect.forkDaemon // Use forkDaemon to run in background without blocking
        );
        
        // Return the session
        return session;
      });
      
      // Run the program with the service layer
      await Effect.runPromise(
        pipe(
          program,
          Effect.provide(ServiceLayer)
        )
      );
    } catch (err) {
      console.error('Failed to start streaming:', err);
      setError(err instanceof Error ? err : new Error(String(err)));
      setIsStreaming(false);
      sessionRef.current = null;
    }
  }, [sessionId, isStreaming, onMessage, onError]);

  // Persist streaming messages to Convex
  const persistMessageToConvex = useCallback(async (message: Message) => {
    try {
      // Map message type to Convex format
      const messageType = mapMessageTypeToConvex(message.message_type);
      
      if (!messageType) {
        console.log('🔇 [STREAMING] Skipping message persistence for type:', message.message_type);
        return;
      }
      
      const targetSessionId = persistToSessionId || sessionId;
      
      console.log('💾 [STREAMING] Persisting message to Convex:', {
        streamingSessionId: sessionId,
        persistToSessionId: targetSessionId,
        messageId: message.id,
        messageType,
        contentLength: message.content.length
      });
      
      await addClaudeMessage({
        sessionId: targetSessionId,
        messageId: message.id,
        messageType,
        content: message.content,
        timestamp: message.timestamp,
        toolInfo: message.tool_info ? {
          toolName: message.tool_info.tool_name,
          toolUseId: message.tool_info.tool_use_id,
          input: message.tool_info.input,
          output: message.tool_info.output || undefined, // Convert null to undefined
        } : undefined,
        metadata: { source: 'claude_streaming' },
      });
      
      console.log('✅ [STREAMING] Message persisted to Convex successfully');
    } catch (error) {
      console.error('❌ [STREAMING] Failed to persist message to Convex:', error);
      // Don't throw - streaming should continue even if persistence fails
    }
  }, [sessionId, persistToSessionId, addClaudeMessage]);

  // Helper function to map streaming message types to Convex types
  const mapMessageTypeToConvex = (streamingType: string): 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking' | null => {
    switch (streamingType) {
      case 'assistant':
        return 'assistant';
      case 'user':
        return 'user';
      case 'tool_use':
        return 'tool_use';
      case 'tool_result':
        return 'tool_result';
      case 'thinking':
        return 'thinking'; // Include reasoning/thinking messages
      // Skip system messages, summaries, errors
      case 'system':
      case 'summary':
      case 'error':
        return null;
      default:
        console.warn('🤔 [STREAMING] Unknown message type:', streamingType);
        return null;
    }
  };

  // Send message using the existing Tauri command
  const sendMessage = useCallback(async (message: string) => {
    if (!sessionRef.current) return;
    
    try {
      // Use the existing send_message command
      const result = await invoke<{ success: boolean; error?: string }>('send_message', {
        sessionId,
        message
      });
      
      if (!result.success) {
        throw new Error(result.error || 'Failed to send message');
      }
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      onError?.(error);
      throw error;
    }
  }, [sessionId, onError]);

  // Stop streaming
  const stopStreaming = useCallback(async () => {
    if (!sessionRef.current) return;
    
    try {
      // Clean up the session
      if (sessionRef.current) {
        sessionRef.current.cleanup();
      }
    } catch (err) {
      console.error('Failed to stop streaming:', err);
    } finally {
      sessionRef.current = null;
      setIsStreaming(false);
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopStreaming();
    };
  }, [stopStreaming]);

  return {
    messages,
    isStreaming,
    error,
    startStreaming,
    sendMessage,
    stopStreaming
  };
}