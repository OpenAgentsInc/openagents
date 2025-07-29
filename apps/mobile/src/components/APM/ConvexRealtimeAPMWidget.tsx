import React, { useEffect, useState, useCallback } from 'react';
import { View, Text, StyleSheet, TouchableOpacity, Animated, Platform } from 'react-native';
import { useConvexRealtimeAPM } from '../../hooks/useConvexRealtimeAPM';

interface ConvexRealtimeAPMWidgetProps {
  /** Position of the widget on screen */
  position?: 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left';
  /** Compact mode for smaller display */
  compact?: boolean;
  /** Show trend indicator */
  showTrend?: boolean;
  /** Device ID for tracking */
  deviceId?: string;
  /** Custom styling */
  style?: any;
  /** Callback when widget is pressed */
  onPress?: () => void;
  /** Whether to show the widget */
  visible?: boolean;
  /** Include historical data for trend calculation */
  includeHistory?: boolean;
}

export const ConvexRealtimeAPMWidget: React.FC<ConvexRealtimeAPMWidgetProps> = ({
  position = 'top-right',
  compact = false,
  showTrend = true,
  deviceId,
  style,
  onPress,
  visible = true,
  includeHistory = true,
}) => {
  const [fadeAnim] = useState(new Animated.Value(visible ? 1 : 0));
  const [pulseAnim] = useState(new Animated.Value(1));
  const [previousAPM, setPreviousAPM] = useState<number>(0);

  const {
    state: { data: apmData, isLoading, error, isActive },
    actions: { refresh },
    data: { getCurrentAPM, isTrendingUp, isTrendingDown, getSessionInfo },
  } = useConvexRealtimeAPM({
    enabled: visible,
    deviceId,
    includeHistory,
    onAPMUpdate: (data) => {
      // Trigger pulse animation when APM increases significantly
      const currentAPM = data.currentAPM;
      if (currentAPM > previousAPM && currentAPM > 0) {
        Animated.sequence([
          Animated.timing(pulseAnim, {
            toValue: 1.15,
            duration: 200,
            useNativeDriver: true,
          }),
          Animated.timing(pulseAnim, {
            toValue: 1,
            duration: 200,
            useNativeDriver: true,
          }),
        ]).start();
      }
      setPreviousAPM(currentAPM);
    },
    onError: (error) => {
      console.error('Convex Realtime APM Error:', error);
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

  const handlePress = useCallback(() => {
    if (onPress) {
      onPress();
    } else {
      // Default action: refresh APM data
      refresh();
    }
  }, [onPress, refresh]);

  const getTrendIcon = (trend: 'up' | 'down' | 'stable') => {
    switch (trend) {
      case 'up':
        return '↗';
      case 'down':
        return '↘';
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
    // Account for different safe areas on iOS and Android
    const topOffset = Platform.OS === 'ios' ? 50 : 30;
    const bottomOffset = Platform.OS === 'ios' ? 50 : 30;
    
    const baseStyle = {
      position: 'absolute' as const,
      zIndex: 1000,
    };

    switch (position) {
      case 'top-left':
        return { ...baseStyle, top: topOffset, left: 16 };
      case 'top-right':
        return { ...baseStyle, top: topOffset, right: 16 };
      case 'bottom-left':
        return { ...baseStyle, bottom: bottomOffset, left: 16 };
      case 'bottom-right':
        return { ...baseStyle, bottom: bottomOffset, right: 16 };
      default:
        return { ...baseStyle, top: topOffset, right: 16 };
    }
  };

  const getStatusColor = () => {
    if (error) return '#EF4444'; // Red for error
    if (isLoading) return '#F59E0B'; // Amber for loading
    if (!isActive) return '#6B7280'; // Gray for inactive
    return '#10B981'; // Green for active
  };

  if (!visible) {
    return null;
  }

  const sessionInfo = getSessionInfo();
  const currentAPM = getCurrentAPM();
  const trend = apmData?.trend ?? 'stable';

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
            backgroundColor: 'rgba(31, 41, 55, 0.95)', // Semi-transparent background
            borderColor: getStatusColor(),
          },
          style,
        ]}
        activeOpacity={0.8}
      >
        {isLoading ? (
          <View style={styles.statusContainer}>
            <Text style={styles.statusText}>⏳</Text>
            {!compact && <Text style={styles.statusLabel}>Loading</Text>}
          </View>
        ) : error ? (
          <View style={styles.statusContainer}>
            <Text style={styles.statusText}>⚠️</Text>
            {!compact && <Text style={styles.statusLabel}>Error</Text>}
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
                    {sessionInfo.totalActions} actions • {Math.round(sessionInfo.duration / 60000)}m
                  </Text>
                )}
                {apmData?.trendPercentage && Math.abs(apmData.trendPercentage) >= 10 && (
                  <Text style={[styles.trendPercentage, { color: getTrendColor(trend) }]}>
                    {apmData.trendPercentage > 0 ? '+' : ''}{apmData.trendPercentage.toFixed(0)}%
                  </Text>
                )}
              </View>
            )}
            
            {/* Status indicator */}
            <View style={[
              styles.indicator,
              { backgroundColor: getStatusColor() }
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
    borderWidth: 1.5,
    shadowColor: '#000',
    shadowOffset: {
      width: 0,
      height: 4,
    },
    shadowOpacity: 0.3,
    shadowRadius: 4.65,
    elevation: 8,
    // Backdrop blur effect (iOS only)
    ...(Platform.OS === 'ios' && {
      backgroundColor: 'rgba(31, 41, 55, 0.8)',
    }),
  },
  compactContainer: {
    padding: 10,
    minWidth: 65,
    minHeight: 40,
  },
  fullContainer: {
    padding: 14,
    minWidth: 140,
    minHeight: 70,
  },
  compactLayout: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
  },
  fullLayout: {
    alignItems: 'center',
  },
  label: {
    fontSize: 11,
    color: '#9CA3AF',
    fontWeight: '600',
    marginBottom: 4,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  valueRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    marginBottom: 3,
  },
  apmValue: {
    fontSize: 20,
    fontWeight: 'bold',
    color: '#F9FAFB',
    fontVariant: ['tabular-nums'],
  },
  trendIcon: {
    fontSize: 16,
    fontWeight: 'bold',
  },
  subtitle: {
    fontSize: 10,
    color: '#6B7280',
    textAlign: 'center',
    fontWeight: '500',
  },
  trendPercentage: {
    fontSize: 9,
    fontWeight: '600',
    marginTop: 2,
  },
  statusContainer: {
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: 35,
  },
  statusText: {
    fontSize: 18,
  },
  statusLabel: {
    fontSize: 10,
    color: '#9CA3AF',
    marginTop: 2,
    fontWeight: '500',
  },
  indicator: {
    position: 'absolute',
    top: 6,
    right: 6,
    width: 8,
    height: 8,
    borderRadius: 4,
  },
});

export default ConvexRealtimeAPMWidget;