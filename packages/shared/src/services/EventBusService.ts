import { Effect, Data, Schema, PubSub, Queue, Fiber, Layer } from "effect";
import { ConfigService } from "./ConfigService";

/**
 * EventBusService - Pub/Sub architecture for event-driven communication
 * 
 * Following EffectPatterns best practices:
 * - "decouple-fibers-with-queue-pubsub" pattern
 * - Type-safe event schemas with validation
 * - Automatic backpressure control
 * - Event filtering and routing
 * - Dead letter queue for failed events
 * - Metrics and monitoring integration
 * - Graceful fiber management
 */

// Tagged error types for event bus operations
export class EventBusError extends Data.TaggedError("EventBusError")<{
  operation: string;
  eventType?: string;
  message: string;
  cause?: unknown;
}> {}

export class EventValidationError extends Data.TaggedError("EventValidationError")<{
  eventType: string;
  payload: unknown;
  errors: unknown[];
}> {}

export class SubscriptionError extends Data.TaggedError("SubscriptionError")<{
  subscriberId: string;
  eventType: string;
  message: string;
  cause?: unknown;
}> {}

// Event metadata schema
export const EventMetadataSchema = Schema.Struct({
  id: Schema.String,
  timestamp: Schema.Number,
  source: Schema.String,
  correlationId: Schema.optional(Schema.String),
  retryCount: Schema.Number,
  priority: Schema.Literal("low", "normal", "high", "critical")
});

export type EventMetadata = Schema.Schema.Type<typeof EventMetadataSchema>;

// Base event schema
export const BaseEventSchema = <T>(payloadSchema: Schema.Schema<T>) =>
  Schema.Struct({
    type: Schema.String,
    payload: payloadSchema,
    metadata: EventMetadataSchema
  });

export type BaseEvent<T> = {
  type: string;
  payload: T;
  metadata: EventMetadata;
};

// Define payload schemas first
const UserPayloadSchema = Schema.Struct({
  userId: Schema.String,
  action: Schema.Literal("login", "logout", "register", "update", "delete"),
  details: Schema.optional(Schema.Record(Schema.String, Schema.Unknown))
});

const SessionPayloadSchema = Schema.Struct({
  sessionId: Schema.String,
  action: Schema.Literal("start", "end", "pause", "resume"),
  duration: Schema.optional(Schema.Number),
  metadata: Schema.optional(Schema.Record(Schema.String, Schema.Unknown))
});

const SystemPayloadSchema = Schema.Struct({
  component: Schema.String,
  level: Schema.Literal("info", "warn", "error", "fatal"),
  message: Schema.String,
  details: Schema.optional(Schema.Record(Schema.String, Schema.Unknown))
});

const ClaudePayloadSchema = Schema.Struct({
  messageId: Schema.String,
  action: Schema.Literal("start", "chunk", "complete", "error"),
  content: Schema.optional(Schema.String),
  tokens: Schema.optional(Schema.Number),
  error: Schema.optional(Schema.String)
});

// Predefined event schemas for common application events
export const UserEventSchema = BaseEventSchema(UserPayloadSchema);
export const SessionEventSchema = BaseEventSchema(SessionPayloadSchema);
export const SystemEventSchema = BaseEventSchema(SystemPayloadSchema);
export const ClaudeEventSchema = BaseEventSchema(ClaudePayloadSchema);

// Event subscriber configuration
export interface EventSubscriber<T> {
  id: string;
  eventType: string;
  schema: Schema.Schema<BaseEvent<T>>;
  handler: (event: BaseEvent<T>) => Effect.Effect<void, unknown, never>;
  filter?: (event: BaseEvent<T>) => boolean;
  retryPolicy?: {
    maxRetries: number;
    backoffMs: number;
  };
  deadLetterQueue?: boolean;
}

// Event bus statistics
export interface EventBusStats {
  totalEvents: number;
  activeSubscribers: number;
  queueSize: number;
  deadLetterQueueSize: number;
  eventsPerSecond: number;
  averageProcessingTime: number;
  failureRate: number;
}

// Event bus configuration schema
export const EventBusConfigSchema = Schema.Struct({
  maxQueueSize: Schema.Number.pipe(Schema.positive()),
  maxSubscribers: Schema.Number.pipe(Schema.positive()),
  defaultRetries: Schema.Number.pipe(Schema.int(), Schema.between(0, 10)),
  processingTimeout: Schema.Number.pipe(Schema.positive()),
  deadLetterQueueEnabled: Schema.Boolean,
  metricsEnabled: Schema.Boolean,
  batchSize: Schema.Number.pipe(Schema.positive())
});

export type EventBusConfig = Schema.Schema.Type<typeof EventBusConfigSchema>;

const DEFAULT_EVENT_BUS_CONFIG: EventBusConfig = {
  maxQueueSize: 1000,
  maxSubscribers: 100,
  defaultRetries: 3,
  processingTimeout: 30000,
  deadLetterQueueEnabled: true,
  metricsEnabled: true,
  batchSize: 10
};

/**
 * EventBusService using Effect.Service pattern with PubSub
 * 
 * This implements the "decouple-fibers-with-queue-pubsub" pattern from EffectPatterns,
 * enabling event-driven architecture with automatic backpressure and error handling.
 */
export class EventBusService extends Effect.Service<EventBusService>()(
  "EventBusService",
  {
    effect: Effect.gen(function* () {
      const configService = yield* ConfigService;
      const config = yield* configService.getPath("eventBus", DEFAULT_EVENT_BUS_CONFIG);
      
      // Create the main event PubSub with bounded capacity for backpressure
      const eventPubSub = yield* PubSub.bounded<BaseEvent<unknown>>(config.maxQueueSize);
      
      // Dead letter queue for failed events
      const deadLetterQueue = config.deadLetterQueueEnabled 
        ? yield* Queue.bounded<BaseEvent<unknown>>(config.maxQueueSize / 10)
        : undefined;
      
      // Subscriber management
      const subscribers = new Map<string, {
        subscriber: EventSubscriber<unknown>;
        fiber: Fiber.Fiber<void, unknown>;
        stats: {
          processed: number;
          failed: number;
          lastProcessed: number;
        };
      }>();
      
      // Event bus statistics
      let stats: EventBusStats = {
        totalEvents: 0,
        activeSubscribers: 0,
        queueSize: 0,
        deadLetterQueueSize: 0,
        eventsPerSecond: 0,
        averageProcessingTime: 0,
        failureRate: 0
      };
      
      // Helper to generate unique event IDs
      const generateEventId = () => `event_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      
      // Helper to create event metadata
      const createMetadata = (source: string, priority: EventMetadata["priority"] = "normal"): EventMetadata => ({
        id: generateEventId(),
        timestamp: Date.now(),
        source,
        retryCount: 0,
        priority
      });
      
      // Process events for a specific subscriber
      const processSubscriberEvents = <T>(
        subscriberInfo: EventSubscriber<T>,
        subscription: Queue.Dequeue<BaseEvent<unknown>>
      ) => Effect.gen(function* () {
        while (true) {
          const event = yield* Queue.take(subscription);
          
          // Filter events by type and custom filter
          if (event.type !== subscriberInfo.eventType) continue;
          if (subscriberInfo.filter && !subscriberInfo.filter(event as BaseEvent<T>)) continue;
          
          const startTime = Date.now();
          
          try {
            // Validate event against subscriber schema
            const validatedEvent = yield* Schema.decode(subscriberInfo.schema)(event as BaseEvent<T>).pipe(
              Effect.catchTag("ParseError", (error) =>
                Effect.fail(new EventValidationError({
                  eventType: subscriberInfo.eventType,
                  payload: event.payload,
                  errors: [String(error)]
                }))
              )
            );
            
            // Process the event with timeout
            yield* subscriberInfo.handler(validatedEvent).pipe(
              Effect.timeout(`${config.processingTimeout} millis`),
              Effect.catchAll((error) => {
                // Handle retry logic
                const retryPolicy = subscriberInfo.retryPolicy || {
                  maxRetries: config.defaultRetries,
                  backoffMs: 1000
                };
                
                if (event.metadata.retryCount < retryPolicy.maxRetries) {
                  // Retry the event
                  const retryEvent = {
                    ...event,
                    metadata: {
                      ...event.metadata,
                      retryCount: event.metadata.retryCount + 1
                    }
                  };
                  
                  return Effect.gen(function* () {
                    yield* Effect.sleep(`${retryPolicy.backoffMs * Math.pow(2, event.metadata.retryCount)} millis`);
                    yield* PubSub.publish(eventPubSub, retryEvent);
                  });
                } else if (deadLetterQueue && subscriberInfo.deadLetterQueue !== false) {
                  // Send to dead letter queue
                  return Queue.offer(deadLetterQueue, event).pipe(
                    Effect.catchAll(() => Effect.void) // Ignore DLQ failures
                  );
                } else {
                  // Log and ignore the error
                  return Effect.logError(`Failed to process event ${event.metadata.id} for subscriber ${subscriberInfo.id}: ${String(error)}`);
                }
              })
            );
            
            // Update subscriber stats
            const subscriberData = subscribers.get(subscriberInfo.id);
            if (subscriberData) {
              subscriberData.stats.processed++;
              subscriberData.stats.lastProcessed = Date.now();
            }
            
          } catch (error) {
            // Update failure stats
            const subscriberData = subscribers.get(subscriberInfo.id);
            if (subscriberData) {
              subscriberData.stats.failed++;
            }
            
            yield* Effect.logError(`Subscriber ${subscriberInfo.id} failed to process event: ${String(error)}`);
          }
          
          // Update processing time metrics
          const processingTime = Date.now() - startTime;
          stats.averageProcessingTime = (stats.averageProcessingTime + processingTime) / 2;
        }
      });
      
      return {
        /**
         * Publish an event to the event bus
         */
        publish: <T>(
          eventType: string,
          payload: T,
          schema: Schema.Schema<T>,
          options: {
            source?: string;
            priority?: EventMetadata["priority"];
            correlationId?: string;
          } = {}
        ) => Effect.gen(function* () {
          // Validate the payload
          const validatedPayload = yield* Schema.decode(schema)(payload).pipe(
            Effect.catchTag("ParseError", (error) =>
              Effect.fail(new EventValidationError({
                eventType,
                payload,
                errors: [String(error)]
              }))
            )
          );
          
          // Create the event
          const event: BaseEvent<T> = {
            type: eventType,
            payload: validatedPayload,
            metadata: {
              ...createMetadata(options.source || "unknown", options.priority),
              correlationId: options.correlationId
            }
          };
          
          // Publish to PubSub
          yield* PubSub.publish(eventPubSub, event as BaseEvent<unknown>);
          
          // Update stats
          stats.totalEvents++;
          
          return event.metadata.id;
        }),
        
        /**
         * Subscribe to events of a specific type
         */
        subscribe: <T>(subscriber: EventSubscriber<T>) => Effect.gen(function* () {
          if (subscribers.size >= config.maxSubscribers) {
            yield* Effect.fail(new EventBusError({
              operation: "subscribe",
              message: `Maximum number of subscribers (${config.maxSubscribers}) reached`
            }));
          }
          
          if (subscribers.has(subscriber.id)) {
            yield* Effect.fail(new EventBusError({
              operation: "subscribe",
              eventType: subscriber.eventType,
              message: `Subscriber with ID ${subscriber.id} already exists`
            }));
          }
          
          // Create subscription to the PubSub
          const subscription = yield* PubSub.subscribe(eventPubSub);
          
          // Start processing fiber for this subscriber
          const processingFiber = yield* processSubscriberEvents(subscriber, subscription).pipe(
            Effect.catchAll((error) => 
              Effect.logError(`Subscriber ${subscriber.id} processing failed: ${String(error)}`)
            ),
            Effect.fork
          );
          
          // Register the subscriber
          subscribers.set(subscriber.id, {
            subscriber: subscriber as EventSubscriber<unknown>,
            fiber: processingFiber,
            stats: {
              processed: 0,
              failed: 0,
              lastProcessed: 0
            }
          });
          
          stats.activeSubscribers = subscribers.size;
          
          yield* Effect.logInfo(`Subscribed ${subscriber.id} to ${subscriber.eventType} events`);
        }),
        
        /**
         * Unsubscribe from events
         */
        unsubscribe: (subscriberId: string) => Effect.gen(function* () {
          const subscriberData = subscribers.get(subscriberId);
          if (!subscriberData) {
            yield* Effect.fail(new EventBusError({
              operation: "unsubscribe",
              message: `Subscriber ${subscriberId} not found`
            }));
          }
          
          // Interrupt the processing fiber
          yield* Fiber.interrupt(subscriberData!.fiber);
          
          // Remove from subscribers
          subscribers.delete(subscriberId);
          stats.activeSubscribers = subscribers.size;
          
          yield* Effect.logInfo(`Unsubscribed ${subscriberId}`);
        }),
        
        /**
         * Get all active subscribers
         */
        getSubscribers: () => Effect.succeed(
          Array.from(subscribers.values()).map(s => ({
            id: s.subscriber.id,
            eventType: s.subscriber.eventType,
            stats: s.stats
          }))
        ),
        
        /**
         * Get event bus statistics
         */
        getStats: () => Effect.gen(function* () {
          // Update current queue sizes
          const currentQueueSize = yield* PubSub.size(eventPubSub);
          const dlqSize = deadLetterQueue ? yield* Queue.size(deadLetterQueue) : 0;
          
          return {
            ...stats,
            queueSize: currentQueueSize,
            deadLetterQueueSize: dlqSize,
            failureRate: stats.totalEvents > 0 
              ? Array.from(subscribers.values()).reduce((sum, s) => sum + s.stats.failed, 0) / stats.totalEvents
              : 0
          };
        }),
        
        /**
         * Process dead letter queue manually
         */
        processDeadLetterQueue: () => Effect.gen(function* () {
          if (!deadLetterQueue) {
            yield* Effect.fail(new EventBusError({
              operation: "processDeadLetterQueue",
              message: "Dead letter queue is not enabled"
            }));
          }
          
          let processedCount = 0;
          
          while (true) {
            const maybeEvent = yield* Queue.poll(deadLetterQueue!);
            
            if (maybeEvent._tag === "None") break;
            
            const event = maybeEvent.value;
            
            // Re-publish the event with reset retry count
            const retryEvent = {
              ...event,
              metadata: {
                ...event.metadata,
                retryCount: 0
              }
            };
            
            yield* PubSub.publish(eventPubSub, retryEvent);
            processedCount++;
          }
          
          return { processedCount };
        }),
        
        /**
         * Shutdown the event bus gracefully
         */
        shutdown: () => Effect.gen(function* () {
          yield* Effect.logInfo("Shutting down EventBusService...");
          
          // Interrupt all subscriber fibers
          const fibers = Array.from(subscribers.values()).map(s => s.fiber);
          yield* Effect.forEach(fibers, Fiber.interrupt, { concurrency: "unbounded" });
          
          // Clear subscribers
          subscribers.clear();
          stats.activeSubscribers = 0;
          
          yield* Effect.logInfo("EventBusService shutdown complete");
        }),
        
        /**
         * Health check for the event bus
         */
        healthCheck: () => Effect.gen(function* () {
          const currentStats = yield* Effect.succeed(stats);
          const queueSize = yield* PubSub.size(eventPubSub);
          
          const isHealthy = queueSize < config.maxQueueSize * 0.8 && // Queue not too full
                           currentStats.failureRate < 0.1; // Low failure rate
          
          return {
            status: isHealthy ? "healthy" as const : "unhealthy" as const,
            stats: currentStats,
            queueUtilization: queueSize / config.maxQueueSize,
            timestamp: Date.now()
          };
        }),
        
        /**
         * Get current configuration
         */
        getConfig: () => Effect.succeed(config)
      };
    })
  }
) {
  /**
   * Live layer that provides the EventBusService
   */
  static Live = Layer.effect(
    EventBusService,
    Effect.gen(function* () {
      return yield* EventBusService;
    })
  ).pipe(
    Layer.provide(ConfigService.Default)
  );
  
  /**
   * Test implementation for mocking in tests
   */
  static Test = () => {
    const mockEvents: BaseEvent<unknown>[] = [];
    const mockSubscribers = new Map<string, EventSubscriber<unknown>>();
    
    return EventBusService.of({
      _tag: "EventBusService" as const,
      publish: (eventType, payload, schema, options = {}) => Effect.gen(function* () {
        const validatedPayload = yield* Schema.decode(schema)(payload).pipe(
          Effect.catchTag("ParseError", (error) =>
            Effect.fail(new EventValidationError({
              eventType,
              payload,
              errors: [String(error)]
            }))
          )
        );
        const event: BaseEvent<unknown> = {
          type: eventType,
          payload: validatedPayload,
          metadata: {
            id: `test_${Date.now()}`,
            timestamp: Date.now(),
            source: options.source || "test",
            retryCount: 0,
            priority: options.priority || "normal",
            correlationId: options.correlationId
          }
        };
        
        mockEvents.push(event);
        
        // Simulate processing by relevant subscribers
        for (const subscriber of mockSubscribers.values()) {
          if (subscriber.eventType === eventType) {
            yield* subscriber.handler(event as any).pipe(
              Effect.catchAll(() => Effect.void)
            );
          }
        }
        
        return event.metadata.id;
      }),
      
      subscribe: (subscriber) => Effect.sync(() => {
        mockSubscribers.set(subscriber.id, subscriber as EventSubscriber<unknown>);
      }),
      
      unsubscribe: (subscriberId) => Effect.sync(() => {
        mockSubscribers.delete(subscriberId);
      }),
      
      getSubscribers: () => Effect.succeed(
        Array.from(mockSubscribers.values()).map(s => ({
          id: s.id,
          eventType: s.eventType,
          stats: { processed: 0, failed: 0, lastProcessed: 0 }
        }))
      ),
      
      getStats: () => Effect.succeed({
        totalEvents: mockEvents.length,
        activeSubscribers: mockSubscribers.size,
        queueSize: 0,
        deadLetterQueueSize: 0,
        eventsPerSecond: 0,
        averageProcessingTime: 1,
        failureRate: 0
      }),
      
      processDeadLetterQueue: () => Effect.succeed({ processedCount: 0 }),
      shutdown: () => Effect.void,
      healthCheck: () => Effect.succeed({
        status: "healthy" as const,
        stats: {
          totalEvents: mockEvents.length,
          activeSubscribers: mockSubscribers.size,
          queueSize: 0,
          deadLetterQueueSize: 0,
          eventsPerSecond: 0,
          averageProcessingTime: 1,
          failureRate: 0
        },
        queueUtilization: 0,
        timestamp: Date.now()
      }),
      getConfig: () => Effect.succeed(DEFAULT_EVENT_BUS_CONFIG)
    });
  };
  
  /**
   * Test layer that provides the test EventBusService
   */
  static TestLive = Layer.succeed(EventBusService, EventBusService.Test());
}