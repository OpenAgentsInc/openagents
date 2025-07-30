import React, { useEffect, useState, useCallback } from 'react';
import { View, Text, StyleSheet, TouchableOpacity, Animated } from 'react-native';
import { useQuery } from 'convex/react';
import { api } from '@openagentsinc/convex';
import { useRealtimeAPM } from '../../../../../packages/shared/src/hooks/useRealtimeAPM';

interface RealtimeAPMWidgetProps {
  /** Position of the widget on screen */
  position?: 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left';
  /** Compact mode for smaller display */
  compact?: boolean;
  /** Show trend indicator */
  showTrend?: boolean;
  /** Update interval in milliseconds */
  updateInterval?: number;
  /** Custom styling */
  style?: any;
  /** Callback when widget is pressed */
  onPress?: () => void;
  /** Whether to show the widget */
  visible?: boolean;
}

export const RealtimeAPMWidget: React.FC<RealtimeAPMWidgetProps> = ({
  position = 'top-right',
  compact = false,
  showTrend = true,
  updateInterval = 3000,
  style,
  onPress,
  visible = true,
}) => {
  const [fadeAnim] = useState(new Animated.Value(visible ? 1 : 0));
  const [pulseAnim] = useState(new Animated.Value(1));
  
  const {
    state: { data: apmData, isLoading, error, isSubscribed },
    trackMessage,
    trackSession,
    setActive,
  } = useRealtimeAPM({
    enabled: visible,
    updateInterval,
    enableStreaming: true,
    enableTrendCalculation: showTrend,
    onAPMUpdate: (data) => {
      // Pulse animation when APM updates
      if (data.currentAPM > 0) {
        Animated.sequence([
          Animated.timing(pulseAnim, {
            toValue: 1.1,
            duration: 150,
            useNativeDriver: true,
          }),
          Animated.timing(pulseAnim, {
            toValue: 1,
            duration: 150,
            useNativeDriver: true,
          }),
        ]).start();
      }
    },
    onError: (error) => {
      console.error('Realtime APM Error:', error);
    },
  });

  // Fade in/out animation when visibility changes
  useEffect(() => {
    Animated.timing(fadeAnim, {
      toValue: visible ? 1 : 0,
      duration: 300,
      useNativeDriver: true,
    }).start();
  }, [visible, fadeAnim]);

  // Handle app state changes to track activity
  useEffect(() => {
    const handleAppStateChange = (nextAppState: string) => {
      setActive(nextAppState === 'active');
    };

    // In a real implementation, you'd listen to AppState changes here
    // This is a simplified version
    setActive(true);

    return () => {
      setActive(false);
    };
  }, [setActive]);

  const handlePress = useCallback(() => {
    if (onPress) {
      onPress();
    }
  }, [onPress]);

  const getTrendIcon = (trend: 'up' | 'down' | 'stable') => {
    switch (trend) {
      case 'up':
        return '↗️';
      case 'down':
        return '↘️';
      default:
        return '→';
    }
  };

  const getTrendColor = (trend: 'up' | 'down' | 'stable') => {
    switch (trend) {
      case 'up':
        return '#10B981'; // Green
      case 'down':
        return '#EF4444'; // Red
      default:
        return '#6B7280'; // Gray
    }
  };

  const getPositionStyle = () => {
    const baseStyle = {
      position: 'absolute' as const,
      zIndex: 1000,
    };

    switch (position) {
      case 'top-left':
        return { ...baseStyle, top: 50, left: 16 };
      case 'top-right':
        return { ...baseStyle, top: 50, right: 16 };
      case 'bottom-left':
        return { ...baseStyle, bottom: 50, left: 16 };
      case 'bottom-right':
        return { ...baseStyle, bottom: 50, right: 16 };
      default:
        return { ...baseStyle, top: 50, right: 16 };
    }
  };

  if (!visible) {
    return null;
  }

  const currentAPM = apmData?.currentAPM ?? 0;
  const trend = apmData?.trend ?? 'stable';
  const isActive = apmData?.isActive ?? false;

  return (
    <Animated.View
      style={[
        getPositionStyle(),
        {
          opacity: fadeAnim,
          transform: [{ scale: pulseAnim }],
        },
      ]}
    >
      <TouchableOpacity
        onPress={handlePress}
        style={[
          styles.container,
          compact ? styles.compactContainer : styles.fullContainer,
          {
            backgroundColor: isActive ? '#1F2937' : '#374151',
            borderColor: isSubscribed ? '#10B981' : '#6B7280',
          },
          style,
        ]}
        activeOpacity={0.8}
      >
        {isLoading ? (
          <View style={styles.loadingContainer}>
            <Text style={styles.loadingText}>...</Text>
          </View>
        ) : error ? (
          <View style={styles.errorContainer}>
            <Text style={styles.errorText}>!</Text>
          </View>
        ) : (
          <>
            {compact ? (
              <View style={styles.compactLayout}>
                <Text style={styles.apmValue}>
                  {currentAPM.toFixed(1)}
                </Text>
                {showTrend && (
                  <Text style={[styles.trendIcon, { color: getTrendColor(trend) }]}>
                    {getTrendIcon(trend)}
                  </Text>
                )}
              </View>
            ) : (
              <View style={styles.fullLayout}>
                <Text style={styles.label}>Current APM</Text>
                <View style={styles.valueRow}>
                  <Text style={styles.apmValue}>
                    {currentAPM.toFixed(1)}
                  </Text>
                  {showTrend && (
                    <Text style={[styles.trendIcon, { color: getTrendColor(trend) }]}>
                      {getTrendIcon(trend)}
                    </Text>
                  )}
                </View>
                {apmData && (
                  <Text style={styles.subtitle}>
                    {apmData.totalActions} actions • {Math.round(apmData.sessionDuration / 60000)}m
                  </Text>
                )}
              </View>
            )}
            
            {/* Connection indicator */}
            <View style={[
              styles.indicator,
              { backgroundColor: isSubscribed ? '#10B981' : '#6B7280' }
            ]} />
          </>
        )}
      </TouchableOpacity>
    </Animated.View>
  );
};

const styles = StyleSheet.create({
  container: {
    borderRadius: 12,
    borderWidth: 1,
    shadowColor: '#000',
    shadowOffset: {
      width: 0,
      height: 2,
    },
    shadowOpacity: 0.25,
    shadowRadius: 3.84,
    elevation: 5,
  },
  compactContainer: {
    padding: 8,
    minWidth: 60,
  },
  fullContainer: {
    padding: 12,
    minWidth: 120,
  },
  compactLayout: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 4,
  },
  fullLayout: {
    alignItems: 'center',
  },
  label: {
    fontSize: 10,
    color: '#9CA3AF',
    fontWeight: '500',
    marginBottom: 2,
    textTransform: 'uppercase',
  },
  valueRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    marginBottom: 2,
  },
  apmValue: {
    fontSize: 18,
    fontWeight: 'bold',
    color: '#F9FAFB',
  },
  trendIcon: {
    fontSize: 14,
    fontWeight: 'bold',
  },
  subtitle: {
    fontSize: 9,
    color: '#6B7280',
    textAlign: 'center',
  },
  loadingContainer: {
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: 30,
  },
  loadingText: {
    fontSize: 16,
    color: '#9CA3AF',
    fontWeight: 'bold',
  },
  errorContainer: {
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: 30,
  },
  errorText: {
    fontSize: 16,
    color: '#EF4444',
    fontWeight: 'bold',
  },
  indicator: {
    position: 'absolute',
    top: 4,
    right: 4,
    width: 6,
    height: 6,
    borderRadius: 3,
  },
});

// Hook for tracking actions from external components
export const useAPMActionTracking = () => {
  const { trackMessage, trackSession } = useRealtimeAPM({
    enabled: true,
    enableStreaming: false, // Only track actions, don't stream
  });

  return {
    trackMessage: useCallback(() => {
      trackMessage();
    }, [trackMessage]),
    trackSession: useCallback(() => {
      trackSession();
    }, [trackSession]),
  };
};

export default RealtimeAPMWidget;