import { Effect, Data, Layer, Context, Stream, Duration, Schedule, Runtime, Fiber } from "effect";
import {
  RealtimeAPMService,
  RealtimeAPMData,
  RealtimeAPMError,
  RealtimeAPMConfig,
  APMStreamError,
  APMCalculationError,
  APMTrendData,
} from "./RealtimeAPMService";
import { generateDeviceId } from "./SimpleAPMService";

// Tagged error types for Convex integration
export class ConvexAPMError extends Data.TaggedError("ConvexAPMError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class ConvexConnectionError extends Data.TaggedError("ConvexConnectionError")<{
  endpoint: string;
  message: string;
  cause?: unknown;
}> {}

// Convex client interface (to be provided by consuming app)
export interface ConvexClient {
  query: (name: string, args?: any) => Promise<any>;
  mutation: (name: string, args?: any) => Promise<any>;
  subscribe: (name: string, args: any, callback: (data: any) => void) => () => void;
}

export interface ConvexRealtimeAPMService extends RealtimeAPMService {
  readonly trackAction: (
    actionType: "message" | "session" | "tool" | "github",
    metadata?: any
  ) => Effect.Effect<{ newAPM: number; totalActions: number }, ConvexAPMError>;
  readonly syncWithBackend: Effect.Effect<void, ConvexAPMError>;
  readonly subscribeToConvexUpdates: (
    callback: (data: RealtimeAPMData) => void
  ) => Effect.Effect<() => void, ConvexAPMError>;
}

export const ConvexRealtimeAPMService = Context.GenericTag<ConvexRealtimeAPMService>("ConvexRealtimeAPMService");

// Helper functions for Convex integration
const trackActionInConvex = (
  convexClient: ConvexClient,
  deviceId: string,
  actionType: "message" | "session" | "tool" | "github",
  metadata?: any
): Effect.Effect<{ newAPM: number; totalActions: number }, ConvexAPMError> =>
  Effect.gen(function* () {
    try {
      const result = yield* Effect.tryPromise({
        try: () => convexClient.mutation("confect.apm.trackRealtimeAction", {
          deviceId,
          actionType,
          timestamp: Date.now(),
          metadata,
        }),
        catch: (error) => new ConvexAPMError({
          operation: "trackAction",
          message: `Failed to track action in Convex: ${error}`,
          cause: error,
        }),
      });

      if (!result.success) {
        return yield* Effect.fail(new ConvexAPMError({
          operation: "trackAction",
          message: "Convex mutation returned success: false",
        }));
      }

      return {
        newAPM: result.newAPM,
        totalActions: result.totalActions,
      };
    } catch (error) {
      return yield* Effect.fail(new ConvexAPMError({
        operation: "trackAction",
        message: `Unexpected error: ${error}`,
        cause: error,
      }));
    }
  });

const getRealtimeAPMFromConvex = (
  convexClient: ConvexClient,
  deviceId?: string,
  includeHistory?: boolean
): Effect.Effect<RealtimeAPMData | null, ConvexAPMError> =>
  Effect.gen(function* () {
    try {
      const result = yield* Effect.tryPromise({
        try: () => convexClient.query("confect.apm.getRealtimeAPM", {
          deviceId,
          includeHistory: includeHistory ?? false,
        }),
        catch: (error) => new ConvexAPMError({
          operation: "getRealtimeAPM",
          message: `Failed to get realtime APM from Convex: ${error}`,
          cause: error,
        }),
      });

      return result;
    } catch (error) {
      return yield* Effect.fail(new ConvexAPMError({
        operation: "getRealtimeAPM",
        message: `Unexpected error: ${error}`,
        cause: error,
      }));
    }
  });

const subscribeToConvexAPMUpdates = (
  convexClient: ConvexClient,
  deviceId: string,
  callback: (data: RealtimeAPMData) => void
): Effect.Effect<() => void, ConvexAPMError> =>
  Effect.gen(function* () {
    try {
      const unsubscribe = yield* Effect.sync(() => {
        return convexClient.subscribe(
          "confect.apm.getRealtimeAPM",
          { deviceId, includeHistory: false },
          (data: RealtimeAPMData | null) => {
            if (data) {
              callback(data);
            }
          }
        );
      });

      yield* Effect.log(`🔄 [ConvexRealtimeAPM] Subscribed to Convex APM updates for device: ${deviceId}`);

      return unsubscribe;
    } catch (error) {
      return yield* Effect.fail(new ConvexAPMError({
        operation: "subscribeToConvexUpdates",
        message: `Failed to subscribe to Convex updates: ${error}`,
        cause: error,
      }));
    }
  });

// Create the Convex-integrated RealtimeAPMService
export const makeConvexRealtimeAPMService = (
  convexClient: ConvexClient,
  config: Partial<RealtimeAPMConfig> = {}
): Layer.Layer<ConvexRealtimeAPMService> => {
  const defaultConfig: RealtimeAPMConfig = {
    updateInterval: 3000,
    trendThreshold: 10,
    maxHistorySize: 10,
    enableTrendCalculation: true,
    enableStreaming: true,
  };

  const finalConfig = { ...defaultConfig, ...config };

  return Layer.succeed(ConvexRealtimeAPMService, {
    config: finalConfig,

    // Base RealtimeAPMService methods - these use Convex as the source of truth
    getCurrentAPM: Effect.gen(function* () {
      const deviceId = yield* generateDeviceId().pipe(
        Effect.mapError((error) => new RealtimeAPMError({
          operation: "generateDeviceId",
          message: String(error),
          cause: error,
        }))
      );
      const data = yield* getRealtimeAPMFromConvex(convexClient, deviceId, true).pipe(
        Effect.mapError((error) => new RealtimeAPMError({
          operation: "getCurrentAPM",
          message: String(error),
          cause: error,
        }))
      );
      
      if (!data) {
        return yield* Effect.fail(new RealtimeAPMError({
          operation: "getCurrentAPM",
          message: "No APM data available from Convex",
        }));
      }

      return data;
    }),

    subscribeToAPMUpdates: (callback: (data: RealtimeAPMData) => void) =>
      Effect.gen(function* () {
        const deviceId = yield* generateDeviceId().pipe(
          Effect.mapError((error) => new APMStreamError({
            streamType: "initialization",
            message: String(error),
            cause: error,
          }))
        );
        
        // Create a stream that polls Convex for updates
        const apmStream = Stream.repeatEffect(
          Effect.gen(function* () {
            const data = yield* getRealtimeAPMFromConvex(convexClient, deviceId, false);
            
            if (data) {
              return data;
            }
            
            // Return a default data structure if no data available
            return {
              currentAPM: 0,
              trend: 'stable' as const,
              sessionDuration: 0,
              totalActions: 0,
              lastUpdateTimestamp: Date.now(),
              isActive: false,
              deviceId,
            };
          }).pipe(
            Effect.catchAll(error =>
              Effect.fail(new APMStreamError({
                streamType: "convex",
                message: `Convex polling failed: ${error}`,
                cause: error,
              }))
            )
          )
        ).pipe(
          Stream.schedule(Schedule.fixed(Duration.millis(finalConfig.updateInterval)))
        );

        const fiber = yield* Effect.fork(
          Stream.runForEach(apmStream, (apmData) =>
            Effect.sync(() => {
              callback(apmData);
            }).pipe(
              Effect.catchAll(error =>
                Effect.log(`APM callback error: ${error}`)
              )
            )
          )
        );

        yield* Effect.log('🔄 [ConvexRealtimeAPM] Started Convex APM subscription');

        return fiber;
      }),

    calculateCurrentSessionAPM: Effect.gen(function* () {
      const deviceId = yield* generateDeviceId().pipe(
        Effect.mapError((error) => new APMCalculationError({
          calculationType: "deviceIdGeneration",
          message: String(error),
          cause: error,
        }))
      );
      const data = yield* getRealtimeAPMFromConvex(convexClient, deviceId, false).pipe(
        Effect.mapError((error) => new APMCalculationError({
          calculationType: "convexQuery",
          message: String(error),
          cause: error,
        }))
      );
      
      return data?.currentAPM ?? 0;
    }),

    calculateAPMTrend: (currentAPM: number, history: number[]) =>
      Effect.gen(function* () {
        if (history.length === 0) {
          return {
            previousAPM: currentAPM,
            currentAPM,
            trend: 'stable' as const,
            trendPercentage: 0,
          };
        }

        const previousAPM = history[history.length - 1];
        
        if (previousAPM === 0) {
          return {
            previousAPM,
            currentAPM,
            trend: (currentAPM > 0 ? 'up' : 'stable') as 'up' | 'stable',
            trendPercentage: currentAPM > 0 ? 100 : 0,
          };
        }

        const percentageChange = ((currentAPM - previousAPM) / previousAPM) * 100;
        const absChange = Math.abs(percentageChange);
        
        let trend: 'up' | 'down' | 'stable' = 'stable';
        if (absChange >= finalConfig.trendThreshold) {
          trend = percentageChange > 0 ? 'up' : 'down';
        }

        return {
          previousAPM,
          currentAPM,
          trend: trend as 'up' | 'down' | 'stable',
          trendPercentage: Number(percentageChange.toFixed(1)),
        };
      }).pipe(
        Effect.catchAll(error =>
          Effect.fail(new APMCalculationError({
            calculationType: "convexTrend",
            message: `Failed to calculate APM trend: ${error}`,
            cause: error,
          }))
        )
      ),

    startRealtimeAPMStream: (sessionDataRef, isActiveRef) =>
      Effect.gen(function* () {
        // For Convex integration, we don't use local refs
        // Instead, we create a stream that polls Convex
        const deviceId = yield* generateDeviceId().pipe(
          Effect.mapError((error) => new APMStreamError({
            streamType: "initialization",
            message: String(error),
            cause: error,
          }))
        );
        
        const apmStream = Stream.repeatEffect(
          Effect.gen(function* () {
            const data = yield* getRealtimeAPMFromConvex(convexClient, deviceId, true).pipe(
              Effect.mapError((error) => new APMStreamError({
                streamType: "convex",
                message: String(error),
                cause: error,
              }))
            );
            
            if (!data) {
              return yield* Effect.fail(new APMStreamError({
                streamType: "convex",
                message: "No data available from Convex",
              }));
            }

            yield* Effect.log(`📊 [ConvexRealtimeAPM] Stream update: ${data.currentAPM} APM, ${data.trend} trend`);

            return data;
          })
        ).pipe(
          Stream.schedule(Schedule.fixed(Duration.millis(finalConfig.updateInterval)))
        );

        return apmStream;
      }),

    // Extended methods specific to Convex integration
    trackAction: (actionType: "message" | "session" | "tool" | "github", metadata?: any) =>
      Effect.gen(function* () {
        const deviceId = yield* generateDeviceId().pipe(
          Effect.mapError((error) => new ConvexAPMError({
            operation: "generateDeviceId",
            message: String(error),
            cause: error,
          }))
        );
        const result = yield* trackActionInConvex(convexClient, deviceId, actionType, metadata);
        
        yield* Effect.log(`📊 [ConvexRealtimeAPM] Tracked ${actionType} action, new APM: ${result.newAPM}`);
        
        return result;
      }),

    syncWithBackend: Effect.gen(function* () {
      // For Convex integration, sync is handled automatically
      // This method can be used for any manual sync operations if needed
      yield* Effect.log('📊 [ConvexRealtimeAPM] Backend sync completed (automatic with Convex)');
    }),

    subscribeToConvexUpdates: (callback: (data: RealtimeAPMData) => void) =>
      Effect.gen(function* () {
        const deviceId = yield* generateDeviceId().pipe(
          Effect.mapError((error) => new ConvexAPMError({
            operation: "generateDeviceId",
            message: String(error),
            cause: error,
          }))
        );
        return yield* subscribeToConvexAPMUpdates(convexClient, deviceId, callback);
      }),
  });
};

// Convenience wrapper for external usage
export const withConvexRealtimeAPMService = <R, E, A>(
  effect: Effect.Effect<A, E, R | ConvexRealtimeAPMService>,
  convexClient: ConvexClient,
  config?: Partial<RealtimeAPMConfig>
): Effect.Effect<A, E, R> =>
  Effect.provide(effect, makeConvexRealtimeAPMService(convexClient, config));

// Export utility functions
export {
  trackActionInConvex,
  getRealtimeAPMFromConvex,
  subscribeToConvexAPMUpdates,
};