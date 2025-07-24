# Implementing Effect for Tauri Event Streaming in OpenAgents

This document outlines the strategy for implementing Effect framework on the OpenAgents frontend to handle streaming events from Tauri, replacing the current polling architecture with a robust, type-safe, and performant event-driven system.

## Executive Summary

The current OpenAgents architecture uses a 50ms polling mechanism to fetch messages from Claude Code, resulting in unnecessary overhead and latency. By implementing Effect framework with Tauri's event system, we can achieve:

- **Real-time streaming** with sub-millisecond latency
- **Type-safe error handling** with tagged errors
- **Automatic resource management** with scoped event listeners
- **Backpressure handling** through Effect's Stream API
- **Concurrent event processing** using Fibers
- **Testable architecture** with dependency injection

## Current Architecture Problems

1. **Polling Overhead**: 20 IPC calls per second per session
2. **Message Latency**: Up to 50ms delay for new messages
3. **Resource Waste**: Constant polling even when idle
4. **No Progressive Updates**: Messages appear in chunks rather than streaming
5. **Manual Resource Management**: Event listeners require manual cleanup
6. **Limited Error Recovery**: Basic try-catch without retry strategies

## Proposed Effect Architecture

### 1. Core Event Streaming Layer

```typescript
// services/TauriEventService.ts
import { Effect, Stream, Queue, Ref, Schedule, pipe } from 'effect';
import { listen, emit, Event as TauriEvent } from '@tauri-apps/api/event';

// Tagged error types for precise error handling
export class StreamingError {
  readonly _tag = 'StreamingError';
  constructor(readonly message: string, readonly cause?: unknown) {}
}

export class ConnectionError {
  readonly _tag = 'ConnectionError';
  constructor(readonly sessionId: string, readonly cause?: unknown) {}
}

export class MessageParsingError {
  readonly _tag = 'MessageParsingError';
  constructor(readonly rawMessage: string, readonly cause?: unknown) {}
}

// Service definition
export class TauriEventService extends Effect.Service<TauriEventService>()(
  'TauriEventService',
  {
    sync: () => ({
      // Create a scoped event listener with automatic cleanup
      listen: (eventName: string) =>
        Effect.acquireRelease(
          Effect.promise(async () => {
            const unlisten = await listen<unknown>(eventName, (event) => {
              // Events will be processed through a queue
            });
            return { unlisten, eventName };
          }),
          ({ unlisten }) => Effect.sync(() => unlisten())
        ),

      // Emit events to Tauri backend
      emit: (event: string, payload: unknown) =>
        Effect.tryPromise({
          try: () => emit(event, payload),
          catch: (error) => new StreamingError(`Failed to emit event: ${event}`, error)
        }),

      // Create a streaming queue for an event
      createEventStream: (eventName: string, bufferSize = 100) =>
        Effect.gen(function* () {
          const queue = yield* Queue.bounded<TauriEvent<unknown>>(bufferSize);
          
          const unlisten = yield* Effect.promise(() =>
            listen(eventName, (event) => {
              Queue.offer(queue, event).pipe(Effect.runPromise);
            })
          );

          const cleanup = () => {
            unlisten();
            Queue.shutdown(queue).pipe(Effect.runPromise);
          };

          return { queue, cleanup };
        })
    })
  }
) {}
```

### 2. Claude Message Streaming Implementation

```typescript
// services/ClaudeStreamingService.ts
import { Effect, Stream, Queue, Ref, Schedule, pipe, Layer } from 'effect';
import { TauriEventService } from './TauriEventService';

interface ClaudeMessage {
  id: string;
  message_type: 'assistant' | 'user' | 'system' | 'error' | 'tool_use';
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

interface StreamingSession {
  sessionId: string;
  messageQueue: Queue.Queue<ClaudeMessage>;
  isActive: Ref.Ref<boolean>;
  cleanup: () => void;
}

export class ClaudeStreamingService extends Effect.Service<ClaudeStreamingService>()(
  'ClaudeStreamingService',
  {
    sync: () => ({
      // Start streaming for a session
      startStreaming: (sessionId: string) =>
        Effect.gen(function* () {
          const eventService = yield* TauriEventService;
          
          // Create event stream for this session
          const { queue: eventQueue, cleanup } = yield* eventService.createEventStream(
            `claude:${sessionId}:message`
          );
          
          // Create message queue for processed messages
          const messageQueue = yield* Queue.bounded<ClaudeMessage>(1000);
          const isActive = yield* Ref.make(true);
          
          // Process events into messages
          const processor = yield* pipe(
            Stream.fromQueue(eventQueue),
            Stream.mapEffect((event) =>
              parseClaudeMessage(event.payload).pipe(
                Effect.retry(
                  Schedule.exponential('10 millis', 2).pipe(
                    Schedule.compose(Schedule.recurs(3))
                  )
                ),
                Effect.catchAll((error) =>
                  Effect.logError(`Failed to parse message: ${error}`).pipe(
                    Effect.as(null)
                  )
                )
              )
            ),
            Stream.filter((msg): msg is ClaudeMessage => msg !== null),
            Stream.tap((msg) => Queue.offer(messageQueue, msg)),
            Stream.runDrain
          ).pipe(Effect.fork);
          
          const session: StreamingSession = {
            sessionId,
            messageQueue,
            isActive,
            cleanup: () => {
              cleanup();
              Ref.set(isActive, false).pipe(Effect.runSync);
              Fiber.interrupt(processor).pipe(Effect.runPromise);
            }
          };
          
          return session;
        }),

      // Get message stream for a session
      getMessageStream: (session: StreamingSession) =>
        Stream.fromQueue(session.messageQueue).pipe(
          Stream.takeWhile(() => Ref.get(session.isActive).pipe(Effect.runSync))
        ),

      // Send message to Claude
      sendMessage: (sessionId: string, message: string) =>
        Effect.gen(function* () {
          const eventService = yield* TauriEventService;
          
          yield* eventService.emit('claude:send_message', {
            sessionId,
            message
          }).pipe(
            Effect.retry(
              Schedule.exponential('100 millis').pipe(
                Schedule.compose(Schedule.recurs(3))
              )
            ),
            Effect.catchTags({
              StreamingError: (error) =>
                Effect.fail(
                  new ConnectionError(
                    sessionId,
                    `Failed to send message: ${error.message}`
                  )
                )
            })
          );
        })
    }),

    dependencies: [TauriEventService.Default]
  }
) {}

// Helper function to parse Claude messages
const parseClaudeMessage = (payload: unknown): Effect.Effect<ClaudeMessage | null, MessageParsingError> =>
  Effect.try({
    try: () => {
      if (typeof payload === 'string') {
        return JSON.parse(payload) as ClaudeMessage;
      }
      return payload as ClaudeMessage;
    },
    catch: (error) => new MessageParsingError(String(payload), error)
  });
```

### 3. React Integration with Hooks

```typescript
// hooks/useClaudeStreaming.ts
import { useEffect, useState, useCallback, useRef } from 'react';
import { Effect, Stream, Runtime, Scope, Exit, pipe } from 'effect';
import { ClaudeStreamingService } from '../services/ClaudeStreamingService';

interface UseClaudeStreamingOptions {
  sessionId: string;
  onMessage?: (message: ClaudeMessage) => void;
  onError?: (error: Error) => void;
}

export function useClaudeStreaming({
  sessionId,
  onMessage,
  onError
}: UseClaudeStreamingOptions) {
  const [messages, setMessages] = useState<ClaudeMessage[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  
  const scopeRef = useRef<Scope.CloseableScope | null>(null);
  const sessionRef = useRef<StreamingSession | null>(null);
  const runtimeRef = useRef<Runtime.Runtime<ClaudeStreamingService>>();

  // Initialize runtime
  useEffect(() => {
    runtimeRef.current = Runtime.defaultRuntime.pipe(
      Runtime.provideService(ClaudeStreamingService, ClaudeStreamingService.make({}))
    );
  }, []);

  // Start streaming
  const startStreaming = useCallback(async () => {
    if (!runtimeRef.current || isStreaming) return;
    
    setIsStreaming(true);
    setError(null);
    
    const program = Effect.gen(function* () {
      const scope = yield* Scope.make();
      scopeRef.current = scope;
      
      const service = yield* ClaudeStreamingService;
      const session = yield* service.startStreaming(sessionId);
      sessionRef.current = session;
      
      // Process message stream
      yield* pipe(
        service.getMessageStream(session),
        Stream.tap((message) =>
          Effect.sync(() => {
            setMessages(prev => [...prev, message]);
            onMessage?.(message);
          })
        ),
        Stream.runDrain
      ).pipe(
        Effect.catchTags({
          ConnectionError: (e) => Effect.sync(() => {
            const error = new Error(e.message);
            setError(error);
            onError?.(error);
          }),
          StreamingError: (e) => Effect.sync(() => {
            const error = new Error(e.message);
            setError(error);
            onError?.(error);
          })
        })
      );
    });
    
    const exit = await Runtime.runPromiseExit(runtimeRef.current)(program);
    
    if (Exit.isFailure(exit)) {
      console.error('Streaming failed:', exit.cause);
      setError(new Error('Streaming failed'));
    }
    
    setIsStreaming(false);
  }, [sessionId, isStreaming, onMessage, onError]);

  // Send message
  const sendMessage = useCallback(async (message: string) => {
    if (!runtimeRef.current || !sessionRef.current) return;
    
    const program = Effect.gen(function* () {
      const service = yield* ClaudeStreamingService;
      yield* service.sendMessage(sessionId, message);
    });
    
    await Runtime.runPromise(runtimeRef.current)(program);
  }, [sessionId]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (sessionRef.current) {
        sessionRef.current.cleanup();
      }
      if (scopeRef.current) {
        Scope.close(scopeRef.current, Exit.unit).pipe(Effect.runPromise);
      }
    };
  }, []);

  return {
    messages,
    isStreaming,
    error,
    startStreaming,
    sendMessage
  };
}
```

### 4. Integration with Existing Architecture

```typescript
// App.tsx modifications
import { useClaudeStreaming } from './hooks/useClaudeStreaming';
import { ClaudeStreamingService } from './services/ClaudeStreamingService';
import { TauriEventService } from './services/TauriEventService';
import { Layer, Runtime } from 'effect';

// Create service layer
const AppServiceLayer = Layer.mergeAll(
  TauriEventService.Default,
  ClaudeStreamingService.Default
);

function App() {
  // ... existing state
  
  // Replace polling with streaming for each session
  const streamingSessions = useRef<Map<string, ReturnType<typeof useClaudeStreaming>>>(new Map());
  
  // Create session with streaming
  const createSession = async () => {
    if (!newProjectPath) return;
    
    // Create temporary session immediately
    const tempSessionId = `temp-${Date.now()}`;
    const newSession: Session = {
      id: tempSessionId,
      projectPath: newProjectPath,
      messages: [],
      inputMessage: "",
      isLoading: false,
      isInitializing: true,
    };
    setSessions(prev => [...prev, newSession]);
    openChatPane(tempSessionId, newProjectPath);
    
    try {
      // Initialize Claude session
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath: newProjectPath,
      });
      
      if (result.success && result.data) {
        const realSessionId = result.data;
        
        // Update session with real ID
        setSessions(prev => prev.map(s =>
          s.id === tempSessionId
            ? { ...s, id: realSessionId, isInitializing: false }
            : s
        ));
        
        // Start streaming for this session
        const streaming = useClaudeStreaming({
          sessionId: realSessionId,
          onMessage: (message) => {
            setSessions(prev => prev.map(s =>
              s.id === realSessionId
                ? { ...s, messages: [...s.messages, message] }
                : s
            ));
          },
          onError: (error) => {
            console.error(`Streaming error for session ${realSessionId}:`, error);
          }
        });
        
        streamingSessions.current.set(realSessionId, streaming);
        await streaming.startStreaming();
      }
    } catch (error) {
      console.error("Session creation error:", error);
      setSessions(prev => prev.filter(s => s.id !== tempSessionId));
    }
  };
  
  // Send message with streaming
  const sendMessage = async (sessionId: string) => {
    const session = sessions.find(s => s.id === sessionId);
    const streaming = streamingSessions.current.get(sessionId);
    
    if (!session || !streaming || !session.inputMessage.trim()) return;
    
    const messageToSend = session.inputMessage;
    
    // Clear input and set loading
    setSessions(prev => prev.map(s =>
      s.id === sessionId
        ? { ...s, inputMessage: "", isLoading: true }
        : s
    ));
    
    try {
      await streaming.sendMessage(messageToSend);
    } catch (error) {
      console.error("Send message error:", error);
    } finally {
      setSessions(prev => prev.map(s =>
        s.id === sessionId
          ? { ...s, isLoading: false }
          : s
      ));
    }
  };
  
  // Cleanup streaming on session stop
  const stopSession = async (sessionId: string) => {
    const streaming = streamingSessions.current.get(sessionId);
    if (streaming) {
      // Streaming cleanup handled by hook
      streamingSessions.current.delete(sessionId);
    }
    
    // ... existing stop logic
  };
  
  // ... rest of component
}
```

### 5. Rust Backend Integration

To complete the integration, the Rust backend needs to emit events instead of just storing messages:

```rust
// src-tauri/src/claude_code/manager.rs modifications
use tauri::Manager;

impl ClaudeSession {
    async fn handle_assistant_message(&mut self, data: &serde_json::Value, app_handle: &AppHandle) {
        // ... existing message processing
        
        // Emit message event to frontend
        if let Some(message) = /* processed message */ {
            let event_name = format!("claude:{}:message", self.id);
            let _ = app_handle.emit(&event_name, &message);
        }
    }
}
```

## Benefits of Effect Implementation

### 1. **Type Safety**
- Compile-time guarantees for error handling
- Tagged errors for precise error discrimination
- Type-safe service dependencies

### 2. **Resource Management**
- Automatic cleanup of event listeners
- Scoped resource management prevents memory leaks
- Graceful shutdown handling

### 3. **Performance**
- Zero polling overhead
- Sub-millisecond message delivery
- Built-in backpressure handling
- Efficient concurrent processing

### 4. **Developer Experience**
- Composable streaming operations
- Testable with dependency injection
- Clear separation of concerns
- Declarative error handling

### 5. **Reliability**
- Automatic retry with exponential backoff
- Circuit breaker patterns available
- Graceful degradation on failures
- Comprehensive error tracking

## Migration Strategy

### Phase 1: Foundation (Week 1)
1. Install Effect dependencies
2. Create service layer architecture
3. Implement TauriEventService
4. Add Rust event emission

### Phase 2: Core Implementation (Week 2)
1. Implement ClaudeStreamingService
2. Create React hooks for streaming
3. Update session management
4. Remove polling mechanism

### Phase 3: Enhanced Features (Week 3)
1. Add retry strategies
2. Implement backpressure handling
3. Add performance monitoring
4. Create error recovery flows

### Phase 4: Polish (Week 4)
1. Add comprehensive logging
2. Implement metrics collection
3. Optimize bundle size
4. Complete documentation

## Performance Metrics

Expected improvements over current polling architecture:

| Metric | Current (Polling) | With Effect Streaming |
|--------|------------------|--------------------|
| Message Latency | 25ms avg (50ms max) | <1ms |
| IPC Calls/Second | 20 per session | Only on new messages |
| CPU Usage (Idle) | ~5% | <0.1% |
| Memory Usage | Linear with sessions | Bounded by buffer size |
| Error Recovery | Manual | Automatic with retries |

## Testing Strategy

### Unit Tests
```typescript
// Test service in isolation
test('ClaudeStreamingService handles message parsing errors', async () => {
  const result = await Effect.runPromise(
    parseClaudeMessage('invalid json').pipe(
      Effect.either
    )
  );
  
  expect(Either.isLeft(result)).toBe(true);
  expect(result.left._tag).toBe('MessageParsingError');
});
```

### Integration Tests
```typescript
// Test with mock Tauri events
test('Streaming session processes messages correctly', async () => {
  const runtime = Runtime.defaultRuntime.pipe(
    Runtime.provideService(
      TauriEventService,
      mockTauriEventService
    )
  );
  
  // ... test streaming behavior
});
```

## Conclusion

Implementing Effect for Tauri event streaming in OpenAgents will transform the application from a polling-based architecture to a robust, event-driven system. This change will dramatically improve performance, reliability, and developer experience while providing a solid foundation for future enhancements like multi-session management, collaborative features, and advanced error recovery strategies.

The Effect framework's functional programming paradigm and strong type system align perfectly with Rust's safety guarantees, creating a full-stack type-safe streaming architecture that can handle the demands of real-time AI agent communication.