import { useState, useEffect, useCallback, useRef } from 'react';
import { Effect, Runtime, Ref } from 'effect';
import { 
  APMTrackingOptions,
  APMStats,
  APMSessionData,
  generateDeviceId,
  createInitialSessionData,
  trackMessage,
  trackSession,
  calculateStats,
  flushSessionData,
  startPeriodicSync,
  subscribeToAppStateChanges
} from '../services/SimpleAPMService';

interface UseSimpleAPMOptions extends APMTrackingOptions {
  enabled?: boolean;
}

interface UseSimpleAPMReturn {
  // Tracking functions
  trackMessageSent: () => void;
  trackSessionCreated: () => void;
  flushSessionData: () => Promise<void>;
  
  // State
  stats: APMStats | null;
  isEnabled: boolean;
  isActive: boolean;
  isLoading: boolean;
  error: string | null;
  
  // Compatibility with existing API
  syncNow: () => Promise<void>;
  sessionId: string;
  activityCount: number;
  isTracking: boolean;
}

const DEFAULT_OPTIONS: UseSimpleAPMOptions = {
  enabled: true,
  trackMessages: true,
  trackSessions: true,
  trackAppState: true,
  syncInterval: 5 * 60 * 1000, // 5 minutes
};

/**
 * Simplified React hook for APM tracking using Effect-TS patterns.
 * 
 * This provides the same API as the original useAPMTracking hook while using
 * Effect patterns for better resource management and error handling.
 */
export function useSimpleAPM(options: UseSimpleAPMOptions = {}): UseSimpleAPMReturn {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  
  const [stats, setStats] = useState<APMStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  
  const sessionDataRef = useRef<Ref.Ref<APMSessionData> | null>(null);
  const isActiveRef = useRef<Ref.Ref<boolean> | null>(null);
  const syncFiberRef = useRef<any>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const initializingRef = useRef(false);
  
  // Initialize APM service
  useEffect(() => {
    if (initializingRef.current) return;
    initializingRef.current = true;
    const initializeAPM = async () => {
      try {
        setIsLoading(true);
        setError(null);
        
        // Initialize device ID and session data
        const initProgram = Effect.gen(function* () {
          const deviceId = yield* generateDeviceId();
          const initialData = yield* createInitialSessionData(deviceId);
          
          const sessionDataRefValue = yield* Ref.make(initialData);
          const isActiveRefValue = yield* Ref.make(true);
          
          return { sessionDataRefValue, isActiveRefValue };
        });
        
        const { sessionDataRefValue, isActiveRefValue } = await Runtime.runPromise(Runtime.defaultRuntime)(initProgram);
        
        sessionDataRef.current = sessionDataRefValue;
        isActiveRef.current = isActiveRefValue;
        
        // Start periodic sync if enabled
        if (opts.enabled && opts.syncInterval) {
          const fiber = await Runtime.runPromise(Runtime.defaultRuntime)(
            startPeriodicSync(sessionDataRefValue, isActiveRefValue, opts.syncInterval)
          );
          syncFiberRef.current = fiber;
        }
        
        // Subscribe to app state changes if enabled
        if (opts.enabled && opts.trackAppState) {
          const cleanup = await Runtime.runPromise(Runtime.defaultRuntime)(
            subscribeToAppStateChanges(
              sessionDataRefValue, 
              isActiveRefValue,
              (state) => {
                console.log(`📱 [APM] App state changed to: ${state}`);
              }
            )
          );
          cleanupRef.current = typeof cleanup === 'function' ? cleanup : null;
        }
        
        setIsLoading(false);
        
      } catch (err) {
        console.error('Failed to initialize APM service:', err);
        setError(String(err));
        setIsLoading(false);
      }
    };
    
    initializeAPM();
    
    // Cleanup on unmount
    return () => {
      initializingRef.current = false;
      
      // Safely interrupt fiber if it exists
      if (syncFiberRef.current?.interrupt) {
        try {
          syncFiberRef.current.interrupt();
        } catch (error) {
          console.warn('Failed to interrupt APM sync fiber:', error);
        }
        syncFiberRef.current = null;
      }
      
      // Run cleanup function if it exists
      if (cleanupRef.current) {
        try {
          cleanupRef.current();
        } catch (error) {
          console.warn('Failed to run APM cleanup:', error);
        }
        cleanupRef.current = null;
      }
    };
  }, []);
  
  // Update stats periodically
  useEffect(() => {
    if (!sessionDataRef.current || !isActiveRef.current || isLoading) return;
    
    const updateStats = async () => {
      try {
        const isActive = await Runtime.runPromise(Runtime.defaultRuntime)(
          Ref.get(isActiveRef.current!)
        );
        
        const currentStats = await Runtime.runPromise(Runtime.defaultRuntime)(
          calculateStats(sessionDataRef.current!, isActive)
        );
        
        setStats(currentStats);
      } catch (err) {
        console.error('Failed to update APM stats:', err);
      }
    };
    
    // Update stats immediately and then every 5 seconds
    updateStats();
    const interval = setInterval(updateStats, 5000);
    
    return () => clearInterval(interval);
  }, [isLoading]);
  
  // Tracking functions
  const trackMessageSent = useCallback(() => {
    if (!sessionDataRef.current || !opts.enabled || !opts.trackMessages) return;
    
    Runtime.runPromise(Runtime.defaultRuntime)(
      trackMessage(sessionDataRef.current)
    ).catch(error => {
      console.error('Failed to track message:', error);
      setError(String(error));
    });
  }, [opts.enabled, opts.trackMessages]);
  
  const trackSessionCreated = useCallback(() => {
    if (!sessionDataRef.current || !opts.enabled || !opts.trackSessions) return;
    
    Runtime.runPromise(Runtime.defaultRuntime)(
      trackSession(sessionDataRef.current)
    ).catch(error => {
      console.error('Failed to track session:', error);
      setError(String(error));
    });
  }, [opts.enabled, opts.trackSessions]);
  
  const flushSessionDataFn = useCallback(async () => {
    if (!sessionDataRef.current || !isActiveRef.current) return;
    
    try {
      const isActive = await Runtime.runPromise(Runtime.defaultRuntime)(
        Ref.get(isActiveRef.current)
      );
      
      await Runtime.runPromise(Runtime.defaultRuntime)(
        flushSessionData(sessionDataRef.current, isActive)
      );
    } catch (err) {
      console.error('Failed to flush session data:', err);
      setError(String(err));
      throw err;
    }
  }, []);
  
  // Compute derived values
  const isEnabled = opts.enabled !== false;
  const isActive = stats?.isActive ?? false;
  const sessionId = stats?.sessionData.deviceId ?? '';
  const activityCount = stats?.totalActions ?? 0;
  const isTracking = isActive && isEnabled;
  
  return {
    // Tracking functions
    trackMessageSent,
    trackSessionCreated,
    flushSessionData: flushSessionDataFn,
    
    // State
    stats,
    isEnabled,
    isActive,
    isLoading,
    error,
    
    // Compatibility API
    syncNow: flushSessionDataFn,
    sessionId,
    activityCount,
    isTracking,
  };
}