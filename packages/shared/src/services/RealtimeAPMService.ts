import { Effect, Data, Ref, Schedule, Duration, Stream, Queue, Layer, Context, Runtime, Fiber } from "effect";
import { 
  APMError, 
  APMTrackingError, 
  APMSessionData, 
  APMStats,
  generateDeviceId,
  createInitialSessionData,
  trackMessage,
  trackSession,
  calculateStats,
} from "./SimpleAPMService";
import { getStoredJson, setStoredJson } from "./SimpleStorageService";

// Tagged error types for realtime APM operations
export class RealtimeAPMError extends Data.TaggedError("RealtimeAPMError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class APMCalculationError extends Data.TaggedError("APMCalculationError")<{
  calculationType: string;
  message: string;
  cause?: unknown;
}> {}

export class APMStreamError extends Data.TaggedError("APMStreamError")<{
  streamType: string;
  message: string;
  cause?: unknown;
}> {}

// Realtime APM data types
export interface RealtimeAPMData {
  currentAPM: number;
  trend: 'up' | 'down' | 'stable';
  sessionDuration: number;
  totalActions: number;
  lastUpdateTimestamp: number;
  isActive: boolean;
  deviceId: string;
}

export interface APMTrendData {
  previousAPM: number;
  currentAPM: number;
  trend: 'up' | 'down' | 'stable';
  trendPercentage: number;
}

export interface RealtimeAPMConfig {
  updateInterval: number; // in milliseconds, default: 3000
  trendThreshold: number; // minimum percentage change to show trend, default: 10
  maxHistorySize: number; // max number of APM readings to keep for trend calculation, default: 10
  enableTrendCalculation: boolean;
  enableStreaming: boolean;
}

// Default configuration
const DEFAULT_REALTIME_CONFIG: RealtimeAPMConfig = {
  updateInterval: 3000, // 3 seconds
  trendThreshold: 10, // 10% change threshold
  maxHistorySize: 10,
  enableTrendCalculation: true,
  enableStreaming: true,
};

// Context for RealtimeAPMService
export interface RealtimeAPMService {
  readonly config: RealtimeAPMConfig;
  readonly getCurrentAPM: Effect.Effect<RealtimeAPMData, RealtimeAPMError>;
  readonly subscribeToAPMUpdates: (
    callback: (data: RealtimeAPMData) => void
  ) => Effect.Effect<Fiber.Fiber<void, APMStreamError>, APMStreamError>;
  readonly calculateCurrentSessionAPM: Effect.Effect<number, APMCalculationError>;
  readonly calculateAPMTrend: (
    currentAPM: number,
    history: number[]
  ) => Effect.Effect<APMTrendData, APMCalculationError>;
  readonly startRealtimeAPMStream: (
    sessionDataRef: Ref.Ref<APMSessionData>,
    isActiveRef: Ref.Ref<boolean>
  ) => Effect.Effect<Stream.Stream<RealtimeAPMData, APMStreamError>, APMStreamError>;
}

export const RealtimeAPMService = Context.GenericTag<RealtimeAPMService>("RealtimeAPMService");

// APM history management
const createAPMHistory = () =>
  Effect.gen(function* () {
    const history = yield* getStoredJson<number[]>("realtime_apm_history", []).pipe(
      Effect.catchAll(() => Effect.succeed([]))
    );
    return yield* Ref.make(history);
  });

const updateAPMHistory = (historyRef: Ref.Ref<number[]>, newAPM: number, maxSize: number) =>
  Effect.gen(function* () {
    yield* Ref.update(historyRef, history => {
      const newHistory = [...history, newAPM];
      return newHistory.length > maxSize ? newHistory.slice(-maxSize) : newHistory;
    });
    
    const currentHistory = yield* Ref.get(historyRef);
    yield* setStoredJson("realtime_apm_history", currentHistory);
  });

// Core realtime APM calculation
const calculateCurrentSessionAPM = (
  sessionDataRef: Ref.Ref<APMSessionData>,
  isActive: boolean
): Effect.Effect<number, APMCalculationError> =>
  Effect.gen(function* () {
    const sessionData = yield* Ref.get(sessionDataRef);
    const now = Date.now();
    
    const duration = now - sessionData.sessionStart;
    const totalActions = sessionData.messagesSent + sessionData.sessionsCreated;
    
    if (duration <= 0) {
      return 0;
    }
    
    if (!isActive) {
      return 0; // Return 0 APM when not active
    }
    
    // Calculate APM: (total actions / duration in minutes)
    const apm = totalActions / (duration / 60000);
    
    return Math.max(0, Number(apm.toFixed(2)));
  }).pipe(
    Effect.catchAll(error => 
      Effect.fail(new APMCalculationError({
        calculationType: "currentSession",
        message: `Failed to calculate current session APM: ${error}`,
        cause: error
      }))
    )
  );

// APM trend calculation
const calculateAPMTrend = (
  currentAPM: number,
  history: number[],
  threshold: number
): Effect.Effect<APMTrendData, APMCalculationError> =>
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
    if (absChange >= threshold) {
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
        calculationType: "trend",
        message: `Failed to calculate APM trend: ${error}`,
        cause: error
      }))
    )
  );

// Get current realtime APM data
const getCurrentAPM = (
  sessionDataRef: Ref.Ref<APMSessionData>,
  isActiveRef: Ref.Ref<boolean>,
  historyRef: Ref.Ref<number[]>,
  config: RealtimeAPMConfig
): Effect.Effect<RealtimeAPMData, RealtimeAPMError> =>
  Effect.gen(function* () {
    const deviceId = yield* generateDeviceId();
    const isActive = yield* Ref.get(isActiveRef);
    const sessionData = yield* Ref.get(sessionDataRef);
    const history = yield* Ref.get(historyRef);
    
    const currentAPM = yield* calculateCurrentSessionAPM(sessionDataRef, isActive);
    
    const trendData = config.enableTrendCalculation
      ? yield* calculateAPMTrend(currentAPM, history, config.trendThreshold)
      : { trend: 'stable' as const, previousAPM: currentAPM, currentAPM, trendPercentage: 0 };
    
    const now = Date.now();
    const sessionDuration = now - sessionData.sessionStart;
    const totalActions = sessionData.messagesSent + sessionData.sessionsCreated;
    
    return {
      currentAPM,
      trend: trendData.trend,
      sessionDuration,
      totalActions,
      lastUpdateTimestamp: now,
      isActive,
      deviceId,
    };
  }).pipe(
    Effect.catchAll(error =>
      Effect.fail(new RealtimeAPMError({
        operation: "getCurrentAPM",
        message: `Failed to get current APM: ${error}`,
        cause: error
      }))
    )
  );

// Create APM update stream
const startRealtimeAPMStream = (
  sessionDataRef: Ref.Ref<APMSessionData>,
  isActiveRef: Ref.Ref<boolean>,
  config: RealtimeAPMConfig
): Effect.Effect<Stream.Stream<RealtimeAPMData, APMStreamError>, APMStreamError> =>
  Effect.gen(function* () {
    const historyRef = yield* createAPMHistory();
    
    const apmStream = Stream.repeatEffect(
      Effect.gen(function* () {
        const apmData = yield* getCurrentAPM(sessionDataRef, isActiveRef, historyRef, config);
        
        // Update history for trend calculations
        if (config.enableTrendCalculation) {
          yield* updateAPMHistory(historyRef, apmData.currentAPM, config.maxHistorySize);
        }
        
        yield* Effect.log(`📊 [RealtimeAPM] Current APM: ${apmData.currentAPM}, Trend: ${apmData.trend}`);
        
        return apmData;
      }).pipe(
        Effect.catchAll(error =>
          Effect.fail(new APMStreamError({
            streamType: "realtime",
            message: `Stream update failed: ${error}`,
            cause: error
          }))
        )
      )
    ).pipe(
      Stream.schedule(Schedule.fixed(Duration.millis(config.updateInterval)))
    );
    
    return apmStream;
  }).pipe(
    Effect.catchAll(error =>
      Effect.fail(new APMStreamError({
        streamType: "initialization",
        message: `Failed to start realtime APM stream: ${error}`,
        cause: error
      }))
    )
  );

// Subscribe to APM updates with callback
const subscribeToAPMUpdates = (
  sessionDataRef: Ref.Ref<APMSessionData>,
  isActiveRef: Ref.Ref<boolean>,
  config: RealtimeAPMConfig,
  callback: (data: RealtimeAPMData) => void
): Effect.Effect<Fiber.Fiber<void, APMStreamError>, APMStreamError> =>
  Effect.gen(function* () {
    const stream = yield* startRealtimeAPMStream(sessionDataRef, isActiveRef, config);
    
    const fiber = yield* Effect.fork(
      Stream.runForEach(stream, (apmData) =>
        Effect.sync(() => {
          callback(apmData);
        }).pipe(
          Effect.catchAll(error =>
            Effect.log(`APM callback error: ${error}`)
          )
        )
      )
    );
    
    yield* Effect.log('🔄 [RealtimeAPM] Started APM subscription');
    
    return fiber;
  });

// Implementation layer
export const makeRealtimeAPMService = (config: Partial<RealtimeAPMConfig> = {}): Layer.Layer<RealtimeAPMService> => {
  const finalConfig = { ...DEFAULT_REALTIME_CONFIG, ...config };
  
  return Layer.succeed(RealtimeAPMService, {
    config: finalConfig,
    getCurrentAPM: Effect.gen(function* () {
      // For standalone usage, create temporary refs
      const deviceId = yield* generateDeviceId().pipe(
        Effect.mapError((error) => new RealtimeAPMError({
          operation: "generateDeviceId",
          message: String(error),
          cause: error,
        }))
      );
      const sessionData = yield* createInitialSessionData(deviceId);
      const sessionDataRef = yield* Ref.make(sessionData);
      const isActiveRef = yield* Ref.make(true);
      const historyRef = yield* createAPMHistory().pipe(
        Effect.mapError((error) => new RealtimeAPMError({
          operation: "createAPMHistory",
          message: String(error),
          cause: error,
        }))
      );
      
      return yield* getCurrentAPM(sessionDataRef, isActiveRef, historyRef, finalConfig);
    }),
    subscribeToAPMUpdates: (callback: (data: RealtimeAPMData) => void) =>
      Effect.gen(function* () {
        // For standalone usage, create temporary refs
        const deviceId = yield* generateDeviceId().pipe(
          Effect.mapError((error) => new APMStreamError({
            streamType: "initialization",
            message: String(error),
            cause: error,
          }))
        );
        const sessionData = yield* createInitialSessionData(deviceId);
        const sessionDataRef = yield* Ref.make(sessionData);
        const isActiveRef = yield* Ref.make(true);
        
        return yield* subscribeToAPMUpdates(sessionDataRef, isActiveRef, finalConfig, callback);
      }),
    calculateCurrentSessionAPM: Effect.gen(function* () {
      // For standalone usage, create temporary refs
      const deviceId = yield* generateDeviceId().pipe(
        Effect.mapError((error) => new APMCalculationError({
          calculationType: "deviceIdGeneration",
          message: String(error),
          cause: error,
        }))
      );
      const sessionData = yield* createInitialSessionData(deviceId);
      const sessionDataRef = yield* Ref.make(sessionData);
      
      return yield* calculateCurrentSessionAPM(sessionDataRef, true);
    }),
    calculateAPMTrend: (currentAPM: number, history: number[]) =>
      calculateAPMTrend(currentAPM, history, finalConfig.trendThreshold),
    startRealtimeAPMStream: (sessionDataRef: Ref.Ref<APMSessionData>, isActiveRef: Ref.Ref<boolean>) =>
      startRealtimeAPMStream(sessionDataRef, isActiveRef, finalConfig),
  });
};

// Convenience functions for external usage
export const withRealtimeAPMService = <R, E, A>(
  effect: Effect.Effect<A, E, R | RealtimeAPMService>,
  config?: Partial<RealtimeAPMConfig>
): Effect.Effect<A, E, R> =>
  Effect.provide(effect, makeRealtimeAPMService(config));

// Export all functions for external usage
export {
  calculateCurrentSessionAPM,
  calculateAPMTrend,
  getCurrentAPM,
  startRealtimeAPMStream,
  subscribeToAPMUpdates,
  createAPMHistory,
  updateAPMHistory,
};