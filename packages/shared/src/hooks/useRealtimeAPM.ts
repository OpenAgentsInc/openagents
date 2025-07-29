import { useEffect, useState, useCallback, useRef } from 'react';
import { Effect, Runtime, Fiber, Ref } from 'effect';
import {
  RealtimeAPMService,
  RealtimeAPMData,
  RealtimeAPMError,
  RealtimeAPMConfig,
  makeRealtimeAPMService,
  APMStreamError,
} from '../services/RealtimeAPMService';
import {
  APMSessionData,
  generateDeviceId,
  createInitialSessionData,
} from '../services/SimpleAPMService';

// Hook configuration interface
export interface UseRealtimeAPMConfig extends Partial<RealtimeAPMConfig> {
  enabled?: boolean;
  onError?: (error: RealtimeAPMError | APMStreamError) => void;
  onAPMUpdate?: (data: RealtimeAPMData) => void;
}

// Hook state interface
export interface RealtimeAPMState {
  data: RealtimeAPMData | null;
  isLoading: boolean;
  error: RealtimeAPMError | APMStreamError | null;
  isSubscribed: boolean;
}

// Default configuration for the hook
const DEFAULT_HOOK_CONFIG: Required<UseRealtimeAPMConfig> = {
  enabled: true,
  updateInterval: 3000,
  trendThreshold: 10,
  maxHistorySize: 10,
  enableTrendCalculation: true,
  enableStreaming: true,
  onError: () => {},
  onAPMUpdate: () => {},
};

/**
 * Hook for realtime APM tracking and updates
 * Integrates with Effect-TS RealtimeAPMService to provide live APM data
 */
export function useRealtimeAPM(config: UseRealtimeAPMConfig = {}): {
  state: RealtimeAPMState;
  getCurrentAPM: () => Promise<RealtimeAPMData | null>;
  trackMessage: () => void;
  trackSession: () => void;
  setActive: (active: boolean) => void;
  subscribe: () => void;
  unsubscribe: () => void;
} {
  const finalConfig = { ...DEFAULT_HOOK_CONFIG, ...config };
  
  // Hook state
  const [state, setState] = useState<RealtimeAPMState>({
    data: null,
    isLoading: true,
    error: null,
    isSubscribed: false,
  });
  
  // Refs for Effect-TS integration
  const runtimeRef = useRef<Runtime.Runtime<never> | null>(null);
  const sessionDataRef = useRef<Ref.Ref<APMSessionData> | null>(null);
  const isActiveRef = useRef<Ref.Ref<boolean> | null>(null);
  const subscriptionFiberRef = useRef<Fiber.Fiber<void, APMStreamError> | null>(null);
  const deviceIdRef = useRef<string | null>(null);
  
  // Initialize runtime and refs
  useEffect(() => {
    if (!finalConfig.enabled) return;
    
    const initializeRealtime = async () => {
      try {
        // Create Effect runtime with RealtimeAPMService layer
        const serviceLayer = makeRealtimeAPMService({
          updateInterval: finalConfig.updateInterval,
          trendThreshold: finalConfig.trendThreshold,
          maxHistorySize: finalConfig.maxHistorySize,
          enableTrendCalculation: finalConfig.enableTrendCalculation,
          enableStreaming: finalConfig.enableStreaming,
        });
        
        runtimeRef.current = Runtime.defaultRuntime;
        
        // Initialize device ID and session data
        const deviceIdEffect = generateDeviceId();
        const deviceId = await Runtime.runPromise(runtimeRef.current)(deviceIdEffect);
        deviceIdRef.current = deviceId;
        
        const sessionDataEffect = createInitialSessionData(deviceId);
        const sessionData = await Runtime.runPromise(runtimeRef.current)(sessionDataEffect);
        
        // Create refs for session tracking
        const sessionDataRefEffect = Ref.make(sessionData);
        const isActiveRefEffect = Ref.make(true);
        
        sessionDataRef.current = await Runtime.runPromise(runtimeRef.current)(sessionDataRefEffect);
        isActiveRef.current = await Runtime.runPromise(runtimeRef.current)(isActiveRefEffect);
        
        setState(prev => ({ ...prev, isLoading: false }));
        
      } catch (error) {
        console.error('Failed to initialize realtime APM:', error);
        setState(prev => ({
          ...prev,
          isLoading: false,
          error: error as RealtimeAPMError,
        }));
        finalConfig.onError(error as RealtimeAPMError);
      }
    };
    
    initializeRealtime();
    
    return () => {
      // Cleanup on unmount
      if (subscriptionFiberRef.current && runtimeRef.current) {
        Runtime.runSync(runtimeRef.current)(Fiber.interrupt(subscriptionFiberRef.current));
      }
    };
  }, [finalConfig.enabled]);
  
  // Get current APM data
  const getCurrentAPM = useCallback(async (): Promise<RealtimeAPMData | null> => {
    if (!runtimeRef.current || !sessionDataRef.current || !isActiveRef.current) {
      return null;
    }
    
    try {
      const serviceLayer = makeRealtimeAPMService(finalConfig);
      
      const getCurrentAPMEffect = Effect.gen(function* () {
        const service = yield* RealtimeAPMService;
        return yield* service.getCurrentAPM;
      });
      
      const result = await Runtime.runPromise(runtimeRef.current)(
        Effect.provide(getCurrentAPMEffect, serviceLayer)
      );
      
      setState(prev => ({ ...prev, data: result, error: null }));
      
      return result;
    } catch (error) {
      console.error('Failed to get current APM:', error);
      const apmError = error as RealtimeAPMError;
      setState(prev => ({ ...prev, error: apmError }));
      finalConfig.onError(apmError);
      return null;
    }
  }, [finalConfig]);
  
  // Track message action
  const trackMessage = useCallback(() => {
    if (!runtimeRef.current || !sessionDataRef.current) return;
    
    const trackMessageEffect = Effect.gen(function* () {
      yield* Ref.update(sessionDataRef.current!, data => ({
        ...data,
        messagesSent: data.messagesSent + 1
      }));
    });
    
    Runtime.runSync(runtimeRef.current)(trackMessageEffect);
  }, []);
  
  // Track session action
  const trackSession = useCallback(() => {
    if (!runtimeRef.current || !sessionDataRef.current) return;
    
    const trackSessionEffect = Effect.gen(function* () {
      yield* Ref.update(sessionDataRef.current!, data => ({
        ...data,
        sessionsCreated: data.sessionsCreated + 1
      }));
    });
    
    Runtime.runSync(runtimeRef.current)(trackSessionEffect);
  }, []);
  
  // Set active state
  const setActive = useCallback((active: boolean) => {
    if (!runtimeRef.current || !isActiveRef.current) return;
    
    const setActiveEffect = Ref.set(isActiveRef.current, active);
    Runtime.runSync(runtimeRef.current)(setActiveEffect);
  }, []);
  
  // Subscribe to realtime updates
  const subscribe = useCallback(() => {
    if (!runtimeRef.current || !sessionDataRef.current || !isActiveRef.current) {
      console.warn('Cannot subscribe: Runtime or refs not initialized');
      return;
    }
    
    if (subscriptionFiberRef.current) {
      console.warn('Already subscribed to APM updates');
      return;
    }
    
    const subscribeEffect = Effect.gen(function* () {
      const serviceLayer = makeRealtimeAPMService(finalConfig);
      
      const subscriptionEffect = Effect.gen(function* () {
        const service = yield* RealtimeAPMService;
        
        return yield* service.subscribeToAPMUpdates((data: RealtimeAPMData) => {
          setState(prev => ({ 
            ...prev, 
            data, 
            error: null,
            isSubscribed: true 
          }));
          finalConfig.onAPMUpdate(data);
        });
      });
      
      return yield* Effect.provide(subscriptionEffect, serviceLayer);
    });
    
    Runtime.runPromise(runtimeRef.current)(subscribeEffect)
      .then(fiber => {
        subscriptionFiberRef.current = fiber;
        setState(prev => ({ ...prev, isSubscribed: true }));
        console.log('📊 [useRealtimeAPM] Subscribed to APM updates');
      })
      .catch(error => {
        console.error('Failed to subscribe to APM updates:', error);
        const streamError = error as APMStreamError;
        setState(prev => ({ ...prev, error: streamError }));
        finalConfig.onError(streamError);
      });
  }, [finalConfig]);
  
  // Unsubscribe from realtime updates
  const unsubscribe = useCallback(() => {
    if (!runtimeRef.current || !subscriptionFiberRef.current) {
      return;
    }
    
    const interruptEffect = Fiber.interrupt(subscriptionFiberRef.current);
    
    Runtime.runSync(runtimeRef.current)(interruptEffect);
    subscriptionFiberRef.current = null;
    setState(prev => ({ ...prev, isSubscribed: false }));
    
    console.log('📊 [useRealtimeAPM] Unsubscribed from APM updates');
  }, []);
  
  // Auto-subscribe when enabled
  useEffect(() => {
    if (finalConfig.enabled && finalConfig.enableStreaming && !state.isLoading && !state.isSubscribed) {
      subscribe();
    }
    
    return () => {
      if (state.isSubscribed) {
        unsubscribe();
      }
    };
  }, [finalConfig.enabled, finalConfig.enableStreaming, state.isLoading, state.isSubscribed, subscribe, unsubscribe]);
  
  return {
    state,
    getCurrentAPM,
    trackMessage,
    trackSession,
    setActive,
    subscribe,
    unsubscribe,
  };
}

/**
 * Simplified hook for just getting current APM without streaming
 * Useful for components that only need occasional APM checks
 */
export function useCurrentAPM(config: UseRealtimeAPMConfig = {}): {
  currentAPM: number | null;
  isLoading: boolean;
  error: RealtimeAPMError | null;
  refresh: () => Promise<void>;
} {
  const [currentAPM, setCurrentAPM] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<RealtimeAPMError | null>(null);
  
  const { getCurrentAPM } = useRealtimeAPM({
    ...config,
    enableStreaming: false, // Disable streaming for this simplified hook
  });
  
  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      const apmData = await getCurrentAPM();
      setCurrentAPM(apmData?.currentAPM ?? null);
    } catch (err) {
      const apmError = err as RealtimeAPMError;
      setError(apmError);
      config.onError?.(apmError);
    } finally {
      setIsLoading(false);
    }
  }, [getCurrentAPM, config]);
  
  // Initial load
  useEffect(() => {
    if (config.enabled !== false) {
      refresh();
    }
  }, [config.enabled, refresh]);
  
  return {
    currentAPM,
    isLoading,
    error,
    refresh,
  };
}