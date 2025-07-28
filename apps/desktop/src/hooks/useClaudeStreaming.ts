import { useEffect, useState, useCallback, useRef } from 'react';
import { Effect, Stream, pipe, Layer } from 'effect';
import { ClaudeStreamingService, ClaudeStreamingServiceLive, StreamingSession, Message } from '../services/ClaudeStreamingService';
import { TauriEventServiceLive } from '../services/TauriEventService';
import { invoke } from '@tauri-apps/api/core';
import { useMutation, useQuery } from 'convex/react';
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

// Create the service layer once - use type assertion for Layer compatibility
const ServiceLayer = Layer.mergeAll(
  TauriEventServiceLive as any,
  ClaudeStreamingServiceLive as any
) as any;

export function useClaudeStreaming({
  sessionId,
  persistToSessionId,
  onMessage,
  onError
}: UseClaudeStreamingOptions): UseClaudeStreamingResult {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [hasLoadedHistory, setHasLoadedHistory] = useState(false);
  
  const sessionRef = useRef<StreamingSession | null>(null);
  const addClaudeMessage = useMutation(api.claude.addClaudeMessage);
  const lastProcessedMessageIdRef = useRef<string | null>(null);
  const isProcessingMessageRef = useRef(false);
  
  // Real-time subscription to messages from Convex
  const targetSessionId = persistToSessionId || sessionId;
  const convexMessages = useQuery(api.claude.getSessionMessages, {
    sessionId: targetSessionId,
  });

  // Load historical messages from Convex
  const loadHistoricalMessages = useCallback(async () => {
    try {
      console.log('📚 [STREAMING] Loading historical messages for session:', targetSessionId);
      
      // Use the real-time subscription messages if available
      // If convexMessages is undefined, it means the query is still loading
      if (convexMessages === undefined) {
        console.log('⏳ [STREAMING] Waiting for Convex messages to load...');
        return;
      }
      const historicalMessages = convexMessages;
      
      if (historicalMessages && historicalMessages.length > 0) {
        // Convert Convex message format to streaming message format
        const convertedMessages: Message[] = historicalMessages.map((msg: any) => ({
          id: msg.messageId,
          message_type: msg.messageType,
          content: msg.content,
          timestamp: msg.timestamp,
          tool_info: msg.toolInfo ? {
            tool_name: msg.toolInfo.toolName,
            tool_use_id: msg.toolInfo.toolUseId,
            input: msg.toolInfo.input,
            output: msg.toolInfo.output || undefined,
          } : undefined,
        }));
        
        console.log('✅ [STREAMING] Loaded', convertedMessages.length, 'historical messages');
        setMessages(convertedMessages);
        
        // Notify listeners about the historical messages
        convertedMessages.forEach(msg => onMessage?.(msg));
      } else {
        console.log('📭 [STREAMING] No historical messages found');
      }
      
      setHasLoadedHistory(true);
    } catch (error) {
      console.error('❌ [STREAMING] Failed to load historical messages:', error);
      // Don't fail streaming if we can't load history
      setHasLoadedHistory(true);
    }
  }, [targetSessionId, convexMessages, onMessage]);

  // Start streaming
  const startStreaming = useCallback(async () => {
    if (isStreaming || sessionRef.current) return;
    
    setIsStreaming(true);
    setError(null);
    
    // Load historical messages first if not already loaded
    if (!hasLoadedHistory) {
      await loadHistoricalMessages();
    }
    
    try {
      // Create the streaming program
      const program = Effect.gen(function* () {
        const service = yield* ClaudeStreamingService;
        const session = yield* service.startStreaming(sessionId);
        sessionRef.current = session;
        
        // Run the stream processing in the background
        yield* pipe(
          service.getMessageStream(session),
          Stream.tap((message: any) =>
            Effect.sync(() => {
              setMessages((prev) => {
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
            }).pipe(Effect.asVoid)
          ),
          Effect.forkDaemon // Use forkDaemon to run in background without blocking
        );
        
        // Return the session
        return session;
      });
      
      // Run the program with the service layer
      await Effect.runPromise(
        program.pipe(
          Effect.provide(ServiceLayer),
          Effect.orDie
        ) as any
      );
    } catch (err) {
      console.error('Failed to start streaming:', err);
      setError(err instanceof Error ? err : new Error(String(err)));
      setIsStreaming(false);
      sessionRef.current = null;
    }
  }, [sessionId, isStreaming, onMessage, onError, hasLoadedHistory, loadHistoricalMessages]);

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

  // Reset state when session changes
  useEffect(() => {
    setHasLoadedHistory(false);
    setMessages([]);
    lastProcessedMessageIdRef.current = null;
    isProcessingMessageRef.current = false;
  }, [sessionId, persistToSessionId]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopStreaming();
    };
  }, [stopStreaming]);

  // Detect new messages from mobile and trigger Claude response
  useEffect(() => {
    if (!convexMessages || convexMessages.length === 0 || !isStreaming) return;
    
    // Prevent processing if already in progress
    if (isProcessingMessageRef.current) return;
    
    // Find the last user message from Convex efficiently
    let lastUserMessage: typeof convexMessages[0] | null = null;
    for (let i = convexMessages.length - 1; i >= 0; i--) {
      if (convexMessages[i].messageType === 'user') {
        lastUserMessage = convexMessages[i];
        break;
      }
    }
    
    if (!lastUserMessage) return;
    
    // Check if this is a new message we haven't processed
    if (lastProcessedMessageIdRef.current !== lastUserMessage.messageId) {
      console.log('🔄 [STREAMING] Detected new user message from mobile:', {
        messageId: lastUserMessage.messageId,
        content: lastUserMessage.content.substring(0, 50) + '...',
        timestamp: lastUserMessage.timestamp
      });
      
      // Update the last processed message ID
      lastProcessedMessageIdRef.current = lastUserMessage.messageId;
      
      // Check if this message already exists in our local messages
      const messageExists = messages.some(m => m.id === lastUserMessage.messageId);
      
      if (!messageExists) {
        console.log('🚀 [STREAMING] Triggering Claude response for mobile message');
        
        // Set processing flag to prevent duplicate triggers
        isProcessingMessageRef.current = true;
        
        // Trigger Claude Code to respond to the message
        invoke<{ success: boolean; error?: string }>('trigger_claude_response', {
          sessionId,
          message: lastUserMessage.content,
        }).then(result => {
          if (result.success) {
            console.log('✅ [STREAMING] Successfully triggered Claude response');
          } else {
            console.error('❌ [STREAMING] Failed to trigger Claude response:', result.error);
          }
        }).catch(error => {
          console.error('❌ [STREAMING] Error triggering Claude response:', error);
        }).finally(() => {
          // Reset processing flag after completion
          isProcessingMessageRef.current = false;
        });
      }
    }
  }, [convexMessages, isStreaming, sessionId]); // Removed 'messages' from dependencies

  return {
    messages,
    isStreaming,
    error,
    startStreaming,
    sendMessage,
    stopStreaming
  };
}