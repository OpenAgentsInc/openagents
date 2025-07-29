import { useEffect, useState, useCallback, useRef } from 'react';
import { useQuery, useMutation } from 'convex/react';
import { api } from '../../convex/_generated/api';
import { AppState, AppStateStatus } from 'react-native';

export interface ConvexRealtimeAPMData {
  currentAPM: number;
  trend: 'up' | 'down' | 'stable';
  sessionDuration: number;
  totalActions: number;
  lastUpdateTimestamp: number;
  isActive: boolean;
  deviceId: string;
  trendPercentage?: number;
  history?: number[];
}

export interface UseConvexRealtimeAPMConfig {
  enabled?: boolean;
  deviceId?: string;
  includeHistory?: boolean;
  onAPMUpdate?: (data: ConvexRealtimeAPMData) => void;
  onError?: (error: any) => void;
}

export interface ConvexRealtimeAPMState {
  data: ConvexRealtimeAPMData | null;
  isLoading: boolean;
  error: any;
  isActive: boolean;
}

/**
 * Hook for Convex-based realtime APM tracking
 * Integrates directly with Convex queries and mutations for mobile app
 */
export function useConvexRealtimeAPM(config: UseConvexRealtimeAPMConfig = {}) {
  const {
    enabled = true,
    deviceId,
    includeHistory = false,
    onAPMUpdate,
    onError,
  } = config;

  const [isActive, setIsActive] = useState(true);
  const [error, setError] = useState<any>(null);
  const appStateRef = useRef<AppStateStatus>(AppState.currentState);

  // Convex queries and mutations
  const realtimeAPMData = useQuery(
    api.confect.apm.getRealtimeAPM,
    enabled ? { deviceId, includeHistory } : 'skip'
  );

  const trackAction = useMutation(api.confect.apm.trackRealtimeAction);
  const updateAPM = useMutation(api.confect.apm.updateRealtimeAPM);

  // State derived from Convex data
  const state: ConvexRealtimeAPMState = {
    data: realtimeAPMData ?? null,
    isLoading: realtimeAPMData === undefined,
    error,
    isActive,
  };

  // Handle APM data updates
  useEffect(() => {
    if (realtimeAPMData && onAPMUpdate) {
      onAPMUpdate(realtimeAPMData);
    }
  }, [realtimeAPMData, onAPMUpdate]);

  // Handle app state changes
  useEffect(() => {
    if (!enabled) return;

    const handleAppStateChange = (nextAppState: AppStateStatus) => {
      const wasActive = appStateRef.current === 'active';
      const isNowActive = nextAppState === 'active';
      
      appStateRef.current = nextAppState;
      setIsActive(isNowActive);

      // Log app state transitions
      if (wasActive !== isNowActive) {
        console.log(`📱 [ConvexRealtimeAPM] App state changed: ${appStateRef.current} -> ${nextAppState}`);
        
        if (realtimeAPMData?.deviceId) {
          // Update backend with new active state
          updateAPM({
            deviceId: realtimeAPMData.deviceId,
            currentAPM: realtimeAPMData.currentAPM,
            totalActions: realtimeAPMData.totalActions,
            sessionDuration: realtimeAPMData.sessionDuration,
            isActive: isNowActive,
          }).catch(error => {
            console.error('Failed to update APM active state:', error);
            setError(error);
            onError?.(error);
          });
        }
      }
    };

    const subscription = AppState.addEventListener('change', handleAppStateChange);
    
    return () => subscription?.remove();
  }, [enabled, realtimeAPMData, updateAPM, onError]);

  // Track message action
  const trackMessageAction = useCallback(async () => {
    if (!enabled || !realtimeAPMData?.deviceId) {
      console.warn('Cannot track message: APM not enabled or device ID not available');
      return null;
    }

    try {
      const result = await trackAction({
        deviceId: realtimeAPMData.deviceId,
        actionType: 'message',
        timestamp: Date.now(),
      });

      console.log(`📊 [ConvexRealtimeAPM] Message tracked, new APM: ${result.newAPM}`);
      return result;
    } catch (error) {
      console.error('Failed to track message action:', error);
      setError(error);
      onError?.(error);
      return null;
    }
  }, [enabled, realtimeAPMData?.deviceId, trackAction, onError]);

  // Track session action
  const trackSessionAction = useCallback(async () => {
    if (!enabled || !realtimeAPMData?.deviceId) {
      console.warn('Cannot track session: APM not enabled or device ID not available');
      return null;
    }

    try {
      const result = await trackAction({
        deviceId: realtimeAPMData.deviceId,
        actionType: 'session',
        timestamp: Date.now(),  
      });

      console.log(`📊 [ConvexRealtimeAPM] Session tracked, new APM: ${result.newAPM}`);
      return result;
    } catch (error) {
      console.error('Failed to track session action:', error);
      setError(error);
      onError?.(error);
      return null;
    }
  }, [enabled, realtimeAPMData?.deviceId, trackAction, onError]);

  // Track tool action
  const trackToolAction = useCallback(async (metadata?: any) => {
    if (!enabled || !realtimeAPMData?.deviceId) {
      console.warn('Cannot track tool: APM not enabled or device ID not available');
      return null;
    }

    try {
      const result = await trackAction({
        deviceId: realtimeAPMData.deviceId,
        actionType: 'tool',
        timestamp: Date.now(),
        metadata,
      });

      console.log(`📊 [ConvexRealtimeAPM] Tool tracked, new APM: ${result.newAPM}`);
      return result;
    } catch (error) {
      console.error('Failed to track tool action:', error);
      setError(error);
      onError?.(error);
      return null;
    }
  }, [enabled, realtimeAPMData?.deviceId, trackAction, onError]);

  // Manual refresh
  const refresh = useCallback(() => {
    setError(null);
    // Convex will automatically refetch the query
  }, []);

  // Get current APM value
  const getCurrentAPM = useCallback((): number => {
    return realtimeAPMData?.currentAPM ?? 0;
  }, [realtimeAPMData?.currentAPM]);

  // Check if trending up
  const isTrendingUp = useCallback((): boolean => {
    return realtimeAPMData?.trend === 'up';
  }, [realtimeAPMData?.trend]);

  // Check if trending down
  const isTrendingDown = useCallback((): boolean => {
    return realtimeAPMData?.trend === 'down';
  }, [realtimeAPMData?.trend]);

  // Get session info
  const getSessionInfo = useCallback(() => {
    if (!realtimeAPMData) {
      return {
        duration: 0,
        totalActions: 0,
        apm: 0,
        isActive: false,
      };
    }

    return {
      duration: realtimeAPMData.sessionDuration,
      totalActions: realtimeAPMData.totalActions,
      apm: realtimeAPMData.currentAPM,
      isActive: realtimeAPMData.isActive,
    };
  }, [realtimeAPMData]);

  return {
    state,
    actions: {
      trackMessage: trackMessageAction,
      trackSession: trackSessionAction,
      trackTool: trackToolAction,
      refresh,
    },
    data: {
      getCurrentAPM,
      isTrendingUp,
      isTrendingDown,
      getSessionInfo,
    },
  };
}

/**
 * Simplified hook for just tracking actions without subscribing to updates
 * Useful for components that only need to report actions
 */
export function useAPMActionTracker(config: { deviceId?: string; enabled?: boolean } = {}) {
  const { enabled = true, deviceId } = config;
  const trackAction = useMutation(api.confect.apm.trackRealtimeAction);

  // Generate a stable device ID if not provided
  const [stableDeviceId] = useState(() => {
    if (deviceId) return deviceId;
    
    // Simple device ID generation for mobile
    const timestamp = Date.now();
    const random = Math.random().toString(36).substring(2, 15);
    return `mobile-${timestamp}-${random}`;
  });

  const trackMessage = useCallback(async () => {
    if (!enabled) return null;

    try {
      const result = await trackAction({
        deviceId: stableDeviceId,
        actionType: 'message',
        timestamp: Date.now(),
      });

      return result;
    } catch (error) {
      console.error('Failed to track message:', error);
      return null;
    }
  }, [enabled, stableDeviceId, trackAction]);

  const trackSession = useCallback(async () => {
    if (!enabled) return null;

    try {
      const result = await trackAction({
        deviceId: stableDeviceId,
        actionType: 'session',
        timestamp: Date.now(),
      });

      return result;
    } catch (error) {
      console.error('Failed to track session:', error);
      return null;
    }
  }, [enabled, stableDeviceId, trackAction]);

  return {
    trackMessage,
    trackSession,
    deviceId: stableDeviceId,
  };
}