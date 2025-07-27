import { useEffect, useRef, useCallback } from 'react';
import { useMutation } from 'convex/react';
import { api } from '../convex/_generated/api';
import { AppState, AppStateStatus, Platform } from 'react-native';

interface APMTrackingOptions {
  enabled?: boolean;
  trackMessages?: boolean;
  trackSessions?: boolean;
  trackAppState?: boolean;
}

interface APMSessionData {
  sessionStart: number;
  sessionEnd?: number;
  messagesSent: number;
  sessionsCreated: number;
  appStateChanges: number;
}

const DEFAULT_OPTIONS: APMTrackingOptions = {
  enabled: true,
  trackMessages: true,
  trackSessions: true,
  trackAppState: true,
};

export function useAPMTracking(options: APMTrackingOptions = {}) {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  
  // Convex mutations
  const trackDeviceSession = useMutation(api.claude.trackDeviceSession);
  const calculateUserAPM = useMutation(api.claude.calculateUserAPM);
  
  // Session tracking state
  const sessionData = useRef<APMSessionData>({
    sessionStart: Date.now(),
    messagesSent: 0,
    sessionsCreated: 0,
    appStateChanges: 0,
  });
  
  const currentAppState = useRef<AppStateStatus>(AppState.currentState);
  const sessionActive = useRef<boolean>(true);
  const deviceId = useRef<string | null>(null);

  // Initialize device ID
  useEffect(() => {
    // Generate a persistent device ID based on timestamp and random string
    // In a real app, you'd store this in secure storage for persistence
    const generateDeviceId = () => {
      const timestamp = Date.now();
      const random = Math.random().toString(36).substring(2, 15);
      return `mobile-${Platform.OS}-${timestamp}-${random}`;
    };

    if (!deviceId.current) {
      deviceId.current = generateDeviceId();
      console.log('📱 [APM] Generated device ID:', deviceId.current);
    }
  }, []);

  // Track app state changes
  useEffect(() => {
    if (!opts.enabled || !opts.trackAppState) return;

    const handleAppStateChange = (nextAppState: AppStateStatus) => {
      const previousState = currentAppState.current;
      currentAppState.current = nextAppState;

      sessionData.current.appStateChanges++;

      if (previousState === 'background' && nextAppState === 'active') {
        // App became active - start new session period
        sessionData.current.sessionStart = Date.now();
        sessionActive.current = true;
        console.log('📱 [APM] App became active, starting new session period');
      } else if (previousState === 'active' && nextAppState === 'background') {
        // App went to background - end current session period
        sessionActive.current = false;
        console.log('📱 [APM] App went to background, ending session period');
        
        // Send current session data to backend
        sendSessionData();
      }
    };

    const subscription = AppState.addEventListener('change', handleAppStateChange);

    return () => {
      subscription?.remove();
    };
  }, [opts.enabled, opts.trackAppState]);

  // Send session data to backend
  const sendSessionData = useCallback(async () => {
    if (!opts.enabled || !deviceId.current) return;

    const session = sessionData.current;
    const now = Date.now();

    try {
      await trackDeviceSession({
        deviceId: deviceId.current,
        deviceType: 'mobile',
        sessionStart: session.sessionStart,
        sessionEnd: sessionActive.current ? undefined : now,
        actions: {
          messages: session.messagesSent,
          toolUses: 0, // Mobile doesn't directly use tools
          githubEvents: 0,
        },
        metadata: {
          platform: Platform.OS,
          version: '1.0.0', // Could be from package.json or constants
        },
      });

      console.log('📊 [APM] Session data sent to backend:', {
        deviceId: deviceId.current,
        duration: now - session.sessionStart,
        actions: session.messagesSent,
      });

      // Trigger APM calculation
      await calculateUserAPM({});

    } catch (error) {
      console.error('❌ [APM] Failed to send session data:', error);
    }
  }, [trackDeviceSession, calculateUserAPM, opts.enabled]);

  // Send session data when app is closing or going to background
  useEffect(() => {
    return () => {
      if (sessionActive.current) {
        sendSessionData();
      }
    };
  }, [sendSessionData]);

  // Periodic session data sync (every 5 minutes)
  useEffect(() => {
    if (!opts.enabled) return;

    const interval = setInterval(() => {
      if (sessionActive.current) {
        sendSessionData();
        // Reset counters after sending
        sessionData.current = {
          sessionStart: Date.now(),
          messagesSent: 0,
          sessionsCreated: 0,
          appStateChanges: sessionData.current.appStateChanges,
        };
      }
    }, 5 * 60 * 1000); // 5 minutes

    return () => clearInterval(interval);
  }, [opts.enabled, sendSessionData]);

  // Track message sent
  const trackMessageSent = useCallback(() => {
    if (!opts.enabled || !opts.trackMessages) return;
    
    sessionData.current.messagesSent++;
    console.log('📝 [APM] Message tracked, total:', sessionData.current.messagesSent);
  }, [opts.enabled, opts.trackMessages]);

  // Track session created
  const trackSessionCreated = useCallback(() => {
    if (!opts.enabled || !opts.trackSessions) return;
    
    sessionData.current.sessionsCreated++;
    console.log('🆕 [APM] Session creation tracked, total:', sessionData.current.sessionsCreated);
  }, [opts.enabled, opts.trackSessions]);

  // Manual session data flush
  const flushSessionData = useCallback(() => {
    sendSessionData();
  }, [sendSessionData]);

  // Get current session stats
  const getSessionStats = useCallback(() => {
    const now = Date.now();
    const duration = now - sessionData.current.sessionStart;
    const totalActions = sessionData.current.messagesSent + sessionData.current.sessionsCreated;
    const apm = duration > 0 ? (totalActions / (duration / 60000)) : 0;

    return {
      duration,
      totalActions,
      apm,
      isActive: sessionActive.current,
      ...sessionData.current,
    };
  }, []);

  return {
    trackMessageSent,
    trackSessionCreated,
    flushSessionData,
    getSessionStats,
    isEnabled: opts.enabled,
    isActive: sessionActive.current,
  };
}