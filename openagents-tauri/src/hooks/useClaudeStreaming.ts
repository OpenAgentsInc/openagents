import { useEffect, useState, useCallback, useRef } from 'react';
import { Effect, Stream, pipe, Layer, Scope, Exit } from 'effect';
import { ClaudeStreamingService, ClaudeStreamingServiceLive, StreamingSession, Message } from '../services/ClaudeStreamingService';
import { TauriEventService, TauriEventServiceLive } from '../services/TauriEventService';
import { invoke } from '@tauri-apps/api/core';

interface UseClaudeStreamingOptions {
  sessionId: string;
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
  onMessage,
  onError
}: UseClaudeStreamingOptions): UseClaudeStreamingResult {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  
  const sessionRef = useRef<StreamingSession | null>(null);
  const scopeRef = useRef<Scope.CloseableScope | null>(null);

  // Start streaming
  const startStreaming = useCallback(async () => {
    if (isStreaming || sessionRef.current) return;
    
    setIsStreaming(true);
    setError(null);
    
    const program = Effect.gen(function* () {
      // Create a scope for this streaming session
      const scope = yield* Scope.make();
      scopeRef.current = scope;
      
      // Get the service and start streaming
      const service = yield* ClaudeStreamingService;
      const session = yield* service.startStreaming(sessionId);
      sessionRef.current = session;
      
      // Process message stream in the background
      console.log('Starting to process message stream for session:', sessionId);
      yield* pipe(
        service.getMessageStream(session),
        Stream.tap((message) =>
          Effect.sync(() => {
            console.log('Received message in hook:', message);
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
            onMessage?.(message);
          })
        ),
        Stream.runDrain,
        Effect.catchAll((error) => 
          Effect.sync(() => {
            console.error('Stream processing error:', error);
            const err = new Error(String(error));
            setError(err);
            onError?.(err);
          })
        ),
        Scope.extend(scope),
        Effect.forkScoped
      );
    });
    
    try {
      await Effect.runPromise(
        pipe(
          program,
          Effect.provide(ServiceLayer),
          Effect.scoped
        )
      );
    } catch (err) {
      console.error('Failed to start streaming:', err);
      setError(err instanceof Error ? err : new Error(String(err)));
      setIsStreaming(false);
      sessionRef.current = null;
      scopeRef.current = null;
    }
  }, [sessionId, isStreaming, onMessage, onError]);

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
      // Close the scope, which will clean up all resources
      if (scopeRef.current) {
        await Effect.runPromise(
          pipe(
            Scope.close(scopeRef.current, Exit.void),
            Effect.provide(ServiceLayer),
            Effect.scoped
          )
        );
      }
      
      // Clean up the session
      if (sessionRef.current) {
        sessionRef.current.cleanup();
      }
    } catch (err) {
      console.error('Failed to stop streaming:', err);
    } finally {
      sessionRef.current = null;
      scopeRef.current = null;
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