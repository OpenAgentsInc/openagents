import { Effect, Queue, Context } from 'effect';
import { listen, emit, Event as TauriEvent, UnlistenFn } from '@tauri-apps/api/event';

// Tagged error types for precise error handling
export class StreamingError {
  readonly _tag = 'StreamingError' as const;
  constructor(readonly message: string, readonly cause?: unknown) {}
}

export class ConnectionError {
  readonly _tag = 'ConnectionError' as const;
  constructor(readonly sessionId: string, readonly cause?: unknown) {}
}

export class MessageParsingError {
  readonly _tag = 'MessageParsingError' as const;
  constructor(readonly rawMessage: string, readonly cause?: unknown) {}
}

export type TauriEventError = StreamingError | ConnectionError | MessageParsingError;

// Event stream context
export interface EventStreamContext {
  queue: Queue.Queue<unknown>; // Will hold TauriEvent<unknown> but typed as unknown for compatibility
  cleanup: () => void;
}

// Service interface
export interface TauriEventService {
  // Create event listener
  listen: (eventName: string) => Effect.Effect<
    { unlisten: UnlistenFn; eventName: string },
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
export const TauriEventServiceLive = TauriEventService.of({
  listen: (eventName: string) =>
    Effect.tryPromise({
      try: async () => {
        const unlisten = await listen<unknown>(eventName, () => {
          // Handler will be set up by createEventStream
        });
        return { unlisten, eventName };
      },
      catch: (error) => new StreamingError(`Failed to listen to event: ${eventName}`, error)
    }),

  emit: (event: string, payload: unknown) =>
    Effect.tryPromise({
      try: () => emit(event, payload),
      catch: (error) => new StreamingError(`Failed to emit event: ${event}`, error)
    }),

  createEventStream: (eventName: string, bufferSize = 100) =>
    Effect.gen(function* () {
      // Create the queue with type unknown to match interface
      const queue = yield* Queue.bounded<unknown>(bufferSize);
      
      // Set up the event listener with proper error handling
      const unlisten = yield* Effect.tryPromise({
        try: async () => {
          console.log(`Setting up event listener for: ${eventName}`);
          return await listen(eventName, (event: TauriEvent<unknown>) => {
            // Queue the event in a fire-and-forget manner
            Effect.runPromise(Queue.offer(queue, event)).catch(error => {
              console.error(`Failed to enqueue event for ${eventName}:`, error);
            });
          });
        },
        catch: (error) => new StreamingError(`Failed to create event stream: ${eventName}`, error)
      });

      const cleanup = () => {
        console.log(`Cleaning up event listener for: ${eventName}`);
        try {
          unlisten();
        } catch (e) {
          console.error(`Error during unlisten for ${eventName}:`, e);
        }
        
        // Shutdown queue in background
        Effect.runPromise(Queue.shutdown(queue)).catch(error => {
          console.error(`Failed to shutdown queue for ${eventName}:`, error);
        });
      };

      return { queue, cleanup };
    })
});