import { Effect, Stream, Queue, pipe, Layer, Schedule } from 'effect';
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
  messageQueue: Queue.Queue<unknown>; // Holds the event payload directly
  cleanup: () => void;
}

// Service definition using Effect.Service pattern
export class ClaudeStreamingService extends Effect.Service<ClaudeStreamingService>()(
  'ClaudeStreamingService',
  {
    sync: () => ({
      // Start streaming for a session
      startStreaming: (sessionId: string) => 
        Effect.gen(function* () {
          const messageQueue = yield* Queue.unbounded<Message>()
          return { id: sessionId, messageQueue }
        }),

      // Get message stream for a session
      getMessageStream: (session: StreamingSession) => 
        Stream.fromQueue(session.messageQueue),

      // Send message to Claude
      sendMessage: (sessionId: string, message: string) => 
        Effect.void,

      // Stop streaming for a session
      stopStreaming: (session: StreamingSession) => 
        Effect.void
    }),
    
    dependencies: [TauriEventService]
  }
) {}

// Helper function to parse Claude messages
const parseClaudeMessage = (payload: unknown): Effect.Effect<Message | null, MessageParsingError> =>
  Effect.try({
    try: () => {
      // Handle string payloads that need JSON parsing
      if (typeof payload === 'string') {
        try {
          const parsed = JSON.parse(payload);
          // Validate the parsed message has required fields
          if (parsed && typeof parsed === 'object' && 'id' in parsed && 'message_type' in parsed) {
            return parsed as Message;
          }
        } catch {
          // If JSON parsing fails, return null
          return null;
        }
      }
      
      // Handle direct object payloads
      if (payload && typeof payload === 'object' && 'id' in payload && 'message_type' in payload) {
        return payload as Message;
      }
      
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
          // Create event stream for this session
          const eventName = `claude:${sessionId}:message`;
          const { queue: eventQueue, cleanup } = yield* eventService.createEventStream(eventName);
          
          // Create a simple streaming session
          const session: StreamingSession = {
            sessionId,
            messageQueue: eventQueue,
            cleanup: () => {
              cleanup();
            }
          };
          
          return session;
        }),

      getMessageStream: (session: StreamingSession) =>
        pipe(
          Stream.fromQueue(session.messageQueue),
          Stream.mapEffect((payload: unknown) => {
            // Parse the payload directly as it's already extracted
            return parseClaudeMessage(payload).pipe(
              Effect.catchAll(() => Effect.succeed(null))
            );
          }),
          Stream.filter((msg): msg is Message => msg !== null)
        ),

      sendMessage: (sessionId: string, message: string): Effect.Effect<void, TauriEventError, never> =>
        pipe(
          eventService.emit('claude:send_message', { sessionId, message }),
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
        ),

      stopStreaming: (session: StreamingSession) =>
        Effect.gen(function* () {
          // Call cleanup to unlisten from events
          yield* Effect.sync(() => session.cleanup());
          
          // Shutdown the message queue
          yield* Queue.shutdown(session.messageQueue);
        })
    });
  })
);