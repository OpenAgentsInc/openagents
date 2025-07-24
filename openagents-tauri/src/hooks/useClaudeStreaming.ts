import { useEffect, useState, useCallback, useRef } from 'react';
import { Effect, Stream, ManagedRuntime, pipe, Layer, Fiber } from 'effect';
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

// Create the service layer
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
  const runtimeRef = useRef<ManagedRuntime.ManagedRuntime<ClaudeStreamingService | TauriEventService, never>>();
  const processorFiberRef = useRef<Fiber.RuntimeFiber<void, never> | null>(null);

  // Initialize runtime
  useEffect(() => {
    const runtime = ManagedRuntime.make(ServiceLayer);
    runtimeRef.current = runtime;
    
    // Cleanup runtime on unmount
    return () => {
      runtime.dispose();
    };
  }, []);

  // Start streaming
  const startStreaming = useCallback(async () => {
    if (!runtimeRef.current || isStreaming) return;
    
    setIsStreaming(true);
    setError(null);
    
    const program = Effect.gen(function* () {
      const service = yield* ClaudeStreamingService;
      const session = yield* service.startStreaming(sessionId);
      sessionRef.current = session;
      
      // Process message stream
      const processor = yield* pipe(
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
            onMessage?.(message);
          })
        ),
        Stream.runDrain
      ).pipe(
        Effect.catchAll((error) => 
          Effect.sync(() => {
            const err = new Error(String(error));
            setError(err);
            onError?.(err);
          })
        ),
        Effect.fork
      );
      
      processorFiberRef.current = processor;
      return processor;
    });
    
    try {
      await runtimeRef.current.runPromise(program);
    } catch (err) {
      console.error('Failed to start streaming:', err);
      setError(err instanceof Error ? err : new Error(String(err)));
      setIsStreaming(false);
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
    if (!runtimeRef.current || !sessionRef.current) return;
    
    const program = Effect.gen(function* () {
      const service = yield* ClaudeStreamingService;
      yield* service.stopStreaming(sessionRef.current!);
    });
    
    try {
      await runtimeRef.current.runPromise(program);
    } catch (err) {
      console.error('Failed to stop streaming:', err);
    } finally {
      sessionRef.current = null;
      processorFiberRef.current = null;
      setIsStreaming(false);
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (sessionRef.current) {
        stopStreaming();
      }
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