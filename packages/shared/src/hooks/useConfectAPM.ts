import { useState, useEffect, useCallback, useRef } from 'react';
import { Effect, Runtime, Ref } from 'effect';
import { 
  APMTrackingOptions,
  APMSessionData,
  APMStats,
  generateDeviceId,
  createInitialSessionData,
  trackMessage,
  trackSession,
  calculateStats,
  startPeriodicSync,
  subscribeToAppStateChanges,
} from '../services/SimpleAPMService';
import type { AuthState } from '../services/SimpleAuthService';
// Import types only to avoid runtime dependency issues
interface ConfectIntegrationConfig {
  convexUrl: string;
  enableRealTimeSync?: boolean;
  debugMode?: boolean;
}

interface UseConfectAPMConfig extends APMTrackingOptions {
  convexUrl: string;
  enableRealTimeSync?: boolean;
  authState?: AuthState; // Provide auth state for user-scoped tracking
}

interface UseConfectAPMReturn {
  // APM state (same as useSimpleAPM)
  stats: APMStats | null;
  isActive: boolean;
  isLoading: boolean;
  error: string | null;
  
  // APM actions (enhanced with Confect)
  trackMessageSent: () => Promise<void>;
  trackSessionCreated: () => Promise<void>;
  syncNow: () => Promise<void>;
  clearError: () => void;
  
  // Confect-specific features
  getUserAPMStats: (includeDeviceBreakdown?: boolean) => Promise<any>;
  calculateBackendAPM: (timeWindow?: string) => Promise<void>;
  subscribeToSessionUpdates: (onUpdate: (sessions: any[]) => void) => () => void;
}

/**
 * Enhanced React hook for APM tracking using Effect-TS patterns with Confect integration.
 * 
 * Extends useSimpleAPM with automatic backend synchronization and real-time
 * cross-device APM calculation through Confect Effect-powered functions.
 */
export function useConfectAPM(config: UseConfectAPMConfig): UseConfectAPMReturn {
  const [stats, setStats] = useState<APMStats | null>(null);
  const [isActive, setIsActive] = useState(true);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  
  const sessionDataRef = useRef<Ref.Ref<APMSessionData> | null>(null);
  const isActiveRef = useRef<Ref.Ref<boolean> | null>(null);
  const confectServicesRef = useRef<any>(null);
  const periodicSyncFiberRef = useRef<any>(null);
  const initializingRef = useRef(false);
  
  // Initialize APM service with Confect integration
  useEffect(() => {
    if (initializingRef.current) return;
    initializingRef.current = true;
    
    const initializeAPM = async () => {
      try {
        setError(null);
        
        const initProgram = Effect.gen(function* () {
          // Generate device ID
          const deviceId = yield* generateDeviceId();
          
          // Create session data ref
          const sessionData = yield* createInitialSessionData(deviceId);
          const sessionDataRefValue = yield* Ref.make(sessionData);
          
          // Create active state ref
          const isActiveRefValue = yield* Ref.make(true);
          
          // Initialize Confect integration (handle import gracefully)
          const confectIntegration = yield* Effect.tryPromise({
            try: () => import('../../../convex/confect/integration'),
            catch: (error) => new Error(`Failed to load Confect integration: ${error}`)
          });
          
          const confectServices = yield* confectIntegration.createIntegratedEffectServices({
            convexUrl: config.convexUrl,
            enableRealTimeSync: config.enableRealTimeSync,
          });
          
          // Start periodic sync with enhanced backend integration
          const enhancedFlushData = (sessionDataRef: Ref.Ref<APMSessionData>, isActive: boolean) =>
            Effect.gen(function* () {
              const sessionData = yield* Ref.get(sessionDataRef);
              
              // Use enhanced backend sync that includes user association
              yield* confectServices.syncAPMData(sessionData, config.authState);
              
              // Reset counters after successful sync
              yield* Ref.update(sessionDataRef, data => ({
                ...data,
                sessionStart: Date.now(),
                messagesSent: 0,
                sessionsCreated: 0,
              }));
            });
          
          const periodicSyncFiber = yield* startPeriodicSync(
            sessionDataRefValue,
            isActiveRefValue,
            config.syncInterval
          );
          
          // Subscribe to app state changes
          const appStateCleanup = yield* subscribeToAppStateChanges(
            sessionDataRefValue,
            isActiveRefValue,
            (state: string) => {
              setIsActive(state === 'active');
            }
          );
          
          return {
            sessionDataRefValue,
            isActiveRefValue,
            confectServices,
            periodicSyncFiber,
            appStateCleanup,
            deviceId
          };
        });
        
        const {
          sessionDataRefValue,
          isActiveRefValue,
          confectServices,
          periodicSyncFiber,
          deviceId
        } = await Runtime.runPromise(Runtime.defaultRuntime)(initProgram);
        
        sessionDataRef.current = sessionDataRefValue;
        isActiveRef.current = isActiveRefValue;
        confectServicesRef.current = confectServices;
        periodicSyncFiberRef.current = periodicSyncFiber;
        
        // Initial stats calculation
        const initialStats = await Runtime.runPromise(Runtime.defaultRuntime)(
          calculateStats(sessionDataRefValue, true)
        );
        setStats(initialStats);
        setIsLoading(false);
        
        console.log(`📊 [CONFECT-APM] Initialized with device ID: ${deviceId}`);
        
      } catch (err) {
        console.error('Failed to initialize Confect APM service:', err);
        setError(String(err));
        setIsLoading(false);
      }
    };
    
    initializeAPM();
    
    // Cleanup on unmount
    return () => {
      initializingRef.current = false;
      
      if (periodicSyncFiberRef.current) {
        try {
          // Interrupt the periodic sync fiber
          Runtime.runSync(Runtime.defaultRuntime)(periodicSyncFiberRef.current.interrupt);
        } catch (err) {
          console.warn('Failed to cleanup periodic sync:', err);
        }
      }
    };
  }, [config.convexUrl, config.syncInterval, config.authState?.user?.id]);
  
  // Enhanced track message with immediate stats update
  const trackMessageSent = useCallback(async () => {
    if (!sessionDataRef.current || !isActiveRef.current) {
      throw new Error('APM service not initialized');
    }
    
    try {
      const updateProgram = Effect.gen(function* () {
        yield* trackMessage(sessionDataRef.current!);
        const newStats = yield* calculateStats(sessionDataRef.current!, true);
        return newStats;
      });
      
      const newStats = await Runtime.runPromise(Runtime.defaultRuntime)(updateProgram);
      setStats(newStats);
      
    } catch (err) {
      console.error('❌ [CONFECT-APM] Failed to track message:', err);
      setError(String(err));
    }
  }, []);
  
  // Enhanced track session with immediate stats update
  const trackSessionCreated = useCallback(async () => {
    if (!sessionDataRef.current || !isActiveRef.current) {
      throw new Error('APM service not initialized');
    }
    
    try {
      const updateProgram = Effect.gen(function* () {
        yield* trackSession(sessionDataRef.current!);
        const newStats = yield* calculateStats(sessionDataRef.current!, true);
        return newStats;
      });
      
      const newStats = await Runtime.runPromise(Runtime.defaultRuntime)(updateProgram);
      setStats(newStats);
      
    } catch (err) {
      console.error('❌ [CONFECT-APM] Failed to track session:', err);
      setError(String(err));
    }
  }, []);
  
  // Enhanced sync now with Confect backend
  const syncNow = useCallback(async () => {
    if (!sessionDataRef.current || !confectServicesRef.current) {
      throw new Error('APM service not initialized');
    }
    
    try {
      const sessionData = await Runtime.runPromise(Runtime.defaultRuntime)(
        Ref.get(sessionDataRef.current)
      );
      
      await Runtime.runPromise(Runtime.defaultRuntime)(
        confectServicesRef.current.syncAPMData(sessionData, config.authState)
      );
      
      console.log('✅ [CONFECT-APM] Manual sync completed');
      
    } catch (err) {
      console.error('❌ [CONFECT-APM] Manual sync failed:', err);
      setError(String(err));
    }
  }, [config.authState]);
  
  // Get user APM stats from Confect backend
  const getUserAPMStats = useCallback(async (includeDeviceBreakdown: boolean = false) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    return await Runtime.runPromise(Runtime.defaultRuntime)(
      confectServicesRef.current.getUserAPMStats(includeDeviceBreakdown)
    );
  }, []);
  
  // Trigger backend APM calculation
  const calculateBackendAPM = useCallback(async (timeWindow: string = "1h") => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    // This would trigger the calculateUserAPM mutation on the backend
    await Runtime.runPromise(Runtime.defaultRuntime)(
      Effect.tryPromise({
        try: async () => {
          await confectServicesRef.current.client.mutation("calculateUserAPM", {
            timeWindow: timeWindow as any
          });
        },
        catch: (error) => error
      })
    );
  }, []);
  
  // Subscribe to real-time session updates
  const subscribeToSessionUpdates = useCallback((onUpdate: (sessions: any[]) => void) => {
    if (!confectServicesRef.current) {
      throw new Error('Confect services not initialized');
    }
    
    return confectServicesRef.current.subscribeToSessions(onUpdate);
  }, []);
  
  // Clear error function
  const clearError = useCallback(() => {
    setError(null);
  }, []);
  
  return {
    // APM state (same as useSimpleAPM)
    stats,
    isActive,
    isLoading,
    error,
    
    // APM actions (enhanced with Confect)
    trackMessageSent,
    trackSessionCreated,
    syncNow,
    clearError,
    
    // Confect-specific features
    getUserAPMStats,
    calculateBackendAPM,
    subscribeToSessionUpdates,
  };
}