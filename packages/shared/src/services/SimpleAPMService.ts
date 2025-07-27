import { Effect, Data, Ref, Schedule, Duration, Runtime } from "effect";
import { getStoredJson, setStoredJson } from "./SimpleStorageService";
import { isReactNative, getPlatformId } from '../utils/platform';

// Tagged error types for APM operations
export class APMError extends Data.TaggedError("APMError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class APMTrackingError extends Data.TaggedError("APMTrackingError")<{
  trackingType: string;
  message: string;
  cause?: unknown;
}> {}

// APM data types
export interface APMTrackingOptions {
  enabled?: boolean;
  trackMessages?: boolean;
  trackSessions?: boolean;
  trackAppState?: boolean;
  syncInterval?: number;
}

export interface APMSessionData {
  sessionStart: number;
  sessionEnd?: number;
  messagesSent: number;
  sessionsCreated: number;
  appStateChanges: number;
  deviceId: string;
  platform: string;
}

export interface APMStats {
  duration: number;
  totalActions: number;
  apm: number;
  isActive: boolean;
  sessionData: APMSessionData;
}

// Default configuration
const DEFAULT_OPTIONS: Required<APMTrackingOptions> = {
  enabled: true,
  trackMessages: true,
  trackSessions: true,
  trackAppState: true,
  syncInterval: 5 * 60 * 1000, // 5 minutes
};

// Simple APM service functions
export const generateDeviceId = () =>
  Effect.gen(function* () {
    const existingId = yield* getStoredJson("apm_device_id", null).pipe(
      Effect.catchAll(() => Effect.succeed(null))
    );
    
    if (existingId) {
      return existingId;
    }
    
    const platform = getPlatformId();
    const timestamp = Date.now();
    const random = Math.random().toString(36).substring(2, 15);
    const newId = `${platform}-${timestamp}-${random}`;
    
    yield* setStoredJson("apm_device_id", newId);
    
    return newId;
  });

export const createInitialSessionData = (deviceId: string) =>
  Effect.succeed<APMSessionData>({
    sessionStart: Date.now(),
    messagesSent: 0,
    sessionsCreated: 0,
    appStateChanges: 0,
    deviceId,
    platform: getPlatformId(),
  });

export const trackMessage = (sessionDataRef: Ref.Ref<APMSessionData>) =>
  Effect.gen(function* () {
    yield* Ref.update(sessionDataRef, data => ({
      ...data,
      messagesSent: data.messagesSent + 1
    }));
    
    const current = yield* Ref.get(sessionDataRef);
    yield* Effect.log(`📝 [APM] Message tracked, total: ${current.messagesSent}`);
  });

export const trackSession = (sessionDataRef: Ref.Ref<APMSessionData>) =>
  Effect.gen(function* () {
    yield* Ref.update(sessionDataRef, data => ({
      ...data,
      sessionsCreated: data.sessionsCreated + 1
    }));
    
    const current = yield* Ref.get(sessionDataRef);
    yield* Effect.log(`🆕 [APM] Session tracked, total: ${current.sessionsCreated}`);
  });

export const calculateStats = (sessionDataRef: Ref.Ref<APMSessionData>, isActive: boolean) =>
  Effect.gen(function* () {
    const sessionData = yield* Ref.get(sessionDataRef);
    const now = Date.now();
    
    const duration = now - sessionData.sessionStart;
    const totalActions = sessionData.messagesSent + sessionData.sessionsCreated;
    const apm = duration > 0 ? (totalActions / (duration / 60000)) : 0;
    
    return {
      duration,
      totalActions,
      apm,
      isActive,
      sessionData
    };
  });

export const sendSessionDataToBackend = (sessionData: APMSessionData) =>
  Effect.tryPromise({
    try: async () => {
      // Mock implementation - replace with actual Convex mutations
      await new Promise(resolve => setTimeout(resolve, 100));
      console.log('📊 [APM] Session data sent to backend:', {
        deviceId: sessionData.deviceId,
        duration: (sessionData.sessionEnd || Date.now()) - sessionData.sessionStart,
        actions: sessionData.messagesSent + sessionData.sessionsCreated,
      });
    },
    catch: (error) => new APMError({
      operation: "sendSessionData",
      message: String(error),
      cause: error
    })
  });

export const flushSessionData = (sessionDataRef: Ref.Ref<APMSessionData>, isActive: boolean) =>
  Effect.gen(function* () {
    const sessionData = yield* Ref.get(sessionDataRef);
    const now = Date.now();
    
    yield* Effect.log(`📊 [APM] Flushing session data for device: ${sessionData.deviceId}`);
    
    yield* sendSessionDataToBackend({
      ...sessionData,
      sessionEnd: isActive ? undefined : now
    });
    
    // Reset counters after successful sync
    yield* Ref.update(sessionDataRef, data => ({
      ...data,
      sessionStart: now,
      messagesSent: 0,
      sessionsCreated: 0,
    }));
  });

export const startPeriodicSync = (
  sessionDataRef: Ref.Ref<APMSessionData>, 
  isActiveRef: Ref.Ref<boolean>,
  intervalMs: number = DEFAULT_OPTIONS.syncInterval
) =>
  Effect.gen(function* () {
    const periodicSync = Effect.repeat(
      Effect.gen(function* () {
        const isActive = yield* Ref.get(isActiveRef);
        if (isActive) {
          yield* flushSessionData(sessionDataRef, isActive).pipe(
            Effect.catchAll(error => 
              Effect.log(`Periodic sync failed: ${error}`)
            )
          );
        }
      }),
      Schedule.fixed(Duration.millis(intervalMs))
    );
    
    const fiber = yield* Effect.fork(periodicSync);
    yield* Effect.log(`⏰ [APM] Started periodic sync every ${intervalMs}ms`);
    
    return fiber;
  });

// App state monitoring functions
export const subscribeToAppStateChanges = (
  sessionDataRef: Ref.Ref<APMSessionData>,
  isActiveRef: Ref.Ref<boolean>,
  onAppStateChange?: (state: string) => void
) =>
  Effect.gen(function* () {
    // Mobile app state monitoring
    if (isReactNative()) {
      return yield* Effect.tryPromise({
        try: async () => {
          const { AppState } = await import('react-native');
          
          const handleAppStateChange = (nextAppState: string) => {
            Effect.runSync(Effect.gen(function* () {
              const previousActive = yield* Ref.get(isActiveRef);
              const newIsActive = nextAppState === 'active';
              
              yield* Ref.set(isActiveRef, newIsActive);
              yield* Ref.update(sessionDataRef, data => ({
                ...data,
                appStateChanges: data.appStateChanges + 1
              }));
              
              if (onAppStateChange) {
                onAppStateChange(nextAppState);
              }
              
              if (previousActive && !newIsActive) {
                // Going to background - flush data
                yield* flushSessionData(sessionDataRef, false).pipe(
                  Effect.catchAll(error => 
                    Effect.log(`Failed to flush on background: ${error}`)
                  )
                );
              } else if (!previousActive && newIsActive) {
                // Becoming active - start new session period
                yield* Ref.update(sessionDataRef, data => ({
                  ...data,
                  sessionStart: Date.now()
                }));
              }
            }));
          };
          
          const subscription = AppState.addEventListener('change', handleAppStateChange);
          
          return () => subscription?.remove?.();
        },
        catch: (error) => new APMError({
          operation: "subscribeToAppStateChanges",
          message: String(error),
          cause: error
        })
      });
    } else {
      // Web/Desktop visibility API
      return yield* Effect.sync(() => {
        const handleVisibilityChange = () => {
          const state = document.visibilityState === 'visible' ? 'active' : 'background';
          
          Effect.runSync(Effect.gen(function* () {
            const previousActive = yield* Ref.get(isActiveRef);
            const newIsActive = state === 'active';
            
            yield* Ref.set(isActiveRef, newIsActive);
            yield* Ref.update(sessionDataRef, data => ({
              ...data,
              appStateChanges: data.appStateChanges + 1
            }));
            
            if (onAppStateChange) {
              onAppStateChange(state);
            }
            
            if (previousActive && !newIsActive) {
              yield* flushSessionData(sessionDataRef, false).pipe(
                Effect.catchAll(error => 
                  Effect.log(`Failed to flush on background: ${error}`)
                )
              );
            } else if (!previousActive && newIsActive) {
              yield* Ref.update(sessionDataRef, data => ({
                ...data,
                sessionStart: Date.now()
              }));
            }
          }));
        };
        
        document.addEventListener('visibilitychange', handleVisibilityChange);
        
        return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
      });
    }
  });