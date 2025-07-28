import { Effect, Queue, Context, Data, Layer } from 'effect';
import { listen, emit, Event as TauriEvent } from '@tauri-apps/api/event';

// Tagged error types using Data.TaggedError for better Effect integration
export class StreamingError extends Data.TaggedError('StreamingError')<{
  message: string;
  cause?: unknown;
}> {}

export class ConnectionError extends Data.TaggedError('ConnectionError')<{
  sessionId: string;
  cause?: unknown;
}> {}

export class MessageParsingError extends Data.TaggedError('MessageParsingError')<{
  rawMessage: string;
  cause?: unknown;
}> {}

export type TauriEventError = StreamingError | ConnectionError | MessageParsingError;

// Event stream context
export interface EventStreamContext {
  queue: Queue.Queue<unknown>; // Holds the event payload directly
  cleanup: () => void;
}

// Service interface
export interface TauriEventService {
  // Create event listener
  listen: (eventName: string) => Effect.Effect<
    { unlisten: () => void; eventName: string },
    StreamingError,
    never
  >;

  // Emit events to Tauri backend
  emit: (event: string, payload: unknown) => Effect.Effect<void, StreamingError>;

  // Create a streaming queue for an event
  createEventStream: (
    eventName: string,
    bufferSize?: number
  ) => Effect.Effect<EventStreamContext, StreamingError>;
}

// Service tag
export const TauriEventService = Context.GenericTag<TauriEventService>('TauriEventService');

// Service implementation
const TauriEventServiceImpl: TauriEventService = {
  listen: (eventName: string) =>
    Effect.tryPromise({
      try: async () => {
        const unlisten = await listen<unknown>(eventName, () => {
          // Handler will be set up by createEventStream
        });
        return { unlisten, eventName };
      },
      catch: (error) => new StreamingError({ message: `Failed to listen to event: ${eventName}`, cause: error })
    }),

  emit: (event: string, payload: unknown) =>
    Effect.tryPromise({
      try: () => emit(event, payload),
      catch: (error) => new StreamingError({ message: `Failed to emit event: ${event}`, cause: error })
    }),

  createEventStream: (eventName: string, bufferSize = 100) =>
    Effect.gen(function* () {
      // Create the queue with type unknown to match interface
      const queue = yield* Queue.bounded<unknown>(bufferSize);
      
      // Set up the event listener with proper error handling
      const unlisten = yield* Effect.tryPromise({
        try: async () => {
          return await listen(eventName, (event: TauriEvent<unknown>) => {
            // Extract the payload from the Tauri event
            const payload = event.payload;
            // Queue the payload in a fire-and-forget manner
            Effect.runPromise(Queue.offer(queue, payload)).catch(() => {
              // Silently handle queue errors
            });
          });
        },
        catch: (error) => new StreamingError({ message: `Failed to create event stream: ${eventName}`, cause: error })
      });

      const cleanup = () => {
        try {
          unlisten();
        } catch {
          // Silently handle unlisten errors
        }
        
        // Shutdown queue in background
        Effect.runPromise(Queue.shutdown(queue)).catch(() => {
          // Silently handle shutdown errors
        });
      };

      return { queue, cleanup };
    })
};

// Export the service layer
export const TauriEventServiceLive = Layer.succeed(TauriEventService, TauriEventServiceImpl);
