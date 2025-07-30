import { useState, useEffect, useCallback, useRef } from 'react';
// import { Effect, Runtime } from 'effect';

// Platform detection utilities (we'll need to implement these)
interface PlatformInfo {
  OS: string;
  isWeb: boolean;
  isReactNative: boolean;
}

// Mock platform detection - in real implementation this would be more sophisticated
const getPlatformInfo = (): PlatformInfo => {
  if (typeof window !== 'undefined') {
    return {
      OS: 'web',
      isWeb: true,
      isReactNative: false,
    };
  }
  
  // For React Native, we'd import Platform from 'react-native'
  // For now, assume mobile if not web
  return {
    OS: 'mobile',
    isWeb: false,
    isReactNative: true,
  };
};

// Device connection types (matching our schema)
export interface DeviceInfo {
  deviceType: 'desktop' | 'mobile' | 'web';
  platform: string;
  appVersion: string;
  userAgent?: string;
  lastSeen: number;
  capabilities: string[];
}

export interface DeviceConnection {
  deviceId: string;
  userId: string;
  deviceInfo: DeviceInfo;
  status: 'online' | 'offline' | 'idle';
  sessionToken: string;
  roomToken: string;
  connectedAt: number;
  lastHeartbeat: number;
}

export interface UseDevicePresenceConfig {
  userId?: string;
  enableAutoConnect?: boolean;
  heartbeatInterval?: number; // milliseconds
  enableAppStateMonitoring?: boolean;
}

export interface UseDevicePresenceReturn {
  // Device state
  devices: DeviceConnection[];
  isLoading: boolean;
  error: string | null;
  
  // Current device session info
  sessionInfo: {
    deviceId: string;
    sessionToken: string;
    roomToken: string;
  } | null;
  
  // Actions
  connectDevice: (deviceInfo: Partial<DeviceInfo>) => Promise<void>;
  disconnectDevice: () => Promise<void>;
  sendHeartbeat: () => Promise<void>;
  refreshDevices: () => Promise<void>;
  clearError: () => void;
  
  // Status helpers
  isConnected: boolean;
  connectedDesktops: DeviceConnection[];
  connectedMobiles: DeviceConnection[];
  hasActiveDesktop: boolean;
}

/**
 * React hook for device presence tracking using Effect-TS patterns with Confect integration.
 * 
 * Provides real-time device sync, automatic connection/disconnection, heartbeat management,
 * and cross-platform presence tracking.
 */
export function useDevicePresence(config: UseDevicePresenceConfig = {}): UseDevicePresenceReturn {
  const {
    userId,
    enableAutoConnect = true,
    heartbeatInterval = 10000, // 10 seconds
    enableAppStateMonitoring = true,
  } = config;

  // State management
  const [devices, setDevices] = useState<DeviceConnection[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sessionInfo, setSessionInfo] = useState<{
    deviceId: string;
    sessionToken: string;
    roomToken: string;
  } | null>(null);

  // Refs for cleanup and intervals
  const heartbeatIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const appStateListenerRef = useRef<(() => void) | null>(null);
  const isConnectedRef = useRef(false);

  // Platform info
  const platformInfo = getPlatformInfo();

  // Generate device info based on current platform
  const generateDeviceInfo = useCallback((overrides: Partial<DeviceInfo> = {}): DeviceInfo => {
    const defaultInfo: DeviceInfo = {
      deviceType: platformInfo.isWeb ? 'web' : 'mobile',
      platform: platformInfo.OS,
      appVersion: '1.0.0', // In real app, get from app config
      userAgent: platformInfo.isWeb ? navigator.userAgent : undefined,
      lastSeen: Date.now(),
      capabilities: [
        'real-time-sync',
        ...(platformInfo.isReactNative ? ['push-notifications', 'mobile-sync'] : []),
        ...(platformInfo.isWeb ? ['web-sync'] : []),
      ],
    };

    return { ...defaultInfo, ...overrides };
  }, [platformInfo]);

  // Mock Convex API calls (in real implementation, these would use useMutation/useQuery)
  const mockConvexAPI = {
    connectDevice: async (deviceInfo: DeviceInfo) => {
      console.log('🔗 [DEVICE-PRESENCE] Connecting device:', deviceInfo);
      
      // Simulate API call
      await new Promise(resolve => setTimeout(resolve, 500));
      
      return {
        deviceId: `device_${Date.now()}_${Math.random().toString(36).substring(2)}`,
        sessionToken: `session_${Date.now()}_${Math.random().toString(36).substring(2)}`,
        roomToken: `room_${userId}`,
      };
    },

    getUserDevices: async (userId: string) => {
      console.log('📱 [DEVICE-PRESENCE] Getting devices for user:', userId);
      
      // Simulate API call
      await new Promise(resolve => setTimeout(resolve, 300));
      
      // Mock response with some devices
      return [
        {
          deviceId: 'desktop_mac_001',
          userId,
          deviceInfo: {
            deviceType: 'desktop' as const,
            platform: 'macos',
            appVersion: '1.0.0',
            lastSeen: Date.now() - 30000,
            capabilities: ['claude-code', 'file-sync'],
          },
          status: 'online' as const,
          sessionToken: 'session_desktop_001',
          roomToken: `room_${userId}`,
          connectedAt: Date.now() - 300000,
          lastHeartbeat: Date.now() - 5000,
        },
      ] as DeviceConnection[];
    },

    sendHeartbeat: async (deviceId: string, _sessionToken: string) => {
      console.log('💓 [DEVICE-PRESENCE] Sending heartbeat for:', deviceId);
      await new Promise(resolve => setTimeout(resolve, 100));
      return null;
    },

    disconnectDevice: async (sessionToken: string) => {
      console.log('🔌 [DEVICE-PRESENCE] Disconnecting device with session:', sessionToken);
      await new Promise(resolve => setTimeout(resolve, 200));
      return null;
    },
  };

  // Connect device to user's presence room
  const connectDevice = useCallback(async (deviceInfoOverrides: Partial<DeviceInfo> = {}) => {
    if (!userId) {
      console.warn('⚠️ [DEVICE-PRESENCE] No userId provided, cannot connect device');
      return;
    }

    try {
      setIsLoading(true);
      setError(null);

      const deviceInfo = generateDeviceInfo(deviceInfoOverrides);
      const result = await mockConvexAPI.connectDevice(deviceInfo);

      setSessionInfo({
        deviceId: result.deviceId,
        sessionToken: result.sessionToken,
        roomToken: result.roomToken,
      });

      isConnectedRef.current = true;

      console.log('✅ [DEVICE-PRESENCE] Device connected successfully:', result);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to connect device';
      setError(errorMessage);
      console.error('❌ [DEVICE-PRESENCE] Device connection failed:', err);
    } finally {
      setIsLoading(false);
    }
  }, [userId, generateDeviceInfo]);

  // Disconnect device and clean up
  const disconnectDevice = useCallback(async () => {
    if (!sessionInfo?.sessionToken) {
      console.warn('⚠️ [DEVICE-PRESENCE] No session to disconnect');
      return;
    }

    try {
      await mockConvexAPI.disconnectDevice(sessionInfo.sessionToken);
      setSessionInfo(null);
      isConnectedRef.current = false;

      console.log('✅ [DEVICE-PRESENCE] Device disconnected successfully');
    } catch (err) {
      console.error('❌ [DEVICE-PRESENCE] Device disconnection failed:', err);
    }
  }, [sessionInfo?.sessionToken]);

  // Send heartbeat to keep connection alive
  const sendHeartbeat = useCallback(async () => {
    if (!sessionInfo?.deviceId || !sessionInfo?.sessionToken) {
      return;
    }

    try {
      await mockConvexAPI.sendHeartbeat(sessionInfo.deviceId, sessionInfo.sessionToken);
      console.log('💓 [DEVICE-PRESENCE] Heartbeat sent successfully');
    } catch (err) {
      console.error('❌ [DEVICE-PRESENCE] Heartbeat failed:', err);
      setError('Connection lost. Attempting to reconnect...');
    }
  }, [sessionInfo]);

  // Refresh devices list
  const refreshDevices = useCallback(async () => {
    if (!userId) {
      return;
    }

    try {
      const userDevices = await mockConvexAPI.getUserDevices(userId);
      setDevices(userDevices);
      console.log(`📊 [DEVICE-PRESENCE] Refreshed devices: ${userDevices.length} found`);
    } catch (err) {
      console.error('❌ [DEVICE-PRESENCE] Failed to refresh devices:', err);
      setError('Failed to refresh devices');
    }
  }, [userId]);

  // Clear error state
  const clearError = useCallback(() => {
    setError(null);
  }, []);

  // Set up heartbeat interval
  useEffect(() => {
    if (sessionInfo && isConnectedRef.current) {
      heartbeatIntervalRef.current = setInterval(() => {
        sendHeartbeat();
      }, heartbeatInterval) as unknown as NodeJS.Timeout;

      console.log(`⏰ [DEVICE-PRESENCE] Heartbeat started (${heartbeatInterval}ms interval)`);

      return () => {
        if (heartbeatIntervalRef.current) {
          clearInterval(heartbeatIntervalRef.current);
          heartbeatIntervalRef.current = null;
          console.log('⏰ [DEVICE-PRESENCE] Heartbeat stopped');
        }
      };
    }
    
    // Return cleanup function even if condition is false
    return () => {
      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
        heartbeatIntervalRef.current = null;
      }
    };
  }, [sessionInfo, heartbeatInterval, sendHeartbeat]);

  // Set up app state monitoring (for mobile apps)
  useEffect(() => {
    if (!enableAppStateMonitoring || !platformInfo.isReactNative) {
      return () => {}; // Return empty cleanup function
    }

    const setupAppStateMonitoring = async () => {
      try {
        // This would use React Native's AppState in a real implementation
        console.log('📱 [DEVICE-PRESENCE] App state monitoring would be set up here');
        
        // Mock app state listener
        const mockListener = () => {
          console.log('📱 [DEVICE-PRESENCE] App state changed (mock)');
        };
        
        appStateListenerRef.current = mockListener;
      } catch (err) {
        console.error('❌ [DEVICE-PRESENCE] Failed to set up app state monitoring:', err);
      }
    };

    setupAppStateMonitoring();

    return () => {
      if (appStateListenerRef.current) {
        appStateListenerRef.current();
        appStateListenerRef.current = null;
        console.log('📱 [DEVICE-PRESENCE] App state monitoring cleaned up');
      }
    };
  }, [enableAppStateMonitoring, platformInfo.isReactNative]);

  // Auto-connect on mount if enabled
  useEffect(() => {
    if (enableAutoConnect && userId && !sessionInfo) {
      console.log('🔄 [DEVICE-PRESENCE] Auto-connecting device...');
      connectDevice();
    }
  }, [enableAutoConnect, userId, sessionInfo, connectDevice]);

  // Refresh devices periodically
  useEffect(() => {
    if (userId) {
      // Initial load
      refreshDevices();
      
      // Set up periodic refresh
      const refreshInterval = setInterval(refreshDevices, 30000); // 30 seconds
      
      return () => clearInterval(refreshInterval);
    }
    
    // Return empty cleanup function if no userId
    return () => {};
  }, [userId, refreshDevices]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (isConnectedRef.current && sessionInfo) {
        console.log('🧹 [DEVICE-PRESENCE] Cleaning up on unmount');
        disconnectDevice();
      }
    };
  }, []);

  // Computed values
  const isConnected = sessionInfo !== null && isConnectedRef.current;
  const connectedDesktops = devices.filter(d => 
    d.deviceInfo.deviceType === 'desktop' && d.status === 'online'
  );
  const connectedMobiles = devices.filter(d => 
    d.deviceInfo.deviceType === 'mobile' && d.status === 'online'
  );
  const hasActiveDesktop = connectedDesktops.length > 0;

  return {
    // Device state
    devices,
    isLoading,
    error,
    
    // Session info
    sessionInfo,
    
    // Actions
    connectDevice,
    disconnectDevice,
    sendHeartbeat,
    refreshDevices,
    clearError,
    
    // Status helpers
    isConnected,
    connectedDesktops,
    connectedMobiles,
    hasActiveDesktop,
  };
}