import { Effect, Stream, Queue, Ref, Schedule, pipe, Context, Layer, Fiber } from 'effect';
import { 
  TauriEventService, 
  TauriEventError, 
  ConnectionError, 
  MessageParsingError
} from './TauriEventService';

// Import the Message type from our existing code
export interface Message {
  id: string;
  message_type: 'assistant' | 'user' | 'system' | 'error' | 'tool_use' | 'thinking' | 'summary';
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

export interface StreamingSession {
  sessionId: string;
  messageQueue: Queue.Queue<Message>;
  isActive: Ref.Ref<boolean>;
  cleanup: () => void;
  processor: Fiber.RuntimeFiber<void, TauriEventError>;
}

// Service interface
export interface ClaudeStreamingService {
  // Start streaming for a session
  startStreaming: (sessionId: string) => Effect.Effect<StreamingSession, TauriEventError>;

  // Get message stream for a session
  getMessageStream: (session: StreamingSession) => Stream.Stream<Message, never>;

  // Send message to Claude
  sendMessage: (sessionId: string, message: string) => Effect.Effect<void, TauriEventError>;

  // Stop streaming for a session
  stopStreaming: (session: StreamingSession) => Effect.Effect<void, never>;
}

// Service tag
export const ClaudeStreamingService = Context.GenericTag<ClaudeStreamingService>('ClaudeStreamingService');

// Helper function to parse Claude messages
const parseClaudeMessage = (payload: unknown): Effect.Effect<Message | null, MessageParsingError> =>
  Effect.try({
    try: () => {
      console.log('[parseClaudeMessage] Parsing payload:', payload);
      // Handle string payloads that need JSON parsing
      if (typeof payload === 'string') {
        try {
          const parsed = JSON.parse(payload);
          // Validate the parsed message has required fields
          if (parsed && typeof parsed === 'object' && 'id' in parsed && 'message_type' in parsed) {
            console.log('[parseClaudeMessage] Successfully parsed string payload:', parsed);
            return parsed as Message;
          }
        } catch {
          // If JSON parsing fails, return null
          console.log('[parseClaudeMessage] Failed to parse string payload');
          return null;
        }
      }
      
      // Handle direct object payloads
      if (payload && typeof payload === 'object' && 'id' in payload && 'message_type' in payload) {
        console.log('[parseClaudeMessage] Successfully parsed object payload:', payload);
        return payload as Message;
      }
      
      console.log('[parseClaudeMessage] Payload does not match expected format, returning null');
      return null;
    },
    catch: (error) => new MessageParsingError(String(payload), error)
  });

// Service implementation
export const ClaudeStreamingServiceLive = Layer.effect(
  ClaudeStreamingService,
  Effect.gen(function* () {
    const eventService = yield* TauriEventService;
    
    return ClaudeStreamingService.of({
      startStreaming: (sessionId: string) =>
        Effect.gen(function* () {
          console.log(`[ClaudeStreamingService] Starting streaming for session: ${sessionId}`);
          // Create event stream for this session
          const eventName = `claude:${sessionId}:message`;
          console.log(`[ClaudeStreamingService] Creating event stream for: ${eventName}`);
          const { queue: eventQueue, cleanup } = yield* eventService.createEventStream(eventName);
          
          // Create message queue for processed messages
          const messageQueue = yield* Queue.bounded<Message>(1000);
          const isActive = yield* Ref.make(true);
          
          // Process events into messages
          console.log(`[ClaudeStreamingService] Setting up message processor for session: ${sessionId}`);
          const processor = yield* pipe(
            Stream.fromQueue(eventQueue),
            Stream.tap((event) => Effect.sync(() => console.log(`[ClaudeStreamingService] Processing event:`, event))),
            Stream.mapEffect((event) =>
              parseClaudeMessage(event.payload).pipe(
                Effect.retry(
                  Schedule.exponential('10 millis', 2).pipe(
                    Schedule.compose(Schedule.recurs(3))
                  )
                ),
                Effect.catchAll((error) =>
                  Effect.gen(function* () {
                    yield* Effect.logError(`Failed to parse message: ${error}`);
                    return null;
                  })
                )
              )
            ),
            Stream.filter((msg): msg is Message => msg !== null),
            Stream.tap((msg) => Queue.offer(messageQueue, msg)),
            Stream.runDrain
          ).pipe(
            Effect.fork,
            Effect.interruptible
          );
          
          const session: StreamingSession = {
            sessionId,
            messageQueue,
            isActive,
            processor,
            cleanup: () => {
              cleanup();
              Ref.set(isActive, false).pipe(Effect.runSync);
              Fiber.interrupt(processor).pipe(Effect.runPromise).catch(error => {
                console.error(`Failed to interrupt processor for session ${sessionId}:`, error);
              });
            }
          };
          
          return session;
        }),

      getMessageStream: (session: StreamingSession) =>
        pipe(
          Stream.fromQueue(session.messageQueue),
          Stream.takeWhile(() => 
            Ref.get(session.isActive).pipe(Effect.runSync)
          )
        ),

      sendMessage: (sessionId: string, message: string) =>
        Effect.gen(function* () {
          yield* eventService.emit('claude:send_message', {
            sessionId,
            message
          }).pipe(
            Effect.retry(
              Schedule.exponential('100 millis').pipe(
                Schedule.compose(Schedule.recurs(3))
              )
            ),
            Effect.catchTag('StreamingError', (error) =>
              Effect.fail(
                new ConnectionError(
                  sessionId,
                  `Failed to send message: ${error.message}`
                )
              )
            )
          );
        }),

      stopStreaming: (session: StreamingSession) =>
        Effect.gen(function* () {
          // Set active to false
          yield* Ref.set(session.isActive, false);
          
          // Interrupt the processor fiber
          yield* Fiber.interrupt(session.processor);
          
          // Shutdown the message queue
          yield* Queue.shutdown(session.messageQueue);
          
          // Call cleanup to unlisten from events
          yield* Effect.sync(() => session.cleanup());
        })
    });
  })
);